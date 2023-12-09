# in flake.nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs = { nixpkgs.follows = "nixpkgs"; };
    };
    environments.url = "github:insipx/environments";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nixpkgs, flake-utils, fenix, environments }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        isDarwin = pkgs.stdenv.isDarwin;
        frameworks = pkgs.darwin.apple_sdk.frameworks;
        fenixPkgs = fenix.packages.${system};
        linters = import "${environments}/linters.nix" { inherit pkgs; };
        rust-toolchain = with fenixPkgs;
          combine [
            minimal.rustc
            minimal.cargo
            complete.clippy
            complete.rustfmt
            targets.wasm32-unknown-unknown.latest.rust-std
          ];
        nativeBuildInputs = with pkgs; [ pkg-config ];
        buildInputs = with pkgs;
          [
            # (fenixPkgs.fromToolchainFile { file = ./rust-toolchain.toml; })
            rust-toolchain
            rust-analyzer
            llvmPackages_16.libcxxClang
            mktemp
            markdownlint-cli
            shellcheck
            buf
            curl
            wasm-pack
            twiggy
            wasm-bindgen-cli
            binaryen
            linters
            tokio-console
          ] ++ lib.optionals isDarwin [
            libiconv
            frameworks.CoreServices
            frameworks.Carbon
            frameworks.ApplicationServices
            frameworks.AppKit
          ];
      in with pkgs; {
        devShells.default = mkShell { inherit buildInputs nativeBuildInputs; };
      });
}
