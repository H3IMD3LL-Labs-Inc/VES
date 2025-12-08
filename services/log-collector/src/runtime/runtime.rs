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
use tracing::instrument;

/// Core Agent runtime initialization and setup.
#[instrument(
    name = "core_agent_runtime::run",
    target = "runtime::runtime",
    skip_all,
    level = "trace"
)]
pub async fn run_log_collector(config_path: PathBuf) -> Result<()> {
    tracing::info!("Starting Core Agent runtime");

    // Start measuring cold start time
    let cold_startup_start = Instant::now();

    // Initialize global shutdown channel
    let shutdown = Shutdown::new();
    tracing::debug!("Initialized runtime global shutdown channel");
    let shutdown_signal = shutdown.clone();

    // Start Ctrc+C shutdown signal listener
    tokio::spawn(async move {
        tracing::info!("Starting CTRL+C global shutdown signal listener");
        signal::ctrl_c()
            .await
            .expect("Failed to listen for CTRL+C shutdown signal");
        tracing::debug!(
            "CTRL+C shutdown signal detected, broadcasting shutdown to runtime components"
        );
        shutdown_signal.trigger();
    });

    // Start metrics server
    tokio::spawn({
        let mut shutdown_rx = shutdown.subscribe();
        async move {
            tracing::info!("Metrics server asynchronous background task started");

            tokio::select! {
                _ = start_metrics_server("0.0.0.0:9000") => {
                    tracing::debug!("Metrics server asynchronous background task exited normally");
                },
                _ = shutdown_rx.recv() => {
                    tracing::debug!("Metrics server asynchronous background task gracefully shutting down");
                }
            }
        }
    });

    // Start log throughput auto-refresh
    tokio::spawn({
        let mut shutdown_rx = shutdown.subscribe();
        async move {
            tracing::debug!("Logs throughput auto-refresh asynchronous background task started");

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        tracing::debug!("Logs throughput auto-refresh asynchronous background task gracefully shutting down");
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
    });

    // Start p99 latency auto-refresh background task
    tokio::spawn({
        let mut shutdown_rx = shutdown.subscribe();
        async move {
            tracing::debug!("p99 latency auto-refresh asynchronous background task started");

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        tracing::debug!("p99 latency auto-refresh asynchronous background task shutting down");
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
    });

    // Start memory and CPU usage auto-refresh background task
    tokio::spawn({
        let mut shutdown_rx = shutdown.subscribe();
        async move {
            tracing::debug!("Node metrics auto-refresh asynchronous background task started");

            let mut sys = System::new_all();
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        tracing::debug!("Node metrics auto-refresh asynchronous background task started");
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

                        tracing::debug!(cpu_percent = avg_cpu, mem_bytes = mem_bytes, "Node metrics updated");
                    }
                }
            }
        }
    });

    // Load Core Agent configurations
    tracing::info!(
        configuration_file_path = %config_path.display(),
        "Loading Core Agent configuration file"
    );
    let cfg = Config::load(&config_path)?;
    tracing::info!(
        configuration_file_path = %config_path.display(),
        "Core Agent configuration file loaded successfully"
    );

    // Initialize Log Collector shared sub-systems
    tracing::debug!("Initializing Core Agent internal data processing components");

    let buffer = Arc::new(Mutex::new(InMemoryBuffer::new(cfg.buffer).await));
    let shipper = Arc::new(Mutex::new(Shipper::new(cfg.shipper).await));
    let parser = Default::default(); // TODO: Replace with configurable parser, leave as is for now....

    tracing::debug!(
        buffer = ?buffer,
        shipper = ?shipper,
        parser = ?parser,
        "Core Agent data processing components initialized successfully"
    );

    tracing::debug!(
        "Creating shared components service, to ensure both local and over-the-network Core Agent modes share same state"
    );
    let service = Arc::new(LogCollectorService {
        parser,
        buffer_batcher: Arc::clone(&buffer),
        shipper: Arc::clone(&shipper),
    });

    let mut task_handles: Vec<JoinHandle<()>> = Vec::new();

    // Spawn local-mode observability data file watcher
    if cfg.general.enable_local_mode {
        if let Some(wcfg) = &cfg.watcher {
            if wcfg.enabled {
                tracing::info!(
                    log_dir = %wcfg.log_dir,
                    "Starting local observability data file watcher"
                );

                let log_dir = PathBuf::from(&wcfg.log_dir);
                let checkpoint_path = PathBuf::from(&wcfg.checkpoint_path);
                let poll_interval_ms = wcfg.poll_interval_ms;
                let recursive = wcfg.recursive;

                tracing::debug!(
                    "Cloning Arc to shared components service, ensuring Core Agent local observability data file watcher and gRPC server maintain the same internal state"
                );
                // Clone the Arc so watcher shares the internal components with `network_mode_enabled`
                let shared_service = Arc::clone(&service);

                tracing::debug!("Creating global shutdown channel subscribers");
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
                                        tracing::error!(error = %e, "Local observability data file watcher runtime error");
                                    }
                                }
                                _ = shutdown_rx2.recv() => {
                                    tracing::info!("Local observability data file watcher received shutdown signal, cleaning up resources");
                                    watcher.shutdown().await;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to start local observability data file watcher at runtime");
                        }
                    }
                });

                task_handles.push(handle);
            } else {
                tracing::info!(
                    "Local obervability data file watcher disabled in [watcher] configuration"
                );
            }
        } else {
            tracing::info!(
                "No local observability data file watcher configuration found in configuration file, skipping local mode."
            );
        }
    } else {
        tracing::info!("Local mode disabled in [general] configuration.");
    }

    // Start gRPC server
    if cfg.general.enable_network_mode {
        tracing::info!("Starting Core Agent gRPC server");

        tracing::debug!("Subscribing Core Agent gRPC server to global shutdown channel");
        // Subscribe to the global shutdown channel
        let mut shutdown_rx = shutdown.subscribe();

        let addr = "[::1]:50052".parse()?;
        tracing::debug!(
            server_addr = ?addr,
            "Core Agent gRPC server started"
        );

        let service_clone = (*service).clone();
        tracing::debug!(
            arc_clone = ?service_clone,
            "Cloning Arc to shared components service, ensuring Core Agent gRPC server maintain the same internal state as local observability data file watcher"
        );

        let handle = tokio::spawn(async move {
            if let Err(e) = Server::builder()
                .add_service(LogCollectorServer::new(service_clone))
                .serve_with_shutdown(addr, async move {
                    shutdown_rx.recv().await.ok();
                    tracing::trace!("Core Agent gRPC server gracefully shutting down");
                })
                .await
            {
                tracing::error!(error = %e, "Core Agent gRPC server error");
            }
        });

        task_handles.push(handle);
    } else {
        tracing::info!(
            "Network mode disabled in [general] configuration, Core Agent gRPC server not started"
        );
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
    tracing::info!(
        "Core Agent global shutdown triggered, awaiting data processing components to finish"
    );

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

    tracing::info!("Core Agent runtime graceful shutdown successful");
    Ok(())
}
