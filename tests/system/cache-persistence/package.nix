{
  lib,
  rustPlatform,
  makeWrapper,
  selector4nix,
  nix,
  nix-serve-ng,
}:

rustPlatform.buildRustPackage {
  pname = "selector4nix-system-test-cache-persistence";
  version = "0.0.0";

  src = import ../../../nix/source.nix { inherit lib; };

  cargoLock = {
    lockFile = ../../../Cargo.lock;
  };

  buildAndTestSubdir = "tests/system/cache-persistence";

  nativeBuildInputs = [ makeWrapper ];

  postInstall = ''
    wrapProgram $out/bin/selector4nix-system-test-cache-persistence \
      --set SELECTOR4NIX_BIN "${lib.getExe selector4nix}" \
      --set NIX_BIN "${lib.getExe nix}" \
      --set NIX_SERVE_BIN "${lib.getExe nix-serve-ng}"
  '';

  meta = {
    mainProgram = "selector4nix-system-test-cache-persistence";
    platforms = lib.platforms.unix;
  };
}
