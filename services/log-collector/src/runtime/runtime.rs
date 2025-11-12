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

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use sysinfo::System;
use tokio::{signal, sync::Mutex, task::JoinHandle};
use tonic::transport::Server;

pub async fn run_log_collector(config_path: PathBuf) -> Result<()> {
    // TODO: Ensure proper tracing is added for debugging
    println!("Starting Log Collector runtime...");

    // Start measuring cold start time
    let cold_startup_start = Instant::now();

    // Initialize global shutdown channel
    let shutdown = Shutdown::new();
    let shutdown_signal = shutdown.clone();

    // Start Ctrc+C shutdown signal listener
    tokio::spawn(async move {
        signal::ctrl_c()
            .await
            .expect("Failed to listen for CTRL+C shutdown signal");
        println!("CTRL+C shutdown signal detected, broadcasting shutdown...");
        shutdown_signal.trigger();
    });

    // Start metrics server (background task)
    tokio::spawn({
        let mut shutdown_rx = shutdown.subscribe();
        async move {
            tokio::select! {
                _ = start_metrics_server("0.0.0.0:9000") => {},
                _ = shutdown_rx.recv() => {
                    println!("Metrics server shutting down...");
                }
            }
        }
    });

    // Start log throughput auto-refresh background task
    tokio::spawn({
        let mut shutdown_rx = shutdown.subscribe();
        async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        println!("Logs Throughput updater shutting down...");
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
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        println!("P99 latency updater shutting down...");
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
            let mut sys = System::new_all();
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        println!("System metrics updater shutting down...");
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

                        println!("CPU: {:.1}%  MEM: {:.2} MB", avg_cpu, mem_bytes as f64 / 1_000_000.0);
                    }
                }
            }
        }
    });

    // Load Log Collector configurations
    println!("Loading configurations...");
    let cfg = Config::load(&config_path)?;
    println!("Configuration loaded successfully...");

    // Initialize Log Collector shared sub-systems
    println!("Initializing Log Collector components...");

    let buffer = Arc::new(Mutex::new(InMemoryBuffer::new(cfg.buffer).await));
    let shipper = Arc::new(Mutex::new(Shipper::new(cfg.shipper).await));
    let parser = Default::default(); // TODO: Replace with configurable parser, leave as is for now....

    println!("Log Collector components initialized successfully...");

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
                println!("Starting local file watcher on: {}", wcfg.log_dir);

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
                                        eprintln!("Watcher error: {e}");
                                    }
                                }
                                _ = shutdown_rx2.recv() => {
                                    println!("Watcher received shutdown signal, cleaning up resources...");
                                    watcher.shutdown().await;
                                }
                            }
                        }
                        Err(e) => eprintln!("Failed to start watcher: {e}"),
                    }
                });

                task_handles.push(handle);
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

        let handle = tokio::spawn(async move {
            if let Err(e) = Server::builder()
                .add_service(LogCollectorServer::new(service_clone))
                .serve_with_shutdown(addr, async move {
                    shutdown_rx.recv().await.ok();
                    println!("Log Collector gRPC server shutting down gracefully...");
                })
                .await
            {
                eprintln!("Log Collector gRPC server error: {e}");
            }
        });

        task_handles.push(handle);
    } else {
        println!("Network mode disabled in [general] configuration.");
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
    println!("Log Collector global shutdown triggered, awaiting components to finish...");

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

    println!("Log Collector successfully shut down.");
    Ok(())
}
