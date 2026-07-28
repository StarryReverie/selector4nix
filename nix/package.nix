{
  callPackage,
  craneLib,
  lib,
  selector4nix,
}:

let
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../crates
      ../docs/selector4nix.example.toml
      ../docs/credentials.example.toml
      ../tests
    ];
  };

  commonArgsWithoutArtificats = {
    inherit src;
    inherit (craneLib.crateNameFromCargoToml { inherit src; }) pname version;

    strictDeps = true;
    __structuredAttrs = true;
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgsWithoutArtificats;

  commonArgs = commonArgsWithoutArtificats // {
    inherit cargoArtifacts;
  };
in
craneLib.buildPackage (
  commonArgs
  // {
    cargoTestExtraArgs = "--workspace --exclude 'selector4nix-system-test-*'";

    passthru.tests = {
      system-test-cache-persistence = callPackage ../tests/cache-persistence/package.nix {
        inherit commonArgs craneLib selector4nix;
      };

      system-test-nar-info-querying = callPackage ../tests/nar-info-querying/package.nix {
        inherit commonArgs craneLib selector4nix;
      };
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
)
