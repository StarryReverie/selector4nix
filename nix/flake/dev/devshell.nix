{
  config,
  inputs,
  self,
  ...
}:
{
  perSystem =
    { system, pkgsDev, ... }:
    {
      _module.args.pkgsDev = (import inputs.nixpkgs) {
        inherit system;
        overlays = [ inputs.rust-overlay.overlays.default ];
      };

      devShells.default = pkgsDev.mkShellNoCC {
        packages = [
          (pkgsDev.rust-bin.fromRustupToolchainFile ./../../../rust-toolchain.toml)
          pkgsDev.cargo-hakari
          pkgsDev.nix-serve-ng
          pkgsDev.nixfmt
          pkgsDev.nixfmt-tree
          pkgsDev.nodejs
        ];
      };
    };
}
