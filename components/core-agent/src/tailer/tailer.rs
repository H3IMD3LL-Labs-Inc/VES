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
    pub async fn run(mut self) -> Result<()> {
        // [TODO]:
        // 1. Run an already spawned Tailer, responding to TailerManager instructions
        // 2. Transmit Payload to the Parser...

        Ok(())
    }
}

pub fn start_tailer(
    inode: u64,
    path: PathBuf,
    tailers: &mut HashMap<Inode, TailerHandle>,
    output: mpsc::Sender<TailerPayload>,
) {
    if tailers.contains_key(&inode) {
        return;
    }

    // [TODO]: CancellationToken handling/creation

    let new_tailer = Tailer::new(
        inode, path, 0, output.clone(), cancel.clone(),
    );

    let handle = tokio::task::spawn(
        new_tailer.run()
    );

    tailers.insert(
        inode, TailerHandle { join: handle, cancel: cancel.clone() }
    );

    return;
}

pub fn stop_tailer(
    inode: u64,
    path: PathBuf,
    tailers: &mut HashMap<Inode, TailerHandle>,
) {
    // [TODO]: Gracefully Shutdown & stop a running Tailer
}
