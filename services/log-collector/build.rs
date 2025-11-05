fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_protos(
            &["../../proto/collector.proto", "../../proto/embedder.proto"],
            &["../../proto"],
        )?;
    Ok(())
}
