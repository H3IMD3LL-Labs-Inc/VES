// Local crates
use crate::{
    tailer::models::TailerEvent,
    watcher::models::WatcherEvent
};

// External crates
use std::iter;

/// Translate `WatcherEvent`s received by the `TailerManager`  to `TailerEvent`s,
/// which the TailerManager understands internally
pub fn translate_to_tailer_event(
    event: WatcherEvent
) -> impl IntoIterator<Item = TailerEvent> {

    match event {
        WatcherEvent::FileDiscovered(_) => {
            vec![TailerEvent::Start]
        }
        WatcherEvent::FileRotated { .. } => {
            vec![TailerEvent::Stop, TailerEvent::Start]
        }
        WatcherEvent::FileRemoved(_) => {
            vec![TailerEvent::Stop]
        }
    }
}
