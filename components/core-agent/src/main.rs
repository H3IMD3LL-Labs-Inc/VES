mod buffer_batcher;
mod cli;
mod helpers;
mod instrumentation;
mod metrics;
mod parser;
mod proto;
mod runtime;
mod server;
mod shipper;
mod tailer;
mod watcher;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    instrumentation::tracing::init_tracing();
    instrumentation::tracing::init_panic_handler();

    // Main entrypoint simply delegates control to CLI layer.
    // The CLI parses user commands and then calls into the appropriate logic
    cli::cli::run().await
}
