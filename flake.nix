{
  description = "Nix Flake of selector4nix";

  inputs = {
    flake-parts = {
      url = "github:hercules-ci/flake-parts/main";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    nixpkgs = {
      url = "github:NixOS/nixpkgs/nixos-unstable-small";
    };
  };

  outputs =
    { flake-parts, ... }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      imports = [
        inputs.flake-parts.flakeModules.partitions

        ./nix/flake/inputs.nix
        ./nix/flake/module.nix
        ./nix/flake/overlay.nix
        ./nix/flake/package.nix
      ];

      partitions = {
        dev = {
          module = ./nix/flake/dev;
          extraInputsFlake = ./nix/flake/dev;
        };
      };

      partitionedAttrs = {
        checks = "dev";
        devShells = "dev";
        formatter = "dev";
      };
    };
}
