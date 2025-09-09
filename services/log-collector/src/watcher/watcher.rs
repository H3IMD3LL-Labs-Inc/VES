use anyhow::{Context, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::{Result, Value};
use std::collections::HashMap;
use std::fmt::Error;
use std::fs;
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio::task;
use tokio_stream::StreamExt;

use crate::tailer::Tailer;

/// TODO: Handle with config.rs, maybe idk LOL :)
const CHECKPOINT_PATH: &str = "/path/to/checkpoint.json";
const LOG_DIR: &str = "/var/log/containers";
const POD_LOG_DIR: &str = "/var/log/pods";
const POLL_INTERNAL_SECS: u64 = 5; // Fallback poll interval

#[derive(Debug, Serialize, Deserialize)]
struct FileState {
    path: PathBuf,
    inode: u64,
    offset: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Checkpoint {
    files: HashMap<PathBuf, FileState>,
}

/// Main log watcher struct.
#[derive(Debug, Serialize, Deserialize)]
struct LogWatcher {
    log_dir: PathBuf,
    checkpoint_path: PathBuf,
    checkpoint: Checkpoint,
    active_files: HashMap<u64, PathBuf>,
}

/// FileState Methods
impl FileState {
    /// Recalculate file current state based on the filesystem (Currently only supports Unix/Unix-like OS)
    async fn determine_file_state(&self) -> FileState {
        let metadata = tokio::fs::metadata(&self.path)
            .await
            .expect("Failed to get file metadata for file");

        #[cfg(unix)]
        let inode = {
            use std::os::unix::fs::MetadataExt;
            metadata.ino()
        };

        let offset = metadata.len();

        FileState {
            path: self.path.clone(),
            inode,
            offset,
        }
    }
}

/// Checkpoint Methods
impl Checkpoint {
    /// Reads the checkpoint file from disk, it it exists
    async fn load_checkpoint(&mut self, saved_file_path: &Path) -> Result<()> {
        let saved_file_path_buf = saved_file_path.to_path_buf();

        if self.files.contains_key(&saved_file_path_buf) {
            let contents = tokio::fs::read_to_string(&saved_file_path).await?;

            let checkpoint_data: Checkpoint = serde_json::from_str(&contents)
                .map_err(|e| anyhow::anyhow!("Failed to deserialized JSON: {e}"))?;

            let disk_file_state = checkpoint_data.files.get(&saved_file_path_buf);
            let in_memory_file_state = self.files.get(&saved_file_path_buf);

            match (disk_file_state, in_memory_file_state) {
                (Some(disk), Some(memory)) => {
                    if disk.inode == memory.inode {
                        // TODO: Same file -> Safe to restore
                    } else {
                        // TODO: Different inode -> File was rotated or replaced
                    }
                }
                (Some(disk), None) => {
                    // TODO: Decide what to do in this case
                }
                (None, Some(_mem)) => {
                    // TODO: Decide what to do in this case
                }
                // No need to match (None, None), we already confirmed the file exists with .contains_key()
                (None, None) => {
                    // No need to match (None, None), we already confirmed the file exists with .contains_key()
                }
            }
        } else {
            eprintln!(
                "File {:?} not found in in-memory checkpoint",
                saved_file_path_buf
            );
        }

        Ok(())
    }

    async fn save_checkpoint(
        &mut self,
        save_path: &Path,
        file_path: PathBuf,
        file_inode: u64,
        file_offset: u64,
    ) -> Result<()> {
        let file_state = FileState {
            path: file_path.clone(),
            inode: file_inode,
            offset: file_offset,
        };

        self.files.insert(file_path, file_state);

        let checkpoint_data_json = serde_json::to_string_pretty(&self)?;

        tokio::fs::write(save_path, checkpoint_data_json).await?;

        Ok(())
    }
}

/// Log watcher struct implementation
impl LogWatcher {
    /// Creates a new log watcher instance and loads the last checkpoint, if it exists.
    pub async fn new_watcher(log_dir: PathBuf, checkpoint_path: PathBuf) -> Result<Self> {
        let mut checkpoint = Checkpoint {
            files: HashMap::new(),
        };

        checkpoint.load_checkpoint(&checkpoint_path).await?;

        Ok(Self {
            log_dir,
            checkpoint_path,
            checkpoint,
            active_files: HashMap::new(),
        })
    }

    /// Main async loop that orchestrates the watcher
    pub async fn run_watcher(&mut self) -> Result<()> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            notify::Config::default(),
        )?;
        watcher.watch(&self.log_dir, RecursiveMode::NonRecursive)?;

        let mut event_rx = tokio_stream::wrappers::ReceiverStream::new(rx);

        self.discover_initial_files().await?;

        loop {
            tokio::select! {
                Some(event) = event_rx.next() => {
                    self.handle_event(event).await?;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERNAL_SECS)) => {
                    self.discover_new_files().await?;
                }
            }
        }
    }

    /// Handles file system events from the `notify` watcher
    async fn handle_event(&mut self, event: Event) -> Result<()> {
        match event.kind {
            notify::EventKind::Create(notify::event::CreateKind::File) => {
                for path in &event.paths {
                    let metadata = tokio::fs::metadata(path).await?;
                    let inode = metadata.ino();
                    self.handle_new_file(inode, path).await?;
                }
            }
            notify::EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
                for path in &event.paths {
                    self.handle_file_change(path).await?;
                }
            }
            notify::EventKind::Remove(notify::event::RemoveKind::File) => {
                for path in &event.paths {
                    self.handle_file_removal(path).await?;
                }
            }
            notify::EventKind::Modify(notify::event::ModifyKind::Name) => {
                // This event can be tricky. A rename is often a remove + create
                // or move event. `notify` handles this well enough that we can rely
                // primarily on the `Create` and `Remove` events.
            }
            _ => (),
        }
        Ok(())
    }

    /// Discovers initial log files when a new watcher starts
    async fn discover_initial_files(&mut self) -> Result<()> {
        let mut entries = tokio::fs::read_dir(&self.log_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Only handle .log files
            if path.extension().map_or(false, |ext| ext == "log") {
                // Check if a checkpoint for this file already exists
                let (inode, offset) = match tokio::fs::metadata(&path).await {
                    Ok(metadata) => (metadata.ino(), {
                        self.checkpoint
                            .files
                            .get(&path)
                            .map(|f| f.offset)
                            .unwrap_or(0)
                    }),
                    Err(_) => continue,
                };

                // Track in active_files
                self.active_files.insert(inode, path.clone());

                // Start a tailer
                let mut tailer = Tailer {
                    file_path: path.clone(),
                    file_offset: offset,
                    file_handle: String::new(),
                };
                tokio::spawn(async move {
                    let _ = tailer.new_tailer().await;
                });

                // If file is new(not in LogWatcher.checkpoint), save it
                if !self.checkpoint.files.contains_key(&path) {
                    self.checkpoint
                        .save_checkpoint(&self.checkpoint_path, path, inode, 0)
                        .await?;
                }
            }
        }

        Ok(())
    }

    /// Discovers new files that might have been missed by `notify`
    pub async fn discover_new_files(&mut self) -> Result<()> {
        let entries = fs::read_dir(&self.log_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only consider `.log` files
            if path.extension().map_or(false, |ext| ext == "log") {
                let metadata = entry.metadata()?;
                let inode = metadata.ino();

                self.handle_new_file(inode, &path).await?;
            }
        }

        Ok(())
    }

    /// Handles newly discovered files
    async fn handle_new_file(&mut self, inode: u64, path: &Path) -> Result<()> {
        if self.active_files.contains_key(&inode) {
            return Ok(());
        } else {
            let new_file_path = path.clone().to_path_buf();
            self.active_files.insert(inode, new_file_path);

            let mut tailer = Tailer {
                file_path: PathBuf::from(new_file_path),
                file_offset: 0,
                file_handle: String::new(),
            };

            // NOTE: This currently does not have a shutdown/stop mechanism.
            // Will run in a perpetual loop.
            tailer.new_tailer().await;

            self.checkpoint
                .save_checkpoint(&self.checkpoint_path, path.to_path_buf(), inode, 0)
                .await?;
        }

        Ok(())
    }

    /// Handles untracked file changes (missed modification)
    /// This will not be implemented until a use-case is determined ;-)
    async fn handle_file_change(&mut self, path: &Path) -> Result<()> {
        // TODO: We lost track of a file somehow, treat is as a new file

        // Otherwise, tailer already handles new writes, so nothing to do
        Ok(())
    }

    /// Handles file removal (deletion - whether intentional or accidental)
    async fn handle_file_removal(&mut self, path: &Path) -> Result<()> {
        let file_path_buf = path.to_path_buf();

        // Check if tracking this file in memory
        if let Some(file_state) = self.checkpoint.files.get(&file_path_buf) {
            // Try to get metadata from disk
            match tokio::fs::metadata(&file_path_buf).await {
                Ok(metadata) => {
                    // File exists on disk
                    let inode_on_disk = metadata.ino();

                    if inode_on_disk != file_state.inode {
                        // Inode differe -> file was rotated/replaced
                        println!(
                            "File rotated: {:?}, old inode: {}, new inode: {}",
                            file_path_buf, file_state.inode, inode_on_disk
                        );

                        // Reset offset or create a new FileState
                        self.checkpoint
                            .save_checkpoint(
                                &self.checkpoint_path,
                                file_path_buf.clone(),
                                inode_on_disk,
                                0,
                            )
                            .await?;
                    } else {
                        // Same inode -> file is still valid, nothing to do
                    }
                }
                Err(_) => {
                    // File not present on disk -> true deletion
                    println!("File deleted on disk: {:?}", file_path_buf);

                    // Remove from in-memory active_files
                    self.active_files.retain(|_, p| p != &file_path_buf);

                    // Remove from Checkpoint
                    self.checkpoint.files.remove(&file_path_buf);

                    // Save updated Checkpoint
                    let _ = serde_json::to_string_pretty(&self.checkpoint)
                        .map(|data| tokio::fs::write(&self.checkpoint_path, data));
                }
            }
        } else {
            // Not a tracked file, ignore
            println!(
                "File ignored for removal: {:?}, this file is not being tracked.",
                file_path_buf
            );
        }

        Ok(())
    }
}
