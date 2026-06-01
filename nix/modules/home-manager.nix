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
  options = {
    services.selector4nix = common.serviceOptions;
  };

  config = lib.mkMerge [
    {
      assertions = [
        {
          assertion = (cfg.enable && cfg.configureSubstituter != "keep") -> config.nix.package != null;
          message = ''
            `services.selector4nix.configureSubstituter = "${cfg.configureSubstituter}"` sets `nix.settings.substituters`,
            but Home Manager requires `nix.package` to be set when generating `nix.conf` from `nix.settings`.

            Set `nix.package` (for example, `nix.package = pkgs.nix;`) or set `services.selector4nix.configureSubstituter = "keep"`.
          '';
        }
      ];
    }

    (lib.mkIf cfg.enable (
      lib.mkMerge [
        (lib.mkIf pkgs.stdenv.isLinux {
          systemd.user.services.selector4nix = {
            Unit = {
              Description = "Nix substituter proxy with parallel cache queries and latency-aware selection";
              After = [ "network-online.target" ];
              Wants = [ "network-online.target" ];
            };

            Install.WantedBy = [ "default.target" ];

            Service = {
              Type = "simple";
              ExecStart = "${cfg.package}/bin/selector4nix --no-log-timestamp";

              Environment = [
                "SELECTOR4NIX_CONFIG_FILE=${configFile}"
                "RUST_LOG=selector4nix=${cfg.logLevel}"
              ]
              ++ lib.optionals (cfg.credentialFile != null) [
                "SELECTOR4NIX_CREDENTIAL_FILE=${cfg.credentialFile}"
              ]
              ++ lib.optionals cfg.enablePersistentCaching [
                "SELECTOR4NIX_CACHE_DIR=%C/selector4nix"
              ];

              CacheDirectory = lib.optionals cfg.enablePersistentCaching [
                "selector4nix"
              ];

              Restart = "on-failure";
              RestartSec = 5;
            };
          };
        })

        (lib.mkIf pkgs.stdenv.isDarwin {
          launchd.agents.selector4nix = {
            enable = true;
            config = {
              Label = "cc.starryreverie.selector4nix";

              ProgramArguments = lib.singleton (
                "${pkgs.writeShellScript "launch-selector4nix" (
                  (lib.optionalString cfg.enablePersistentCaching ''
                    mkdir -p "$HOME/Library/Caches/selector4nix"
                    export SELECTOR4NIX_CACHE_DIR="$HOME/Library/Caches/selector4nix"
                  '')
                  + ''
                    exec ${cfg.package}/bin/selector4nix --no-log-timestamp
                  ''
                )}"
              );

              EnvironmentVariables = {
                SELECTOR4NIX_CONFIG_FILE = "${configFile}";
                RUST_LOG = "selector4nix=${cfg.logLevel}";
              }
              // lib.optionalAttrs (cfg.credentialFile != null) {
                SELECTOR4NIX_CREDENTIAL_FILE = "${cfg.credentialFile}";
              };

              KeepAlive = true;
              RunAtLoad = true;
              ProcessType = "Background";
            };
          };
        })
      ]
    ))

    (common.mkSubstituterConfig cfg)
  ];
}
