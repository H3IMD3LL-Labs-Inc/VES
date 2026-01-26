// Local crates
use crate::tailer::{
    models::TailerManager,
    tailer_events::{handle_event, translate_event},
};
use crate::watcher::models::{Checkpoint, WatcherPayload};

// External crates
use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

impl TailerManager {
    /// Create a new `TailerManager` once when the pipeline starts for the first
    /// time or restarts
    pub fn new(
        watcher_rx: broadcast::Receiver<WatcherPayload>,
        shutdown_rx: broadcast::Receiver<()>,
        checkpoint: Checkpoint,
        parent_cancel: CancellationToken,
    ) -> Self {
        let cancel = parent_cancel.child_token();

        Self {
            watcher_rx,
            shutdown_rx,
            cancel,
            tailers: HashMap::new(),
            checkpoint,
        }
    }

    /// Continuously receive `WatcherEvent`s from the Watcher and manage the pipeline's
    /// `Tailer`s based on them. This is the main orchestration loop for all Tailers
    pub async fn run(mut self) -> Result<()> {
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    break;
                },

                Ok(_) = self.shutdown_rx.recv() => {
                    break;
                },

                Ok(payload) = self.watcher_rx.recv() => {
                    let tailer_events = translate_event(payload);

                    for event in tailer_events {
                        handle_event(event).await;
                    }
                }
            }
        }

        Ok(())
    }
}
