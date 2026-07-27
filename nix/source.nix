{ lib }:
lib.fileset.toSource {
  root = ../.;
  fileset = lib.fileset.unions [
    ../Cargo.toml
    ../Cargo.lock
    ../crates
    ../docs/selector4nix.example.toml
    ../docs/credentials.example.toml
    ../src
    ../tests
  ];
}
