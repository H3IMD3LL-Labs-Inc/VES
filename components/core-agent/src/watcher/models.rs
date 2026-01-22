// Local crates
use crate::{
    helpers::load_config::WatcherConfig,
};

// External crates
use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use tokio::sync::mpsc;

/// File inode type-aliasing
pub type Inode = u64;

/// Actual `Watcher` which is responsible for watching the data file configured in *log_dir*
pub struct Watcher {
    pub config: WatcherConfig,
    pub checkpoint: Checkpoint,
    pub output: mpsc::Sender<WatcherPayload>,
}

/// Possible translations for received `notify` events from the node(system) running
/// the Core Agent. This allows events related to the filesystem and tied to the configured
/// *log_dir* to be understood by the Watcher. These `WatcherEvent`s are then sent to the
/// `TailerManager` downstream into the pipeline
#[derive(Debug, Clone)]
pub enum WatcherEvent {
    FileDiscovered {
        inode: Inode,
        path: PathBuf,
    },
    FileRotated {
        old_inode: Inode,
        new_inode: Inode,
        old_path: PathBuf,
        new_path: PathBuf,
    },
    FileRemoved {
        inode: Inode,
        path: PathBuf,
    }
}

/// Current state information for the data file configured in *log_dir*, this state is needed
/// by the `TailerManager` to determine which `WatcherEvent` is tied to which specific `Tailer`
/// as well as allow for graceful restarts incase of crashes/restarts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub path: PathBuf,
    pub inode: Inode,
    pub offset: u64,
}

/// Stores the exact point in the data file configured in *log_dir* where a running `Watcher` is
/// at. This uses FileState to determine information about the data file and gracefully restart the
/// `Watcher`
#[derive(Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub files: HashMap<Inode, FileState>,
}

/// Payload containing the `WatcherEvent` and `FileState` for the data file configured in *log_dir*.
/// This payload allows the `TailerManager` to identify the specific `Tailer` tied to the configured
/// data file.
#[derive(Clone)]
pub struct WatcherPayload {
    pub inode: u64,
    pub path: PathBuf,
    pub event: WatcherEvent,
}
