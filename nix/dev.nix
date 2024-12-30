{ system
, fenix
, mkShell
, darwin
, mktemp
, shellcheck
, curl
, tokio-console
, cargo-nextest
, inferno
, lib
, stdenv
, pkg-config
}:
let
  inherit (darwin.apple_sdk) frameworks;
  rust-toolchain = with fenix.pkgs."${system}";
    combine [
      minimal.rustc
      minimal.cargo
      complete.clippy
      complete.rustfmt
    ];
in
mkShell {
  nativeBuildInputs = [ pkg-config ];
  buildInputs = [
    # (fenixPkgs.fromToolchainFile { file = ./rust-toolchain.toml; })
    fenix.pkgs."${system}".rust-analyzer
    rust-toolchain
    mktemp
    shellcheck
    curl
    tokio-console
    cargo-nextest
    inferno
  ] ++ lib.optionals stdenv.isDarwin [
    frameworks.CoreServices
    frameworks.Carbon
    frameworks.ApplicationServices
    frameworks.AppKit
  ];
}
