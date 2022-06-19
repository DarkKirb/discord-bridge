{
  description = "discord-matrix-bridge";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };

    cargo2nix = {
      url = "github:cargo2nix/cargo2nix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
      inputs.rust-overlay.follows = "rust-overlay";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    cargo2nix,
    ...
  } @ inputs:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [cargo2nix.overlays.default (import rust-overlay)];
      pkgs = import nixpkgs {inherit system overlays;};
      rustPkgs = pkgs.rustBuilder.makePackageSet {
        packageFun = import ./Cargo.nix;
        rustChannel = "1.60.0";
        packageOverrides = pkgs: pkgs.rustBuilder.overrides.all;
      };
    in rec {
      devShells.default = with pkgs;
        mkShell {
          buildInputs = [
            (rust-bin.nightly.latest.default.override {
              extensions = ["rust-src"];
            })
            cargo2nix.packages.${system}.cargo2nix
            cargo-audit
            sqlx-cli
            github-cli
            mold
            clang
            statix
          ];
        };
      packages = rec {
        discord-matrix-bridge = rustPkgs.workspace.discord-matrix-bridge {};
        default = discord-matrix-bridge;
      };
      nixosModules.default = import ./nixos {inherit inputs system;};
      hydraJobs =
        if pkgs.lib.strings.hasSuffix "-linux" system
        then packages
        else {};
      formatter = pkgs.alejandra;
    });
}
