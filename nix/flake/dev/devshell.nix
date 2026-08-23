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

          (import ../../pinned-nix-serve-ng.nix system)
        ];
      };
    };
}
