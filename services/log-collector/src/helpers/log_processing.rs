use crate::buffer_batcher::log_buffer_batcher::InMemoryBuffer;
use crate::parser::parser::NormalizedLog;
use crate::shipper::shipper::Shipper;

/// Internal helper function to perform actual log processing logic, after logs arrive
/// (locally or via network).
///
/// This is intended for use in the following Log Collector modes; Local mode, Over-Network(gRPC)/Server mode
pub async fn process_log_line(
    buffer_batcher: &mut InMemoryBuffer,
    shipper: &Shipper,
    line: String,
) -> Result<String, String> {
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
    if should_flush {
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
    }

    Ok("Log buffered successfully, not flushed.".into())
}

// TODO: Unnecessary clones of parsed_log can be removed to improve performance...
