use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::io::{AsyncSeekExt, SeekFrom};
use tokio::time::{Duration, sleep};

/// Tailer struct
#[derive(Debug, Serialize, Deserialize)]
pub struct Tailer {
    pub file_path: PathBuf,
    pub file_offset: u64,
    pub file_handle: String,
    pub reader: Option<BufReader<File>>,
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
        // 1. Get access to the file handle (from self or the BufReader returned by new_tailer())
        let reader = self.reader.as_mut().expect("Tailer not initialized!");

        let mut line = String::new();

        // 2. Enter infinite loop to actually run a Tailer
        loop {
            tokio::select! {
                bytes = reader.read_line(&mut line) => {
                    let bytes = bytes?;
                    if bytes == 0 {
                        sleep(Duration::from_millis(100)).await;
                    } else {
                        // Process line...
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("🛑 Tailer received shutdown signal for {:?}", self.file_path);
                        break;
                    }
                }
            }
        }

        // Flush offsets / cleanup
        Ok(())
    }
}
