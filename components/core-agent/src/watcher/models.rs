// External crates
use std::{
    collections::HashMap,
    path::PathBuf,
};
use serde::{Serialize, Deserialize};

/// Event translation types
#[derive(Debug, Clone)]
pub enum WatcherEvent {
    FileDiscovered(PathBuf),
    FileRotated(PathBuf),
    FileRemoved(PathBuf)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub path: PathBuf,
    pub inode: u64,
    pub offset: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    // TODO: Refactor to use inode instead of PathBuf as a key
    pub files: HashMap<PathBuf, FileState>,
}
