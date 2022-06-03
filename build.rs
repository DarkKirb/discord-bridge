use std::env;

fn main() -> anyhow::Result<()> {
    let mut config = vergen::Config::default();
    if env::var("NIX_CC").is_ok() {
        // Don’t use git information when building with nix-build
        *config.git_mut().enabled_mut() = false;
    }
    vergen::vergen(vergen::Config::default())
}
