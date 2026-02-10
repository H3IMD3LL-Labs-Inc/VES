// Local crates
use crate::tailer::models::TailerPayload;

// external crates
use bytes::Bytes;

#[allow(unused_doc_comments)]
pub fn build_payload(
    buffer: Vec<Bytes>,
) -> TailerPayload {

    /// This is required ONLY for metrics currently
    let data_size = buffer.iter().map(|b| b.len()).sum();

    TailerPayload {
        raw_data: buffer,
        size: data_size,
    }
}
