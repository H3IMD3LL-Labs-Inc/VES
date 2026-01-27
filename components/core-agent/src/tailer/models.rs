// Local crates
use crate::watcher::models::{Checkpoint, WatcherPayload};

// External crates
use anyhow::Result;
use bytes::Bytes;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// File inode type-aliasing
pub type Inode = u64;

/// Control plane for all Tailers, all running Tailers' actions, **`i.e, creation, deletion, stop, shutdown, restart, etc.`** are guided by this manager.
/// Using `TailerEvent`s the TailerManager is responsible for managing the lifecycle
/// of all Tailers separately and independently. `WatcherEvent`s are received by
/// this manager, translated into appropriate `TailerEvent`s and sent to the specific individual Tailer of the file related
/// to the WatcherEvent via the `Checkpoint`
///
/// `Tailer`s are stored in the TailerManager and identified via their individual `TailerHandle` value
/// and the `Inode` value of the file they are tailing.
///
/// ```
/// WatcherEvent -> TailerManager -> TailerEvent -> Tailer
/// ```
///
pub struct TailerManager {
    pub watcher_rx: broadcast::Receiver<WatcherPayload>,
    pub shutdown_rx: broadcast::Receiver<()>,
    pub cancel: CancellationToken,
    pub tailers: HashMap<Inode, TailerHandle>,
    pub checkpoint: Checkpoint,
    pub output: mpsc::Sender<TailerPayload>,
}

/// Control plane object that represents an individual running `Tailer` task. Allows `TailerManager`
/// to have control over an individual Tailer.
pub struct TailerHandle {
    pub join: JoinHandle<Result<()>>,
    pub cancel: CancellationToken,
}

/// Control plane translations for possible received `WatcherEvent`s. These allow the
/// `TailerManager` to determine what action an individual Tailer should take based on a certain WatcherEvent.
pub enum TailerEvent {
    Start {
        inode: Inode,
        path: PathBuf,
    },
    Stop {
        inode: Inode,
        path: PathBuf,
    },
    Rotate {
        old_inode: Inode,
        new_inode: Inode,
        path: PathBuf,
    },
}

/// An individual `Tailer` running in the pipeline for an individual file being tailed.
/// All Tailers map to a single data file 1:1, and their lifecycle is managed based on
/// events happening on the file a Tailer is mapped to
pub struct Tailer {
    pub inode: Inode,
    pub path: PathBuf,
    pub offset: u64,
    pub output: mpsc::Sender<TailerPayload>,
    pub cancel: CancellationToken,
}

/// `Payload` is the unit of data a Tailer emits downstream. It represents one logical
/// piece of data read from a file. It is not bytes, not lines necessarily, and not
/// file metadata.
pub struct TailerPayload {
    pub inode: Inode,
    pub offset: u64,
    pub raw_data: Bytes,
    pub size: usize,
}
