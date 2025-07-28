use clap::{App, Arg};
use ratnet::prelude::*;
use ratnet::{
    database::{Database, SqliteDatabase},
    nodes::DatabaseNode,
    policy::ServerPolicy,
    router::DefaultRouter,
};
#[cfg(feature = "https")]
use ratnet::transports::HttpsTransport;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let matches = App::new("ratnet")
        .version("1.0")
        .about("RatNet - A secure, decentralized messaging network")
        .arg(
            Arg::with_name("dbfile")
                .short('d')
                .long("dbfile")
                .value_name("FILE")
                .help("Database file path")
                .default_value("ratnet.db"),
        )
        .arg(
            Arg::with_name("public-port")
                .short('p')
                .long("public-port")
                .value_name("PORT")
                .help("HTTPS Public Port")
                .default_value("20001"),
        )
        .arg(
            Arg::with_name("admin-port")
                .short('a')
                .long("admin-port")
                .value_name("PORT")
                .help("HTTPS Admin Port")
                .default_value("20002"),
        )
        .arg(
            Arg::with_name("mode")
                .short('m')
                .long("mode")
                .value_name("MODE")
                .help("Server mode: 'server' or 'p2p'")
                .default_value("server"),
        )
        .get_matches();

    let db_file = matches.value_of("dbfile").unwrap();
    let public_port: u16 = matches.value_of("public-port").unwrap().parse()?;
    let admin_port: u16 = matches.value_of("admin-port").unwrap().parse()?;
    let mode = matches.value_of("mode").unwrap();

    info!("Starting RatNet in {} mode", mode);
    info!("Database: {}", db_file);
    info!("Public port: {}", public_port);
    info!("Admin port: {}", admin_port);

    // Initialize component registrations
    ratnet::init();

    // Create database
    #[cfg(feature = "sqlite")]
    let database = Arc::new(
        SqliteDatabase::new(&format!("sqlite://{}", db_file))
            .await
            .expect("Failed to create database"),
    );

    // Bootstrap database schema
    #[cfg(feature = "sqlite")]
    database
        .bootstrap()
        .await
        .expect("Failed to bootstrap database");

    // Create database-backed node
    #[cfg(feature = "sqlite")]
    let node = Arc::new(
        DatabaseNode::new(database.clone())
            .await
            .expect("Failed to create database node"),
    );

    #[cfg(not(feature = "sqlite"))]
    let node = Arc::new(ratnet::nodes::MemoryNode::new());

    // Create router
    let router = Arc::new(DefaultRouter::new());
    node.set_router(router);

    // Generate SSL certificate and key
    let (cert_pem, key_pem) = ratnet::crypto::certs::generate_test_cert()?;

    // Create transports
    #[cfg(feature = "https")]
    let public_transport = Arc::new(HttpsTransport::new_with_certs(
        format!("0.0.0.0:{}", public_port),
        cert_pem.clone(),
        key_pem.clone(),
        true,
        node.clone(),
    ));
    #[cfg(not(feature = "https"))]
    let public_transport = Arc::new(ratnet::transports::UdpTransport::new(
        format!("0.0.0.0:{}", public_port),
    ));

    #[cfg(feature = "https")]
    let admin_transport = Arc::new(HttpsTransport::new_with_certs(
        format!("127.0.0.1:{}", admin_port),
        cert_pem,
        key_pem,
        true,
        node.clone(),
    ));
    #[cfg(not(feature = "https"))]
    let admin_transport = Arc::new(ratnet::transports::UdpTransport::new(
        format!("127.0.0.1:{}", admin_port),
    ));

    // Create policies based on mode
    let policies: Vec<Arc<dyn Policy>> = match mode {
        "server" => {
            let public_policy = Arc::new(ServerPolicy::new(
                public_transport.clone(),
                format!("0.0.0.0:{}", public_port),
                false,
            )) as Arc<dyn Policy>;
            let admin_policy = Arc::new(ServerPolicy::new(
                admin_transport.clone(),
                format!("127.0.0.1:{}", admin_port),
                true,
            )) as Arc<dyn Policy>;
            vec![public_policy, admin_policy]
        }
        "p2p" => {
            // For P2P mode, we'd need to implement P2P policy
            // For now, fall back to server mode
            info!("P2P mode not yet implemented, falling back to server mode");
            let public_policy = Arc::new(ServerPolicy::new(
                public_transport.clone(),
                format!("0.0.0.0:{}", public_port),
                false,
            )) as Arc<dyn Policy>;
            let admin_policy = Arc::new(ServerPolicy::new(
                admin_transport.clone(),
                format!("127.0.0.1:{}", admin_port),
                true,
            )) as Arc<dyn Policy>;
            vec![public_policy, admin_policy]
        }
        _ => {
            error!("Invalid mode: {}. Use 'server' or 'p2p'", mode);
            return Err("Invalid mode".into());
        }
    };

    node.set_policy(policies);

    // Start the node
    node.start().await?;

    info!("Public server starting on port {}", public_port);
    info!("Admin server starting on port {}", admin_port);

    // Get and display node ID
    match node.id().await {
        Ok(id) => info!("Node ID: {}", id),
        Err(e) => error!("Failed to get node ID: {}", e),
    }

    // Keep the application running
    info!("RatNet server is running. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("Shutting down RatNet server...");

    node.stop().await?;
    info!("RatNet server stopped.");

    Ok(())
}
