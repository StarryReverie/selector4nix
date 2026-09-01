let
  lockFile = builtins.fromJSON (builtins.readFile ./flake/dev/flake.lock);
  flakeCompatNode = lockFile.nodes.${lockFile.nodes.root.inputs.flake-compat}.locked;
  flakeCompatSrc = builtins.fetchTarball {
    url =
      flakeCompatNode.url or (
        if flakeCompatNode.type == "github" then
          let
            inherit (flakeCompatNode) owner repo rev;
          in
          "https://github.com/${owner}/${repo}/archive/${rev}.tar.gz"
        else
          builtins.abort "unsupported input type for flake-compat, unable to fetch the flake-compat repository"
      );
    sha256 = flakeCompatNode.narHash;
  };

  flakeCompat = import flakeCompatSrc;
  flakeCompatArgSet = builtins.functionArgs flakeCompat;

  optionalAttrs = cond: attrs: if cond then attrs else { };
  optionalAttrsIfHasArg = arg: optionalAttrs (builtins.hasAttr arg flakeCompatArgSet);
  optionalArgValue = arg: value: optionalAttrsIfHasArg arg { ${arg} = value; };

  flake = flakeCompat (
    {
      src = ../.;
    }
    // (optionalArgValue "copySourceTreeToStore" false)
    // (optionalArgValue "useBuiltinsFetchTree" true)
  );
in
flake
