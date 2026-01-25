// Local crates
use crate::{
    tailer::models::TailerEvent,
    watcher::models::{
        WatcherPayload,
        WatcherEvent
    },
};

pub fn translate_event(
    payload: WatcherPayload
) -> impl IntoIterator<Item = TailerEvent> {

    match payload.event {
        WatcherEvent::FileDiscovered { inode, path } => {
            vec![TailerEvent::Start { inode, path }]
        }

        WatcherEvent::FileRotated {
            old_inode,
            new_inode,
            old_path,
            new_path,
        } => {
            vec![
                TailerEvent::Stop { inode: old_inode, path: old_path },
                TailerEvent::Start {
                    inode: new_inode,
                    path: new_path,
                }
            ]
        }

        WatcherEvent::FileRemoved { inode, path } => {
            vec![TailerEvent::Stop { inode, path }]
        }
    }
}

/// Handle `TailerEvent`s for a specific `Tailer`. This allows the `TailerManager` to
/// start, stop or manage(determine) appropriate actions a Tailer should take, based on
/// the Tailer's `TailerEvent`s
pub async fn handle_event(event: TailerEvent) {
    // [TODO]: Determine the specific Tailer a TailerEvent belongs to

    // [TODO]: Take the necessary action based on the TailerEvent(Start, Stop & Stop->Start)

    // [TODO]: Give tracing info for the specific Tailer action taken
}
