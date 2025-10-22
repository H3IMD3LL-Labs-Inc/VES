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
use std::path::PathBuf;
use std::sync::Arc;
use tonic::transport::Server;

use crate::buffer_batcher::log_buffer_batcher::InMemoryBuffer;
use crate::helpers::load_config::Config;
use crate::proto::collector::log_collector_server::LogCollectorServer;
use crate::server::server::LogCollectorService;
use crate::shipper::shipper::Shipper;
use crate::watcher::watcher::LogWatcher;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load config
    println!("⏳ Loading configurations....");
    let cfg = Config::load("log_collector.toml")?;

    // Initialize log collector components
    println!("🤖 Initializing Log Collector....");
    let buffer = InMemoryBuffer::new(&cfg.buffer).await;
    let shipper = Shipper::new(&cfg.shipper).await;
    let parser = Default::default(); // TODO: replace with configurable Parser

    // Create shared LogCollectorService instance
    let service = Arc::new(LogCollectorService {
        parser,
        buffer_batcher: buffer,
        shipper,
    });

    // Spawn Local Watcher (optional)
    if cfg.general.enable_local_mode {
        if let Some(wcfg) = &cfg.watcher {
            if wcfg.enabled {
                println!("📂 Starting local file watcher on: {}", wcfg.log_dir);
                let log_dir = PathBuf::from(&wcfg.log_dir);
                let checkpoint_path = PathBuf::from(&wcfg.checkpoint_path);

                // Clone the Arc so watcher shares the internal components with `network_mode`
                let service_clone = Arc::clone(service);

                tokio::spawn(async move {
                    match LogWatcher::new_watcher(log_dir, checkpoint_path).await {
                        Ok(mut watcher) => {
                            if let Err(e) = watcher.run_watcher().await {
                                eprintln!("Watcher error: {e}");
                            }
                        }
                        Err(e) => eprintln!("Failed to start watcher: {e}"),
                    }
                });
            } else {
                println!("Local log watcher disabled in [watcher] config.");
            }
        } else {
            println!("No watcher config found in config file, skipping local mode.");
        }
    } else {
        println!("Local mode disabled in [general] config.")
    }

    // Start gRPC server (optional)
    if cfg.general.enable_network_mode {
        println!("🌐 Starting gRPC server on [::1]:50052");

        Server::builder()
            .add_service(LogCollectorServer::new((*service).clone()))
            .serve("[::1]:50052".parse()?)
            .await?;
    } else {
        println!("🌐 Network mode disabled in [general] config.");
    }
}
