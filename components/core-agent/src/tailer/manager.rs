// Local crates
use crate::tailer::{
    models::{
        TailerManager,
        TailerPayload,
    },
    tailer_events::{handle_event, translate_event},
};
use crate::watcher::models::{Checkpoint, WatcherPayload};

// External crates
use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::{mpsc, broadcast};
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

        // [TODO]: Use output_rx in the next stage of the normalization stage
        // of the pipeline to receive TailerPayloads
        let (output_tx, output_rx) = mpsc::channel::<TailerPayload>(1024);

        Self {
            watcher_rx,
            shutdown_rx,
            cancel,
            tailers: HashMap::new(),
            checkpoint,
            output: output_tx,
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
                    for event in translate_event(payload) {
                        handle_event(event, &mut self.tailers, self.output.clone()).await;
                    }
                }
            }
        }

        Ok(())
    }
}
