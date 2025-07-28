//! UDP transport implementation

use async_trait::async_trait;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::api::{Action, Transport, JSON};
use crate::error::{RatNetError, Result};

/// UDP transport implementation
pub struct UdpTransport {
    name: String,
    listen_addr: String,
    socket: RwLock<Option<Arc<UdpSocket>>>,
    running: AtomicBool,
    byte_limit: AtomicI64,
}

impl UdpTransport {
    /// Create a new UDP transport
    pub fn new(listen_addr: String) -> Self {
        Self {
            name: "udp".to_string(),
            listen_addr,
            socket: RwLock::new(None),
            running: AtomicBool::new(false),
            byte_limit: AtomicI64::new(1024 * 1024), // 1MB default
        }
    }

    /// Get the local address this transport is bound to
    pub async fn local_addr(&self) -> Option<SocketAddr> {
        let socket = self.socket.read().await;
        socket.as_ref().and_then(|s| s.local_addr().ok())
    }
}

#[async_trait]
impl Transport for UdpTransport {
    async fn listen(&self, listen: String, _admin_mode: bool) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            return Err(RatNetError::Transport(
                "Transport already running".to_string(),
            ));
        }

        let listen_addr = if listen.is_empty() {
            self.listen_addr.clone()
        } else {
            listen
        };

        info!("Starting UDP transport on {}", listen_addr);

        let socket = UdpSocket::bind(&listen_addr)
            .await
            .map_err(|e| RatNetError::Transport(format!("Failed to bind UDP socket: {}", e)))?;

        let socket = Arc::new(socket);

        {
            let mut socket_guard = self.socket.write().await;
            *socket_guard = Some(socket.clone());
        }

        self.running.store(true, Ordering::Relaxed);

        info!(
            "UDP transport listening on {}",
            socket.local_addr().unwrap()
        );

        // Spawn task to handle incoming messages
        let socket_clone = socket.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];

            while let Ok((len, peer)) = socket_clone.recv_from(&mut buf).await {
                debug!("Received {} bytes from {}", len, peer);

                let data = &buf[..len];

                // Process the received data
                match UdpTransport::handle_udp_message(data, &socket_clone, peer).await {
                    Ok(_) => debug!("Successfully handled UDP message from {}", peer),
                    Err(e) => error!("Failed to handle UDP message from {}: {}", peer, e),
                }
            }
        });

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn rpc(&self, host: &str, method: Action, args: Vec<Value>) -> Result<Value> {
        if !self.running.load(Ordering::Relaxed) {
            return Err(RatNetError::Transport("Transport not running".to_string()));
        }

        debug!("Making RPC call to {} for method {:?}", host, method);

        // Parse the host address
        let target_addr: SocketAddr = host
            .parse()
            .map_err(|e| RatNetError::Transport(format!("Invalid host address: {}", e)))?;

        // Get our socket
        let socket = {
            let socket_guard = self.socket.read().await;
            socket_guard
                .clone()
                .ok_or_else(|| RatNetError::Transport("Transport not initialized".to_string()))?
        };

        // Create RPC call
        let call = crate::api::RemoteCall {
            action: method,
            args,
        };

        debug!("Created RPC call: {:?}", call);

        // Serialize the call
        let call_bytes = crate::api::remote_call_to_bytes(&call)?;
        debug!("Serialized RPC call to {} bytes", call_bytes.len());

        // Send the request
        let sent_bytes = socket
            .send_to(&call_bytes, target_addr)
            .await
            .map_err(|e| RatNetError::Transport(format!("Failed to send UDP packet: {}", e)))?;
        debug!("Sent {} bytes to {}", sent_bytes, target_addr);

        // Wait for response with timeout
        let mut response_buf = vec![0u8; 65536];
        let timeout = tokio::time::Duration::from_secs(5);

        debug!("Waiting for response from {}...", target_addr);
        match tokio::time::timeout(timeout, socket.recv_from(&mut response_buf)).await {
            Ok(Ok((len, peer))) => {
                debug!("Received {} bytes from {}", len, peer);
                let response_data = &response_buf[..len];

                // Deserialize the response
                let response: crate::api::RemoteResponse =
                    crate::api::remote_response_from_bytes(response_data)?;
                debug!("Deserialized response: {:?}", response);

                if response.is_err() {
                    return Err(RatNetError::Transport(
                        response
                            .error
                            .unwrap_or_else(|| "Unknown error".to_string()),
                    ));
                }

                Ok(response.value.unwrap_or(Value::Null))
            }
            Ok(Err(e)) => {
                debug!("Failed to receive UDP response: {}", e);
                Err(RatNetError::Transport(format!(
                    "Failed to receive UDP response: {}",
                    e
                )))
            }
            Err(_) => {
                debug!("UDP RPC timeout waiting for response from {}", target_addr);
                Err(RatNetError::Transport("UDP RPC timeout".to_string()))
            }
        }
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping UDP transport");

        self.running.store(false, Ordering::Relaxed);

        let mut socket_guard = self.socket.write().await;
        *socket_guard = None;

        Ok(())
    }

    fn byte_limit(&self) -> i64 {
        self.byte_limit.load(Ordering::Relaxed)
    }

    fn set_byte_limit(&self, limit: i64) {
        self.byte_limit.store(limit, Ordering::Relaxed);
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl UdpTransport {
    async fn handle_udp_message(
        data: &[u8],
        socket: &Arc<UdpSocket>,
        peer: SocketAddr,
    ) -> Result<()> {
        debug!("Handling UDP message from {}: {} bytes", peer, data.len());

        // Check byte limit
        if data.len() > 65536 {
            return Err(RatNetError::Transport("UDP message too large".to_string()));
        }

        // Try to deserialize as RPC call
        match crate::api::remote_call_from_bytes(data) {
            Ok(call) => {
                debug!("Received RPC call from {}: {:?}", peer, call.action);

                // Handle different action types
                let response_value = match call.action {
                    Action::ID => {
                        // Echo back the first argument or a default message
                        if let Some(first_arg) = call.args.first() {
                            serde_json::json!({
                                "echo": first_arg,
                                "action": "ID",
                                "peer": peer.to_string()
                            })
                        } else {
                            serde_json::json!({
                                "echo": "UDP message received",
                                "action": "ID",
                                "peer": peer.to_string()
                            })
                        }
                    }
                    Action::AddContact => {
                        serde_json::json!({
                            "status": "contact_added",
                            "action": "AddContact",
                            "args_count": call.args.len()
                        })
                    }
                    Action::AddPeer => {
                        serde_json::json!({
                            "status": "peer_added",
                            "action": "AddPeer",
                            "args_count": call.args.len()
                        })
                    }
                    Action::AddChannel => {
                        serde_json::json!({
                            "status": "channel_added",
                            "action": "AddChannel",
                            "args_count": call.args.len()
                        })
                    }
                    Action::AddProfile => {
                        serde_json::json!({
                            "status": "profile_added",
                            "action": "AddProfile",
                            "args_count": call.args.len()
                        })
                    }
                    Action::Send => {
                        serde_json::json!({
                            "status": "message_sent",
                            "action": "Send",
                            "args_count": call.args.len()
                        })
                    }
                    Action::GetContacts => {
                        serde_json::json!({
                            "status": "contacts_retrieved",
                            "action": "GetContacts",
                            "contacts": []
                        })
                    }
                    Action::GetChannels => {
                        serde_json::json!({
                            "status": "channels_retrieved",
                            "action": "GetChannels",
                            "channels": []
                        })
                    }
                    Action::GetProfiles => {
                        serde_json::json!({
                            "status": "profiles_retrieved",
                            "action": "GetProfiles",
                            "profiles": []
                        })
                    }
                    Action::GetPeers => {
                        serde_json::json!({
                            "status": "peers_retrieved",
                            "action": "GetPeers",
                            "peers": []
                        })
                    }
                    Action::Start => {
                        serde_json::json!({
                            "status": "started",
                            "action": "Start"
                        })
                    }
                    Action::Stop => {
                        serde_json::json!({
                            "status": "stopped",
                            "action": "Stop"
                        })
                    }
                    Action::IsRunning => {
                        serde_json::json!({
                            "status": "running",
                            "action": "IsRunning",
                            "running": true
                        })
                    }
                    _ => {
                        serde_json::json!({
                            "status": "unknown_action",
                            "action": format!("{:?}", call.action),
                            "args_count": call.args.len()
                        })
                    }
                };

                debug!("Created response value: {:?}", response_value);

                let response = crate::api::RemoteResponse::success(response_value);
                let response_bytes = crate::api::remote_response_to_bytes(&response)?;

                debug!(
                    "Sending response to {}: {} bytes",
                    peer,
                    response_bytes.len()
                );

                // Send response back
                let sent_bytes = socket.send_to(&response_bytes, peer).await.map_err(|e| {
                    RatNetError::Transport(format!("Failed to send UDP response: {}", e))
                })?;

                debug!("Sent {} response bytes to {}", sent_bytes, peer);
                Ok(())
            }
            Err(e) => {
                debug!("Failed to deserialize as RPC call from {}: {}", peer, e);
                // If it's not an RPC call, just echo back
                debug!("Received non-RPC UDP message from {}, echoing back", peer);
                let sent_bytes = socket.send_to(data, peer).await.map_err(|e| {
                    RatNetError::Transport(format!("Failed to echo UDP message: {}", e))
                })?;
                debug!("Echoed {} bytes back to {}", sent_bytes, peer);
                Ok(())
            }
        }
    }
}

impl JSON for UdpTransport {
    fn to_json(&self) -> Result<String> {
        let config = serde_json::json!({
            "transport": "udp",
            "listen": self.listen_addr,
            "byte_limit": self.byte_limit()
        });

        serde_json::to_string(&config).map_err(Into::into)
    }

    fn from_json(json: &str) -> Result<Self> {
        let config: Value = serde_json::from_str(json)?;

        let listen_addr = config
            .get("listen")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1:0")
            .to_string();

        let transport = Self::new(listen_addr);

        if let Some(byte_limit) = config.get("byte_limit").and_then(|v| v.as_i64()) {
            transport.set_byte_limit(byte_limit);
        }

        Ok(transport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use tokio_test; // Unused import - may be needed for future testing

    #[tokio::test]
    async fn test_udp_transport_creation() {
        let transport = UdpTransport::new("127.0.0.1:0".to_string());
        assert_eq!(transport.name(), "udp");
        assert_eq!(transport.byte_limit(), 1024 * 1024);
    }

    #[tokio::test]
    async fn test_udp_transport_listen() {
        let transport = UdpTransport::new("127.0.0.1:0".to_string());
        let result = transport.listen("127.0.0.1:0".to_string(), false).await;
        assert!(result.is_ok());

        // Check that we got a valid local address
        let local_addr = transport.local_addr().await;
        assert!(local_addr.is_some());

        // Stop the transport
        let stop_result = transport.stop().await;
        assert!(stop_result.is_ok());
    }

    #[test]
    fn test_rpc_serialization_roundtrip() {
        // Test RPC call serialization
        let call = crate::api::RemoteCall {
            action: Action::ID,
            args: vec![serde_json::Value::String("test".to_string())],
        };

        let call_bytes = crate::api::remote_call_to_bytes(&call).expect("Failed to serialize call");
        println!("Serialized call: {:?}", call_bytes);

        let deserialized_call =
            crate::api::remote_call_from_bytes(&call_bytes).expect("Failed to deserialize call");
        println!("Deserialized call: {:?}", deserialized_call);

        assert_eq!(call.action, deserialized_call.action);
        assert_eq!(call.args.len(), deserialized_call.args.len());

        // Test response serialization
        let response = crate::api::RemoteResponse::success(serde_json::json!({"test": "value"}));
        let response_bytes =
            crate::api::remote_response_to_bytes(&response).expect("Failed to serialize response");
        println!("Serialized response: {:?}", response_bytes);

        let deserialized_response = crate::api::remote_response_from_bytes(&response_bytes)
            .expect("Failed to deserialize response");
        println!("Deserialized response: {:?}", deserialized_response);

        assert_eq!(response.error, deserialized_response.error);
        // The value gets serialized as a JSON string, so we need to parse it back
        if let Some(value) = deserialized_response.value {
            if let Some(json_str) = value.as_str() {
                let parsed_value: serde_json::Value =
                    serde_json::from_str(json_str).expect("Failed to parse JSON string");
                assert_eq!(response.value.unwrap(), parsed_value);
            } else {
                panic!("Expected string value after deserialization");
            }
        } else {
            panic!("Expected value after deserialization");
        }
    }
}
