// Local crates
use crate::{
    helpers::load_config::WatcherConfig,
    watcher::{
        models::{Checkpoint, WatcherEvent, WatcherPayload},
        state::determine_file_state,
    }
};

// External crates
use std::path::Path;
use walkdir::WalkDir;
use tokio::sync::mpsc;
use tracing::info;
use anyhow::Result;

/// Discover initial data files in configured *log_dir* to bootstrap
/// a running Watcher
pub async fn discover_initial_files(
    config: &WatcherConfig,
    checkpoint: &mut Checkpoint,
    output: &mpsc::Sender<WatcherPayload>
) -> Result<()> {
    for entry in build_walker(config).into_iter().filter_map(Result::ok) {
        let path = entry.path().to_path_buf();

        if !valid_file_format(&path) {
            continue;
        }

        let state = determine_file_state(path.clone()).await;
        let inode = state.inode;

        if let Some(existing) = checkpoint.files.get(&inode) {
            info!(
                file = %path.display(),
                offset = existing.offset,
                "Restoring file from Checkpoint"
            );
            continue;
        }

        info!(
            file = %path.display(),
            inode = inode,
            "Discovered new data file at startup"
        );

        checkpoint.files.insert(inode, state);

        let payload = WatcherPayload {
            inode,
            path: path.clone(),
            event: WatcherEvent::FileDiscovered { inode, path },
        };

        output.send(payload).await?;
    }

    Ok(())
}

/// Discover new data files in configured *log_dir* to avoid missing
/// new files when a Watcher is running.
///
/// This is intended to handle edge cases where [**notify**](https://docs.rs/notify/latest/notify/index.html) misses filesystem
/// events and when new data files are discovered.
pub async fn discover_new_files(
    config: &WatcherConfig,
    checkpoint: &mut Checkpoint,
    output: &mpsc::Sender<WatcherPayload>
) -> Result<()> {
    for entry in build_walker(config).into_iter().filter_map(Result::ok) {
        let path = entry.path().to_path_buf();

        if !valid_file_format(&path) {
            continue;
        }

        let state = determine_file_state(path.clone()).await;
        let inode = state.inode;

        if checkpoint.files.contains_key(&inode) {
            continue;
        }

        info!(
            file = %path.display(),
            inode = inode,
            "Discovered new data file during runtime scan, added to Checkpoint"
        );

        checkpoint.files.insert(inode, state);

        let payload = WatcherPayload {
            inode,
            path: path.clone(),
            event: WatcherEvent::FileDiscovered { inode, path },
        };

        output.send(payload).await?;
    }

    Ok(())
}

fn build_walker(config: &WatcherConfig) -> WalkDir {
    let mut filesystem_walker = WalkDir::new(&config.log_dir)
        .follow_links(false)
        .same_file_system(true);

    if !config.recursive.unwrap_or(true) {
        filesystem_walker = filesystem_walker
            .min_depth(0)
            .max_depth(1)
    }

    filesystem_walker
}

fn valid_file_format(path: &Path) -> bool {
    if path.is_dir() {
        return false;
    }

    if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
        if file_name.starts_with('.') {
            return false;
        }
    }

    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("log") | Some("txt")
    )
}
