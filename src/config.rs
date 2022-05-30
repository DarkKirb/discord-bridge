//! Config file module

use std::{
    collections::BTreeMap,
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
};

use anyhow::Result;
use educe::Educe;
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
        Ok(serde_yaml::from_reader(file)?)
    }
}

/// Homeserver configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Homeserver {
    /// URL to homeserver, for example `https://matrix.chir.rs/`
    pub address: Url,
    /// Domain name of homeserver, for example `chir.rs`
    pub domain: String,
    /// Supported MSCs
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mscs: Vec<u16>,
}

/// Database options for postgresql
#[derive(Clone, Educe, Deserialize, Serialize, Default)]
#[educe(Debug)]
pub struct DBOptions {
    /// Hostname of the database
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Port of the database
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Socket path of the database
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket: Option<PathBuf>,
    /// Username of the database
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Password of the database
    #[serde(skip_serializing_if = "Option::is_none")]
    #[educe(Debug(ignore))]
    pub password: Option<String>,
    /// Database name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    /// The ssl mode to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sslmode: Option<String>,
    /// The path to the CA certificate the ssl is checked against
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sslrootcert: Option<PathBuf>,
    /// Capacity of the statement cache
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_cache_capacity: Option<usize>,
    /// Application name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_name: Option<String>,
    /// Extra float digits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_float_digits: Option<i8>,
    /// Additional options
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
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
    /// Bridge username prefix
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    /// Database options
    pub db: DBOptions,
}
