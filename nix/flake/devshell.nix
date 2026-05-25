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
          (pkgs.rust-bin.fromRustupToolchainFile ./../../rust-toolchain.toml)
          pkgs.nix-serve-ng
          pkgs.nixfmt
          pkgs.nixfmt-tree
        ];
      };
    };
}
