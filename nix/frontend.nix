{
  buildNpmPackage,
  importNpmLock,
  lib,
}:

buildNpmPackage (finalAttrs: {
  pname = "selector4nix-frontend";
  version = "0.0.0";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../package.json
      ../package-lock.json
      ../vite.config.js
      ../frontend
    ];
  };

  npmDeps = importNpmLock {
    npmRoot = finalAttrs.src;
  };

  npmConfigHook = importNpmLock.npmConfigHook;

  installPhase = ''
    mkdir -p $out/lib/node_modules/selector4nix-frontend/frontend/
    cp -r frontend/dist $out/lib/node_modules/selector4nix-frontend/frontend/
  '';
})
