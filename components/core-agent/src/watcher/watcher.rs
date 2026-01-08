// Local crates
use crate::{
    helpers::load_config::WatcherConfig,
    watcher::{
        models::{Watcher, WatcherEvent, WatcherPayload, FileState, Checkpoint},
        discovery::*,
        events::*},
};

// External crates
use std::path::Path;
use std::path::PathBuf;
use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use tokio::{sync::{broadcast, mpsc}, time::{interval, Duration}};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn, instrument};

impl Watcher {
    pub fn new(
        config: WatcherConfig,
        checkpoint: Checkpoint,
        output: mpsc::Sender<WatcherPayload>,
    ) -> Self {
        info!("Created new Watcher");

        Self {
            config,
            checkpoint,
            output
        }
    }

    fn build_payload(&mut self, event: WatcherEvent) -> Option<WatcherPayload> {
        match &event {
            WatcherEvent::FileDiscovered { inode, path } => {
                // Insert the data file into Checkpoint
                self.checkpoint.files.insert(
                    *inode,
                    FileState {
                        inode: *inode,
                        path: path.clone(),
                        offset: 0,
                    },
                );

                Some(WatcherPayload {
                    inode: *inode,
                    path: path.clone(),
                    event,
                })
            }

            WatcherEvent::FileRotated {
                old_inode,
                new_inode,
                new_path,
                ..
            } => {
                // Update Checkpoint atomically on file rotation
                self.checkpoint.files.remove(old_inode);
                self.checkpoint.files.insert(
                    *new_inode,
                    FileState {
                        inode: *new_inode,
                        path: new_path.clone(),
                        offset: 0,
                    },
                );

                Some(WatcherPayload {
                    inode: *new_inode,
                    path: new_path.clone(),
                    event,
                })
            }

            WatcherEvent::FileRemoved { inode, .. } => {
                self.checkpoint.files.remove(inode);

                Some(WatcherPayload {
                    inode: *inode,
                    path: PathBuf::new(),
                    event,
                })
            }
        }
    }

    // running Watcher loop
    #[instrument(name = "pipeline::watcher::run", skip_all, level = "debug")]
    pub async fn run(
        mut self,
        mut shutdown_rx: broadcast::Receiver<()>,
        cancel: CancellationToken,
    ) -> Result<()> {
        info!("Starting Watcher");

        // channel to notify Watcher of FileSystem events
        let (fs_tx, mut fs_rx) = mpsc::channel::<Event>(128);

        // notify the running watcher
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                if let Ok(event) = res {
                    let _ = fs_tx.blocking_send(event);
                }
            },
            notify::Config::default(),
        )?;

        let recursive = self.config.recursive.unwrap_or(true);

        watcher.watch(
            Path::new(&self.config.log_dir),
            if recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            },
        )?;

        // bootstrapping initial data files
        discover_initial_files(
            &self.config,
            &mut self.checkpoint,
            &self.output,
        ).await?;

        let mut ticker = interval(Duration::from_secs(5));

        // running Watcher loop
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("Running Watcher task cancelled");
                    break;
                },

                Ok(_) = shutdown_rx.recv() => {
                    info!("Running Watcher received Shutdown broadcast");
                    break;
                },

                _ = ticker.tick() => {
                    // discover new data files while Watcher is running
                    discover_new_files(
                        &self.config,
                        &mut self.checkpoint,
                        &self.output
                    ).await;
                }

                Some(event) = fs_rx.recv() => {
                    let events = translate_event(event);

                    for event in events {
                        if let Some(payload) = self.build_payload(event) {
                            if let Err(e) = self.output.send(payload).await {
                                warn!(error=?e, "Failed sending WatcherPayload");
                                break;
                            }
                        }
                    }
                }
            }
        }

        info!("Running Watcher exited cleanly");
        Ok(())
    }
}
