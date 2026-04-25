{
  inputs,
  self,
  ...
}:
{
  perSystem =
    { config, pkgs, ... }:
    {
      devShells.default = pkgs.mkShellNoCC {
        packages = [
          config.packages.rust-toolchain
          pkgs.nixfmt
          pkgs.nixfmt-tree
        ];
      };
    };
}
