use clap::{App, Arg, SubCommand};
use ratnet::benchmark::*;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("RatNet-rs Benchmark Tool")
        .version("1.0")
        .about("Benchmark RatNet-rs performance with different configurations")
        .subcommand(SubCommand::with_name("quick")
            .about("Run a quick benchmark")
            .arg(Arg::with_name("node-type")
                .short('n')
                .long("node-type")
                .value_name("TYPE")
                .help("Node type: memory, database, filesystem")
                .takes_value(true)
                .default_value("memory"))
            .arg(Arg::with_name("transport")
                .short('t')
                .long("transport")
                .value_name("TRANSPORT")
                .help("Transport type: udp, tls, https")
                .takes_value(true)
                .default_value("udp"))
            .arg(Arg::with_name("messages")
                .short('m')
                .long("messages")
                .value_name("COUNT")
                .help("Number of messages to send")
                .takes_value(true)
                .default_value("1000"))
            .arg(Arg::with_name("size")
                .short('s')
                .long("size")
                .value_name("BYTES")
                .help("Message size in bytes")
                .takes_value(true)
                .default_value("1024")))
        .subcommand(SubCommand::with_name("comprehensive")
            .about("Run comprehensive benchmark suite"))
        .subcommand(SubCommand::with_name("custom")
            .about("Run custom benchmark with detailed configuration")
            .arg(Arg::with_name("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file (JSON)")
                .takes_value(true)
                .required(true))
            .arg(Arg::with_name("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output file for results")
                .takes_value(true)))
        .subcommand(SubCommand::with_name("stress")
            .about("Run stress test with high load")
            .arg(Arg::with_name("duration")
                .short('d')
                .long("duration")
                .value_name("SECONDS")
                .help("Test duration in seconds")
                .takes_value(true)
                .default_value("300"))
            .arg(Arg::with_name("concurrent")
                .short('c')
                .long("concurrent")
                .value_name("COUNT")
                .help("Number of concurrent senders")
                .takes_value(true)
                .default_value("10")))
        .get_matches();

    match matches.subcommand() {
        Some(("quick", args)) => {
            let node_type = match args.value_of("node-type").unwrap() {
                "memory" => NodeType::Memory,
                "database" => NodeType::Database,
                "filesystem" => NodeType::Filesystem,
                _ => {
                    eprintln!("Invalid node type. Use: memory, database, filesystem");
                    return Ok(());
                }
            };

            let transport_type = match args.value_of("transport").unwrap() {
                "udp" => TransportType::Udp,
                "tls" => TransportType::Tls,
                "https" => TransportType::Https,
                _ => {
                    eprintln!("Invalid transport type. Use: udp, tls, https");
                    return Ok(());
                }
            };

            let message_count = args.value_of("messages").unwrap().parse::<usize>()?;
            let message_size = args.value_of("size").unwrap().parse::<usize>()?;

            println!("Running quick benchmark...");
            println!("Node Type: {:?}", node_type);
            println!("Transport: {:?}", transport_type);
            println!("Messages: {}", message_count);
            println!("Message Size: {} bytes", message_size);

            let result = quick_benchmark(node_type, transport_type, message_count, message_size).await;
            
            println!("\n=== Quick Benchmark Results ===");
            println!("Messages Sent: {}", result.messages_sent);
            println!("Messages Received: {}", result.messages_received);
            println!("Bytes Sent: {}", result.bytes_sent);
            println!("Bytes Received: {}", result.bytes_received);
            println!("Throughput: {:.2} messages/sec", result.throughput_mps);
            println!("Throughput: {:.2} MB/sec", result.throughput_mbps);
            println!("Average Latency: {:.2} ms", result.avg_latency_ms);
            println!("Success Rate: {:.2}%", result.success_rate * 100.0);
            println!("Memory Usage: {} bytes", result.memory_usage_bytes);
            println!("Duration: {} ms", result.duration_ms);
        }

        Some(("comprehensive", _)) => {
            println!("Running comprehensive benchmark suite...");
            let report = comprehensive_benchmark().await;
            println!("{}", report);
        }

        Some(("custom", args)) => {
            let config_file = args.value_of("config").unwrap();
            let output_file = args.value_of("output");

            if !Path::new(config_file).exists() {
                eprintln!("Configuration file not found: {}", config_file);
                return Ok(());
            }

            let config_content = std::fs::read_to_string(config_file)?;
            let config: BenchmarkConfig = serde_json::from_str(&config_content)?;

            println!("Running custom benchmark...");
            println!("Configuration: {:?}", config);

            let mut runner = BenchmarkRunner::new(config);
            let results = runner.run_benchmark_suite(3).await;
            let report = runner.generate_report();

            if let Some(output_path) = output_file {
                std::fs::write(output_path, &report)?;
                println!("Results saved to: {}", output_path);
            } else {
                println!("{}", report);
            }
        }

        Some(("stress", args)) => {
            let duration = args.value_of("duration").unwrap().parse::<u64>()?;
            let concurrent = args.value_of("concurrent").unwrap().parse::<usize>()?;

            println!("Running stress test...");
            println!("Duration: {} seconds", duration);
            println!("Concurrent Senders: {}", concurrent);

            // Create stress test configuration
            let config = BenchmarkConfig {
                message_count: 10000,
                message_size: 4096,
                message_delay_ms: 0,
                concurrent_senders: concurrent,
                duration_seconds: duration,
                node_type: NodeType::Memory,
                transport_type: TransportType::Udp,
                verbose: true,
            };

            let mut runner = BenchmarkRunner::new(config);
            let results = runner.run_benchmark_suite(1).await;
            let report = runner.generate_report();

            println!("{}", report);
        }

        _ => {
            println!("Use --help for usage information");
        }
    }

    Ok(())
} 