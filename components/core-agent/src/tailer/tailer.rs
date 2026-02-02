// Local crates
use crate::tailer::models::{
    Inode,
    Tailer,
    TailerHandle,
    TailerPayload,
};

// External crates
use anyhow::Result;
use std::path::PathBuf;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

impl Tailer {
    /// Create a new individual Tailer for a specific file(inode)
    pub fn new(
        inode: Inode,
        path: PathBuf,
        offset: u64,
        output: mpsc::Sender<TailerPayload>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            inode,
            path,
            offset,
            output,
            cancel,
        }
    }

    /// Run loop for an already spawned/created `Tailer`, where the lifecycle
    /// which is managed by the `TailerManager`, `Payload` transmission, and
    /// management for an individual running Tailer takes place.
    pub async fn run(self) -> Result<()> {
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    break;
                }
                // [TODO]: Payload transmission goes here...
            }
        }

        Ok(())
    }
}

pub fn start_tailer(
    inode: u64,
    path: PathBuf,
    tailers: &mut HashMap<Inode, TailerHandle>,
    output: mpsc::Sender<TailerPayload>,
    cancel: &CancellationToken,
) {
    if tailers.contains_key(&inode) {
        return;
    }

    let tailer_cancel = cancel.child_token();

    let new_tailer = Tailer::new(
        inode,
        path,
        0,
        output.clone(),
        tailer_cancel.clone(),
    );

    let handle = tokio::task::spawn(
        new_tailer.run()
    );

    tailers.insert(
        inode, TailerHandle { join: handle, cancel: tailer_cancel }
    );

    return;
}

pub fn stop_tailer(
    inode: u64,
    tailers: &mut HashMap<Inode, TailerHandle>,
) {
    if let Some(handle) = tailers.remove(&inode) {
        handle.cancel.cancel();
    }

    return;
}

// [TODO]: Payload builder and transmission logic
