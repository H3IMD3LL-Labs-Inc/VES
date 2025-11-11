use lazy_static::lazy_static;
use prometheus::{
    Counter, Encoder, Gauge, Histogram, TextEncoder, register_counter, register_gauge,
    register_histogram,
};
use std::time::Instant;

/// Helper to observe histogram duration
pub fn observe_duration(hist: Histogram, start: Instant) {
    let elapsed = start.elapsed().as_secs_f64();
    hist.observe(elapsed);
}

lazy_static! {
    // ======== Local Log Ingestion Metrics ========
    pub static ref LINES_READ_TOTAL: Counter = register_counter!(
        "logcollector_lines_read_total",
        "Total number of lines read by tailers"
    ).unwrap();

    pub static ref FILE_ROTATION_TOTAL: Counter = register_counter!(
        "logcollector_file_rotation_total",
        "Number of file rotations handled by tailers"
    ).unwrap();

    pub static ref FD_OPEN_TOTAL: Gauge = register_gauge!(
        "logcollector_fd_open_total",
        "Number of open file descriptors"
    ).unwrap();

    // ======== Log Parsing Metrics ========
    pub static ref PARSE_DURATION_SECONDS: Histogram = register_histogram!(
        "logcollector_parse_duration_seconds",
        "Histogram of log parsing durations (seconds)",
        vec![0.0005, 0.001, 0.002, 0.005, 0.01, 0.02]
    ).unwrap();

    pub static ref PARSE_FAILURES_TOTAL: Counter = register_counter!(
        "logcollector_parse_failures_total",
        "Number of failed log parsing attempts"
    ).unwrap();

    pub static ref PARSE_BACKPRESSURE_DURATION_SECONDS: Histogram = register_histogram!(
        "logcollector_parse_backpressure_duration_seconds",
        "Histogram of parser backpressure durations (seconds)",
        vec![0.001, 0.002, 0.005, 0.01, 0.02]
    ).unwrap();

    // ======== InMemoryBuffer Metrics ========
    pub static ref BUFFER_FLUSH_DURATION_SECONDS: Histogram = register_histogram!(
        "logcollector_buffer_flush_duration_seconds",
        "Histogram of InMemoryBuffer flush durations (seconds)",
        vec![0.001, 0.005, 0.01, 0.02, 0.05]
    ).unwrap();

    pub static ref BUFFER_FLUSH_ERRORS_TOTAL: Counter = register_counter!(
        "logcollector_buffer_flush_errors_total",
        "Total number of InMemoryBuffer flush errors"
    ).unwrap();

    pub static ref BUFFER_UTITILIZATION_RATIO: Gauge = register_gauge!(
        "logcollector_buffer_utilization_ratio",
        "Ratio of InMemoryBuffer utilization(0.0 - 1.0)"
    ).unwrap();

    pub static ref BUFFER_SIZE: Gauge = register_gauge!(
        "logcollector_buffer_size",
        "Current number of logs in the in-memory buffer"
    ).unwrap();

    // ========= Log Shipper Metrics ========
    pub static ref SHIP_DURATION_SECONDS: Histogram = register_histogram!(
        "logcollector_ship_duration_seconds",
        "Histogram og log shipping durations (seconds)",
        vec![0.001, 0.005, 0.01, 0.02, 0.05]
    ).unwrap();

    pub static ref SHIP_QUEUE_DEPTH: Gauge = register_gauge!(
        "logcollector_ship_queue_depth",
        "Number of pending batches in shipper queue"
    ).unwrap();

    pub static ref SHIP_FAILURES_TOTAL: Counter = register_counter!(
        "logcollector_ship_failures_total",
        "Total failed ship attempts"
    ).unwrap();

    // ========= Throughput Metrics ========
    pub static ref PROCESSED_LOGS_TOTAL: Counter = register_counter!(
        "logcollector_processed_logs_total",
        "Total number of successfully processed logs"
    ).unwrap();

    pub static ref AVERAGE_BATCH_SIZE: Gauge = register_gauge!(
        "logcollector_average_batch_size",
        "Average processed logs batch size"
    ).unwrap();

    // ======== Lifecycle & System Metrics ========
    pub static ref SHUTDOWN_INVOCATIONS_TOTAL: Counter = register_counter!(
        "logcollector_shutdown_invocations_total",
        "Number of graceful shutdown invocations"
    ).unwrap();

    pub static ref RESTARTS_TOTAL: Counter = register_counter!(
        "logcollector_restarts_total",
        "Number of LogCollector restarts"
    ).unwrap();

    pub static ref DROPPED_LOGS_TOTAL: Counter = register_counter!(
        "logcollector_dropped_logs_total",
        "Total number of logs dropped (due to backpressure or errors)"
    ).unwrap();

    pub static ref ACTIVE_TASKS: Gauge = register_gauge!(
        "logcollector_active_tasks",
        "Number of active async tasks in runtime"
    ).unwrap();

    // ======== High-level Derived Metrics ========
    pub static ref P99_LATENCY: Gauge = register_gauge!(
        "logcollector_p99_latency_ms",
        "99th percentile end-to-end latency (milliseconds)"
    ).unwrap();

    pub static ref THROUGHPUT: Gauge = register_gauge!(
        "logcollector_throughput_logs_per_sec",
        "Number of logs processed per second (smoothed)"
    ).unwrap();
}
