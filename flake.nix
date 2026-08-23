{
  description = "Nix Flake of selector4nix";

  inputs = {
    crane = {
      url = "github:ipetkov/crane/master";
    };

    flake-parts = {
      url = "github:hercules-ci/flake-parts/main";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    nixpkgs = {
      url = "github:NixOS/nixpkgs/nixos-unstable";
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
        ./nix/flake
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
        inputsDev = "dev";
      };
    };
}
