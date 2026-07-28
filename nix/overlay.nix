final: prev: {
  selector4nix =
    let
      finalPackageAwaredCallPackage = import ../lib/final-package-awared-call-package.nix prev.lib;
    in
    finalPackageAwaredCallPackage prev.callPackage ./package.nix "selector4nix" { };
}
