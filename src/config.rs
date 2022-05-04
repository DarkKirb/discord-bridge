//! Config file module

use serde::{Deserialize, Serialize};
use url::Url;

/// Configuration file
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct File {
    /// Homeserver configuration
    pub homeserver: Homeserver,
}

/// Homeserver configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Homeserver {
    /// URL to homeserver, for example `https://matrix.chir.rs/`
    pub address: Url,
    /// Domain name of homeserver, for example `chir.rs`
    pub domain: String,
}
