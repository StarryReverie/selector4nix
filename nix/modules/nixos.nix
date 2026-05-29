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
      systemd.services.selector4nix = {
        description = "Nix substituter proxy with parallel cache queries and latency-aware selection";
        wantedBy = [ "multi-user.target" ];
        after = [ "network-online.target" ];
        wants = [ "network-online.target" ];

        serviceConfig = {
          Type = "simple";
          ExecStart = "${cfg.package}/bin/selector4nix --no-log-timestamp";

          Environment = [
            "SELECTOR4NIX_CONFIG_FILE=${configFile}"
            "RUST_LOG=selector4nix=${cfg.logLevel}"
          ]
          ++ lib.optionals (cfg.credentialFile != null) [
            "SELECTOR4NIX_CREDENTIAL_FILE=%d/credentials.toml"
          ]
          ++ lib.optionals cfg.enablePersistentCaching [
            "SELECTOR4NIX_CACHE_DIR=%C/selector4nix"
          ];

          LoadCredential = lib.optionals (cfg.credentialFile != null) [
            "credentials.toml:${cfg.credentialFile}"
          ];

          CacheDirectory = lib.optionals cfg.enablePersistentCaching [
            "selector4nix"
          ];

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

    (common.mkSubstituterConfig cfg)
  ];
}
