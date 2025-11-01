use crate::buffer_batcher::log_buffer_batcher::{self, InMemoryBuffer};
use crate::parser::parser::NormalizedLog;
use crate::shipper::shipper::Shipper;

// Internal helper function to perform actual log processing logic, after logs arrive
// (locally or via network).
//
// TODOs:
//  1. Take raw log line
//  2. Parse it into NormalizedLog
//  3. Perform buffer-batching on the NormalizedLog
//  4. Check flush conditions and perform flush
//  5. Flush and ship logs if conditions are met
//  6. Return a human-readable `Result<()>`
pub async fn process_log_line(
    parser: &NormalizedLog,
    buffer_batcher: &InMemoryBuffer,
    shipper: &Shipper,
    line: String,
) -> Result<String, String> {
    let parsed_log = match NormalizedLog::select_parser(&line).await {
        Ok(p) => p,
        Err(e) => return Err(format!("Failed to parse log: {}", e)),
    };

    if let Err(e) = buffer_batcher.push(parsed_log.clone()).await {
        return Err(format!("InMemoryBuffer error: {}", e));
    }

    let should_flush = buffer_batcher.queue.len() >= buffer_batcher.batch_size
        || buffer_batcher.last_flush_at.elapsed().as_millis()
            >= buffer_batcher.batch_timeout_ms as u128;

    if should_flush {
        match buffer_batcher.flush(parsed_log.clone()).await {
            Ok(Some(flushed_logs)) => {
                if let Err(e) = shipper.send(flushed_logs.clone()).await {
                    return Err(format!("Shipper error: {}", e));
                }
                return Ok(format!(
                    "Flushed {} logs successfully and sent to shipper",
                    flushed_logs.len()
                ));
            }
            Ok(None) => return Ok("No logs to flush".into()),
            Err(e) => return Err(format!("Flush error: {}", e)),
        }
    }

    Ok("Log buffered successfully, not flushed.".into())
}
