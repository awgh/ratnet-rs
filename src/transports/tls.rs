//! TLS transport implementation

use async_trait::async_trait;
use bytes::Bytes;
use rustls::{Certificate, ClientConfig, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};
use tracing::{debug, error, info, warn};

use crate::api::{Node, *};
use crate::error::{RatNetError, Result};

/// TLS transport implementation
pub struct TlsTransport {
    name: String,
    listen_addr: String,
    running: Arc<AtomicBool>,
    byte_limit: Arc<AtomicI64>,
    cached_sessions: Arc<RwLock<HashMap<String, Arc<TlsStream<TcpStream>>>>>,
    cert_pem: Vec<u8>,
    key_pem: Vec<u8>,
    ecc_mode: bool,
    node: Arc<dyn Node>,
}

impl TlsTransport {
    /// Create a new TLS transport
    pub fn new(listen_addr: String, node: Arc<dyn Node>) -> Self {
        Self {
            name: "tls".to_string(),
            listen_addr,
            running: Arc::new(AtomicBool::new(false)),
            byte_limit: Arc::new(AtomicI64::new(8000 * 1024)), // 8MB limit like Go version
            cached_sessions: Arc::new(RwLock::new(HashMap::new())),
            cert_pem: Vec::new(),
            key_pem: Vec::new(),
            ecc_mode: false,
            node,
        }
    }

    /// Create a new TLS transport with certificates
    pub fn new_with_certs(
        listen_addr: String,
        cert_pem: Vec<u8>,
        key_pem: Vec<u8>,
        ecc_mode: bool,
        node: Arc<dyn Node>,
    ) -> Self {
        Self {
            name: "tls".to_string(),
            listen_addr,
            running: Arc::new(AtomicBool::new(false)),
            byte_limit: Arc::new(AtomicI64::new(8000 * 1024)),
            cached_sessions: Arc::new(RwLock::new(HashMap::new())),
            cert_pem,
            key_pem,
            ecc_mode,
            node,
        }
    }

    /// Parse certificates from PEM data
    fn parse_certs(&self) -> Result<Vec<Certificate>> {
        let mut cursor = std::io::Cursor::new(&self.cert_pem);
        let cert_ders = certs(&mut cursor)
            .map_err(|e| RatNetError::Transport(format!("Failed to parse certificates: {}", e)))?;

        Ok(cert_ders.into_iter().map(Certificate).collect())
    }

    /// Parse private key from PEM data
    fn parse_private_key(&self) -> Result<PrivateKey> {
        let mut cursor = std::io::Cursor::new(&self.key_pem);
        let keys = pkcs8_private_keys(&mut cursor)
            .map_err(|e| RatNetError::Transport(format!("Failed to parse private key: {}", e)))?;

        if keys.is_empty() {
            return Err(RatNetError::Transport("No private keys found".to_string()));
        }

        Ok(PrivateKey(keys[0].clone()))
    }

    /// Create TLS server config
    fn create_server_config(&self) -> Result<ServerConfig> {
        let certs = self.parse_certs()?;
        let key = self.parse_private_key()?;

        ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| {
                RatNetError::Transport(format!("Failed to create TLS server config: {}", e))
            })
    }

    /// Create TLS client config
    fn create_client_config() -> Result<ClientConfig> {
        let config = ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(InsecureVerifier))
            .with_no_client_auth();

        Ok(config)
    }

    /// Handle incoming TLS connection
    async fn handle_connection(
        &self,
        mut stream: TlsStream<TcpStream>,
        admin_mode: bool,
    ) -> Result<()> {
        debug!("Handling TLS connection, admin_mode: {}", admin_mode);

        while self.running.load(Ordering::Relaxed) {
            // Read message buffer
            let buffer = match self.read_buffer(&mut stream).await {
                Ok(buf) => buf,
                Err(e) => {
                    warn!("Failed to read buffer: {}", e);
                    break;
                }
            };

            // Parse as RemoteCall
            let call = match remote_call_from_bytes(&buffer) {
                Ok(call) => call,
                Err(e) => {
                    warn!("Failed to parse RemoteCall: {}", e);
                    let response = RemoteResponse::error(format!("Parse error: {}", e));
                    let response_bytes = remote_response_to_bytes(&response)?;
                    let _ = self.write_buffer(&mut stream, &response_bytes).await;
                    continue;
                }
            };

            // Route to node
            let response = if admin_mode {
                self.node
                    .admin_rpc(self.clone_arc(), call)
                    .await
                    .unwrap_or_else(|e| RemoteResponse::error(e.to_string()))
            } else {
                self.node
                    .public_rpc(self.clone_arc(), call)
                    .await
                    .unwrap_or_else(|e| RemoteResponse::error(e.to_string()))
            };

            // Serialize and send response
            let response_bytes = remote_response_to_bytes(&response)?;
            if let Err(e) = self.write_buffer(&mut stream, &response_bytes).await {
                warn!("Failed to write response: {}", e);
                break;
            }
        }
        Ok(())
    }
    // Helper to get Arc<dyn Transport> for node calls
    fn clone_arc(&self) -> Arc<dyn Transport> {
        // SAFETY: This is safe because TlsTransport is always behind Arc
        // and implements Transport + Send + Sync
        Arc::new(Self {
            name: self.name.clone(),
            listen_addr: self.listen_addr.clone(),
            running: self.running.clone(),
            byte_limit: self.byte_limit.clone(),
            cached_sessions: self.cached_sessions.clone(),
            cert_pem: self.cert_pem.clone(),
            key_pem: self.key_pem.clone(),
            ecc_mode: self.ecc_mode,
            node: self.node.clone(),
        })
    }

    /// Read a length-prefixed buffer from the stream
    async fn read_buffer(&self, stream: &mut TlsStream<TcpStream>) -> Result<Vec<u8>> {
        // Read 4-byte length header
        let mut len_bytes = [0u8; 4];
        stream.read_exact(&mut len_bytes).await?;
        let len = u32::from_be_bytes(len_bytes) as usize;

        if len > self.byte_limit.load(Ordering::Relaxed) as usize {
            return Err(RatNetError::Transport(format!(
                "Message too large: {} bytes",
                len
            )));
        }

        // Read the message data
        let mut buffer = vec![0u8; len];
        stream.read_exact(&mut buffer).await?;

        Ok(buffer)
    }

    /// Write a length-prefixed buffer to the stream
    async fn write_buffer(&self, stream: &mut TlsStream<TcpStream>, data: &[u8]) -> Result<()> {
        // Write 4-byte length header
        let len = data.len() as u32;
        stream.write_all(&len.to_be_bytes()).await?;

        // Write the message data
        stream.write_all(data).await?;
        stream.flush().await?;

        Ok(())
    }

    /// Connect to a remote TLS server
    pub async fn connect(&self, addr: &str) -> Result<TlsStream<TcpStream>> {
        let config = Self::create_client_config()?;
        let connector = TlsConnector::from(Arc::new(config));

        let stream = TcpStream::connect(addr).await?;
        let domain = rustls::ServerName::try_from("localhost")
            .map_err(|e| RatNetError::Transport(format!("Invalid server name: {}", e)))?;

        let tls_stream = connector
            .connect(domain, stream)
            .await
            .map_err(|e| RatNetError::Transport(format!("TLS connection failed: {}", e)))?;

        debug!("Connected to TLS server at {}", addr);
        Ok(tokio_rustls::TlsStream::Client(tls_stream))
    }

    async fn handle_tls_connection(
        stream: TcpStream,
        cert_pem: Vec<u8>,
        key_pem: Vec<u8>,
        admin_mode: bool,
    ) -> Result<()> {
        if cert_pem.is_empty() || key_pem.is_empty() {
            // Handle as plain TCP for now
            debug!("Handling plain TCP connection (no certificates)");
            return Ok(());
        }

        // Create TLS acceptor
        let certs = Self::parse_certs_static(&cert_pem)?;
        let key = Self::parse_private_key_static(&key_pem)?;

        let config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| {
                RatNetError::Transport(format!("Failed to create TLS server config: {}", e))
            })?;

        let acceptor = TlsAcceptor::from(Arc::new(config));

        // Accept TLS connection
        let tls_stream = acceptor
            .accept(stream)
            .await
            .map_err(|e| RatNetError::Transport(format!("TLS accept failed: {}", e)))?;

        debug!("TLS connection established");

        // Handle the TLS connection
        Self::handle_connection_internal(tokio_rustls::TlsStream::Server(tls_stream), admin_mode)
            .await
    }

    fn parse_certs_static(cert_pem: &[u8]) -> Result<Vec<Certificate>> {
        let mut cursor = std::io::Cursor::new(cert_pem);
        let cert_ders = certs(&mut cursor)
            .map_err(|e| RatNetError::Transport(format!("Failed to parse certificates: {}", e)))?;

        Ok(cert_ders.into_iter().map(Certificate).collect())
    }

    fn parse_private_key_static(key_pem: &[u8]) -> Result<PrivateKey> {
        let mut cursor = std::io::Cursor::new(key_pem);
        let keys = pkcs8_private_keys(&mut cursor)
            .map_err(|e| RatNetError::Transport(format!("Failed to parse private key: {}", e)))?;

        if keys.is_empty() {
            return Err(RatNetError::Transport("No private keys found".to_string()));
        }

        Ok(PrivateKey(keys[0].clone()))
    }

    async fn handle_connection_internal(
        mut stream: TlsStream<TcpStream>,
        admin_mode: bool,
    ) -> Result<()> {
        let mut buffer = vec![0u8; 4096];

        loop {
            match stream.read(&mut buffer).await {
                Ok(0) => {
                    debug!("TLS connection closed by peer");
                    break;
                }
                Ok(n) => {
                    let data = &buffer[..n];
                    debug!("Received {} bytes over TLS", n);

                    // Process the received data
                    // In a full implementation, this would:
                    // 1. Deserialize the RPC call
                    // 2. Route it to the appropriate handler
                    // 3. Send back a response

                    // For now, just echo back
                    if let Err(e) = stream.write_all(data).await {
                        error!("Failed to write response: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("TLS read error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Transport for TlsTransport {
    async fn listen(&self, listen: String, admin_mode: bool) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            return Err(RatNetError::Transport(
                "TLS transport is already running".to_string(),
            ));
        }

        info!("Starting TLS transport on {}", listen);

        // Only create TLS config if we have certificates
        if self.cert_pem.is_empty() || self.key_pem.is_empty() {
            warn!("TLS transport started without certificates - only client connections possible");
        }

        // Bind TCP listener
        let listener = TcpListener::bind(&listen)
            .await
            .map_err(|e| RatNetError::Transport(format!("Failed to bind TLS listener: {}", e)))?;

        self.running.store(true, Ordering::Relaxed);
        info!("TLS transport listening on {}", listen);

        // Accept connections
        let running = self.running.clone();
        let cert_pem = self.cert_pem.clone();
        let key_pem = self.key_pem.clone();

        tokio::spawn(async move {
            while running.load(Ordering::Relaxed) {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        debug!("Accepted TCP connection from {}", addr);

                        let cert_pem = cert_pem.clone();
                        let key_pem = key_pem.clone();
                        let admin_mode = admin_mode;

                        tokio::spawn(async move {
                            if let Err(e) = TlsTransport::handle_tls_connection(
                                stream, cert_pem, key_pem, admin_mode,
                            )
                            .await
                            {
                                error!("Error handling TLS connection from {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn rpc(
        &self,
        host: &str,
        method: Action,
        args: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        if !self.running.load(Ordering::Relaxed) {
            return Err(RatNetError::Transport(
                "TLS transport not running".to_string(),
            ));
        }

        debug!("Making TLS RPC call to {} for method {:?}", host, method);

        // Create RPC call
        let call = crate::api::RemoteCall {
            action: method,
            args,
        };

        // Serialize the call
        let call_bytes = crate::api::remote_call_to_bytes(&call)?;

        // Connect to the remote host
        let mut stream = self.connect(host).await?;

        // Send the request
        self.write_buffer(&mut stream, &call_bytes).await?;

        // Read the response
        let response_bytes = self.read_buffer(&mut stream).await?;

        // Deserialize the response
        let response: crate::api::RemoteResponse =
            crate::api::remote_response_from_bytes(&response_bytes)?;

        if response.is_err() {
            return Err(RatNetError::Transport(
                response
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        Ok(response.value.unwrap_or(serde_json::Value::Null))
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping TLS transport");
        self.running.store(false, Ordering::Relaxed);

        // Clear cached sessions
        self.cached_sessions.write().await.clear();

        info!("TLS transport stopped");
        Ok(())
    }

    fn byte_limit(&self) -> i64 {
        self.byte_limit.load(Ordering::Relaxed)
    }

    fn set_byte_limit(&self, limit: i64) {
        self.byte_limit.store(limit, Ordering::Relaxed);
        debug!("Set TLS transport byte limit to {}", limit);
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl JSON for TlsTransport {
    fn to_json(&self) -> Result<String> {
        Ok(format!(
            r#"{{"name":"{}","listen_addr":"{}","running":{},"byte_limit":{},"ecc_mode":{}}}"#,
            self.name,
            self.listen_addr,
            self.running.load(Ordering::Relaxed),
            self.byte_limit.load(Ordering::Relaxed),
            self.ecc_mode
        ))
    }

    fn from_json(_s: &str) -> Result<Self> {
        // TODO: Implement JSON deserialization
        Err(RatNetError::NotImplemented(
            "TLS transport JSON deserialization not implemented".to_string(),
        ))
    }
}

/// Insecure certificate verifier for testing
struct InsecureVerifier;

impl rustls::client::ServerCertVerifier for InsecureVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &Certificate,
        _intermediates: &[Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> std::result::Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}
