mod buffer_batcher;
mod control_observability;
mod filter_redactor;
mod helpers;
mod metadata_enricher;
mod models;
mod parser;
mod proto;
mod server;
mod shipper;
mod tailer;
mod watcher;

use anyhow::Result;
use tonic::transport::Server;

use crate::buffer_batcher::log_buffer_batcher::InMemoryBuffer;
use crate::helpers::load_config::Config;
use crate::proto::collector::log_collector_server::LogCollectorServer;
use crate::server::server::LogCollectorService;
use crate::shipper::shipper::Shipper;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load config
    println!("⏳ Loading configurations....");
    let cfg = Config::load("log_collector.toml")?;

    // Initialize log collector components
    println!("🤖 Initializing Log Collector....");
    let buffer = InMemoryBuffer::new(&cfg.buffer).await;
    let shipper = Shipper::new(&cfg.shipper).await;
    // TODO: Add watcher configurations
    // TODO: Add parser configurations

    // Create shared LogCollectorService instance
    println!("🏗️ Building gRPC server...");
    let service = LogCollectorService {
        parser: Default::default(),
        buffer_batcher: buffer,
        shipper,
    };

    // TODO: Spawn Local Watcher (optional)
    // watcher.rs/tailer.rs -> parser.rs -> log_buffer_batcher.rs -> shipper.rs

    // Starts gRPC server (optional)
    println!("🚀 Starting gRPC server on [::1]:50082");
    Server::builder()
        .add_service(LogCollectorServer::new(service))
        .serve("[::1]:50052".parse()?)
        .await?;

    Ok(())
}
