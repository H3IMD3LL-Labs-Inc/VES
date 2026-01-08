// Local crates
use crate::{
    watcher::models::WatcherEvent,
};

// External crates
use std::fs;
use std::path::Path;
use std::os::unix::fs::MetadataExt;
use notify::{
    Event,
    EventKind,
    event::{
        CreateKind, ModifyKind, RemoveKind, RenameMode
    },
};
use tracing::{info, warn};

fn inode_for(path: &Path) -> Option<u64> {
    fs::metadata(path).ok().map(|m| m.ino())
}

/// Translate notify EventKinds in configured *log_dir* to a `WatcherEvent` type
pub fn translate_event(event: Event) -> Vec<WatcherEvent> {
    let mut out = Vec::new();

    match event.kind {
        EventKind::Create(CreateKind::File) => {
            info!("File created event");
            for path in event.paths {
                info!(?path, "New file created in the filesystem");
                if let Some(inode) = inode_for(&path) {
                    info!(?path, "New file created in the filesystem");
                    out.push(WatcherEvent::FileDiscovered {
                        inode,
                        path
                    });
                }
            }
        }

        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            if event.paths.len() == 2 {
                let old_path = event.paths[0].clone();
                let new_path = event.paths[1].clone();

                let old_inode = inode_for(&old_path);
                let new_inode = inode_for(&new_path);

                if let (Some(old_inode), Some(new_inode)) = (old_inode, new_inode) {
                    info!("File rotated in the filesystem");
                    out.push(WatcherEvent::FileRotated {
                        old_inode,
                        new_inode,
                        old_path,
                        new_path,
                    });
                }
            }
        }

        EventKind::Remove(RemoveKind::File) => {
            for path in event.paths {
                // inode may no longer exist, best effort lookup sometimes
                // metadata still works if file is unlinked but open
                if let Some(inode) = inode_for(&path) {
                    out.push(WatcherEvent::FileRemoved {
                       inode,
                       path
                    });
                }
            }
        }

        other => {
            warn!(?other, "Unfamiliar filesystem event occurred");
        }
    }

    out
}
