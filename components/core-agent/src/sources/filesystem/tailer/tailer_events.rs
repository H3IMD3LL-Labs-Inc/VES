// Local crates
use crate::{
    tailer::{
        tailer::{
            start_tailer,
            stop_tailer,
        },
        models::{
            Inode,
            TailerHandle,
            TailerEvent,
            TailerPayload,
        },
    },
    watcher::models::{
        WatcherPayload,
        WatcherEvent
    },
};

// External crates
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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

pub async fn handle_event(
    event: TailerEvent,
    tailers: &mut HashMap<Inode, TailerHandle>,
    output: mpsc::Sender<TailerPayload>,
    cancel: &CancellationToken,
) {
    match event {
        TailerEvent::Start { inode, path } => {
            start_tailer(
                inode, path, tailers, output, cancel
            )
        }
        TailerEvent::Stop { inode, path } => {
            stop_tailer(inode, tailers)
        }
        TailerEvent::Rotate { old_inode, new_inode, path } => {
            stop_tailer(old_inode, tailers);
            start_tailer(new_inode, path, tailers, output, cancel)
        }
    }
}
