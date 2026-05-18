{
  withSystem,
}:

{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.selector4nix;
  common = import ./module-common.nix { inherit withSystem; } { inherit lib pkgs; };
  configFile = common.mkConfigFile cfg;
in
{
  options.services.selector4nix = common.serviceOptions;

  config = lib.mkMerge [
    (lib.mkIf cfg.enable {
      launchd.daemons.selector4nix = {
        command = "${cfg.package}/bin/selector4nix";
        environment = {
          SELECTOR4NIX_CONFIG_FILE = "${configFile}";
          RUST_LOG = "selector4nix=${cfg.logLevel}";
        };
        serviceConfig = {
          KeepAlive = true;
          RunAtLoad = true;
          ProcessType = "Background";
        };
      };
    })

    (common.mkSubstituterConfig cfg)
  ];
}
