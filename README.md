# `selector4nix`

A Nix substituter proxy with parallel cache queries and latency-aware selection.

## Overview

`selector4nix` sits between your Nix client and multiple upstream substituters, acting as a smart proxy:

- Queries all configured substituters in parallel for `.narinfo` lookups
- Selects the fastest responding substituter based on latency and priority
- Automatically detects and skips unavailable substituters, retrying them with exponential backoff
- Continuously probes substituters to detect failures early and verify recovery
- Proxy private cache substituters with additional credentials

Note that `selector4nix` only intends to work as a proxy rather than a full-featured cache substituter. NAR files are streamed directly from the best substituter without being cached locally. However, it does cache `.narinfo` files for better responsiveness.

The recommended way to use `selector4nix` is deploying it locally on each host. Since no large NAR file caching is used, `selector4nix` is pretty lightweight in terms of both memory footprint and CPU usage. In contrast, hosting `selector4nix` on a central node in your LAN for other machines doesn't scale well.

## Configuration

### General

`selector4nix` reads a TOML configuration file from the first of these locations:

1. The path specified by the `--config-file` command line argument
2. The path specified by the `SELECTOR4NIX_CONFIG_FILE` environment variable
3. `./selector4nix.toml` in the current directory
4. `/etc/selector4nix/selector4nix.toml`

An example configuration is demonstrated below. For a complete reference of all available fields, see [`docs/configuration.md`](/docs/configuration.md). An annotated example configuration file is also available at [`docs/selector4nix.example.toml`](/docs/selector4nix.example.toml).

```toml
[server]
ip = "127.0.0.1"
# port = 5496 # Default port

[[substituters]]
url = "https://cache.nixos.org/"
# priority = 40 # Default priority

[[substituters]]
url = "https://mirrors.ustc.edu.cn/nix-channels/store/"
priority = 45 # The higher the value, the lower the priority of this substituter

[[substituters]]
url = "https://cache.garnix.io/"
storage_url = "https://garnix-cache.com/" # Garnix doesn't serve NAR files on https://cache.garnix.io/nar/
```

For NixOS, nix-darwin, and Home Manager users, it is recommended to use the modules provided by this project for declarative setup and configuration.

### Credentials

`selector4nix` can optionally read a TOML credentials file for authenticating with private caches. It is loaded from the first of these locations:

1. The path specified by the `--credential-file` command line argument
2. The path specified by the `SELECTOR4NIX_CREDENTIAL_FILE` environment variable
3. `./credentials.toml` in the current directory
4. `/etc/selector4nix/credentials.toml`

If no credentials file is found, all upstream requests are made without authentication. Credentials are only used for `/nix-cache-info` lookups and `.narinfo` queries; NAR file downloads typically rely on pre-signed URLs.

An example credentials file is demonstrated below. For a complete reference, see [`docs/credentials.md`](/docs/credentials.md). An annotated example credentials file is also available at [`docs/credentials.example.toml`](/docs/credentials.example.toml).

```toml
[[credentials]]
url = "https://my.private-cache.com/"
login = "my-username"
secret = "my-secret"
```

## Usage

### Ad-hoc

Start the proxy in an ad-hoc style, on whatever OS:

```sh
selector4nix # A configuration file must be discoverable (see Configuration above)
```

On the same machine, point Nix to the proxy, then everything is done. All NAR info queries and subsequent NAR fetching will transparently go through the `selector4nix` proxy.

```sh
nix build --option substituters "http://127.0.0.1:5496/" ...
```

If you care about definite robustness, you can place it before other substituters so it takes priority while keeping fallbacks:

```sh
nix build --option substituters "http://127.0.0.1:5496/ https://cache.nixos.org/" ...
```

### Import the NixOS/Nix-darwin/Home Manager Module (flake)

Firstly, the `selector4nix` module should be imported into your system or home configuration, optionally with a Nixpkgs overlay.

On the usage of Nixpkgs overlay, `selector4nix.overlays.default` is exposed and you can configure your system Nixpkgs with the overlay. This is useful when you want to build the `selector4nix` package with the toolchain provided by your system Nixpkgs instance rather than the one defined in the flake output.

For NixOS:

```nix
# flake.nix
{
  inputs.selector4nix.url = "github:StarryReverie/selector4nix";

  outputs = { nixpkgs, selector4nix, ... }@inputs: {
    nixosConfigurations.my-host = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        selector4nix.nixosModules.selector4nix
        # Optional: use the overlay to provide `pkgs.selector4nix`
        # { nixpkgs.overlays = [ selector4nix.overlays.selector4nix ]; }
      ];
    };
  };
}
```

For nix-darwin:

```nix
# nix-darwin flake.nix
{
  inputs.selector4nix.url = "github:StarryReverie/selector4nix";

  outputs = { nixpkgs, nix-darwin, selector4nix, ... }@inputs: {
    darwinConfigurations.my-host = nix-darwin.lib.darwinSystem {
      system = "aarch64-darwin";
      modules = [
        selector4nix.darwinModules.selector4nix
        # Optional: use the overlay to provide `pkgs.selector4nix`
        # { nixpkgs.overlays = [ selector4nix.overlays.selector4nix ]; }
      ];
    };
  };
}
```

For Home Manager:

```nix
# Home Manager flake.nix
{
  inputs.selector4nix.url = "github:StarryReverie/selector4nix";

  outputs = { nixpkgs, home-manager, selector4nix, ... }@inputs: {
    homeConfigurations.my-user = home-manager.lib.homeManagerConfiguration {
      modules = [
        selector4nix.homeManagerModules.selector4nix
      ];
      # Optional: use the overlay to provide `pkgs.selector4nix`
      # pkgs = import nixpkgs {
      #   system = "x86_64-linux";
      #   overlays = [ selector4nix.overlays.selector4nix ];
      # };
    };
  };
}
```

### Import the NixOS/Nix-darwin/Home Manager Module (niv, npins, etc.)

For those who don't use flakes, a flake-less setup is also possible.

For NixOS:

```nix
# configuration.nix
{ config, ... }:
{
  # Assume that there exists a `selector4nix` input in the lexical scope.
  imports = [ (import "${selector4nix}/nix/modules/nixos.nix" { withSystem = throw "unreachable"; }) ];
  nixpkgs.overlays = [ (import "${selector4nix}/nix/overlay.nix") ];
}
```

For nix-darwin and Home Manager, the setup is similar. The differences are how you import a module and an overlay, and the corresponding module path:

- NixOS: `"${selector4nix}/nix/modules/nixos.nix"`
- nix-darwin: `"${selector4nix}/nix/modules/darwin.nix"`
- Home Manager: `"${selector4nix}/nix/modules/home-manager.nix"`

### Configure the Service

In your NixOS, nix-darwin, or Home Manager configuration:

```nix
# configuration.nix
{ config, ... }:
{
  services.selector4nix = {
    enable = true;

    # This automatically overwrites the substituter list.
    # Alternatives are "keep" (default) and "prepend".
    configureSubstituter = "overwrite";

    settings = {
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
}
```

See [Configuration](#configuration) above for details.

## Build

### Cargo

`selector4nix` uses the Rust 2024 edition, which requires Rust 1.85 or later. The toolchain is pinned to 1.95.0 via `rust-toolchain.toml`.

```sh
cargo build --release
```

To install the binary to `~/.cargo/bin`:

```sh
cargo install --path .
```

### Nix

Replace `<system>` in the commands below with your target platform: `x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, or `aarch64-darwin`.

Build from the current directory:

```sh
nix --extra-experimental-features "nix-command flakes" build .#packages.<system>.selector4nix
```

## License

This project is licensed under [GPL-3.0-or-later](/LICENSE).

Copyright (C) 2026 Justin Chen
