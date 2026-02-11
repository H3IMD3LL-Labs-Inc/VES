// Local crates
use crate::tailer::models::TailerPayload;

// external crates
use bytes::Bytes;

#[allow(unused_doc_comments)]
pub fn build_payload(
    chunk: Bytes,
) -> TailerPayload {

    /// This is required ONLY for metrics currently
    let data_size = chunk.len();

    TailerPayload {
        raw_data: chunk,
        size: data_size,
    }
}
