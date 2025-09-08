use anyhow::{Context, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio::task;
use tokio_stream::StreamExt;

use crate::tailer;

/// Constants
/// TODO: Handle with config.rs
const CHECKPOINT_PATH: &str = "/path/to/checkpoint.json";
const LOG_DIR: &str = "/var/log/containers";
const POD_LOG_DIR: &str = "/var/log/pods";
const POLL_INTERNAL_SECS: u64 = 5; // Fallback poll interval

/// State representation of a single log file, used for checkpointing.
/// Must be serializable and deserializable
#[derive(Debug, Serialize, Deserialize)]
struct FileState {
    path: PathBuf,
    inode: u64,
    offset: u64,
}

/// A map of file paths to their last known state, for persistent tracking.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Checkpoint {
    files: HashMap<PathBuf, FileState>,
}

/// Main log watcher struct.
/// Contains state needed to manage the watching process.
#[derive(Debug, Serialize, Deserialize)]
struct LogWatcher {
    log_dir: PathBuf,
    checkpoint_path: PathBuf,
    checkpoint: Checkpoint,
    active_files: HashMap<u64, PathBuf>,
}

/// File state struct implementation
impl FileState {
    /// Recalculate file current state based on the filesystem
    async fn determine_file_state(&self) -> FileState {
        // Query file metadata
        let metadata = tokio::fs::metadata(&self.path)
            .await
            .expect("Failed to get file metadata for file");

        // Platform-specific ways of extracting file inode:
        // metadata.ino() requires nix or libc crate
        #[cfg(unix)]
        let inode = {
            use std::os::unix::fs::MetadataExt;
            metadata.ino()
        };

        // File Offset
        let offset = metadata.len();

        FileState {
            path: self.path.clone(),
            inode,
            offset,
        }
    }
}

/// Checkpoint struct implementation
impl Checkpoint {
    /// Reads the checkpoint file from disk, it it exists
    async fn load_checkpoint(self, path: &PathBuf) -> Result<Checkpoint> {
        // Fetch/Check if the checkpoint actually exists
        // if it exists fetch the filestate, and compare it to
        // the file state of the saved checkpoint in disk
        if self.files.contains_key(path) {}

        // Return it if it exists, if not show a message/error indicating it doesn't exist
    }

    /// Saves the current watcher's file state to the checkpoint file
    async fn save_checkpoint(
        &self,
        save_path: &Path,
        file_path: PathBuf,
        file_inode: u64,
        file_offset: u64,
    ) -> Result<Checkpoint> {
        // Determine the file's current state
        let file_state = FileState {
            path: file_path,
            inode: file_inode,
            offset: file_offset,
        };

        // File state snapshot
        let new_state = file_state.determine_file_state().await;

        // Create a new checkpoint JSON file and store it
        let checkpoint_data = serde_json::to_string_pretty(&new_state)?;

        // Persist it on disk and add its checkpoint_path to LogWatcher
        tokio::fs::write(save_path, checkpoint_data).await?;
        
        Checkpoint {
            files: HashMap<file_state.path,
        }
    }
}

/// Log watcher struct implementation
impl LogWatcher {
    /// Creates a new log watcher instance and loads the last checkpoint.
    pub async fn new_watcher(log_dir: PathBuf, checkpoint_path: PathBuf) -> Result<Self> {
        let checkpoint = Self::load_checkpoint(&checkpoint_path)
            .await
            .unwrap_or_default();
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

        // Initial discovery of files and state
        // TODO: Implement `discover_initial_files()` method
        self.discover_initial_files().await?;

        loop {
            tokio::select! {
                // Wait for filesystem events
                Some(event) = event_rx.next() => {
                    self.handle_event(event).await?;
                }

                // Fallback polling for files, especially for log retention on some systems
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
                    self.handle_new_file(path).await?;
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

    /// TODO: Implement `discover_initial_files()` method used in `run_watcher()` method

    /// Discovers new files that might have been missed by `notify`
    pub async fn discover_new_files(&mut self) -> Result<()> {
        let mut entries = fs::read_dir(&self.log_dir)?;
        while let Some(OK(entry)) = entries.next() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "log") {
                self.handle_new_file(&path).await?;
            },
        }
    }

    /// Handle newly discovered files
    async fn handle_new_file(&mut self, inode: u64, path: &Path) -> Result<()> {
        if self.active_files.contains_key(&inode) {
            return Ok(());
        } else {
            let new_file_path = path.clone().to_path_buf();
            self.active_files.insert(inode, new_file_path);
        }

        // TODO: Use the `new` Tailer function if the file is confirmed to be new

        Ok(())
    }

    /// Handles file changes(modification)
    async fn handle_file_change(&mut self, path: &Path) -> Result<()> {
        // 1. We lost track somehow, treat is as a new file

        // Otherwise, tailer already handles new writes, so nothing to do
        Ok(())
    }

    /// Handle file removal
    async fn handle_file_removal(&mut self, path: &Path) -> Result<()> {
        // 1. Find file inode

        // 2. Remove from active files

        // 3. Remove from checkpoint (or mark as closed to avoid unnecessary waste)

        // 4. Save checkpoint so we don't try resuming it

        Ok(())
    }

    /// Reads the checkpoint file from disk, it it exists
    async fn load_checkpoint(path: &Path) -> Result<Checkpoint> {
        // ... (Async logic to read JSON from file)
    }

    /// Saves the current watcher state to the checkpoint file
    async fn save_checkpoint(&self) -> Result<()> {
        // ... (Async logic to serialize Checkpoint to JSON and write to file)
    }
}
