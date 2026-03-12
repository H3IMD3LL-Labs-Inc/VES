// Local crates
use crate::tailer::async_read::CustomAsyncReadExt;
use crate::tailer::models::TailerReader;

// External crates
use bytes::Bytes;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

const READ_BUFFER_SIZE: usize = 16384;

/// Read data from a Tailer's source as Bytes, and return a buffer of the
/// read bytes. This data is used to consurct the TailerPayload.
///
/// See [TailerPayload builder](components/core-agent/src/tailer/payload.rs).
impl<F> TailerReader<F>
where
    F: std::future::Future<Output = ()> + Unpin,
{
    pub fn new(
        file: File,
        stop: F,
    ) -> Self {
        Self {
            reader: file.read_until_future(stop),
            buffer: vec![0u8; READ_BUFFER_SIZE],
        }
    }

    pub async fn read_data_chunk(
        &mut self,
    ) -> std::io::Result<Option<Bytes>> {
        let n = self.reader.read(&mut self.buffer).await?;

        if n == 0 {
            return Ok(None);
        }

        let chunk = Bytes::copy_from_slice(&self.buffer[..n]);

        Ok(Some(chunk))
    }
}
