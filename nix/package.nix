{
  callPackage,
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "selector4nix";
  version = "0.4.2";

  src = import ./source.nix { inherit lib; };

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  passthru.tests = {
    system-test-nar-info-querying = callPackage ../tests/system/nar-info-querying/package.nix {
      inherit rustPlatform;
      selector4nix = finalAttrs.finalPackage;
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
})
