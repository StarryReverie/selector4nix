{
  config,
  inputs,
  self,
  flake-parts-lib,
  withSystem,
  ...
}:
{
  flake = {
    nixosModules = {
      default = config.flake.nixosModules.selector4nix;
      selector4nix = flake-parts-lib.importApply ../nixos-module.nix {
        inherit withSystem;
      };
    };
  };
}
