use crate::{
    buffer_batcher::log_buffer_batcher::InMemoryBuffer,
    helpers::{load_config::Config, shutdown::Shutdown},
    metrics::http::start_metrics_server,
    proto::collector::log_collector_server::LogCollectorServer,
    server::server::LogCollectorService,
    shipper::shipper::Shipper,
    watcher::watcher::LogWatcher,
};

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::{signal, sync::Mutex};
use tonic::transport::Server;

pub async fn run_log_collector(config_path: PathBuf) -> Result<()> {
    // TODO: Ensure proper tracing is added for debugging

    // Start metrics server
    tokio::spawn(async {
        start_metrics_server("0.0.0.0:9000").await;
    });

    // Initialize global shutdown channel
    let shutdown = Shutdown::new();

    // Clone shutdown channel for signal listener
    let shutdown_signal = shutdown.clone();

    // Load config
    println!("Loading configurations...");
    let cfg = Config::load(&config_path)?;
    println!("Configuration loaded successfully...");

    // Initialize log collector components
    println!("Initializing Log Collector components...");
    let buffer = Arc::new(Mutex::new(InMemoryBuffer::new(cfg.buffer).await));
    let shipper = Shipper::new(cfg.shipper).await;
    let parser = Default::default(); // TODO: Replace with configurable parser, leave as is for now....
    println!("Log Collector components initialized successfully...");

    // Create shared LogCollectorService instance
    let service = Arc::new(LogCollectorService {
        parser,
        buffer_batcher: Arc::clone(&buffer),
        shipper,
    });

    // Spawn local logs watcher (local-mode)
    if cfg.general.enable_local_mode {
        if let Some(wcfg) = &cfg.watcher {
            if wcfg.enabled {
                println!("Starting local file watcher on: {}", wcfg.log_dir);

                let log_dir = PathBuf::from(&wcfg.log_dir);
                let checkpoint_path = PathBuf::from(&wcfg.checkpoint_path);
                let poll_interval_ms = wcfg.poll_interval_ms;
                let recursive = wcfg.recursive;

                // Clone the Arc so watcher shares the internal components with `network_mode_enabled`
                let shared_service = Arc::clone(&service);

                // Subscribe to the global shutdown channel
                let mut shutdown_rx = shutdown.subscribe();

                tokio::spawn(async move {
                    // Builder function — creates and configures LogWatcher before it starts doing
                    // any work. Takes all parameters controlling how it behaves at runtime.
                    match LogWatcher::new_watcher(
                        log_dir,
                        checkpoint_path,
                        poll_interval_ms,
                        recursive,
                        shared_service,
                    )
                    .await
                    {
                        Ok(mut watcher) => {
                            tokio::select! {
                                res = watcher.run_watcher() => {
                                    if let Err(e) = res {
                                        eprintln!("Watcher error: {e}");
                                    }
                                }
                                _ = shutdown_rx.recv() => {
                                    println!("Watcher received shutdown signal, cleaning up resources...");
                                    // TODO: Actually implement `.stop()` method in Watcher for use when a shutdown signal is received
                                    watcher.stop().await;
                                }
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

        // Subscribe to the global shutdown channel
        let mut shutdown_rx = shutdown.subscribe();
        let addr = "[::1]:50052".parse()?;
        let service_clone = (*service).clone();

        tokio::select! {
            res = Server::builder()
                .add_service(LogCollectorServer::new(service_clone))
                .serve(addr) => {
                    if let Err(e) = res {
                        eprintln!("gRPC server error: {e}");
                    }
                }

            _ = shutdown_rx.recv() => {
                println!("Shutdown signal received, stopping gRPC server...");
                // Server::builder() doesn't provide a stop API directly, this just ensures
                // it exits promptly as serve() is canceled.
            }
        };
    } else {
        println!("Network mode disabled in [general] configuration.");
    }

    // Handle system signale
    tokio::spawn(async move {
        signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C signal");
        println!("Ctrl+C signal detected, broadcasting shutdown...");
        shutdown_signal.trigger();
    });

    // Await graceful termination
    shutdown.wait_for_shutdown().await;
    println!("Log Collector successfully shutdown.");

    Ok(())
}
