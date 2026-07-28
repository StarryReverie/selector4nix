lib:

let
  makePackageOverridable =
    finalPackageName: package:
    package
    // {
      override =
        args:
        let
          overriddenPackage = ((package.override args).override { ${finalPackageName} = newPackage; });
          newPackage = makePackageOverridable finalPackageName overriddenPackage;
        in
        newPackage;

      overrideAttrs =
        args:
        let
          overriddenPackage = ((package.overrideAttrs args).override { ${finalPackageName} = newPackage; });
          newPackage = makePackageOverridable finalPackageName overriddenPackage;
        in
        newPackage;
    };

  finalPackageAwaredCallPackage =
    callPackage: func: finalPackageName: attrs:
    let
      called = callPackage func (attrs // { ${finalPackageName} = finalPackage; });
      finalPackage = makePackageOverridable finalPackageName called;
    in
    finalPackage;
in
finalPackageAwaredCallPackage
