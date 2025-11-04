mod buffer_batcher;
mod cli;
mod control_observability;
mod filter_redactor;
mod helpers;
mod metadata_enricher;
mod metrics;
mod models;
mod parser;
mod proto;
mod runtime;
mod server;
mod shipper;
mod tailer;
mod watcher;

use anyhow::Result;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Main entrypoint simply delegates control to CLI layer.
    // The CLI parses user commands and then calls into the appropriate logic
    cli::cli::run().await
}
