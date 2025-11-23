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
    name = "processing_raw_log_line",
    target = "helpers::log_processing",
    skip_all,
    level = "trace"
)]
pub async fn process_log_line(
    buffer_batcher: &mut InMemoryBuffer,
    shipper: &Shipper,
    line: String,
) -> Result<String, String> {
    tracing::trace!(line = &line, "Starting to process raw log line");

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

    tracing::trace!(
        line = &line,
        "Selecting parser and parsing incoming raw log line",
    );
    // Parse the RawLog line into NormalizedLog format
    let parsed_log = match NormalizedLog::select_parser(&line).await {
        Ok(p) => {
            tracing::debug!(line = &line, "Successfully parsed raw log line");
            p
        }
        Err(e) => {
            let msg = format!("Failed to parse raw log line: {}, {}", &line, e);
            tracing::error!(line = &line, error = %e, message = %msg);
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
        "Log pushed to in-memory buffer"
    );

    let queue_len = buffer_batcher.queue.len();
    let batch_size = buffer_batcher.batch_size;
    let elapsed_ms = buffer_batcher.last_flush_at.elapsed().as_millis() as u128;
    let timeout_ms = buffer_batcher.batch_timeout_ms as u128;
    let size_triggered = queue_len >= batch_size;
    let timeout_triggered = elapsed_ms >= timeout_ms;

    // Determine whether to flush the InMemoryBuffer
    let should_flush = size_triggered || timeout_triggered;

    tracing::trace!(
        queue_len,
        batch_size,
        elapsed_ms,
        timeout_ms,
        size_triggered,
        timeout_triggered,
        "Evaluating whether InMemoryBuffer should flush"
    );

    // Flush InMemoryBuffer based on user configured flush conditions
    if should_flush {
        tracing::info!("Flush condition met, attempting flush");

        if size_triggered {
            tracing::info!(
                trigger = "batch_size",
                queue_len,
                batch_size,
                "InMemoryBuffer flush triggered: batch size threshold reached, attempting flush"
            );
        }

        if timeout_triggered {
            tracing::info!(
                trigger = "timeout_ms",
                elapsed_ms,
                timeout_ms,
                "InMemoryBuffer flush triggered: timeout threshold reached, attempting flush"
            );
        }

        // TODO: cloning parsed_log is an inefficiency
        match buffer_batcher.flush(parsed_log.clone()).await {
            Ok(Some(flushed_buffer)) => {
                tracing::info!(
                    flushed_count = flushed_buffer.queue.len(),
                    "Flushed logs successfully"
                );

                if let Err(e) = shipper.send(flushed_buffer.clone()).await {
                    let msg = format!("Shipper error: {}", e);
                    tracing::error!(error = %e, %msg);
                    record_prometheus_metrics();
                    return Err(msg);
                }

                tracing::info!("Shipper successfully sent flushed logs");

                record_prometheus_metrics();
                return Ok(format!(
                    "Flushed {} logs successfully and sent to shipper",
                    flushed_buffer.queue.len()
                ));
            }
            Ok(None) => {
                tracing::debug!("Flush was triggered but no logs were available/present to flush");
                record_prometheus_metrics();
                return Ok("No logs to flush".into());
            }
            Err(e) => {
                let msg = format!("Flush error: {}", e);
                tracing::error!(error = %e, %msg);
                record_prometheus_metrics();
                return Err(msg);
            }
        }
    } else {
        tracing::trace!("InMemoryBuffer flush conditions not met, not flushing processed log");
        record_prometheus_metrics();
        return Ok("Log buffered successfully, not flushed.".into());
    };
}
