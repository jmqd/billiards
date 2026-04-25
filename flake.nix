{
  description = "Rust dev environment";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];

        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain =
          pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        python = pkgs.python313;

        billiards = rustPlatform.buildRustPackage {
          pname = "billiards";
          version = "0.1.0";
          src = ./.;
          cargoLock = { lockFile = ./Cargo.lock; };
          nativeBuildInputs = [ ];
          buildInputs = [ ];
        };
      in {
        packages.default = billiards;
        apps.default = flake-utils.lib.mkApp { drv = billiards; };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustToolchain
            openssl
            llvmPackages.bolt
            python
            uv

            # Cargo checks / lints / tools
            cargo-audit
            cargo-deny
            cargo-edit
            cargo-license
            cargo-pgo
            cargo-udeps
            cargo-watch
            just
            poppler-utils
          ];

          shellHook = ''
            # Keep Python dependencies owned by gymnasium/pyproject.toml + gymnasium/uv.lock.
            export UV_PYTHON=${python}/bin/python
            export UV_PYTHON_DOWNLOADS=never
            export UV_PROJECT_ENVIRONMENT=$PWD/gymnasium/.venv
            export UV_LINK_MODE=copy

            # Prefer the uv-managed project venv once synced; otherwise prefer Nix Python/tools
            # over user pyenv/asdf shims.
            export PATH=$PWD/gymnasium/.venv/bin:${python}/bin:${pkgs.uv}/bin:$PATH
            hash -r 2>/dev/null || true

            # Tells rust-analyzer where the stdlib sources are
            export RUST_SRC_PATH=${rustToolchain}/lib/rustlib/src/rust/library
          '';
        };
      });
}
