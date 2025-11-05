pub mod collector {
    tonic::include_proto!("collector");
}
pub mod common {
    tonic::include_proto!("common");
}
pub mod embedder {
    tonic::include_proto!("embedder");
}

//include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/proto/collector.rs"));
//include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/proto/common.rs"));
//include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/proto/embedder.rs"))
