//! Discord-Matrix bridge

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;

pub mod config;
pub use config::File as ConfigFile;

/// Application service to connect discord to matrix
#[derive(Clone, Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[clap(short, long)]
    config: PathBuf,
    /// Path to registration file
    #[clap(short, long)]
    registration: PathBuf,
    /// Command to execute
    #[clap(subcommand)]
    subcommand: Command,
}

/// Subcommand list
#[derive(Clone, Debug, Subcommand)]
enum Command {
    /// Generate a registration file
    GenerateRegistration,
    /// Start the server
    Start,
}

/// Main program entrypoint
#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    color_eyre::install()?;
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    println!("{:?}", args);

    Ok(())
}
