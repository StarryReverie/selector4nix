{
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
  pname = "selector4nix";
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../components
      ../src
    ];
  };

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  meta = {
    description = "Nix substituter proxy with parallel cache queries and latency-aware selection";
    homepage = "https://github.com/starryreverie/selector4nix";
    mainProgram = "selector4nix";
    license = lib.licenses.gpl3Plus;
    maintainers = with lib.maintainers; [ starryreverie ];
    platforms = lib.platforms.unix;
  };
}
