use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::io::{AsyncSeekExt, SeekFrom};
use tokio::time::{Duration, sleep};

use crate::buffer_batcher::log_buffer_batcher::InMemoryBuffer;
use crate::helpers::log_processing::process_log_line;
use crate::parser::parser::NormalizedLog;
use crate::server::server::LogCollectorService;
use crate::shipper::shipper::Shipper;

/// Tailer struct
#[derive(Debug, Serialize, Deserialize)]
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
    pub async fn new_tailer(&mut self) -> Result<()> {
        // Open the file
        let mut file = match File::open(&self.file_path).await {
            Ok(file) => file,
            Err(why) => {
                return Err(anyhow::anyhow!(
                    "Couldn't open {}: {}",
                    &self.file_path.display(),
                    why
                ));
            }
        };

        // Seek to the provided offset (to allow resumption from checkpoint)
        file.seek(SeekFrom::Start(self.file_offset)).await?;

        // BufReader wrapper to allow efficient line reading
        self.reader = Some(BufReader::new(file));

        Ok(())
    }

    /// Orchestration loop for spawned Tailer
    pub async fn run_tailer(
        &mut self,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        // Get access to the file handle (from self or the BufReader returned by new_tailer())
        let reader = self.reader.as_mut().expect("Tailer not initialized!");

        let mut line = String::new();

        // Enter infinite loop to actually run a Tailer
        loop {
            tokio::select! {
                bytes = reader.read_line(&mut line) => {
                    let bytes = bytes?;
                    if bytes == 0 {
                        sleep(Duration::from_millis(100)).await;
                    } else {
                        // Actual log processing logic
                        match process_log_line(
                            &self.service.parser,
                            &self.service.buffer_batcher,
                            &self.service.shipper,
                            line.clone(),
                        )
                        .await {
                            Ok(status) => println!("Processed log from {:?}: {}", self.file_path, status),
                            Err(err) => eprintln!("Error processing log from {:?}: {}", self.file_path, err),
                        }
                    }
                    line.clear();
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("Tailer received shutdown signal for {:?}", self.file_path);
                        break;
                    }
                }
            }

            // Flush offsets / cleanup
            return Ok(());
        }
    }
}
