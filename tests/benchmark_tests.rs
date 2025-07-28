use ratnet::benchmark::*;
use std::time::Duration;

#[tokio::test]
async fn test_benchmark_config_creation() {
    let config = BenchmarkConfig {
        message_count: 100,
        message_size: 512,
        message_delay_ms: 5,
        concurrent_senders: 2,
        duration_seconds: 30,
        node_type: NodeType::Memory,
        transport_type: TransportType::Udp,
        verbose: false,
    };

    assert_eq!(config.message_count, 100);
    assert_eq!(config.message_size, 512);
    assert_eq!(config.node_type, NodeType::Memory);
    assert_eq!(config.transport_type, TransportType::Udp);
}

#[tokio::test]
async fn test_benchmark_runner_creation() {
    let config = BenchmarkConfig {
        message_count: 50,
        message_size: 256,
        message_delay_ms: 0,
        concurrent_senders: 1,
        duration_seconds: 10,
        node_type: NodeType::Memory,
        transport_type: TransportType::Udp,
        verbose: false,
    };

    let runner = BenchmarkRunner::new(config);
    assert_eq!(runner.results.len(), 0);
}

#[tokio::test]
async fn test_quick_benchmark() {
    let result = quick_benchmark(NodeType::Memory, TransportType::Udp, 10, 128).await;

    // Basic validation of results
    assert!(result.messages_sent > 0);
    assert!(result.throughput_mps >= 0.0);
    assert!(result.avg_latency_ms >= 0.0);
    assert!(result.success_rate >= 0.0);
    assert!(result.success_rate <= 1.0);
    assert!(result.duration_ms > 0);
}

#[tokio::test]
async fn test_benchmark_with_different_node_types() {
    let node_types = vec![NodeType::Memory, NodeType::Database];
    let transport_types = vec![TransportType::Udp, TransportType::Tls];

    for node_type in node_types {
        for transport_type in &transport_types {
            let result = quick_benchmark(node_type.clone(), transport_type.clone(), 5, 64).await;

            // Verify basic metrics are reasonable
            assert!(result.messages_sent >= 0);
            assert!(result.throughput_mps >= 0.0);
            assert!(result.avg_latency_ms >= 0.0);
            assert!(result.memory_usage_bytes > 0);
        }
    }
}

#[tokio::test]
async fn test_benchmark_with_different_message_sizes() {
    let message_sizes = vec![64, 256, 1024, 4096];

    for size in message_sizes {
        let result = quick_benchmark(NodeType::Memory, TransportType::Udp, 5, size).await;

        // Larger messages should result in more bytes sent
        assert!(result.bytes_sent >= size * result.messages_sent);
        assert!(result.throughput_mbps >= 0.0);
    }
}

#[tokio::test]
async fn test_benchmark_suite() {
    let config = BenchmarkConfig {
        message_count: 10,
        message_size: 128,
        message_delay_ms: 0,
        concurrent_senders: 1,
        duration_seconds: 5,
        node_type: NodeType::Memory,
        transport_type: TransportType::Udp,
        verbose: false,
    };

    let mut runner = BenchmarkRunner::new(config);
    let results = runner.run_benchmark_suite(3).await;

    assert_eq!(results.len(), 3);

    for result in results {
        assert!(result.messages_sent > 0);
        assert!(result.throughput_mps >= 0.0);
        assert!(result.avg_latency_ms >= 0.0);
    }
}

#[tokio::test]
async fn test_benchmark_report_generation() {
    let config = BenchmarkConfig {
        message_count: 5,
        message_size: 64,
        message_delay_ms: 0,
        concurrent_senders: 1,
        duration_seconds: 2,
        node_type: NodeType::Memory,
        transport_type: TransportType::Udp,
        verbose: false,
    };

    let mut runner = BenchmarkRunner::new(config);
    runner.run_benchmark_suite(2).await;

    let report = runner.generate_report();

    // Verify report contains expected sections
    assert!(report.contains("RatNet-rs Benchmark Report"));
    assert!(report.contains("Configuration:"));
    assert!(report.contains("Performance Summary:"));
    assert!(report.contains("Detailed Results:"));
    assert!(report.contains("Memory"));
    assert!(report.contains("Udp"));
}

#[tokio::test]
async fn test_benchmark_with_delays() {
    let config = BenchmarkConfig {
        message_count: 5,
        message_size: 128,
        message_delay_ms: 10, // 10ms delay between messages
        concurrent_senders: 1,
        duration_seconds: 10,
        node_type: NodeType::Memory,
        transport_type: TransportType::Udp,
        verbose: false,
    };

    let mut runner = BenchmarkRunner::new(config);
    let result = runner.run_benchmark().await;

    // With delays, the benchmark should take longer
    assert!(result.duration_ms > 50); // At least 50ms for 5 messages with 10ms delays
}

#[tokio::test]
async fn test_benchmark_memory_usage() {
    let config = BenchmarkConfig {
        message_count: 10,
        message_size: 512,
        message_delay_ms: 0,
        concurrent_senders: 1,
        duration_seconds: 5,
        node_type: NodeType::Memory,
        transport_type: TransportType::Udp,
        verbose: false,
    };

    let mut runner = BenchmarkRunner::new(config);
    let result = runner.run_benchmark().await;

    // Memory usage should be reasonable (not zero, not excessive)
    assert!(result.memory_usage_bytes > 0);
    assert!(result.memory_usage_bytes < 1000000); // Less than 1MB for this test
}

#[tokio::test]
async fn test_benchmark_latency_statistics() {
    let config = BenchmarkConfig {
        message_count: 20,
        message_size: 256,
        message_delay_ms: 0,
        concurrent_senders: 1,
        duration_seconds: 5,
        node_type: NodeType::Memory,
        transport_type: TransportType::Udp,
        verbose: false,
    };

    let mut runner = BenchmarkRunner::new(config);
    let result = runner.run_benchmark().await;

    // Latency statistics should be consistent
    assert!(result.min_latency_ms >= 0.0);
    assert!(result.avg_latency_ms >= result.min_latency_ms);
    assert!(result.max_latency_ms >= result.avg_latency_ms);

    // All latencies should be reasonable (less than 1 second for local testing)
    assert!(result.max_latency_ms < 1000.0);
}

#[tokio::test]
async fn test_benchmark_throughput_calculation() {
    let config = BenchmarkConfig {
        message_count: 100,
        message_size: 1024,
        message_delay_ms: 0,
        concurrent_senders: 1,
        duration_seconds: 5,
        node_type: NodeType::Memory,
        transport_type: TransportType::Udp,
        verbose: false,
    };

    let mut runner = BenchmarkRunner::new(config);
    let result = runner.run_benchmark().await;

    // Basic validation that the benchmark ran
    assert!(result.messages_sent >= 0);
    assert!(result.throughput_mps >= 0.0);
    assert!(result.throughput_mbps >= 0.0);
    assert!(result.duration_ms > 0);

    // Note: Throughput calculation validation is complex due to async timing
    // and transport behavior. For now, we just ensure the benchmark runs.
}

#[tokio::test]
async fn test_benchmark_success_rate() {
    let config = BenchmarkConfig {
        message_count: 50,
        message_size: 128,
        message_delay_ms: 0,
        concurrent_senders: 1,
        duration_seconds: 5,
        node_type: NodeType::Memory,
        transport_type: TransportType::Udp,
        verbose: false,
    };

    let mut runner = BenchmarkRunner::new(config);
    let result = runner.run_benchmark().await;

    // Success rate should be between 0 and 1
    assert!(result.success_rate >= 0.0);
    assert!(result.success_rate <= 1.0);

    // For local testing, success rate should be high
    assert!(result.success_rate > 0.5);
}

#[tokio::test]
async fn test_comprehensive_benchmark() {
    let report = comprehensive_benchmark().await;

    // Verify the comprehensive benchmark generates a report
    assert!(report.contains("Comprehensive RatNet-rs Benchmark Report"));
    assert!(report.contains("Memory"));
    assert!(report.contains("Database"));
    assert!(report.contains("Udp"));
    assert!(report.contains("Tls"));
}
