// Local crates
use prometheus::{
    Counter, Encoder, Gauge, Histogram, TextEncoder, register_counter, register_gauge,
    register_histogram,
};

// External crates
use lazy_static::lazy_static;
use std::time::Instant;

/// Helper to observe histogram duration
pub fn observe_duration(hist: Histogram, start: Instant) {
    let elapsed = start.elapsed().as_secs_f64();
    hist.observe(elapsed);
}

lazy_static! {
    // ======== Core Performance Metrics ========

    /// Sustained throughput (logs/sec/core)
    pub static ref THROUGHPUT_LOGS_PER_SEC: Gauge = register_gauge!(
        "logcollector_throughput_logs_per_sec",
        "Sustained throughput of logs processed per core per second"
    ).unwrap();

    /// Logs processed every second
    ///
    /// This static ref is internal, for use to obtain THROUGHPUT_LOGS_PER_SEC
    pub static ref LOGS_PROCESSED_THIS_SECOND: Counter = register_counter!(
        "logcollector_logs_processed_this_second_total",
        "Temporary counter for logs processed within a current 1-second window"
    ).unwrap();

    /// Average processing latency (derived from process_line_duration histogram)
    pub static ref PROCESS_LINE_DURATION_SECONDS: Histogram = register_histogram!(
        "logcollector_process_line_duration_seconds",
        "Histogram of per-log processing durations in seconds",
        vec![0.0001, 0.0003, 0.0005, 0.001, 0.002, 0.005]
    ).unwrap();

    /// P99 end-to-end latency (milliseconds)
    pub static ref P99_LATENCY_MS: Gauge = register_gauge!(
        "logcollector_p99_latency_ms",
        "99th percentile end-to-end latency (milliseconds)"
    ).unwrap();

    // ======== System Resource Metrics ========

    /// Memory footprint in bytes (Grafana converts to MB)
    pub static ref MEMORY_BYTES: Gauge = register_gauge!(
        "logcollector_memory_bytes",
        "Resident memory usage in bytes"
    ).unwrap();

    /// CPU usage percentage per core
    pub static ref CPU_PERCENT_PER_CORE: Gauge = register_gauge!(
        "logcollector_cpu_percent_per_core",
        "CPU Usage percentage per core"
    ).unwrap();

    // ======== Lifecycle Metrics ========

    /// Cold start initialization duration in seconds
    pub static ref STARTUP_DURATION_SECONDS: Gauge = register_gauge!(
        "logcollector_startup_duration_seconds",
        "Cold start initialization time (seconds)"
    ).unwrap();

    pub static ref SNAPSHOT_RECOVERY_DURATION_SECONDS: Gauge = register_gauge!(
        "logcollector_snapshot_recovery_duration_seconds",
        "Snapshot recovery duration in (seconds)"
    ).unwrap();
}
