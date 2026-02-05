// Local crates
use crate::tailer::models::{
    Inode,
    TailerHandle,
    TailerPayload,
};

// external crates
use std::collections::HashMap;
use std::path::PathBuf;
use bytes::Bytes;
use async_stream::stream;
use tokio_stream::Stream;

impl TailerPayload {
    fn payload_data_size(&self) -> usize {
        self.raw_data.len()
    }

    fn payload_data_empty(&self) -> bool {
        self.raw_data.is_empty()
    }
}

pub fn build_payload(
    offset: u64,
    tailers: HashMap<Inode, TailerHandle>,
) -> TailerPayload {
    // [TODO]: Identify specific Tailer sending the TailerPayload
    // [TODO]: Identify TailerPayload size for metrics/tracing
    // [TODO]: Identify file offset of the data read (from start to finish) and update Checkpoint
    // [TODO]: Build the actual TailerPayload
}

async fn read_next_data_chunk(
    path: PathBuf,
    offset: u64,
    tailers: HashMap<Inode, TailerHandle>,
    // [TODO]: Add Checkpoint argument, too get read offset
) {
    let mut file = File::open(path)?;

    // [TODO]: Get the opened file's offset from Checkpoint

    let stream = build_read_buffer(file, offset);

    while let Some(item) = stream.next().await {
        match item {
            Ok(bytes) => {
                // [TODO]: Handle the chunk of raw_bytes received
            }
            Err(e) => {
                // [TODO]: Handle read error occurrence
                break;
            }
        }
    }
}

async fn build_read_buffer(
    mut file: File,
    mut offset: u64,
) -> impl Stream<Item = io::Result<Vec<u8>>> {
    stream! {
        let mut buffer = vec![0u8; 16384];

        loop {
            if destination_is_full() {
                yield_now().await;
                continue;
            }

            // [TODO]: Reader should use a custom AsyncRead that respects future
            //         resolution, because .read().await() cannot be cleanly
            //         interrupted. The task running it is suspended until the OS
            //         produces bytes. If no bytes arrive, .read().await() future
            //         does not wake up, notice shutdown, return or stop causing
            //         the task to hang forever.
        }
    }
}
