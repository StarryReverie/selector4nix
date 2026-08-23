system:

# `nix-serve-ng` fails to build on aarch64 platforms until
# <https://github.com/NixOS/nixpkgs/issues/539200> is resolved.
let
  pkgs = (import ../.).inputsDev.nixpkgs-nix-serve-ng.legacyPackages.${system};
in
pkgs.nix-serve-ng
