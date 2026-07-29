{
  commonArgs,
  craneLib,
  lib,
  makeWrapper,
  nix,
  nix-serve-ng,
  selector4nix,
}:

let
  pname = "selector4nix-system-test-nar-info-querying";
in
craneLib.buildPackage (
  commonArgs
  // {
    inherit pname;

    cargoExtraArgs = "-p ${pname}";

    nativeBuildInputs = [ makeWrapper ];

    postInstall = ''
      wrapProgram $out/bin/selector4nix-system-test-nar-info-querying \
        --set SELECTOR4NIX_BIN "${lib.getExe selector4nix}" \
        --set NIX_BIN "${lib.getExe nix}" \
        --set NIX_SERVE_BIN "${lib.getExe nix-serve-ng}"
    '';

    meta = {
      mainProgram = pname;
      platforms = lib.platforms.unix;
    };
  }
)
