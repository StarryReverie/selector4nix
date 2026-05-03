{
  self,
  ...
}:
{
  flake.overlays = {
    default = self.overlays.selector4nix;
    selector4nix = final: prev: {
      selector4nix = prev.callPackage ../package.nix { };
    };
  };
}
