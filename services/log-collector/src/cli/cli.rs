use crate::runtime;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "ves_log_collector",
    version,
    about = "High Performance Log Collector Daemon"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run log collector in normal mode(Daemon)
    Run {
        #[arg(short, long, default_value = "/etc/log_collector.toml")]
        config: PathBuf,
    },

    /// TODO: Show log collector metrics reading from /metrics endpoint
    /*Status {
        #[arg(short, long, default_value = "127.0.0.1:9000")]
        endpoint: String,
    },*/

    /// TODO: Validate the configuration file before running
    Validate {
        #[arg(short, long, default_value = "/etc/log_collecor.toml")]
        config: PathBuf,
    },

    /// TODO: Display version information
    Version,
    // TODO: Manage log collector settings, with subcommands, i.e, get, set, list
    //Config,

    // TODO: Test tailing a single file interactively
    //Test,
}

/// Entry function for CLI
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { config } => runtime::runtime::run_log_collector(config).await?,
        Commands::Validate { config } => validate_config(config).await?,
        //Commands::Status { endpoint } => show_status(endpoint).await?,
        Commands::Version => show_version(),
    }

    Ok(())
}

//
// ------------------------ Command Implementations ------------------------------
//

/// Validate configuration file
async fn validate_config(config: PathBuf) -> Result<()> {
    println!("Validating configuration file: {:?}", config);
    let cfg = crate::helpers::load_config::Config::load(&config);
    println!("Configuration valid:\n{:#?}", cfg);
    Ok(())
}

/// TODO: Fetch metrics and status info
//async fn show_status(endpoint: String) -> Result<()> {}

/// Show version information
fn show_version() {
    println!("VES Log Collector {}", env!("CARGO_PKG_VERSION"));
}
