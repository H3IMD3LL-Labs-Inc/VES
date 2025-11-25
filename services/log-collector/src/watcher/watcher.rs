// Local crates
use crate::{
    metrics::metrics::SNAPSHOT_RECOVERY_DURATION_SECONDS, server::server::LogCollectorService,
    tailer::tailer::Tailer,
};

// External crates
use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio_stream::StreamExt;
use tracing::{Instrument, instrument};

#[derive(Debug, Serialize, Deserialize)]
struct FileState {
    path: PathBuf,
    inode: u64,
    offset: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Checkpoint {
    files: HashMap<PathBuf, FileState>,
}

/// Main log watcher struct.
#[derive(Debug)]
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

    tailer_handles: Vec<tokio::task::JoinHandle<()>>,
    tailer_shutdown_txs: Vec<tokio::sync::watch::Sender<bool>>,
}

/// FileState Methods
impl FileState {
    /// Recalculate file current state based on the filesystem (Currently only supports Unix/Unix-like OS)
    #[instrument(
        name = "ves_log_file_state",
        target = "watcher::watcher::FileState",
        skip_all,
        level = "trace"
    )]
    async fn determine_file_state(&self) -> FileState {
        let metadata = match tokio::fs::metadata(&self.path).await {
            Ok(m) => m,
            Err(e) => {
                tracing::error!(
                    error = %e,
                    path = %self.path.display(),
                    "Failed to get file metadata"
                );

                // Return default state
                return FileState {
                    path: self.path.clone(),
                    inode: 0,
                    offset: 0,
                };
            }
        };

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
    // TODO: Add method to create the checkpoint file when VES starts, and
    // check if checkpoint file already exists.

    /// Reads the checkpoint file from disk, it it exists
    #[instrument(
        name = "ves_log_file_checkpoint_loading",
        target = "watcher::watcher::Checkpoint",
        skip_all,
        level = "trace"
    )]
    async fn load_checkpoint(&mut self, saved_file_path: &Path) -> Result<()> {
        let saved_file_path_buf = saved_file_path.to_path_buf();

        tracing::trace!(
            "Attempting to load checkpoint file for {:?}",
            saved_file_path_buf
        );

        // TODO: Currently this only checks whether a given file path exists in
        // the in-memory map. This doesn't consider the following;
        // - Missing checkpoint files on disk
        // - Corrupted JSON
        // - Changed inode (rotated log)
        // - Partial/inconsistent state between memory and disk
        if self.files.contains_key(&saved_file_path_buf) {
            let contents = match tokio::fs::read_to_string(&saved_file_path).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(
                        "Failed to read checkpoint file {:?}: {e}",
                        saved_file_path_buf
                    );
                    return Ok(());
                }
            };

            let checkpoint_data: Checkpoint = match serde_json::from_str(&contents) {
                Ok(c) => {
                    tracing::debug!("Successfully deserialized checkpoint JSON file");
                    c
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to deserialize checkpoing JSON file {:?}: {e}",
                        saved_file_path_buf
                    );
                    return Ok(());
                }
            };

            let disk_file_state = checkpoint_data.files.get(&saved_file_path_buf);
            let in_memory_file_state = self.files.get(&saved_file_path_buf);

            match (disk_file_state, in_memory_file_state) {
                (Some(disk), Some(memory)) => {
                    if disk.inode == memory.inode {
                        // TODO: Same file -> Safe to restore
                        tracing::trace!("Checkpoint matches in-memory state (same inode)");
                    } else {
                        // TODO: Different inode -> File was rotated or replaced
                        tracing::warn!(
                            "Inode mismatch for {:?}: disk={}, memory={}",
                            saved_file_path_buf,
                            disk.inode,
                            memory.inode
                        );
                    }
                }
                (Some(_disk), None) => {
                    // TODO: Decide what to do in this case
                    tracing::warn!(
                        "Disk checkpoint exists for {:?} but not found in memory",
                        saved_file_path_buf
                    );
                }
                (None, Some(_mem)) => {
                    // TODO: Decide what to do in this case
                    tracing::warn!(
                        "Memory checkpoint exists {:?} but no entry in disk checkpoint",
                        saved_file_path_buf
                    );
                }
                // No need to match (None, None), we already confirmed the file exists with .contains_key()
                (None, None) => {
                    tracing::warn!("Unexpected (None, None) checkpoint state reached");
                }
            }
        } else {
            tracing::error!(
                "File {:?} not found in in-memory checkpoint",
                saved_file_path_buf
            );
            return Ok(());
        }

        Ok(())
    }

    #[instrument(
        name = "ves_log_file_checkpoint_saving",
        target = "watcher::watcher::Checkpoint",
        skip_all,
        level = "trace"
    )]
    async fn save_checkpoint(
        &mut self,
        save_path: &Path,
        file_path: PathBuf,
        file_inode: u64,
        file_offset: u64,
    ) -> Result<()> {
        tracing::trace!("Saving checkpoint for {:?}", file_path);

        let file_state = FileState {
            path: file_path.clone(),
            inode: file_inode,
            offset: file_offset,
        };

        tracing::debug!(
            "Updating in-memory state: inode={}, offset={}",
            file_inode,
            file_offset
        );
        self.files.insert(file_path, file_state);

        let checkpoint_data_json = match serde_json::to_string_pretty(&self) {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Failed to serialize checkpoint JSON: {e}");
                return Err(e.into());
            }
        };

        if let Err(e) = tokio::fs::write(save_path, checkpoint_data_json).await {
            tracing::error!("Failed to write checkpoint file at {:?}: {e}", save_path);
            return Err(e.into());
        }

        tracing::trace!("Checkpoint successfully written to {:?}", save_path);
        Ok(())
    }
}

/// Log watcher struct implementation
impl LogWatcher {
    /// Creates a new log watcher instance and loads the last checkpoint, if it exists.
    #[instrument(
        name = "ves_create_new_log_watcher",
        target = "watcher::watcher::LogWatcher",
        skip_all,
        level = "trace"
    )]
    pub async fn new_watcher(
        log_dir: PathBuf,
        checkpoint_path: PathBuf,
        poll_interval_ms: Option<u64>,
        recursive: Option<bool>,
        service: Arc<LogCollectorService>,
    ) -> Result<Self> {
        tracing::trace!("Creating new in-memory Checkpoint files storage");
        let mut checkpoint = Checkpoint {
            files: HashMap::new(),
        };

        // Measure snapshot recovery duration
        let recovery_start = Instant::now();

        tracing::trace!("Attempting to load file checkpoint");
        checkpoint.load_checkpoint(&checkpoint_path).await?;

        let recovery_duration = recovery_start.elapsed().as_secs_f64();
        SNAPSHOT_RECOVERY_DURATION_SECONDS.set(recovery_duration);

        Ok(Self {
            log_dir,
            checkpoint_path,
            checkpoint,
            active_files: HashMap::new(),
            poll_interval_ms,
            recursive,
            service,
            tailer_handles: Vec::new(),
            tailer_shutdown_txs: Vec::new(),
        })
    }

    /// Main async loop that orchestrates the watcher
    #[instrument(
        name = "ves_run_log_watcher",
        target = "watcher::watcher::LogWatcher",
        skip_all,
        level = "trace"
    )]
    pub async fn run_watcher(
        &mut self,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<()> {
        tracing::trace!("Starting local file watcher");

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        let span = tracing::Span::current();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _enter = span.enter();
                match res {
                    Ok(event) => {
                        let _ = tx.blocking_send(event);
                    }
                    Err(e) => {
                        tracing::error!("Local file watcher error callback: {e}");
                    }
                }
            },
            notify::Config::default(),
        )?;

        let mode = if self.recursive.unwrap_or(false) {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        tracing::debug!(
            "Watching directory {:?} (recursive = {:?})",
            self.log_dir,
            self.recursive
        );

        watcher.watch(&self.log_dir, RecursiveMode::NonRecursive)?;

        let mut event_rx = tokio_stream::wrappers::ReceiverStream::new(rx);
        let poll_interval = self.poll_interval_ms.unwrap_or(5000); // Incase of error default to 5000ms/5secs

        tracing::debug!("Running LogWatcher set poll interval = {}ms", poll_interval);

        self.discover_initial_files().await?;

        tracing::trace!("Starting LogWatcher main event loop");
        loop {
            tokio::select! {
                Some(event) = event_rx.next() => {
                    tracing::trace!("FileSystem event received by the running LogWatcher");
                    self.handle_event(event).await?;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(poll_interval)) => {
                    tracing::trace!("Triggering LogWatcher periodic file discovery cycle");
                    self.discover_new_files().await?;
                }
                Ok(_) = shutdown_rx.recv() => {
                    tracing::warn!("Running LogWatcher received shutdown signal");
                    self.shutdown().await;
                    break;
                }
            }
        }

        tracing::trace!("Running LogWatcher loop exited cleanly");
        Ok(())
    }

    /// Broadcast shutdown signal to all Tailers runnings and await them
    #[instrument(
        name = "ves_log_watcher_shutdown_signal_propagate",
        target = "watcher::watcher::LogWatcher",
        skip_all,
        level = "trace"
    )]
    pub async fn shutdown(&mut self) {
        tracing::trace!(
            running_tailers = %self.tailer_shutdown_txs.len(),
            "Broadcasting shutdown signal to all running Tailers"
        );

        // Send shutdown signal to all Tailers
        for tx in &self.tailer_shutdown_txs {
            let _ = tx.send(true);
        }

        // Awaitt all tailers to finish
        for handle in self.tailer_handles.drain(..) {
            let _ = handle.await;
        }

        tracing::trace!("Watcher successfully shutdown gracefully");
    }

    /// Handles file system events from the `notify` watcher
    #[instrument(
        name = "ves_log_watcher_file_system_event",
        target = "watcher::watcher::LogWatcher",
        skip_all,
        level = "trace"
    )]
    async fn handle_event(&mut self, event: Event) -> Result<()> {
        tracing::trace!("Handling incoming filesystem event");

        match event.kind {
            notify::EventKind::Create(notify::event::CreateKind::File) => {
                tracing::trace!("File created event");
                for path in &event.paths {
                    tracing::trace!(?path, "New file detected in the filesystem");
                    let metadata = tokio::fs::metadata(path).await?;
                    let inode = metadata.ino();
                    self.handle_new_file(inode, path).await?;
                }
            }
            // TODO: Implement handle_file_change() to make this functional
            notify::EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
                tracing::trace!("File modified event");
                for path in &event.paths {
                    tracing::trace!(?path, "File change detected");
                    self.handle_file_change(path).await?;
                }
            }
            notify::EventKind::Remove(notify::event::RemoveKind::File) => {
                tracing::trace!("File removed event");
                for path in &event.paths {
                    tracing::trace!(?path, "File removal detected");
                    self.handle_file_removal(path).await?;
                }
            }
            // TODO: Fix..
            /*notify::EventKind::Modify(notify::event::ModifyKind::Name) => {
                // This event can be tricky. A rename is often a remove + create
                // or move event. `notify` handles this well enough that we can rely
                // primarily on the `Create` and `Remove` events.
            }*/
            other => {
                tracing::debug!(?other, "Unfamiliar filesystem event, unhandled");
            }
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
    #[instrument(
        name = "ves_log_watcher_initial_files_discovery",
        target = "watcher::watcher::LogWatcher",
        skip_all,
        level = "trace"
    )]
    async fn discover_initial_files(&mut self) -> Result<()> {
        tracing::trace!(
            log_dir = %self.log_dir.display(),
            "Reading configured log_dir to identify initial local log files to process"
        );
        let mut entries = tokio::fs::read_dir(&self.log_dir).await?;

        // TODO: Ensure logs directory is not empty before proceeding. Display
        // directory file count in tracing.

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            tracing::trace!(?path, "Found directory entry");

            // Only handle .log files
            if path.extension().map_or(false, |ext| ext == "log") {
                tracing::debug!(?path, "Identified .log file");
                // Check if a checkpoint for this file already exists
                let (inode, offset) = match tokio::fs::metadata(&path).await {
                    Ok(metadata) => (metadata.ino(), {
                        self.checkpoint
                            .files
                            .get(&path)
                            .map(|f| f.offset)
                            .unwrap_or(0)
                    }),
                    Err(e) => {
                        tracing::error!(?path, error = %e, "Failed to load metadata for log file");
                        continue;
                    }
                };
                tracing::trace!(
                    ?path,
                    inode,
                    offset,
                    "Resolved log file metadata and checkpoint offset"
                );

                // Track in active_files
                self.active_files.insert(inode, path.clone());

                // Create shutdown channel for this tailer
                let (tx, rx) = tokio::sync::watch::channel(false);

                // Start a tailer
                let mut tailer = Tailer {
                    file_path: path.clone(),
                    file_offset: offset,
                    file_handle: String::new(),
                    reader: None,
                    service: Arc::clone(&self.service),
                };
                tracing::trace!(
                    ?path,
                    inode,
                    offset,
                    "Spawning tailer for discovered log file"
                );

                let handle = tokio::spawn(
                    async move {
                        if let Err(e) = tailer.new_tailer().await {
                            tracing::error!(
                                ?tailer.file_path,
                                error = %e,
                                "Failed to initialize tailer"
                            );
                            return;
                        }
                        if let Err(e) = tailer.run_tailer(rx).await {
                            tracing::error!(
                                ?tailer.file_path,
                                error = %e,
                                "Tailer runtime error"
                            );
                        }
                    }
                    .instrument(tracing::Span::current()),
                );

                self.tailer_handles.push(handle);
                self.tailer_shutdown_txs.push(tx);

                // If file is new(has no checkpoint), save it
                if !self.checkpoint.files.contains_key(&path) {
                    tracing::trace!(?path, inode, "Saving initial checkpoint for new log file");
                    self.checkpoint
                        .save_checkpoint(&self.checkpoint_path, path, inode, 0)
                        .await?;
                }
            }
        }

        tracing::trace!("Initial log file(s) discovery completed");
        Ok(())
    }

    /// Runtime Discovery (discover new files after a new LogWatcher is created and running).
    ///
    /// [Purpose]: Periodic polling backup. Despite log files being watched with `notify`, it's
    /// possible to miss file creation events (e.g., when the system reboots, rotated logs, or a symlink
    /// change). This periodically scans the directory again.
    #[instrument(
        name = "ves_log_watcher_runtime_files_discovery",
        target = "watcher::watcher::LogWatcher",
        skip_all,
        level = "trace"
    )]
    pub async fn discover_new_files(&mut self) -> Result<()> {
        tracing::trace!(
            log_dir = %self.log_dir.display(),
            "Reading log_dir to identify new log files to process"
        );
        let mut entries = tokio::fs::read_dir(&self.log_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            tracing::trace!(?path, "Found new log_dir log file entry");

            // Only consider `.log` files
            if path.extension().map_or(false, |ext| ext == "log") {
                tracing::debug!(?path, "Identified new .log file");
                let metadata = entry.metadata().await?;
                let inode = metadata.ino();

                tracing::trace!(?path, "Handling new .log file discovered in log_dir");
                self.handle_new_file(inode, &path).await?;
            }
        }

        // TODO: Display the log file's actual path as well as log_dir
        tracing::trace!(
            log_dir = %self.log_dir.display(),
            "Discovered and handled new .log file in log_dir"
        );
        Ok(())
    }

    /// Adds new files when `discover_new_files()` is called.
    ///
    /// [Purpose]: Encapsulates the logic for adding one new file. Acting as a reusable
    /// helper so both event-based (`notify`) and polling-based (`discover_new_files()`) systems
    /// can use it.
    #[instrument(
        name = "ves_log_watcher_new_log_files_handling",
        target = "watcher::watcher::LogWatcher",
        skip_all,
        level = "trace"
    )]
    async fn handle_new_file(&mut self, inode: u64, path: &Path) -> Result<()> {
        tracing::trace!(
            log_file_inode = %inode,
            "Checking if the log file exists in active_files before inserting into active_files"
        );
        if self.active_files.contains_key(&inode) {
            tracing::debug!(
                log_file_inode = %inode,
                "Log file found in active_files, skipping insert"
            );
            return Ok(());
        }

        tracing::trace!(
            log_file_inode = %inode,
            log_file_path = %path.display(),
            "Inserting new log file to active_files"
        );
        self.active_files.insert(inode, path.to_path_buf());

        let (tx, rx) = tokio::sync::watch::channel(false);

        let mut tailer = Tailer {
            file_path: path.to_path_buf(),
            file_offset: 0,
            file_handle: String::new(),
            reader: None,
            service: Arc::clone(&self.service),
        };
        tracing::trace!(
            ?path,
            inode,
            %tailer.file_offset,
            ?tailer.file_handle,
            ?tailer.reader,
            ?tailer.service,
            "Spawning new tailer for newly disovered file"
        );

        let handle = tokio::spawn(async move {
            if let Err(e) = tailer.new_tailer().await {
                tracing::error!(
                    ?tailer.file_path,
                    error = %e,
                    "Failed to initialize tailer"
                );
                return;
            }
            if let Err(e) = tailer.run_tailer(rx).await {
                tracing::error!(
                    ?tailer.file_path,
                    error = %e,
                    "Tailer runtime error"
                );
            }
        });

        self.tailer_handles.push(handle);
        self.tailer_shutdown_txs.push(tx);

        tracing::trace!(?path, inode, "Saving initial checkpoint for new log file");
        self.checkpoint
            .save_checkpoint(&self.checkpoint_path, path.to_path_buf(), inode, 0)
            .await?;

        tracing::trace!(new_log_file_path = ?path, inode, "Handled new log file");
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
    #[instrument(
        name = "ves_log_watcher_log_file_removal_handling",
        target = "watcher::watcher::LogWatcher",
        skip_all,
        level = "trace"
    )]
    async fn handle_file_removal(&mut self, path: &Path) -> Result<()> {
        let file_path_buf = path.to_path_buf();
        tracing::trace!(file_path = ?file_path_buf, "Handling file removal for log file");

        // Check if tracking this file in memory
        if let Some(file_state) = self.checkpoint.files.get(&file_path_buf) {
            tracing::debug!(
                file_path = ?file_path_buf,
                inode = %file_state.inode,
                "File is tracked in checkpoint"
            );
            // Try to get metadata from disk
            match tokio::fs::metadata(&file_path_buf).await {
                Ok(metadata) => {
                    // File exists on disk
                    let inode_on_disk = metadata.ino();
                    tracing::trace!(
                        file_path = ?file_path_buf,
                        file_disk_inode = %inode_on_disk,
                        "File exists on disk"
                    );

                    if inode_on_disk != file_state.inode {
                        // Inode differe -> file was rotated/replaced
                        tracing::warn!(
                            file_path = ?file_path_buf,
                            old_inode = %file_state.inode,
                            new_inode = %inode_on_disk,
                            "File inode mismatch detected, file has been rotated or replaced"
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
                        tracing::trace!(
                            file_path = ?file_path_buf,
                            inode = %inode_on_disk,
                            "File still valid, no action needed"
                        );
                    }
                }
                Err(_) => {
                    // File not present on disk -> true deletion
                    tracing::info!(
                        file_path = ?file_path_buf,
                        "File deleted on disk"
                    );

                    // Remove from in-memory active_files
                    self.active_files.retain(|_, p| p != &file_path_buf);
                    tracing::debug!(
                        file_path = ?file_path_buf,
                        "Removed file from active_files"
                    );

                    // Remove from Checkpoint
                    self.checkpoint.files.remove(&file_path_buf);
                    tracing::debug!(
                        file_path = ?file_path_buf,
                        "Removed file from checkpoint"
                    );

                    // Save updated Checkpoint
                    if let Ok(data) = serde_json::to_string_pretty(&self.checkpoint) {
                        if let Err(e) = tokio::fs::write(&self.checkpoint_path, data).await {
                            tracing::error!(
                                file_path = ?file_path_buf,
                                error = %e,
                                "Failed to save updated checkpoint"
                            );
                        } else {
                            tracing::info!(
                                file_path = ?file_path_buf,
                                "Updated checkpoint saved after file removal"
                            );
                        }
                    }
                }
            }
        } else {
            // Not a tracked file, ignore
            tracing::debug!(
                file_path = ?file_path_buf,
                "File is not tracked, ignoring removal"
            );
        }

        Ok(())
    }
}
