// Local crates
use crate::{
    buffer_batcher::log_buffer_batcher::InMemoryBuffer, helpers::log_processing::process_log_line,
    parser::parser::NormalizedLog, server::server::LogCollectorService, shipper::shipper::Shipper,
};

// External crates
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::io::{AsyncSeekExt, SeekFrom};
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use tracing::instrument;

/// Tailer struct
#[derive(Debug)]
pub struct Tailer {
    pub file_path: PathBuf,
    pub file_offset: u64,
    pub file_handle: String,
    pub reader: Option<BufReader<File>>,

    // Each Tailer runs in its own async task, possibly(TBD) on its own thread,
    // and is responsible for continuously reading one file line-by-line.
    //
    // To actually *do something* with each log line (parse, buffer-batch, ship),
    // the Tailer needs access to the shared ingestion pipeline implemented in
    // `LogCollectorService`.
    //
    // Because multiple tailers run concurrently, they all share the same
    // `Arc<LogCollectorService>` reference. This ensures:
    //
    // 1. **Shared State:** They all push data into the same parser, buffer-batcher
    // and shipper.
    // 2. **Thread safety:** Arc makes it possible to clone and move the same service
    // reference safely into each spawned async task.
    // 3. **Consistency:** Any changes (like updated metrics, memory pressure, or flushes)
    // are visible to all Tailers.
    //
    // A Tailer doesn't own or recreate the service — it just uses the Arc clone provided
    // by the LogWatcher to call LogCollectorService methods for each part of the log
    // processing pipeline.
    pub service: Arc<LogCollectorService>,
}

/// Tailer initialization
impl Tailer {
    #[instrument(
        name = "ves_create_new_log_file_tailer",
        target = "tailer::tailer::Tailer",
        skip_all,
        level = "trace"
    )]
    pub async fn new_tailer(&mut self) -> Result<()> {
        // Open the file
        tracing::trace!(
            log_file_path = %self.file_path.display(),
            "Opening file to start tailing"
        );
        let mut file = match File::open(&self.file_path).await {
            Ok(file) => {
                tracing::trace!(
                    log_file_path = %self.file_path.display(),
                    "Successfully opened log file"
                );
                file
            }
            Err(err) => {
                tracing::error!(
                    log_file_path = %self.file_path.display(),
                    error = %err,
                    "Failed to open log file"
                );
                return Err(anyhow::anyhow!(
                    "Couldn't open {}: {}",
                    &self.file_path.display(),
                    err
                ));
            }
        };

        // Seek to the provided offset (to allow resumption from checkpoint)
        tracing::trace!(
            log_file_path = %self.file_path.display(),
            offset = %self.file_offset,
            "Seeking to file offset before tailing"
        );
        if let Err(err) = file.seek(SeekFrom::Start(self.file_offset)).await {
            tracing::error!(
                log_file_path = %self.file_path.display(),
                offset = %self.file_offset,
                error = %err,
                "Failed to seek file to specified offset"
            );
            return Err(err.into());
        }
        tracing::trace!(
            log_file_path = %self.file_path.display(),
            offset = %self.file_offset,
            "Successfully seeked file to specified offset"
        );

        // BufReader wrapper to allow efficient line reading
        self.reader = Some(BufReader::new(file));
        tracing::trace!(
            log_file_path = %self.file_path.display(),
            "BufReader initialized for new tailer"
        );

        Ok(())
    }

    /// Orchestration loop for spawned Tailer
    #[instrument(
        name = "ves_run_log_file_tailer",
        target = "tailer::tailer::Tailer",
        skip_all,
        level = "trace"
    )]
    pub async fn run_tailer(
        &mut self,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        // Get access to the file handle (from self or the BufReader returned by new_tailer())
        tracing::trace!("Fetching handle to log file returned when tailer was spawned");
        let reader = self.reader.as_mut().expect("Tailer not initialized!");

        let mut line = String::new();

        // Enter infinite loop to actually run a Tailer
        loop {
            tokio::select! {
                bytes = reader.read_line(&mut line) => {
                    let bytes = bytes?;
                    if bytes == 0 {
                        tracing::debug!(
                            log_file_path = %self.file_path.display(),
                            "No new logs found in tailed log file"
                        );
                        sleep(Duration::from_millis(100)).await;
                    } else {
                        tracing::debug!(
                            log_file_path = %self.file_path.display(),
                            "New log found in tailed log file acquiring InMemoryBuffer and Shipper locks"
                        );
                        let mut buffer = self.service.buffer_batcher.lock().await; // Acquire InMemoryBuffer Mutex lock
                        let shipper = self.service.shipper.lock().await; // Acquire Shipper lock
                        tracing::debug!(
                            log_file_path = %self.file_path.display(),
                            log_line = %line,
                            buffer_lock = ?buffer,
                            shipper_lock = ?shipper,
                            "Attempting to process log line"
                        );
                        match process_log_line(
                            &mut *buffer,
                            &*shipper,
                            line.clone(),
                        )
                        .await {
                            Ok(status) => {
                                tracing::trace!(
                                    log_file_path = %self.file_path.display(),
                                    status = %status,
                                    log_line = %line,
                                    "Successfully processed log"
                                );
                            }
                            Err(err) => {
                                tracing::error!(
                                    log_file_path = %self.file_path.display(),
                                    error = %err,
                                    line = %line,
                                    "Error processing log"
                                );
                            }
                        }
                    } // InMemoryBuffer lock released automatically
                    tracing::trace!(
                        log_file_path = %self.file_path.display(),
                        log_line = %line,
                        "Clearing log line, before processing next log line"
                    );
                    line.clear();
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::trace!(
                            log_file_path = %self.file_path.display(),
                            "Running Tailer received shutdown signal, acquiring InMemoryBuffer and Shipper locks"
                        );

                        let mut buffer = self.service.buffer_batcher.lock().await;
                        let mut shipper = self.service.shipper.lock().await;

                        // TODO: Shipper logic to handle shutdown successfully....
                        if let Err(err) = buffer.flush_remaining_logs().await {
                            tracing::error!(
                                log_file_path = %self.file_path.display(),
                                buffer_lock = ?buffer,
                                shipper_lock = ?shipper,
                                error = %err,
                                "Error flushing InMemoryBuffer on shutdown"
                            );
                        }
                        break;
                    }
                }
            }
        }

        // Flush offsets/cleanup
        tracing::trace!("Running Tailer stopped");
        Ok(())
    }
}
