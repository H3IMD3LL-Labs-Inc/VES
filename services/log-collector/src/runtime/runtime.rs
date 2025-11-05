use crate::buffer_batcher::log_buffer_batcher::InMemoryBuffer;
use crate::helpers::load_config::Config;
use crate::metrics::http::start_metrics_server;
use crate::proto::collector::log_collector_server::LogCollectorServer;
use crate::server::server::LogCollectorService;
use crate::shipper::shipper::Shipper;
use crate::watcher::watcher::LogWatcher;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tonic::transport::Server;

pub async fn run_log_collector(config_path: PathBuf) -> Result<()> {
    // TODO: Ensure proper tracing is added to ensure observability, monitoring and debugging.
    // TODO: Add shutdown signal and shutdown signal propagation to ensure correct, efficient and error free shutdowns.

    // Start metrics server
    tokio::spawn(async {
        start_metrics_server("0.0.0.0:9000").await;
    });

    // Load config
    println!("Loading configurations...");
    let cfg = Config::load("log_collector.toml");

    // Initialize log collector components
    println!("Initializing Log Collector...");
    let buffer = InMemoryBuffer::new(&cfg.buffer).await;
    let shipper = Shipper::new(&cfg.shipper).await;
    let parser = Default::default(); // TODO: Replace with configurable parser

    // Create shared LogCollectorService instance
    let service = Arc::new(LogCollectorService {
        parser,
        buffer_batcher: buffer,
        shipper,
    });

    // Spawn local logs watcher
    if cfg.general.enable_local_mode {
        if let Some(wcfg) = &cfg.watcher {
            if wcfg.enabled {
                println!("Starting local file watcher on: {}", wcfg.log_dir);
                let log_dir = PathBuf::from(&wcfg.log_dir);
                let checkpoint_path = PathBuf::from(&wcfg.checkpoint_path);

                // Clone the Arc so watcher shares the internal components with `network_mode_enabled`
                let shared_service = Arc::clone(service);

                tokio::spawn(async move {
                    // Builder function — creates and configures LogWatcher before it starts doing
                    // any work. Takes all parameters controlling how it behaves at runtime.
                    match LogWatcher::new_watcher(
                        log_dir,
                        checkpoint_path,
                        shared_service,
                        wcfg.poll_interval_ms,
                        wcfg.recursive,
                    )
                    .await
                    {
                        Ok(mut watcher) => {
                            // Runtime function — run LogWatcher with parameters controlling how it behaves at runtime
                            if let Err(e) = watcher.run_watcher().await {
                                eprintln!("Watcher error: {e}");
                            }
                        }
                        Err(e) => eprintln!("Failed to start watcher: {e}"),
                    }
                });
            } else {
                println!("Local log watcher disabled in [watcher] configuration.");
            }
        } else {
            println!("No watcher configuration found in configuration file, skipping local mode.");
        }
    } else {
        println!("Local mode disabled in [general] configuration.");
    }

    // Start gRPC server
    if cfg.general.enable_network_mode {
        println!("Starting gRPC server on [::1]:50052");

        Server::builder()
            .add_service(LogCollectorServer::new((*service).clone()))
            .server("[::1]:50052".parse()?)
            .await?;
    } else {
        println!("Network mode disabled in [general] configuration.");
    }
}
