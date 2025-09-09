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
}

/// Tailer initialization
impl Tailer {
    pub async fn new_tailer(&mut self) -> Result<()> {
        let mut file = match File::open(&self.file_path).await {
            Ok(file) => file,
            Err(why) => panic!("Couldn't open {}: {}", &self.file_path.display(), why),
        };

        file.seek(SeekFrom::Start(self.file_offset)).await?;
        let mut reader = BufReader::new(file);

        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).await?;

            if bytes == 0 {
                // EOF reached, sleep until new line detected
                sleep(Duration::from_millis(100)).await;
            } else {
                // Continue reading the file until the end
            }

            return Ok(());
        }
    }
}
