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
  common = import ./common.nix { inherit withSystem; } { inherit lib pkgs; };
  configFile = common.mkConfigFile cfg;
in
{
  options.services.selector4nix = common.serviceOptions;

  config = lib.mkMerge [
    (lib.mkIf cfg.enable {
      launchd.daemons.selector4nix = {
        script =
          (lib.optionalString cfg.enablePersistentCaching ''
            mkdir -p /Library/Caches/selector4nix
            export SELECTOR4NIX_CACHE_DIR=/Library/Caches/selector4nix
          '')
          + ''
            exec ${cfg.package}/bin/selector4nix --no-log-timestamp
          '';

        environment = {
          SELECTOR4NIX_CONFIG_FILE = "${configFile}";
          RUST_LOG = "selector4nix=${cfg.logLevel}";
        }
        // lib.optionalAttrs (cfg.credentialFile != null) {
          SELECTOR4NIX_CREDENTIAL_FILE = "${cfg.credentialFile}";
        };

        serviceConfig = {
          Label = "cc.starryreverie.selector4nix";
          KeepAlive = true;
          RunAtLoad = true;
          ProcessType = "Background";
        };
      };
    })

    (common.mkSubstituterConfig cfg)
  ];
}
