//! Discord-Matrix bridge

use std::{borrow::Cow, path::PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};

pub mod config;
pub use config::File as ConfigFile;
use matrix_sdk_appservice::{AppService, AppServiceRegistration};
use once_cell::sync::Lazy;
use sentry::{ClientInitGuard, IntoDsn};
use tracing::debug;
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

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

/// Create a tracing subscriber
fn setup_tracing() -> Result<()> {
    tracing_subscriber::Registry::default()
        .with(tracing_subscriber::fmt::layer().with_filter(EnvFilter::from_default_env()))
        .with(sentry::integrations::tracing::layer())
        .try_init()?;
    Ok(())
}

/// Sets up sentry
fn setup_sentry() -> Result<ClientInitGuard> {
    /// The release name
    static RELEASE_NAME: Lazy<Option<String>> = Lazy::new(|| {
        option_env!("CARGO_PKG_NAME").and_then(|name| {
            option_env!("VERGEN_GIT_SHA").map(|version| format!("{}@{}", name, version))
        })
    });

    setup_tracing()?;
    let client_options = sentry::ClientOptions {
        dsn: std::env::var("SENTRY_DSN").ok().into_dsn()?,
        release: RELEASE_NAME.as_ref().map(|s| Cow::Borrowed(s.as_str())),
        attach_stacktrace: true,
        default_integrations: true,
        ..Default::default()
    };
    Ok(sentry::init(client_options))
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
        eprintln!("{:?}", e);
    }
    Ok(())
}
