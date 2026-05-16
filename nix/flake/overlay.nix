{
  self,
  ...
}:
{
  flake.overlays = {
    default = self.overlays.selector4nix;
    selector4nix = import ../overlay.nix;
  };
}
