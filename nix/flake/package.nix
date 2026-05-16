{
  config,
  inputs,
  self,
  ...
}:
{
  perSystem =
    {
      config,
      system,
      pkgs,
      ...
    }:
    {
      _module.args.pkgs = (import inputs.nixpkgs) {
        inherit system;
        overlays = [ inputs.rust-overlay.overlays.default ];
      };

      packages = {
        default = config.packages.selector4nix;

        selector4nix = pkgs.callPackage ../package.nix {
          rustPlatform = pkgs.makeRustPlatform {
            cargo = config.packages.rust-toolchain;
            rustc = config.packages.rust-toolchain;
          };
        };

        # Build statically linked artifact with `rust-overlay` is broken.
        selector4nix-static = pkgs.pkgsStatic.callPackage ../package.nix { };

        rust-toolchain = pkgs.rust-bin.fromRustupToolchainFile ./../../rust-toolchain.toml;
      };

      legacyPackages = config.packages;
    };
}
