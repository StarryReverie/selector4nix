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
      packages =
        let
          finalPackageAwaredCallPackage = import ../lib/final-package-awared-call-package.nix pkgs.lib;
        in
        {
          default = config.packages.selector4nix;

          selector4nix = finalPackageAwaredCallPackage pkgs.callPackage ../package.nix "selector4nix" {
            craneLib = inputs.crane.mkLib pkgs;
          };

          selector4nixStatic =
            finalPackageAwaredCallPackage pkgs.pkgsStatic.callPackage ../package.nix "selector4nix"
              {
                craneLib = inputs.crane.mkLib pkgs.pkgsStatic;
              };
        };

      legacyPackages = config.packages;
    };
}
