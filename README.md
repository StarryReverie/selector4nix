# Selector4nix

A Nix substituter proxy with parallel cache queries and latency-aware selection.

## Overview

Selector4nix sits between your Nix client and multiple upstream substituters, acting as a smart proxy:

- Queries all configured substituters in parallel for `.narinfo` lookups
- Selects the fastest responding substituter based on latency and priority
- Automatically detects and skips unavailable substituters, retrying them with exponential backoff

## Configuration

Selector4nix reads a TOML configuration file from the first of these locations:

1. The path specified by the `SELECTOR4NIX_CONFIG_FILE` environment variable
2. `./selector4nix.toml` in the current directory
3. `/etc/selector4nix/selector4nix.toml`

A example configuration is demostrated below. A complete reference is available at [`docs/selector4nix.example.toml`](/docs/selector4nix.example.toml).

```toml
[server]
ip = "127.0.0.1"
# port = 5496 # Default port

[[substituters]]
url = "https://cache.nixos.org/"

[[substituters]]
url = "https://mirrors.ustc.edu.cn/nix-channels/store/"
priority = 45 # The higher the value, the lower the priority of this substituter

[[substituters]]
url = "https://cache.garnix.io/"
storage_url = "https://garnix-cache.com/" # Garnix doesn't serve NAR files on https://cache.garnix.io/nar/
```

### Substituter Priority

Lower priority values are preferred. When multiple substituters respond with the same `.narinfo`, selector4nix picks the one with the best combined score of priority and response latency. Set a higher priority for mirrors that you want to use as fallbacks.

## Usage

Start the proxy:

```sh
selector4nix
```

Point Nix to the proxy, placing it before other caches so it takes priority while keeping fallbacks:

```sh
nix build --option substituters "http://127.0.0.1:5496 https://cache.nixos.org/" ...
```

Or configure it persistently in your NixOS configuration:

```nix
{
  nix.settings.substituters = [
    "http://127.0.0.1:5496"
    "https://cache.nixos.org/"
    # Your other substituters
  ];
  nix.settings.trusted-substituters = [
    "http://127.0.0.1:5496"
    "https://cache.nixos.org/"
    # Your other substituters
  ];
}
```

## Build

### Cargo

Selector4nix uses the Rust 2024 edition, which requires Rust 1.85 or later. The toolchain is pinned to 1.93.1 via `rust-toolchain.toml`.

```sh
cargo build --release
```

To install the binary to `~/.cargo/bin`:

```sh
cargo install --path .
```

### Nix

A Nix flake is provided. Replace `<system>` in the commands below with your target platform: `x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, or `aarch64-darwin`.

Build from the current directory:

```sh
nix --extra-experimental-features "nix-command flakes" build .#packages.<system>.selector4nix
```

Add the application exported by this flake to your NixOS configuration:

```nix
# flake.nix
{
  inputs.selector4nix.url = "github:StarryReverie/selector4nix";
  # ...
}

# configuration.nix
{ pkgs, inputs, ... }:
{
  environment.systemPackages = [
    inputs.selector4nix.packages.${pkgs.stdenv.hostPlatform.system}.selector4nix
  ];
}
```

## License

This project is licensed under [GPL-3.0-or-later](/LICENSE).

Copyright (C) 2026 Justin Chen
