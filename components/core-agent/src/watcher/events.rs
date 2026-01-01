// Local crates
use crate::{
    watcher::models::WatcherEvent,
};

// External crates
use notify::{Event, EventKind};
use tracing::{info, warn};

/// Translate notify EventKinds in configured *log_dir* to a `WatcherEvent` type
pub fn translate_event(event: Event) -> Vec<WatcherEvent> {
    let mut out = Vec::new();

    match event.kind {
        EventKind::Create(notify::event::CreateKind::File) => {
            info!("File created event");
            for path in event.paths {
                info!(?path, "New file created in the filesystem");
                out.push(WatcherEvent::FileDiscovered(path));
            }
        }
        EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
            for path in event.paths {
                info!(?path, "File rotated in the filesystem");
                out.push(WatcherEvent::FileRotated(path));
            }
        }
        EventKind::Remove(notify::event::RemoveKind::File) => {
            for path in event.paths {
                info!(?path, "File removed in the filesystem");
                out.push(WatcherEvent::FileRemoved(path));
            }
        }
        other => {
            warn!(?other, "Unfamiliar filesystem event occurred");
        }
    }

    out
}
