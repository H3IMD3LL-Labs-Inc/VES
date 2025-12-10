fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_protos(
            &["../../proto_files/collector.proto", "../../proto_files/embedder.proto"],
            &["../../proto_files"],
        )?;
    Ok(())
}
