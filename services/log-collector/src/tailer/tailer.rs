use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::io::{AsyncSeekExt, SeekFrom};
use tokio::time::{Duration, sleep};

/// Tailer struct
#[derive(Debug, Serialize, Deserialize)]
pub struct Tailer {
    file_path: PathBuf,
    file_offset: u64,
    file_handle: String,
}

/// Tailer initialization
impl Tailer {
    pub async fn new_tailer(&mut self) {
        let file = match File::open(file_path).await {
            Ok(file) => file,
            Err(why) => panic!("Couldn't open {}: {}", file_path.display(), why),
        };

        file.seek(SeekFrom::Start(file_offset)).await?;
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
        }
    }
}
