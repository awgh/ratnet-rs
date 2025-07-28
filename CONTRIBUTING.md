# Contributing to RatNet-rs

Thank you for your interest in contributing to RatNet-rs! This document provides guidelines for contributing to the project.

## Getting Started

### Prerequisites

- Rust 1.70+ (check with `rustc --version`)
- Cargo (comes with Rust)
- Git

### Setting Up Development Environment

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone https://github.com/your-username/ratnet-rs.git
   cd ratnet-rs
   ```
3. Add the upstream remote:
   ```bash
   git remote add upstream https://github.com/awgh/ratnet-rs.git
   ```

## Development Workflow

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test categories
cargo test --test database_tests
cargo test --test transport_tests

# Run tests with output
cargo test -- --nocapture
```

### Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Build with specific features
cargo build --features "tls,https"
```

### Running Examples

```bash
# Basic node example
cargo run --example basic_node

# Database node example
cargo run --example database_node --features sqlite

# TLS transport example
cargo run --example tls_transport --features tls
```

## Code Style

### Rust Conventions

- Follow the [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/style/naming/README.html)
- Use `cargo fmt` to format code
- Use `cargo clippy` to check for common issues

### Code Organization

- Keep modules focused and cohesive
- Use meaningful names for functions, variables, and types
- Add documentation comments (`///`) for public APIs
- Include examples in documentation

### Error Handling

- Use `Result<T, E>` for operations that can fail
- Provide meaningful error messages
- Use `thiserror` for custom error types

## Testing

### Writing Tests

- Write unit tests for individual functions
- Write integration tests for component interactions
- Test both success and error cases
- Use descriptive test names

### Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Arrange
        let input = "test";
        
        // Act
        let result = function(input);
        
        // Assert
        assert_eq!(result, expected);
    }
}
```

## Pull Request Process

### Before Submitting

1. **Update your branch**: `git pull upstream main`
2. **Run tests**: `cargo test`
3. **Check formatting**: `cargo fmt`
4. **Run clippy**: `cargo clippy`
5. **Update documentation** if needed

### Pull Request Guidelines

- **Title**: Clear, descriptive title
- **Description**: Explain what the PR does and why
- **Related issues**: Link to any related issues
- **Tests**: Include tests for new functionality
- **Documentation**: Update docs for new features

### Commit Messages

Use conventional commit format:
```
type(scope): description

[optional body]

[optional footer]
```

Examples:
- `feat(transport): add WebSocket transport support`
- `fix(crypto): resolve memory leak in key generation`
- `docs(api): update README with usage examples`

## Areas for Contribution

### High Priority

- **Performance optimization**: Improve binary size and runtime performance
- **Security hardening**: Audit and improve security measures
- **Documentation**: Improve API documentation and examples
- **Testing**: Add more comprehensive test coverage

### Medium Priority

- **New transports**: Implement additional transport protocols
- **Monitoring**: Add metrics and monitoring capabilities
- **CLI improvements**: Enhance command-line interface
- **Configuration**: Improve configuration management

### Low Priority

- **Examples**: Add more usage examples
- **Benchmarks**: Add performance benchmarks
- **CI/CD**: Improve continuous integration

## Getting Help

- **Issues**: Use GitHub issues for bugs and feature requests
- **Discussions**: Use GitHub Discussions for questions and ideas
- **Code review**: All PRs require review before merging

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please be respectful and inclusive in all interactions.

## License

By contributing to RatNet-rs, you agree that your contributions will be licensed under the same GPL-3.0 license as the project. 