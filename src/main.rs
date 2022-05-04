//! Discord-Matrix bridge

use color_eyre::eyre::Result;

/// Main program entrypoint
#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    color_eyre::install()?;
    tracing_subscriber::fmt::init();

    Ok(())
}
