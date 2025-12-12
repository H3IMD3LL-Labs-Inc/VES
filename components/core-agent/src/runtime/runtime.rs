// Local crates
use crate::{
    buffer_batcher::log_buffer_batcher::InMemoryBuffer, helpers::{load_config::Config, shutdown::Shutdown}, proto::collector::log_collector_server::LogCollectorServer, server::server::LogCollectorService, shipper::shipper::Shipper, tailer::tailer::Tailer, watcher::watcher::LogWatcher
};

// External crates
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::{signal, sync::{Mutex, mpsc}, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;
use tracing::{info, debug, error, instrument};

/// Core Agent runtime initialization and setup.
#[instrument(
    name = "core_agent_runtime::run",
    target = "runtime::runtime",
    skip_all,
    level = "debug"
)]
pub async fn run_log_collector(config_path: PathBuf) -> Result<()> {
    info!("Starting Core Agent...");

    // Initialize global shutdown broadcaster channel
    let shutdown = Shutdown::new();
    info!("Initialized global Shutdown broadcaster channel");
    let shutdown_signal = shutdown.clone();

    // Initialize CancellationToken
    let cancel_token = CancellationToken::new();
    info!("Created task CancellationToken");

    // Load Core Agent configurations
    info!(
        configuration_file_path = %config_path.display(),
        "Loading Core Agent configuration file"
    );
    let cfg = Config::load(&config_path)?;
    info!(
        configuration_file_path = %config_path.display(),
        "Successfully loaded Core Agent configuration file"
    );

    // Data processing pipeline stages channels
    let (watcher_tx, tailer_rx) = mpsc::channel(1024);
    let (tailer_tx, parser_rx) = mpsc::channel(1024);
    let (parser_tx, buffer_batcher_rx) = mpsc::channel(1024);
    let (buffer_batcher_tx, shipper_rx) = mpsc::channel(1024);
    info!("Created pipeline channels...");

    // Pipeline component instantiation
    let watcher = LogWatcher::new_watcher(cfg.clone(), watcher_tx);
    let tailer = Tailer::new_tailer(tailer_rx, tailer_tx);
    let parser = Parser::new(parser_rx, parser_tx);
    let buffer_batcher = InMemoryBuffer::new(buffer_batcher_rx, buffer_batcher_tx);
    let shipper = Shipper::new(shipper_rx, cfg.clone());
    info!("Initialized pipeline components...");

    // Pipeline tasks
    let mut tasks: Vec<JoinHandle<()>> = Vec::new();

    tasks.push(tokio::spawn({
        let s = shutdown.subscribe();
        let c = cancel_token.child_token();
        async move { watcher.run_watcher(s, c).await }
    }));
    tasks.push(tokio::spawn({
        let s = shutdown.subscribe();
        let c = cancel_token.child_token();
        async move { tailer.run_tailer(s, c).await }
    }));
    tasks.push(tokio::spawn({
        let s = shutdown.subscribe();
        let c = cancel_token.child_token();
        async move { parser.run(s, c).await }
    }));
    tasks.push(tokio::spawn({
        let s = shutdown.subscribe();
        let c = cancel_token.child_token();
        async move { buffer_batcher.run(s, c).await }
    }));
    tasks.push(tokio::spawn({
        let s = shutdown.subscribe();
        let c = cancel_token.child_token();
        async move { shipper.run(s, c).await }
    }));

    tokio::spawn({
       let shutdown_signal = shutdown_signal.clone();
       let cancel_token = cancel_token.clone();
       async move {
           // Wait for a signal
           let _ = tokio::signal::ctrl_c()
               .await
               .expect("Failed to listen for CTRL+C");
           info!("CTRL+C received..triggering shutdown...");

           cancel_token.cancel();
           shutdown_signal.trigger();
       }
    });

    // Wait for all tasks to exit
    for handle in tasks {
        let _ = handle.await;
    }
    info!("Core Agent gracefully shut down");
    Ok(())
}
