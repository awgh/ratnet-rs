//! HTTPS transport implementation

use async_trait::async_trait;
use bytes::Bytes;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Method, Request, Response, Server, StatusCode};
use hyper_rustls::HttpsConnector;
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::api::{Node, *};
use crate::error::{RatNetError, Result};

/// HTTPS transport implementation
pub struct HttpsTransport {
    name: String,
    listen_addr: String,
    running: Arc<AtomicBool>,
    byte_limit: Arc<AtomicI64>,
    cert_pem: Vec<u8>,
    key_pem: Vec<u8>,
    ecc_mode: bool,
    client: Client<HttpsConnector<hyper::client::HttpConnector>>,
    node: Arc<dyn Node>,
}

impl HttpsTransport {
    /// Create a new HTTPS transport
    pub fn new(listen_addr: String, node: Arc<dyn Node>) -> Self {
        // Create HTTPS client with rustls
        let tls_config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(InsecureVerifier))
            .with_no_client_auth();

        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(tls_config)
            .https_or_http()
            .enable_http1()
            .build();

        let client = Client::builder().build::<_, Body>(https);

        Self {
            name: "https".to_string(),
            listen_addr,
            running: Arc::new(AtomicBool::new(false)),
            byte_limit: Arc::new(AtomicI64::new(125000)), // 125KB limit like Go version
            cert_pem: Vec::new(),
            key_pem: Vec::new(),
            ecc_mode: false,
            client,
            node,
        }
    }

    /// Create a new HTTPS transport with certificates
    pub fn new_with_certs(
        listen_addr: String,
        cert_pem: Vec<u8>,
        key_pem: Vec<u8>,
        ecc_mode: bool,
        node: Arc<dyn Node>,
    ) -> Self {
        let tls_config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(InsecureVerifier))
            .with_no_client_auth();

        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(tls_config)
            .https_or_http()
            .enable_http1()
            .build();

        let client = Client::builder().build::<_, Body>(https);

        Self {
            name: "https".to_string(),
            listen_addr,
            running: Arc::new(AtomicBool::new(false)),
            byte_limit: Arc::new(AtomicI64::new(125000)),
            cert_pem,
            key_pem,
            ecc_mode,
            client,
            node,
        }
    }

    /// Parse certificates from PEM data
    #[allow(dead_code)]
    fn parse_certs(&self) -> Result<Vec<Certificate>> {
        let mut cursor = std::io::Cursor::new(&self.cert_pem);
        let cert_ders = certs(&mut cursor)
            .map_err(|e| RatNetError::Transport(format!("Failed to parse certificates: {}", e)))?;

        Ok(cert_ders.into_iter().map(Certificate).collect())
    }

    /// Parse private key from PEM data
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

    /// Handle HTTP request
    #[allow(dead_code)]
    async fn handle_request(
        &self,
        req: Request<Body>,
        admin_mode: bool,
    ) -> std::result::Result<Response<Body>, Infallible> {
        debug!("Handling HTTPS request, admin_mode: {}", admin_mode);

        match req.method() {
            &Method::POST => {
                // Read request body
                match hyper::body::to_bytes(req.into_body()).await {
                    Ok(body_bytes) => {
                        if body_bytes.len() > self.byte_limit.load(Ordering::Relaxed) as usize {
                            warn!("Request body too large: {} bytes", body_bytes.len());
                            return Ok::<Response<Body>, Infallible>(
                                Response::builder()
                                    .status(StatusCode::PAYLOAD_TOO_LARGE)
                                    .body(Body::from("Payload too large"))
                                    .unwrap(),
                            );
                        }
                        // Parse as RemoteCall
                        let call = match remote_call_from_bytes(&body_bytes) {
                            Ok(call) => call,
                            Err(e) => {
                                warn!("Failed to parse RemoteCall: {}", e);
                                let response = RemoteResponse::error(format!("Parse error: {}", e));
                                let response_bytes = remote_response_to_bytes(&response)
                                    .unwrap_or_else(|_| Bytes::from_static(b"error"));
                                return Ok(Response::new(Body::from(response_bytes)));
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
                        let response_bytes = remote_response_to_bytes(&response)
                            .unwrap_or_else(|_| Bytes::from_static(b"error"));
                        Ok(Response::new(Body::from(response_bytes)))
                    }
                    Err(e) => {
                        error!("Failed to read request body: {}", e);
                        Ok(Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Body::from("Failed to read request body"))
                            .unwrap())
                    }
                }
            }
            _ => Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::from("Only POST method is supported"))
                .unwrap()),
        }
    }
    // Helper to get Arc<dyn Transport> for node calls
    #[allow(dead_code)]
    fn clone_arc(&self) -> Arc<dyn Transport> {
        Arc::new(Self {
            name: self.name.clone(),
            listen_addr: self.listen_addr.clone(),
            running: self.running.clone(),
            byte_limit: self.byte_limit.clone(),
            cert_pem: self.cert_pem.clone(),
            key_pem: self.key_pem.clone(),
            ecc_mode: self.ecc_mode,
            client: self.client.clone(),
            node: self.node.clone(),
        })
    }

    /// Send HTTP request to remote server
    pub async fn send_request(&self, url: &str, data: Vec<u8>) -> Result<Vec<u8>> {
        let req = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header("content-type", "application/octet-stream")
            .body(Body::from(data))
            .map_err(|e| RatNetError::Transport(format!("Failed to build request: {}", e)))?;

        let resp = self
            .client
            .request(req)
            .await
            .map_err(|e| RatNetError::Transport(format!("HTTP request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(RatNetError::Transport(format!(
                "HTTP request failed with status: {}",
                resp.status()
            )));
        }

        let body_bytes = hyper::body::to_bytes(resp.into_body())
            .await
            .map_err(|e| RatNetError::Transport(format!("Failed to read response body: {}", e)))?;

        Ok(body_bytes.to_vec())
    }
}

#[async_trait]
impl Transport for HttpsTransport {
    async fn listen(&self, listen: String, _admin_mode: bool) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            return Err(RatNetError::Transport(
                "HTTPS transport is already running".to_string(),
            ));
        }

        info!("Starting HTTPS transport on {}", listen);

        let addr: SocketAddr = listen
            .parse()
            .map_err(|e| RatNetError::Transport(format!("Invalid listen address: {}", e)))?;

        let running = self.running.clone();
        let byte_limit = self.byte_limit.clone();

        // Create service factory
        let make_svc = make_service_fn(move |_conn| {
            let running = running.clone();
            let byte_limit = byte_limit.clone();

            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    let running = running.clone();
                    let byte_limit = byte_limit.clone();

                    async move {
                        if !running.load(Ordering::Relaxed) {
                            return Ok::<Response<Body>, Infallible>(
                                Response::builder()
                                    .status(StatusCode::SERVICE_UNAVAILABLE)
                                    .body(Body::from("Service shutting down"))
                                    .unwrap(),
                            );
                        }

                        // Handle request
                        match req.method() {
                            &Method::POST => {
                                match hyper::body::to_bytes(req.into_body()).await {
                                    Ok(body_bytes) => {
                                        if body_bytes.len()
                                            > byte_limit.load(Ordering::Relaxed) as usize
                                        {
                                            warn!(
                                                "Request body too large: {} bytes",
                                                body_bytes.len()
                                            );
                                            return Ok(Response::builder()
                                                .status(StatusCode::PAYLOAD_TOO_LARGE)
                                                .body(Body::from("Payload too large"))
                                                .unwrap());
                                        }

                                        debug!("Received {} bytes over HTTPS", body_bytes.len());

                                        // Echo back for now (in real implementation, this would be RPC handling)
                                        Ok(Response::new(Body::from(body_bytes)))
                                    }
                                    Err(e) => {
                                        error!("Failed to read request body: {}", e);
                                        Ok(Response::builder()
                                            .status(StatusCode::BAD_REQUEST)
                                            .body(Body::from("Failed to read request body"))
                                            .unwrap())
                                    }
                                }
                            }
                            _ => Ok(Response::builder()
                                .status(StatusCode::METHOD_NOT_ALLOWED)
                                .body(Body::from("Only POST method is supported"))
                                .unwrap()),
                        }
                    }
                }))
            }
        });

        // Create server with graceful shutdown
        let shutdown_running = self.running.clone();
        let shutdown_signal = async move {
            while shutdown_running.load(Ordering::Relaxed) {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        };

        let server = Server::bind(&addr)
            .serve(make_svc)
            .with_graceful_shutdown(shutdown_signal);

        self.running.store(true, Ordering::Relaxed);
        info!("HTTPS transport listening on {}", addr);

        // Run server in background
        tokio::spawn(async move {
            if let Err(e) = server.await {
                error!("HTTPS server error: {}", e);
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
                "HTTPS transport not running".to_string(),
            ));
        }

        debug!("Making HTTPS RPC call to {} for method {:?}", host, method);

        // Create RPC call
        let call = crate::api::RemoteCall {
            action: method,
            args,
        };

        // Serialize the call
        let call_bytes = crate::api::remote_call_to_bytes(&call)?;

        // Send the request
        let response_bytes = self.send_request(host, call_bytes.to_vec()).await?;

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
        info!("Stopping HTTPS transport");
        self.running.store(false, Ordering::Relaxed);

        // Give some time for graceful shutdown
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        info!("HTTPS transport stopped");
        Ok(())
    }

    fn byte_limit(&self) -> i64 {
        self.byte_limit.load(Ordering::Relaxed)
    }

    fn set_byte_limit(&self, limit: i64) {
        self.byte_limit.store(limit, Ordering::Relaxed);
        debug!("Set HTTPS transport byte limit to {}", limit);
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl JSON for HttpsTransport {
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
            "HTTPS transport JSON deserialization not implemented".to_string(),
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
