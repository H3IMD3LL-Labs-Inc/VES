// Local crates
use crate::tailer::async_read::CustomAsyncReadExt;

// External crates
use bytes::Bytes;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

const READ_BUFFER_SIZE: usize = 16384;

/// Read data from a Tailer's source as Bytes, and return a buffer of the
/// read bytes. This data is used to consurct the TailerPayload.
///
/// See [TailerPayload builder](components/core-agent/src/tailer/payload.rs).
pub async fn read_data(
    path: PathBuf,
    stop: impl std::future::Future<Output = ()>,
) -> std::io::Result<Vec<Bytes>> {
    let file = File::open(&path).await?;

    tokio::pin!(stop);

    let mut reader = file.read_until_future(stop);

    let mut chunks: Vec<Bytes> = Vec::new();

    loop {
        let mut buffer = vec![0u8; READ_BUFFER_SIZE];

        let n = reader.read(&mut buffer).await?;

        if n == 0 {
            // [TODO]: Handle, EOF or stop condition
            break;
        }

        buffer.truncate(n);

        chunks.push(Bytes::from(buffer));
    }

    Ok(chunks)
}
