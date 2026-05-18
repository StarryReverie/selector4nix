{
  config,
  flake-parts-lib,
  withSystem,
  ...
}:
{
  flake = {
    darwinModules = {
      default = config.flake.darwinModules.selector4nix;
      selector4nix = flake-parts-lib.importApply ../darwin-module.nix {
        inherit withSystem;
      };
    };

    homeManagerModules = {
      default = config.flake.homeManagerModules.selector4nix;
      selector4nix = flake-parts-lib.importApply ../home-manager-module.nix {
        inherit withSystem;
      };
    };

    nixosModules = {
      default = config.flake.nixosModules.selector4nix;
      selector4nix = flake-parts-lib.importApply ../nixos-module.nix {
        inherit withSystem;
      };
    };
  };
}
