{
  buildNpmPackage,
  callPackage,
  craneLib,
  importNpmLock,
  lib,
  selector4nix,
}:

let
  frontendDist = import ./frontend.nix {
    inherit buildNpmPackage importNpmLock lib;
  };

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../crates
      ../docs/selector4nix.example.toml
      ../docs/credentials.example.toml
      ../tests
      ../frontend/templates
    ];
  };

  commonArgsWithoutArtificats = {
    inherit src;

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
    postPatch = ''
      ln -s ${frontendDist}/lib/node_modules/selector4nix-frontend/frontend/dist frontend/dist
    '';

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
      changelog = "https://github.com/starryreverie/selector4nix/blob/v${selector4nix.version}/CHANGELOG.md";
      mainProgram = "selector4nix";
      license = lib.licenses.gpl3Plus;
      maintainers = with lib.maintainers; [ starryreverie ];
      platforms = lib.platforms.unix;
    };
  }
)
