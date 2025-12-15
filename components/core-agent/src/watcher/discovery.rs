// Local crates
use crate::{
    helpers::load_config::WatcherConfig,
    watcher::{
        models::{Checkpoint, WatcherEvent},
        state::determine_file_state,
    }
};

// External crates
use std::path::Path;
use walkdir::WalkDir;
use tokio::sync::mpsc;
use tracing::{info, warn};
use anyhow::Result;

/// Discover initial data files in configured *log_dir* to bootstrap
/// a running Watcher
pub async fn discover_initial_files(
    config: &WatcherConfig,
    checkpoint: &mut Checkpoint,
    output: &mpsc::Sender<WatcherEvent>
) -> Result<()> {
    for entry in build_walker(config).into_iter().filter_map(Result::ok) {
        let path = entry.path().to_path_buf();

        if !valid_file_format(&path) {
            continue;
        }

        let current_state = determine_file_state(path.clone()).await;

        match checkpoint.files.get(&path) {
            Some(existing) if existing.inode == current_state.inode => {
                // Same file determined
                info!(
                    file = %path.display(),
                    offset = existing.offset,
                    "Restoring file from Checkpoint"
                );
            }
            Some(_) => {
                // Inode change determined
                warn!(
                    file = %path.display(),
                    "File inode changed, treating as new file"
                );
                checkpoint.files.insert(path.clone(), current_state);
            }
            None => {
                // New file determined
                checkpoint.files.insert(path.clone(), current_state);
            }
        }

        output
            .send(WatcherEvent::FileDiscovered(path))
            .await?;
    }

    Ok(())
}

/// Discover new data files in configured *log_dir* to avoid missing
/// new files when a Watcher is running.
///
/// This is intended to handle edge cases where [**notify**](https://docs.rs/notify/latest/notify/index.html) misses filesystem
/// events.
pub async fn discover_new_files(
    config: &WatcherConfig,
    checkpoint: &mut Checkpoint,
    output: &mpsc::Sender<WatcherEvent>
) -> Result<()> {
    for entry in build_walker(config).into_iter().filter_map(Result::ok) {
        let path = entry.path().to_path_buf();

        if !valid_file_format(&path) {
            continue;
        }

        if checkpoint.files.contains_key(&path) {
            continue;
        }

        let file_state = determine_file_state(path.clone()).await;
        checkpoint.files.insert(path.clone(), file_state);

        output
            .send(WatcherEvent::FileDiscovered(path))
            .await?;

        info!("Discovered new data file during runtime scan, added to Checkpoint");
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
