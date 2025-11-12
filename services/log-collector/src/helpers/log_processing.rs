use crate::buffer_batcher::log_buffer_batcher::InMemoryBuffer;
use crate::metrics::metrics::{
    LOGS_PROCESSED_THIS_SECOND, PROCESS_LINE_DURATION_SECONDS, THROUGHPUT_LOGS_PER_SEC,
    observe_duration,
};
use crate::parser::parser::NormalizedLog;
use crate::shipper::shipper::Shipper;

use hdrhistogram::Histogram;
use std::sync::Mutex;
use std::time::Instant;

// Global histogram for latency tracking (accurate to microseconds)
lazy_static::lazy_static! {
    pub static ref LAT_HISTOGRAM: Mutex<Histogram<u64>> = Mutex::new(Histogram::new(3).unwrap());
}

/// Internal helper function to perform actual log processing logic, after logs arrive
/// (locally or via network).
///
/// This is intended for use in the following Log Collector modes; Local mode, Over-Network(gRPC)/Server mode
pub async fn process_log_line(
    buffer_batcher: &mut InMemoryBuffer,
    shipper: &Shipper,
    line: String,
) -> Result<String, String> {
    // Start measuring how long processing the log line takes
    let start = Instant::now();

    // Parse the RawLog line into NormalizedLog format
    let parsed_log = match NormalizedLog::select_parser(&line).await {
        Ok(p) => p,
        Err(e) => return Err(format!("Failed to parse log: {}", e)),
    };

    // Push the NormalizedLog to InMemoryBuffer
    if let Err(e) = buffer_batcher.push(parsed_log.clone()).await {
        return Err(format!("InMemoryBuffer error: {}", e));
    }

    // Determine whether to flush the InMemoryBuffer
    let should_flush = buffer_batcher.queue.len() >= buffer_batcher.batch_size
        || buffer_batcher.last_flush_at.elapsed().as_millis()
            >= buffer_batcher.batch_timeout_ms as u128;

    // Flush InMemoryBuffer based on user configured flush conditions
    let result = if should_flush {
        match buffer_batcher.flush(parsed_log.clone()).await {
            Ok(Some(flushed_buffer)) => {
                if let Err(e) = shipper.send(flushed_buffer.clone()).await {
                    return Err(format!("Shipper error: {}", e));
                }
                return Ok(format!(
                    "Flushed {} logs successfully and sent to shipper",
                    flushed_buffer.queue.len()
                ));
            }
            Ok(None) => return Ok("No logs to flush".into()),
            Err(e) => return Err(format!("Flush error: {}", e)),
        }
    } else {
        Ok("Log buffered successfully, not flushed.".into())
    };

    // Record latency into histogram
    let elapsed_us = start.elapsed().as_micros() as u64;
    {
        let mut hist = LAT_HISTOGRAM.lock().unwrap();
        let _ = hist.record(elapsed_us);
    }

    // Record processing duration
    observe_duration(PROCESS_LINE_DURATION_SECONDS.clone(), start);

    // Incremet raw logs processed counter for this second
    LOGS_PROCESSED_THIS_SECOND.inc();

    result
}
