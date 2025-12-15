// Local crates
use crate::watcher::models::FileState;

// External crates
use std::path::PathBuf;
use std::os::unix::fs::MetadataExt;
use tracing::{error};

pub async fn determine_file_state(path: PathBuf) -> FileState {
    let metadata = match tokio::fs::metadata(&path).await {
        Ok(m) => m,
        Err(e) => {
            error!(
                error = %e,
                path = %path.display(),
                "Failed to get file metadata"
            );

            // Return default state
            return FileState {
                path: path,
                inode: 0,
                offset: 0,
            };
        }
    };

    #[cfg(unix)]
    let inode = {
        metadata.ino()
    };

    let offset = metadata.len();

    FileState {
        path: path.clone(),
        inode,
        offset,
    }
}
