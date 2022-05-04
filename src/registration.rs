//! Registration generation

use std::fs;

use crate::ConfigFile;
use color_eyre::Result;
use matrix_sdk::ruma::api::appservice::{Namespace, Namespaces, Registration, RegistrationInit};
use rand::{
    distributions::{Alphanumeric, DistString},
    thread_rng, CryptoRng, Rng,
};

/// Generate a random token
fn generate_token<R: Rng + CryptoRng>(r: &mut R) -> String {
    Alphanumeric.sample_string(r, 32)
}

/// Generate a registration
fn generate_registration(config: &ConfigFile) -> Registration {
    let mut namespaces = Namespaces::new();

    namespaces.users = vec![
        Namespace::new(true, "@_discord_.*".to_owned()),
        Namespace::new(true, "@_discordbot.*".to_owned()),
    ];
    namespaces.aliases = vec![Namespace::new(true, "#_discord_.*".to_owned())];

    let mut rng = thread_rng();
    RegistrationInit {
        id: "discord".to_owned(),
        url: config.bridge.bridge_url.as_str().to_owned(),
        as_token: generate_token(&mut rng),
        hs_token: generate_token(&mut rng),
        sender_localpart: "_discordbot".to_owned(),
        namespaces,
        rate_limited: Some(false),
        protocols: Some(vec!["com.discord".to_owned()]),
    }
    .into()
}

/// Command for generating the registration
///
/// # Errors
/// This function will return an error if writing the registration to the file fails
pub fn generate_registration_cmd(config: &ConfigFile, args: &crate::Args) -> Result<Registration> {
    let registration = generate_registration(config);
    let file = fs::File::create(&args.registration)?;
    serde_json::to_writer(file, &registration)?;
    Ok(registration)
}

#[cfg(test)]
mod tests {
    use std::{
        net::{IpAddr, Ipv4Addr},
        str::FromStr,
    };

    use rand::thread_rng;
    use url::Url;

    use crate::config;

    use super::*;

    #[test]
    fn generate_token_always_unequal() {
        let mut rng = thread_rng();
        assert_ne!(generate_token(&mut rng), generate_token(&mut rng));
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn generate_registration_smoketest() {
        let config = ConfigFile {
            homeserver: config::Homeserver {
                address: Url::from_str("https://matrix.chir.rs/").expect("valid URL"),
                domain: "chir.rs".to_owned(),
            },
            bridge: config::Bridge {
                listen_address: vec![IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))],
                port: 58913,
                bridge_url: Url::from_str("http://localhost:58913/").expect("valid URL"),
            },
        };
        drop(generate_registration(&config));
    }
}
