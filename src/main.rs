//! Discord-Matrix bridge

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;

pub mod config;
pub use config::File as ConfigFile;
use matrix_sdk_appservice::{AppService, AppServiceRegistration};
use tracing::debug;

pub mod psql_store;
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

/// Runs the actual server
///
/// # Errors
/// This function will return an error if reading registration information fails
async fn run_app(config: &ConfigFile, args: &Args) -> Result<()> {
    debug!("Reading registration data");
    let registration = AppServiceRegistration::try_from_yaml_file(&args.registration)?;
    debug!("Creating appservice instance");
    let _appservice = AppService::new(
        config.homeserver.address.as_str(),
        config.homeserver.domain.clone(),
        registration,
    )
    .await?;
    Ok(())
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
        Command::Start => {
            run_app(&config, &args).await?;
        }
    }

    Ok(())
}
