use lazy_static::lazy_static;
use prometheus::{
    Counter, Encoder, Gauge, Histogram, TextEncoder, register_counter, register_gauge,
    register_histogram,
};

lazy_static! {
    pub static ref P99_LATENCY: Gauge = register_gauge!(
        "logcollector_p99_latency_ms",
        "99th percentile latency in milliseconds"
    )
    .unwrap();
    pub static ref THROUGHPUT: Gauge = register_gauge!(
        "logcollector_throughput_logs_per_sec",
        "Number of log lines processed per second"
    )
    .unwrap();
    pub static ref DROPPED_LOGS: Counter = register_counter!(
        "logcollector_dropped_logs_total",
        "Total number of dropped logs"
    )
    .unwrap();
    pub static ref AVERAGE_BATCH_SIZE: Gauge = register_gauge!(
        "logcollector_average_batch_size",
        "Average processed logs batch size"
    )
    .unwrap();
    pub static ref FLUSH_DURATION: Histogram = register_histogram!(
        "logcollector_flush_duration_ms",
        "Histogram of flush durations in milliseconds",
        vec![0.1, 0.5, 1.0, 5.0, 10.0, 50.0, 100.0]
    )
    .unwrap();
}
