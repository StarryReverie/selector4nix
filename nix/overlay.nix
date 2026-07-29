final: prev: {
  selector4nix =
    let
      craneLib = prev.craneLib or ((import ../.).inputs.crane.mkLib prev);
      finalPackageAwaredCallPackage = import ./lib/final-package-awared-call-package.nix prev.lib;
    in
    finalPackageAwaredCallPackage prev.callPackage ./package.nix "selector4nix" {
      inherit craneLib;
    };
}
