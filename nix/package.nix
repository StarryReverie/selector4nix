{
  callPackage,
  lib,
  rustPlatform,
  selector4nix,
}:

rustPlatform.buildRustPackage {
  pname = "selector4nix";
  version = "0.8.0";

  src = import ./source.nix { inherit lib; };

  __structuredAttrs = true;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  cargoTestFlags = [ "--workspace" ];

  passthru.tests = {
    system-test-cache-persistence = callPackage ../tests/cache-persistence/package.nix {
      inherit rustPlatform selector4nix;
    };

    system-test-nar-info-querying = callPackage ../tests/nar-info-querying/package.nix {
      inherit rustPlatform selector4nix;
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
