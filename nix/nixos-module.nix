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
  settingsFormat = pkgs.formats.toml { };
  configFile = settingsFormat.generate "selector4nix.toml" cfg.settings;
in
{
  options = {
    services.selector4nix = {
      enable = lib.mkOption {
        type = lib.types.bool;
        description = "Whether to enable selector4nix";
        default = false;
        example = true;
      };

      package = lib.mkOption {
        type = lib.types.package;
        description = "The selector4nix package to use";
        default =
          pkgs.selector4nix
            or (withSystem pkgs.stdenv.hostPlatform.system ({ config, ... }: config.packages.selector4nix));
      };

      settings = lib.mkOption {
        type = lib.types.submodule {
          freeformType = settingsFormat.type;
          options.server = {
            ip = lib.mkOption {
              type = lib.types.str;
              description = "The IP address that selector4nix listens on";
              default = "127.0.0.1";
              example = "127.0.0.1";
            };

            port = lib.mkOption {
              type = lib.types.port;
              description = "The port that selector4nix listens on";
              default = 5496;
              example = 5496;
            };
          };
        };
        description = "The configuration that will be read by selector4nix";
        default = { };
        example = {
          server = {
            ip = "127.0.0.1";
          };
          substituters = [
            {
              url = "https://cache.nixos.org/";
            }
            {
              url = "https://mirrors.ustc.edu.cn/nix-channels/store/";
              priority = 45;
            }
            {
              url = "https://cache.garnix.io/";
              storage_url = "https://garnix-cache.com/";
            }
          ];
        };
      };

      configureSubstituter = lib.mkOption {
        type = lib.types.bool;
        description = "Whether to prepend the address of selector4nix to the substituter list";
        default = false;
        example = true;
      };
    };
  };

  config = lib.mkMerge [
    (lib.mkIf cfg.enable {
      systemd.services.selector4nix = {
        description = "Nix substituter proxy with parallel cache queries and latency-aware selection";
        wantedBy = [ "multi-user.target" ];
        after = [ "network-online.target" ];
        wants = [ "network-online.target" ];

        serviceConfig = {
          Type = "simple";
          ExecStart = "${cfg.package}/bin/selector4nix";
          Environment = [ "SELECTOR4NIX_CONFIG_FILE=${configFile}" ];
          Restart = "on-failure";
          RestartSec = 5;

          DynamicUser = true;
          CapabilityBoundingSet = [ "" ];
          DeviceAllow = "";
          LockPersonality = true;
          MemoryDenyWriteExecute = true;
          NoNewPrivileges = true;
          PrivateDevices = true;
          PrivateTmp = true;
          ProtectClock = true;
          ProtectControlGroups = true;
          ProtectHome = true;
          ProtectHostname = true;
          ProtectKernelLogs = true;
          ProtectKernelModules = true;
          ProtectKernelTunables = true;
          ProtectSystem = "strict";
          RestrictAddressFamilies = [
            "AF_INET"
            "AF_INET6"
          ];
          RestrictNamespaces = true;
          RestrictRealtime = true;
          RestrictSUIDSGID = true;
          SystemCallArchitectures = "native";
          SystemCallFilter = [
            "@system-service"
            "~@resources"
            "~@privileged"
          ];
          UMask = "0077";
        };
      };
    })

    (lib.mkIf (cfg.enable && cfg.configureSubstituter) {
      nix.settings.substituters = lib.mkBefore [
        "http://${cfg.settings.server.ip}:${builtins.toString cfg.settings.server.port}/"
      ];
    })
  ];
}
