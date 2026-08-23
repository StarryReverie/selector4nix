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
          (pkgsDev.rust-bin.stable.${pkgsDev.rustc.version}.default.override {
            extensions = [
              "rust-analyzer"
              "rust-src"
            ];
          })
          pkgsDev.cargo-hakari
          pkgsDev.nodejs

          pkgsDev.nixfmt
          pkgsDev.treefmt

          pkgsDev.nix-serve-ng
        ];
      };
    };
}
