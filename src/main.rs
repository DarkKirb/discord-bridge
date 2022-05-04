//! Discord-Matrix bridge

/// Main program entrypoint
#[tokio::main]
fn main() -> Result<()> {
    dotenv::dotenv().ok();
    color_eyre::install()?;
    tracing_subscriber::fmt::init();

    Ok(())
}
