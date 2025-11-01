use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_stream::StreamExt;

use crate::server::server::LogCollectorService;
use crate::tailer::tailer::Tailer;

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
pub struct LogWatcher {
    pub log_dir: PathBuf,
    pub checkpoint_path: PathBuf,
    pub checkpoint: Checkpoint,
    pub active_files: HashMap<u64, PathBuf>,
    pub poll_interval_ms: Option<u64>,
    pub recursive: Option<bool>,

    // `LogWatcher` itself doesn't directly call methods on `LogCollectorService`.
    // Instead, it acts as the *factory and supervisor* for `Tailer` instances.
    //
    // It holds an `Arc<LogCollectorService>` so that every Tailer it spawns can
    // share access to the same underlying pipeline — parser, buffer-batcher, shipper.
    //
    // This allows to wire local mode components together once in `main()`, then passing
    // the wiring down through the system so that every concurrent Tailer uses the same
    // core processing and shipping pipeline without duplication.
    //
    // `LogWatcher` mainly still maintains its role of local log file watching and Tailer
    // orchestration. It's not supposed to do any actual processing. This is left to the
    // Tailer which uses an `Arc<LogCollectorService>` to actual implement the processing.
    pub service: Arc<LogCollectorService>,
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

        // TODO: Currently this only checks whether a given file path exists in
        // the in-memory map. This doesn't consider the following;
        // - Missing checkpoint files on disk
        // - Corrupted JSON
        // - Changed inode (rotated log)
        // - Partial/inconsistent state between memory and disk
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
    pub async fn new_watcher(
        log_dir: PathBuf,
        checkpoint_path: PathBuf,
        poll_interval_ms: Option<u64>,
        recursive: Option<bool>,
        service: Arc<LogCollectorService>,
    ) -> Result<Self> {
        let mut checkpoint = Checkpoint {
            files: HashMap::new(),
        };

        checkpoint.load_checkpoint(&checkpoint_path).await?;

        Ok(Self {
            log_dir,
            checkpoint_path,
            checkpoint,
            active_files: HashMap::new(),
            poll_interval_ms,
            recursive,
            service,
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

        let mode = if self.recursive.unwrap_or(false) {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        watcher.watch(&self.log_dir, RecursiveMode::NonRecursive)?;

        let mut event_rx = tokio_stream::wrappers::ReceiverStream::new(rx);
        let poll_interval = self.poll_interval_ms.unwrap_or(5000); // Incase of error default to 5000ms/5secs

        self.discover_initial_files().await?;

        loop {
            tokio::select! {
                Some(event) = event_rx.next() => {
                    self.handle_event(event).await?;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(poll_interval)) => {
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
            // TODO: Implement handle_file_change() to make this functional
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

    /// Initial starting point for a new log watcher startup.
    ///
    /// [Purpose]: new LogWatcher Bootstrap phase. At this stage, we don't need to handle "new"
    /// files being created, just existing ones on disk. Also expects `notify` may not have emitted
    /// any events.
    ///
    /// - Scans the directory provided `log_dir` for .log files.
    /// - Checks if any .log files in the directory are already known (using checkpoint metadata).
    /// - Spawns an async tailer for each file.
    /// - Persists checkpoints for any new files.
    ///
    /// [TODO]: Store handles to manage each new tailer created.
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
                    reader: None,
                    service: Arc::clone(&self.service),
                };

                tokio::spawn(async move {
                    if let Err(e) = tailer.new_tailer().await {
                        eprintln!("Failed to init tailer for {:?}: {:?}", tailer.file_path, e);
                        return;
                    }
                    if let Err(e) = tailer.run_tailer(shutdown_rx).await {
                        eprintln!("Tailer error for {:?}: {:?}", tailer.file_path, e);
                    }
                });

                // If file is new(has no checkpoint), save it
                if !self.checkpoint.files.contains_key(&path) {
                    self.checkpoint
                        .save_checkpoint(&self.checkpoint_path, path, inode, 0)
                        .await?;
                }

                // TODO: Store handle for management (optional)
                // TODO: self.active_tailers.insert(inode, (handle, shutdown_tx));
            }
        }

        Ok(())
    }

    /// Runtime Discover (discover new files after a new LogWatcher is created and running).
    ///
    /// [Purpose]: Periodic polling backup. Despite log files being watched with `notify`, it's
    /// possible to miss file creation events (e.g., when the system reboots, rotated logs, or a symlink
    /// change). This periodically scans the directory again.
    ///
    pub async fn discover_new_files(&mut self) -> Result<()> {
        let entries = tokio::fs::read_dir(&self.log_dir)?;

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

    /// Adds new files when `discover_new_files()` is called.
    ///
    /// [Purpose]: Encapsulates the logic for adding one new file. Acting as a reusable
    /// helper so both event-based (`notify`) and polling-based (`discover_new_files()`) systems
    /// can use it.
    ///
    async fn handle_new_file(&mut self, inode: u64, path: &Path) -> Result<()> {
        if self.active_files.contains_key(&inode) {
            return Ok(());
        } else {
            let new_file_path = path.clone().to_path_buf();

            self.active_files.insert(inode, new_file_path);

            let mut tailer = Tailer {
                file_path: new_file_path.clone(),
                file_offset: 0,
                file_handle: String::new(),
                reader: None,
                service: Arc::clone(&self.service),
            };

            tokio::spawn(async move {
                if let Err(e) = tailer.new_tailer().await {
                    eprintln!(
                        "Failed to initialize tailer for {:?}: {:?}",
                        tailer.file_path, e
                    );
                    return;
                }
                if let Err(e) = tailer.run_tailer(shutdown_rx).await {
                    eprintln!("Tailer error for {:?}: {:?}", tailer.file_path, e);
                }
            });

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
