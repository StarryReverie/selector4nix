{
  config,
  inputs,
  self,
  ...
}:
{
  imports = [
    ./devshell.nix
  ];

  perSystem =
    { system, ... }:
    {
      _module.args.pkgsDev = import inputs.nixpkgs {
        inherit system;
        overlays = [
          inputs.rust-overlay.overlays.default
        ];
      };
    };

  flake.inputsDev = inputs;
}
