// Local crates
use crate::{
    buffer_batcher::log_buffer_batcher::InMemoryBuffer,
    helpers::log_processing::LAT_HISTOGRAM,
    helpers::{load_config::Config, shutdown::Shutdown},
    metrics::http::start_metrics_server,
    metrics::metrics::{
        CPU_PERCENT_PER_CORE, LOGS_PROCESSED_THIS_SECOND, MEMORY_BYTES, P99_LATENCY_MS,
        STARTUP_DURATION_SECONDS, THROUGHPUT_LOGS_PER_SEC,
    },
    proto::collector::log_collector_server::LogCollectorServer,
    server::server::LogCollectorService,
    shipper::shipper::Shipper,
    watcher::watcher::LogWatcher,
};

// External crates
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use sysinfo::System;
use tokio::{signal, sync::Mutex, task::JoinHandle};
use tonic::transport::Server;
use tracing::{Instrument, info_span, instrument};

/// Log Collector runtime initialization and setup.
///
/// This is where the Log Collector creates and starts its runtime environment.
/// Everything needed at runtime; components, configs, etc. is set here.
#[instrument(
    name = "ves_runtime",
    target = "runtime::runtime",
    skip_all,
    level = "trace"
)]
pub async fn run_log_collector(config_path: PathBuf) -> Result<()> {
    tracing::trace!(
        configuration_file_path = %config_path.display(),
        "Starting log collector runtime with configurations"
    );

    // Start measuring cold start time
    let cold_startup_start = Instant::now();

    // Initialize global shutdown channel
    let shutdown = Shutdown::new();
    tracing::trace!("Initialized global shutdown channel");
    let shutdown_signal = shutdown.clone();

    // Start Ctrc+C shutdown signal listener
    tokio::spawn(
        async move {
            signal::ctrl_c()
                .await
                .expect("Failed to listen for CTRL+C shutdown signal");
            tracing::trace!("CTRL+C shutdown signal detected, broadcasting shutdown");
            shutdown_signal.trigger();
        }
        // Attach named span for clarity
        .instrument(info_span!("ves_shutdown_signal_listener")),
    );

    // Start metrics server (background task)
    tokio::spawn({
        let mut shutdown_rx = shutdown.subscribe();
        async move {
            tracing::trace!("Metrics server task started");

            tokio::select! {
                _ = start_metrics_server("0.0.0.0:9000") => {
                    tracing::trace!("Metrics server task exited normally");
                },
                _ = shutdown_rx.recv() => {
                    tracing::trace!("Metrics server task shutting down");
                }
            }
        }
        // Attach named span for clarity
        .instrument(info_span!(
            "ves_metrics_server",
            listen_addr = "0.0.0.0:9000"
        ))
    });

    // Start log throughput auto-refresh background task
    tokio::spawn({
        let mut shutdown_rx = shutdown.subscribe();
        async move {
            tracing::trace!("Logs throughput auto-refresh background task started");

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        tracing::trace!("Logs throughput auto-refresh background task shutting down");
                        break;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                        // Get number of logs processed this second
                        let logs_processed_this_second = LOGS_PROCESSED_THIS_SECOND.get();

                        // Update throughput gauge
                        THROUGHPUT_LOGS_PER_SEC.set(logs_processed_this_second);

                        // Reset counter
                        LOGS_PROCESSED_THIS_SECOND.reset();
                    }
                }
            }
        }
        .instrument(info_span!(
            "ves_log_throughput_updater"
        ))
    });

    // Start p99 latency auto-refresh background task
    tokio::spawn({
        let mut shutdown_rx = shutdown.subscribe();
        async move {
            tracing::trace!("p99 latency auto-refresh background task started");

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        tracing::trace!("p99 latency auto-refresh background task shutting down");
                        break;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                        let mut hist = LAT_HISTOGRAM.lock().unwrap();
                        if hist.len() > 0 {
                            let p99_ms = hist.value_at_quantile(0.99) as f64 / 1000.0;
                            P99_LATENCY_MS.set(p99_ms);
                            hist.reset();
                        }
                    }
                }
            }
        }
        .instrument(info_span!("ves_p99_latency_updater"))
    });

    // Start memory and CPU usage auto-refresh background task
    tokio::spawn({
        let mut shutdown_rx = shutdown.subscribe();
        async move {
            tracing::trace!("Node metrics auto-refresh background task started");

            let mut sys = System::new_all();
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        tracing::trace!("Node metrics auto-refresh background task started");
                        break;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                        sys.refresh_all();

                        // Memory (bytes)
                        let mem_bytes = sys.used_memory() * 1024; // Convert KiB -> bytes
                        MEMORY_BYTES.set(mem_bytes as f64);

                        // Average CPU load across all cores
                        let avg_cpu = sys.cpus()
                            .iter()
                            .map(|cpu| cpu.cpu_usage() as f64)
                            .sum::<f64>() / sys.cpus().len() as f64;
                        CPU_PERCENT_PER_CORE.set(avg_cpu);

                        tracing::trace!(cpu_percent = avg_cpu, mem_bytes = mem_bytes, "Node metrics updated");
                    }
                }
            }
        }
        .instrument(info_span!(
            "ves_node_metrics_updater"
        ))
    });

    // Load Log Collector configurations
    tracing::trace!(
        configuration_file_path = %config_path.display(),
        "Loading VES configuration file"
    );
    let cfg = Config::load(&config_path)?;
    tracing::trace!(
        configuration_file_path = %config_path.display(),
        "VES configuration file loaded successfully"
    );

    // Initialize Log Collector shared sub-systems
    tracing::trace!("Initializing VES log processing components");
    let buffer = Arc::new(Mutex::new(InMemoryBuffer::new(cfg.buffer).await));
    let shipper = Arc::new(Mutex::new(Shipper::new(cfg.shipper).await));
    let parser = Default::default(); // TODO: Replace with configurable parser, leave as is for now....
    tracing::trace!(
        buffer = ?buffer,
        shipper = ?shipper,
        parser = ?parser,
        "VES log processing components initialized successfully"
    );

    // Shared gRPC service state, ensuring both Local and Network modes
    // share the same state
    let service = Arc::new(LogCollectorService {
        parser,
        buffer_batcher: Arc::clone(&buffer),
        shipper: Arc::clone(&shipper),
    });

    let mut task_handles: Vec<JoinHandle<()>> = Vec::new();

    // Spawn local logs watcher (local-mode)
    if cfg.general.enable_local_mode {
        if let Some(wcfg) = &cfg.watcher {
            if wcfg.enabled {
                tracing::trace!(
                    log_dir = %wcfg.log_dir,
                    "Starting local file watcher"
                );

                let log_dir = PathBuf::from(&wcfg.log_dir);
                let checkpoint_path = PathBuf::from(&wcfg.checkpoint_path);
                let poll_interval_ms = wcfg.poll_interval_ms;
                let recursive = wcfg.recursive;

                // Clone the Arc so watcher shares the internal components with `network_mode_enabled`
                let shared_service = Arc::clone(&service);

                // Subscribe to the global shutdown channel
                let shutdown_rx1 = shutdown.subscribe();
                let mut shutdown_rx2 = shutdown.subscribe();

                let handle = tokio::spawn(async move {
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
                                res = watcher.run_watcher(shutdown_rx1) => {
                                    if let Err(e) = res {
                                        tracing::error!(error = %e, "Watcher error");
                                    }
                                }
                                _ = shutdown_rx2.recv() => {
                                    tracing::trace!("Watcher received shutdown signal, cleaning up resources");
                                    watcher.shutdown().await;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to start watcher");
                        }
                    }
                });

                task_handles.push(handle);
            } else {
                tracing::info!("Local log watcher disabled in [watcher] configuration");
            }
        } else {
            tracing::info!(
                "No watcher configuration found in configuration file, skipping local mode."
            );
        }
    } else {
        tracing::info!("Local mode disabled in [general] configuration.");
    }

    // Start gRPC server
    if cfg.general.enable_network_mode {
        tracing::trace!(grpc_server_addr = "[::1]:50052", "Starting gRPC server");

        // Subscribe to the global shutdown channel
        let mut shutdown_rx = shutdown.subscribe();

        let addr = "[::1]:50052".parse()?;
        let service_clone = (*service).clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = Server::builder()
                .add_service(LogCollectorServer::new(service_clone))
                .serve_with_shutdown(addr, async move {
                    shutdown_rx.recv().await.ok();
                    tracing::trace!("VES gRPC server shutting down gracefully");
                })
                .await
            {
                tracing::error!(error = %e, "VES gRPC server error");
            }
        });

        task_handles.push(handle);
    } else {
        tracing::info!("Network mode disabled in [general] configuration");
    }

    // Record cold start duratiion in seconds
    let cold_startup_duration = cold_startup_start.elapsed().as_secs_f64();
    STARTUP_DURATION_SECONDS.set(cold_startup_duration);
    println!(
        "Cold start completed in {:.2}ms",
        cold_startup_duration * 1000.0
    );

    // Await shutdown signal
    shutdown.wait_for_shutdown().await;
    tracing::info!("VES global shutdown triggered, awaiting log processing components to finish");

    // Await all task to ensure clean shutdown
    for handle in task_handles {
        let _ = handle.await;
    }

    // Perform final clean up
    let mut buf = buffer.lock().await;
    buf.shutdown().await?;

    // Flush any pending logs before shipper exits
    let mut shipper_lock = shipper.lock().await;
    shipper_lock.shutdown().await;

    tracing::info!("VES successfully shut down");
    Ok(())
}
