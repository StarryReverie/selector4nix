{ lib }:
lib.fileset.toSource {
  root = ../.;
  fileset = lib.fileset.unions [
    ../Cargo.toml
    ../Cargo.lock
    ../docs/selector4nix.example.toml
    ../docs/credentials.example.toml
    ../components
    ../src
    ../tests
  ];
}
