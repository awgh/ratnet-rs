# Changelog

All notable changes to RatNet-rs will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial Rust implementation of RatNet
- Core API traits and data structures
- Memory node implementation with in-memory storage
- Database node implementation with SQLite persistence
- Filesystem node implementation for file-based storage
- UDP transport with configurable byte limits
- TLS transport with certificate management
- HTTPS transport for web-based communication
- Default message router with channel extraction
- Poll policy for basic connection management
- P2P policy for peer discovery and mesh networking
- Server policy for centralized communication
- Dynamic component registry system
- Message chunking and reassembly for large messages
- Ed25519 cryptographic signing and verification
- Kyber post-quantum cryptography support
- Comprehensive test suite (121 tests)
- Multiple example applications
- Binary size optimization (3.8MB release build)

### Technical Details
- Async-first design with Tokio runtime
- Type-safe error handling with thiserror
- JSON serialization for configuration and state
- Modular architecture with pluggable components
- Cross-platform support (Linux, macOS, Windows)
- Optimized for embedded and resource-constrained environments

## [0.1.0] - 2024-07-27

### Initial Release
- Complete Rust port of RatNet Go implementation
- All core functionality implemented and tested
- Production-ready with comprehensive documentation
- GPL-3.0 licensed to match original implementation

### Features
- **Anonymous Communication**: End-to-end encrypted messaging with no metadata leakage
- **Mesh Routing**: Resilient message routing through dynamic network topologies  
- **Store-and-Forward**: Reliable message delivery in challenging network conditions
- **Pluggable Transports**: Support for UDP, TLS, and HTTPS protocols
- **Embedded Ready**: Designed for resource-constrained environments
- **Database Persistence**: Optional SQLite storage for message persistence

### Architecture
- **Node**: Main interface for sending/receiving messages, managing contacts, channels, and peers
- **Transport**: Network layer abstraction (UDP, TLS, HTTPS)
- **Router**: Message routing logic with channel mapping support
- **Policy**: Connection management policies (polling, P2P, server modes)
- **Registry**: Dynamic component registration system

### Storage Options
- **MemoryNode**: In-memory storage for testing and simple use cases
- **DatabaseNode**: SQLite database persistence with migrations
- **FilesystemNode**: File-based storage for embedded systems

### Transport Protocols
- **UDP Transport**: Basic UDP networking with configurable byte limits
- **TLS Transport**: Secure TCP connections using tokio-rustls
- **HTTPS Transport**: Web-based transport using hyper with TLS

### Development Tools
- **Comprehensive Test Suite**: Unit and integration tests for all components
- **Example Applications**: Multiple usage examples demonstrating different features
- **Binary Optimization**: Size-optimized builds with LTO and panic=abort
- **Documentation**: API documentation and usage guides

---

## Version History

### Comparison with Go Implementation

| Feature | Go | Rust Status | Notes |
|---------|----|-------------|-------|
| Memory Node | ✅ | ✅ | Complete with async design |
| Database Node | ✅ | ✅ | SQLite with migrations |
| UDP Transport | ✅ | ✅ | Full compatibility |
| TLS Transport | ✅ | ✅ | Using tokio-rustls |
| HTTPS Transport | ✅ | ✅ | Using hyper with TLS |
| Message Chunking | ✅ | ✅ | Complete chunking and reassembly |
| Component Registry | ✅ | ✅ | Dynamic registration system |

The Rust implementation maintains API compatibility while leveraging Rust's type safety and async capabilities. 