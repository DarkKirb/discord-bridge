//! Config file module

use std::{fs, net::IpAddr, path::Path};

use color_eyre::Result;
use serde::{Deserialize, Serialize};
use url::Url;

/// Configuration file
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct File {
    /// Homeserver configuration
    pub homeserver: Homeserver,
    /// Bridge configuration
    pub bridge: Bridge,
}

impl File {
    /// Read the configuration file from disk
    ///
    /// # Errors
    /// This function returns an error if accessing the disk fails or the file is invalid
    pub fn read_from_file(f: impl AsRef<Path>) -> Result<Self> {
        let file = fs::File::open(f)?;
        Ok(serde_json::from_reader(file)?)
    }
}

/// Homeserver configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Homeserver {
    /// URL to homeserver, for example `https://matrix.chir.rs/`
    pub address: Url,
    /// Domain name of homeserver, for example `chir.rs`
    pub domain: String,
}

/// Bridge Configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Bridge {
    /// Addresses to listen on
    pub listen_address: Vec<IpAddr>,
    /// Port to listen on
    pub port: u16,
    /// Bridge URL
    pub bridge_url: Url,
}
