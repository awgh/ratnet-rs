# RatNet-rs Benchmarking Tools

This directory contains comprehensive benchmarking tools for measuring RatNet-rs performance across different node types, transports, and load conditions.

## Overview

The benchmarking suite provides:

- **Throughput measurement** (messages per second, MB per second)
- **Latency analysis** (min/avg/max round-trip times)
- **Memory usage tracking**
- **Success rate monitoring**
- **Multi-node and multi-transport testing**
- **Configurable load patterns**

## Quick Start

### 1. Quick Benchmark

Run a simple benchmark with default settings:

```bash
cargo run --example benchmark_cli -- quick
```

### 2. Custom Quick Benchmark

Test specific node type and transport:

```bash
cargo run --example benchmark_cli -- quick \
  --node-type memory \
  --transport udp \
  --messages 1000 \
  --size 1024
```

### 3. Comprehensive Benchmark Suite

Run all combinations of node types and transports:

```bash
cargo run --example benchmark_cli -- comprehensive
```

### 4. Stress Testing

Run high-load stress tests:

```bash
cargo run --example benchmark_cli -- stress \
  --duration 300 \
  --concurrent 10
```

## Configuration

### Node Types

- **Memory**: Fastest, in-memory storage
- **Database**: Persistent SQLite storage
- **Filesystem**: File-based storage

### Transport Types

- **UDP**: Fastest, unreliable
- **TLS**: Secure, reliable TCP
- **HTTPS**: Web-compatible, secure

### Custom Configuration

Create a JSON configuration file:

```json
{
  "message_count": 5000,
  "message_size": 2048,
  "message_delay_ms": 10,
  "concurrent_senders": 4,
  "duration_seconds": 120,
  "node_type": "Database",
  "transport_type": "Tls",
  "verbose": true
}
```

Run with custom config:

```bash
cargo run --example benchmark_cli -- custom \
  --config examples/benchmark_config.json \
  --output results.json
```

## Performance Metrics

### Throughput

- **Messages per second**: Raw message processing rate
- **MB per second**: Data transfer rate
- **Success rate**: Percentage of successful message deliveries

### Latency

- **Minimum latency**: Best-case round-trip time
- **Average latency**: Mean round-trip time
- **Maximum latency**: Worst-case round-trip time

### Resource Usage

- **Memory usage**: RAM consumption in bytes
- **CPU usage**: Processor utilization percentage
- **Duration**: Total benchmark execution time

## Use Cases

### 1. Performance Optimization

Identify bottlenecks and optimize components:

```bash
# Test different message sizes
for size in 64 256 1024 4096; do
  cargo run --example benchmark_cli -- quick --size $size
done
```

### 2. Capacity Planning

Determine system limits and requirements:

```bash
# Stress test to find breaking point
cargo run --example benchmark_cli -- stress --concurrent 50
```

### 3. Comparison Testing

Compare different configurations:

```bash
# Memory vs Database nodes
cargo run --example benchmark_cli -- quick --node-type memory
cargo run --example benchmark_cli -- quick --node-type database
```

### 4. Regression Testing

Ensure performance doesn't degrade:

```bash
# Run comprehensive suite and save results
cargo run --example benchmark_cli -- comprehensive > baseline.txt
```

## Interpreting Results

### Good Performance Indicators

- **High throughput**: >1000 messages/sec for small messages
- **Low latency**: <10ms average for local testing
- **High success rate**: >95% message delivery
- **Reasonable memory**: <100MB for typical workloads

### Performance Bottlenecks

- **Low throughput**: Check transport configuration, network limits
- **High latency**: Investigate transport overhead, processing delays
- **Memory leaks**: Monitor memory usage over time
- **Low success rate**: Check network connectivity, transport reliability

## Advanced Usage

### Programmatic Benchmarking

```rust
use ratnet::benchmark::*;

#[tokio::main]
async fn main() {
    let config = BenchmarkConfig {
        message_count: 1000,
        message_size: 1024,
        message_delay_ms: 0,
        concurrent_senders: 4,
        duration_seconds: 60,
        node_type: NodeType::Memory,
        transport_type: TransportType::Udp,
        verbose: true,
    };

    let mut runner = BenchmarkRunner::new(config);
    let results = runner.run_benchmark_suite(5).await;
    let report = runner.generate_report();
    println!("{}", report);
}
```

### Custom Metrics

Extend the benchmarking framework:

```rust
use ratnet::benchmark::*;

pub struct CustomBenchmarkResult {
    pub base: BenchmarkResult,
    pub custom_metric: f64,
}

impl BenchmarkRunner {
    pub async fn run_custom_benchmark(&mut self) -> CustomBenchmarkResult {
        let base_result = self.run_benchmark().await;
        
        // Add custom metrics
        let custom_metric = calculate_custom_metric(&base_result);
        
        CustomBenchmarkResult {
            base: base_result,
            custom_metric,
        }
    }
}
```

## Troubleshooting

### Common Issues

1. **Benchmark fails to start**
   - Check port availability
   - Verify transport configuration
   - Ensure sufficient system resources

2. **Low performance**
   - Test with smaller message sizes first
   - Check system load and available resources
   - Verify network configuration

3. **Memory issues**
   - Monitor memory usage during long tests
   - Consider using database nodes for large datasets
   - Implement proper cleanup in custom benchmarks

### Debug Mode

Enable verbose output for detailed debugging:

```bash
cargo run --example benchmark_cli -- quick --verbose
```

## Integration with CI/CD

Add benchmarking to your continuous integration:

```yaml
# .github/workflows/benchmark.yml
name: Performance Benchmark
on: [push, pull_request]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    - uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
    - name: Run benchmarks
      run: |
        cargo run --example benchmark_cli -- comprehensive > benchmark_results.txt
    - name: Upload results
      uses: actions/upload-artifact@v4
      with:
        name: benchmark-results
        path: benchmark_results.txt
```

## Contributing

To add new benchmarking features:

1. Extend `BenchmarkConfig` with new parameters
2. Update `BenchmarkResult` with new metrics
3. Implement measurement logic in `BenchmarkRunner`
4. Add corresponding tests
5. Update documentation

## License

This benchmarking suite is part of RatNet-rs and is licensed under GPL-3.0. 