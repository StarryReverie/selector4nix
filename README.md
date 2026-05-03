# `selector4nix`

A Nix substituter proxy with parallel cache queries and latency-aware selection.

## Overview

`selector4nix` sits between your Nix client and multiple upstream substituters, acting as a smart proxy:

- Queries all configured substituters in parallel for `.narinfo` lookups
- Selects the fastest responding substituter based on latency and priority
- Automatically detects and skips unavailable substituters, retrying them with exponential backoff

Note that `selector4nix` only intents to work as a proxy rather than a full-featured cache substituter. NAR files are streamed directly from the best substituter without being cached locally. However, it does cache `.narinfo` files for better responsiveness.

The recommend way to use `selector4nix` is deploying it locally on each host. Since no large NAR file caching is used, `selector4nix` is pretty lightweight in terms of both memory footprint and CPU usage. In contrast, hosting `selector4nix` on a central node in your LAN for other machines doesn't scale well.

## Configuration

`selector4nix` reads a TOML configuration file from the first of these locations:

1. The path specified by the `SELECTOR4NIX_CONFIG_FILE` environment variable
2. `./selector4nix.toml` in the current directory
3. `/etc/selector4nix/selector4nix.toml`

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

For NixOS users, it is recommended to use the NixOS module provided by this project for declarative setup and configuration.

## Usage

Start the proxy in an ad-hoc style:

```sh
selector4nix
```

Point Nix to the proxy, placing it before other substituters so it takes priority while keeping fallbacks:

```sh
nix build --option substituters "http://127.0.0.1:5496 https://cache.nixos.org/" ...
```

Or use the NixOS module for declarative setup:

```nix
# flake.nix
{
  inputs.selector4nix.url = "github:StarryReverie/selector4nix";

  outputs = { nixpkgs, selector4nix, ... }@inputs: {
    nixosConfigurations.my-host = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        # Optional: use the overlay to provide `pkgs.selector4nix`
        # This adds a top-level `selector4nix` package to `pkgs` and the NixOS module will not use
        # the package exported by the flake directly
        { nixpkgs.overlays = [ selector4nix.overlays.default ]; }

        selector4nix.nixosModules.selector4nix
      ];
    };
  };
}
```

In your NixOS configuration:

```nix
# configuration.nix
{ config, ... }:
{
  services.selector4nix = {
    enable = true;
    configureSubstituter = true; # This automatically prepend the proxy to the substituter list
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

## Build

### Cargo

`selector4nix` uses the Rust 2024 edition, which requires Rust 1.85 or later. The toolchain is pinned to 1.93.1 via `rust-toolchain.toml`.

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
