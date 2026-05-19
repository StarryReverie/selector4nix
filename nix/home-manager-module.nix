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
  options = {
    services.selector4nix = common.serviceOptions;
  };

  config = lib.mkMerge [
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
              ProgramArguments = [ "${cfg.package}/bin/selector4nix --no-log-timestamp" ];
              EnvironmentVariables = {
                SELECTOR4NIX_CONFIG_FILE = "${configFile}";
                RUST_LOG = "selector4nix=${cfg.logLevel}";
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
