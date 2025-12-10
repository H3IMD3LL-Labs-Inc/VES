// Local crates
use crate::metrics::metrics::{
    LOGS_PROCESSED_THIS_SECOND, PROCESS_LINE_DURATION_SECONDS, THROUGHPUT_LOGS_PER_SEC,
    observe_duration,
};
use crate::{
    buffer_batcher::log_buffer_batcher::InMemoryBuffer, parser::parser::NormalizedLog,
    shipper::shipper::Shipper,
};

// External crates
use hdrhistogram::Histogram;
use std::sync::Mutex;
use std::time::Instant;
use tracing::instrument;

// Global histogram for latency tracking (accurate to microseconds)
lazy_static::lazy_static! {
    pub static ref LAT_HISTOGRAM: Mutex<Histogram<u64>> = Mutex::new(Histogram::new(3).unwrap());
}

/// Internal helper function to perform actual log processing logic on a raw log received locally(same node as log
/// collector) or over-the-wire(different node as log collector).
#[instrument(
    name = "core_agent_pipeline::processing",
    target = "helpers::log_processing",
    skip_all,
    level = "debug"
)]
pub async fn process_log_line(
    buffer_batcher: &mut InMemoryBuffer,
    shipper: &Shipper,
    line: String,
) -> Result<String, String> {
    tracing::info!(
        unstructured_data = &line,
        "Starting to process unstructured observability data"
    );

    // Start measuring how long processing the log line takes
    let start = Instant::now();

    let record_prometheus_metrics = || {
        let elapsed_us = start.elapsed().as_micros() as u64;
        {
            let mut hist = LAT_HISTOGRAM.lock().unwrap();
            let _ = hist.record(elapsed_us);
        }

        observe_duration(PROCESS_LINE_DURATION_SECONDS.clone(), start);
        LOGS_PROCESSED_THIS_SECOND.inc();
    };

    tracing::debug!(
        unstructured_data = &line,
        "Selecting parser and parsing incoming unstructured observability data",
    );
    // Parse the RawLog line into NormalizedLog format
    let parsed_log = match NormalizedLog::select_parser(&line).await {
        Ok(p) => {
            tracing::debug!(
                unstructured_data = &line,
                "Successfully parsed unstructured observability data"
            );
            p
        }
        Err(e) => {
            let msg = format!("Failed to parse raw log line: {}, {}", &line, e);
            tracing::error!(
                error = %e,
                unstructured_data = &line,
                message = %msg
            );
            record_prometheus_metrics();
            return Err(msg);
        }
    };

    // TODO: cloning parsed_log is an inefficiency
    // Push the NormalizedLog to InMemoryBuffer
    if let Err(e) = buffer_batcher.push(parsed_log.clone()).await {
        let msg = format!("InMemoryBuffer error: {}", e);
        tracing::error!(error = %e, %msg);
        record_prometheus_metrics();
        return Err(msg);
    }

    tracing::debug!(
        queue_len = buffer_batcher.queue.len(),
        "Observability data pushed to InMemoryBuffer"
    );

    let queue_len = buffer_batcher.queue.len();
    let batch_size = buffer_batcher.batch_size;
    let elapsed_ms = buffer_batcher.last_flush_at.elapsed().as_millis() as u128;
    let timeout_ms = buffer_batcher.batch_timeout_ms as u128;
    let size_triggered = queue_len >= batch_size;
    let timeout_triggered = elapsed_ms >= timeout_ms;

    // Determine whether to flush the InMemoryBuffer
    let should_flush = size_triggered || timeout_triggered;

    tracing::debug!(
        queue_len,
        batch_size,
        elapsed_ms,
        timeout_ms,
        size_triggered,
        timeout_triggered,
        "Evaluating InMemoryBuffer flush conditions"
    );

    // Flush InMemoryBuffer based on user configured flush conditions
    if should_flush {
        tracing::info!("InMemoryBuffer flush condition met");

        if size_triggered {
            tracing::debug!(
                trigger = "batch_size",
                queue_len,
                batch_size,
                "InMemoryBuffer flush triggered: batch size threshold reached, attempting flush"
            );
        }

        if timeout_triggered {
            tracing::debug!(
                trigger = "timeout_ms",
                elapsed_ms,
                timeout_ms,
                "InMemoryBuffer flush triggered: timeout threshold reached, attempting flush"
            );
        }

        // TODO: cloning parsed_log is an inefficiency
        match buffer_batcher.flush(parsed_log.clone()).await {
            Ok(Some(flushed_buffer)) => {
                tracing::debug!(
                    flushed_count = flushed_buffer.queue.len(),
                    "Flushed data from InMemoryBuffer successfully"
                );

                if let Err(e) = shipper.send(flushed_buffer.clone()).await {
                    let msg = format!("Shipper error: {}", e);
                    tracing::error!(error = %e, %msg);
                    record_prometheus_metrics();
                    return Err(msg);
                }

                tracing::info!("Shipper successfully sent flushed data");

                record_prometheus_metrics();
                return Ok(format!(
                    "Flushed {} logs successfully and sent to shipper",
                    flushed_buffer.queue.len()
                ));
            }
            Ok(None) => {
                tracing::debug!(
                    "InMemoryBuffer flush was triggered but no data available/present to flush"
                );
                record_prometheus_metrics();
                return Ok("No data to flush".into());
            }
            Err(e) => {
                let msg = format!("Flush error: {}", e);
                tracing::error!(error = %e, %msg);
                record_prometheus_metrics();
                return Err(msg);
            }
        }
    } else {
        tracing::debug!("InMemoryBuffer flush conditions not met, not flushing processed data");
        record_prometheus_metrics();
        return Ok("Data buffered successfully, not flushed.".into());
    };
}
