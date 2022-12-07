//! Discord-Matrix bridge

use std::path::PathBuf;

use anyhow::Result;
use app::App;
use clap::{Parser, Subcommand};

pub mod config;
pub use config::File as ConfigFile;

use sentry::{ClientInitGuard, IntoDsn};
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

pub mod app;
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

/// Sets up sentry
fn setup_sentry() -> Result<ClientInitGuard> {
    tracing_subscriber::Registry::default()
        .with(tracing_subscriber::fmt::layer().with_filter(EnvFilter::from_default_env()))
        .with(sentry::integrations::tracing::layer())
        .try_init()?;

    let client_options = sentry::ClientOptions {
        dsn: std::env::var("SENTRY_DSN").ok().into_dsn()?,
        release: sentry::release_name!(),
        attach_stacktrace: true,
        default_integrations: true,
        ..Default::default()
    };
    Ok(sentry::init(client_options))
}

/// Runs the actual server
///
/// # Errors
/// This function will return an error if running the server fails
async fn run_app(config: &ConfigFile, args: &Args) -> Result<()> {
    App::new(config, args).await?.run().await?;
    Ok(())
}

/// Main program entrypoint
#[tokio::main]
async fn main() -> Result<()> {
    /// The actual main function
    async fn main() -> Result<()> {
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

    dotenv::dotenv().ok();
    let _guard = setup_sentry()?;

    if let Err(e) = main().await {
        sentry::integrations::anyhow::capture_anyhow(&e);
        eprintln!("{e:?}");
    }
    Ok(())
}
