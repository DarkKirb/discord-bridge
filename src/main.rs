//! Discord-Matrix bridge

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;

pub mod config;
pub use config::File as ConfigFile;

pub mod registration;

/// Application service to connect discord to matrix
#[derive(Clone, Debug, Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Path to configuration file
    #[clap(short, long)]
    pub config: PathBuf,
    /// Path to registration file
    #[clap(short, long)]
    pub registration: PathBuf,
    /// Command to execute
    #[clap(subcommand)]
    pub subcommand: Command,
}

/// Subcommand list
#[derive(Copy, Clone, Debug, Subcommand)]
pub enum Command {
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
    let config = ConfigFile::read_from_file(&args.config)?;

    match args.subcommand {
        Command::GenerateRegistration => {
            registration::generate_registration_cmd(&config, &args)?;
        }
        Command::Start => todo!(),
    }

    Ok(())
}
