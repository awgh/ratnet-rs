# RatNet-rs

A Rust implementation of [RatNet](https://github.com/awgh/ratnet), an anonymity network designed for mesh routing and embedded scenarios.

## Overview

RatNet-rs provides a robust foundation for building anonymous peer-to-peer networks with the following capabilities:

- **Anonymous Communication**: End-to-end encrypted messaging with no metadata leakage
- **Mesh Routing**: Resilient message routing through dynamic network topologies
- **Store-and-Forward**: Reliable message delivery in challenging network conditions
- **Pluggable Transports**: Support for UDP, TLS, and HTTPS protocols
- **Embedded Ready**: Designed for resource-constrained environments
- **Database Persistence**: Optional SQLite storage for message persistence

## Use Cases

### Anonymous Messaging Networks
Build private communication networks where message content and routing information remain confidential.

### Mesh Networks
Create resilient networks that can route around failures and adapt to changing topologies.

### IoT and Embedded Systems
Deploy lightweight nodes on resource-constrained devices for secure communication.

### Censorship Resistance
Establish communication channels that can operate in challenging network environments.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
ratnet = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
```

Basic usage:

```rust
use ratnet::prelude::*;
use ratnet::nodes::MemoryNode;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize component registrations
    ratnet::init();
    
    // Create a new node
    let node = Arc::new(MemoryNode::new());
    
    // Start the node
    node.start().await?;
    
    // Get the node's public key
    let node_id = node.id().await?;
    println!("Node ID: {}", node_id);
    
    // Add contacts and peers
    node.add_contact("friend".to_string(), "ed25519:...".to_string()).await?;
    node.add_peer("peer1".to_string(), true, "127.0.0.1:8080".to_string(), None).await?;
    
    Ok(())
}
```

## Examples

The repository includes comprehensive examples demonstrating different aspects of RatNet-rs:

### Basic Usage
```bash
# Run basic in-memory node
cargo run --example basic_node

# Run database-backed node with persistence
cargo run --example database_node
```

### Transport Examples
```bash
# TLS transport example
cargo run --example tls_transport --features tls

# HTTPS transport example  
cargo run --example https_transport --features https

# Message chunking demo
cargo run --example chunking_demo
```

### Filesystem Integration
```bash
# Filesystem-based node
cargo run --example filesystem_node
```

## Architecture

RatNet-rs is built around a modular architecture with these core components:

- **Node**: Main interface for sending/receiving messages, managing contacts, channels, and peers
- **Transport**: Network layer abstraction (UDP, TLS, HTTPS)
- **Router**: Message routing logic with channel mapping support
- **Policy**: Connection management policies (polling, P2P, server modes)
- **Registry**: Dynamic component registration system

## Features

### Core Functionality
- ✅ Anonymous peer-to-peer messaging
- ✅ Channel-based message routing
- ✅ Ed25519 cryptographic signing and verification
- ✅ Message chunking and reassembly
- ✅ Dynamic component registration

### Storage Options
- ✅ In-memory storage (MemoryNode)
- ✅ SQLite database persistence (DatabaseNode)
- ✅ Filesystem-based storage (FilesystemNode)

### Transport Protocols
- ✅ UDP transport with configurable limits
- ✅ TLS transport with certificate management
- ✅ HTTPS transport for web-based communication

### Development Tools
- ✅ Comprehensive test suite
- ✅ Example applications
- ✅ Async-first design with Tokio
- ✅ Type-safe error handling

## Development

### Building from Source

```bash
git clone https://github.com/awgh/ratnet-rs.git
cd ratnet-rs
cargo build
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test categories
cargo test --test database_tests
cargo test --test end_to_end_tests
```

### Feature Flags

- `sqlite`: Enable SQLite database support
- `tls`: Enable TLS transport
- `https`: Enable HTTPS transport
- `p2p`: Enable P2P networking features

## Documentation

- [API Documentation](https://docs.rs/ratnet)
- [Examples](./examples/) - Comprehensive usage examples
- [Tests](./tests/) - Test implementations for reference

## Comparison with Go Implementation

RatNet-rs maintains API compatibility with the original Go implementation while leveraging Rust's type safety and async capabilities:

| Feature | Go | Rust Status |
|---------|----|-------------|
| Memory Node | ✅ | ✅ |
| Database Node | ✅ | ✅ |
| UDP Transport | ✅ | ✅ |
| TLS Transport | ✅ | ✅ |
| HTTPS Transport | ✅ | ✅ |
| Message Chunking | ✅ | ✅ |
| Component Registry | ✅ | ✅ |

## License

GPL-3.0 (same as the original Go implementation)

## Related Projects

- [Original RatNet (Go)](https://github.com/awgh/ratnet)
- [RatNet Transports](https://github.com/awgh/ratnet-transports)
- [Bencrypt](https://github.com/awgh/bencrypt) - Crypto abstraction layer 