{
  config,
  inputs,
  self,
  ...
}:
{
  perSystem =
    {
      config,
      system,
      pkgs,
      ...
    }:
    {
      _module.args.pkgs = (import inputs.nixpkgs) {
        inherit system;
        overlays = [ inputs.rust-overlay.overlays.default ];
      };

      packages = {
        default = config.packages.selector4nix;
        selector4nix = pkgs.callPackage ../package.nix { };
        selector4nix-static = pkgs.pkgsStatic.callPackage ../package.nix { };
      };

      legacyPackages = config.packages;
    };
}
