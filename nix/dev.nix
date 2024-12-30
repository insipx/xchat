{ system
, fenix
, rust-analyzer
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
  fenixPkgs = fenix.packages."${system}";
  rust-toolchain = with fenixPkgs;
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
    rust-analyzer
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
