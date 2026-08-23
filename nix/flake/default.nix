{
  config,
  inputs,
  self,
  ...
}:
{
  imports = [
    ./module.nix
    ./overlay.nix
    ./package.nix
  ];
}
