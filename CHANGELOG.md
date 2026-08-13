# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added a temporary workaround for [harmonia](https://github.com/nix-community/harmonia) and other potential misbehaved servers when requesting NARs from them using chunked streaming. Harmonia won't respond correctly for such requests until [this fix](https://github.com/nix-community/harmonia/pull/1139) is merged.
- Substituter's status is now updated after opening a NAR stream, if the response of the corresponding request to that substituter arrives.

### Fixed

- Fixed NAR request returning 404 when encountering cache miss after restart and the actual upstream NAR URL contains query parameters. This is because previously the query parameters were only stored in the proxy's memory and were hidden from the client. So the server couldn't remember the query parameters after cache invalidation. Now the original query parameters are encoded and returned to the client as a new parameter `upstream_query` within NAR info's `URL` field, so the proxy can reconstruct parameters from clients' requests even if it forgot them.

## [0.9.0] - 2026-08-04

### Added

- Active NAR downloads are now tracked and surfaced on the dashboard. [@XYenon]
- Expired cache entries are now reclaimed by a periodic background task rather than being left until the next lookup or restart.
- Added a Cachix substituter for this project at `https://selector4nix.cachix.org/` with public key `selector4nix.cachix.org-1:wovVlT07In5JCVz2tFgxPQTLpnN8hZT6P/RwfFcz3KE=`. Every commit on the main branch is built and pushed there. The cache depends on `nix-community.cachix.org`, which should also be added to avoid cache misses.
- Added this CHANGELOG.
- Added a screenshot of the dashboard to the README.
- Added substituter `selector4nix.cachix.org` to example configurations throughout the repository.

### Changed

- **Breaking:** Renamed cache configuration keys for clarity:
  - `cache.nar_info_lookup_capacity` -> `cache.nar_info_cache_capacity`
  - `cache.nar_info_lookup_ttl_secs` -> `cache.nar_info_ttl_secs`
  - `cache.nar_location_capacity` -> `cache.nar_file_cache_capacity`
  - `cache.nar_location_ttl_secs` -> `cache.nar_file_ttl_secs`
- **Breaking:** Unknown configuration entries are now rejected instead of being silently ignored.
- **Breaking:** Configuration validation is now stricter:
  - The `substituters` list must contain at least one entry.
  - Timeouts and TTLs of `0` are now rejected rather than silently clamped to 1 second. This affects `network.nar_info_timeout_secs`, `network.nar_timeout_secs`, `cache.nar_info_ttl_secs`, `cache.nar_file_ttl_secs`, and the per-substituter `substituters[].nar_info_timeout_secs` and `substituters[].nar_timeout_secs`.
  - Concurrency and cache capacity values of `0` are now rejected rather than silently accepted. This affects `network.max_concurrent_requests`, `cache.nar_info_cache_capacity`, and `cache.nar_file_cache_capacity`.
- Rewrote the web dashboard with four pages. The previous dashboard is removed and now redirects to the new one.
  - Overview: high-level server status and a substituter list sorted by priority.
  - Transferring: NAR files currently being downloaded and the substituter each comes from.
  - Cache: entry counts and capacities for the NAR info and NAR file caches and their backing stores.
  - Configuration: the effective configuration, grouped by section.

### Removed

- **Breaking:** Removed the `/status` endpoint. It became useless after the new dashboard was introduced.

## [0.8.0] - 2026-07-17

Introduces concurrent chunked NAR streaming and a web status dashboard.

### Added

- Chunked NAR streaming pre-fetches file chunks concurrently through a sliding window with per-host throttling. It is selected automatically when the upstream substituter supports HTTP Range requests, and can be disabled or tuned via `network.chunked_streaming`, `network.streaming_chunk_max_len`, and `network.streaming_window_max_len`.
- NAR streaming now falls back to another substituter early if the selected one becomes unavailable mid-transfer.
- Added a status dashboard and a `/status` endpoint showing substituter health, cache stats, Nix cache info, and runtime network and proxy settings. [@XYenon]
- Added contribution guidelines, architecture documentation, and contributor conventions.

### Removed

- Dropped garnix for CI and cached build artifacts following the service's shutdown.

## [0.7.0] - 2026-06-24

### Added

- The client's User-Agent is now passed through to upstream substituters for narinfo queries and NAR file streaming.
- Credentials are now applied to NAR file downloads, enabling compatibility with private substituters such as Attic. [@XYenon]

### Changed

- NAR file streaming throttling is now applied per substituter host instead of globally. `network.max_concurrent_requests` sets the limit for each host.

## [0.6.1] - 2026-06-08

### Fixed

- NAR info requests are no longer throttled. `network.max_concurrent_requests` now applies only to NAR file streaming.

## [0.6.0] - 2026-06-02

Introduces optional persistent on-disk caching of query results across restarts.

### Added

- NAR info and NAR file location data can now be persisted to an on-disk cache across restarts, enabled by passing `--cache-dir` or setting the Nix module's `enablePersistentCaching`.

### Changed

- The default value of `network.max_concurrent_requests` is lowered from 24 to 12.

## [0.5.0] - 2026-05-26

Introduces credential support for private substituters.

### Added

- Credentials can now be sent to upstream substituters using HTTP Basic Auth, configured via a credentials file. The Nix modules gained a `credentialFile` option.
- Added a `/health` endpoint and a home page at the root.
- The `/nix-cache-info` endpoint accepts an optional `priority` query parameter to override the advertised priority.
- Added structured logging throughout the server.

### Changed

- A successful probe no longer immediately restores a substituter to the normal state.
- The Nix flake is split into default and `dev` partitions, so consumers no longer fetch development-only inputs.

## [0.4.2] - 2026-05-23

### Fixed

- Fixed a race condition in NarInfo query task cancellation.

## [0.4.1] - 2026-05-23

### Fixed

- Uncompressed NAR files referenced by upstream NAR info URLs that end in `.nar` without a compression suffix are now parsed and served correctly instead of failing validation.
- Query parameters in relative NAR URLs are no longer dropped when constructing upstream NAR requests. This preserves signed URL parameters such as `?X-Amz-Signature=...` from private caches.

## [0.4.0] - 2026-05-19

### Added

- Added Home Manager and nix-darwin modules. Previously only a NixOS module was available. [@XYenon]
- Added the `network.ignore_nar_info_error` configuration option. When enabled, NAR info query errors from substituters are treated as not-found instead of infrastructure errors.
- Offline substituters are now distinguished from those with service errors. An offline substituter's NAR info query error is treated as a 404 response, and different retry strategies are applied based on the error type. Offline substituters are retried at a fixed interval, while service errors use exponential backoff.
- Substituters are now actively probed before being restored to the normal state, so a substituter is only recovered after a successful probe.
- Added the `network.periodic_probing` configuration option. When enabled, all substituters are probed every 30 seconds to detect failures early. Probing during retry recovery always runs regardless of this option.
- Added the `--log-file` CLI option to append log output to a file.
- Added oslog support for the Darwin platform so logs are sent to the system log.

### Fixed

- Fixed unseparated CLI arguments in the Home Manager module on Darwin.
- Fixed the Home Manager module to assert that the `nix.package` option is set when configuring user-level `nix.settings.substituters`, as evaluation would otherwise fail. [@XYenon]

## [0.3.1] - 2026-05-18

### Added

- Added a release workflow that builds and uploads pre-built binaries to GitHub Releases for Linux and macOS. [@lxl66566]

## [0.3.0] - 2026-05-18

### Added

- Added per-substituter NAR info querying and NAR streaming timeout overrides via `substituters[].nar_info_timeout_secs` and `substituters[].nar_timeout_secs`.
- Added the `--no-log-timestamp` CLI option to hide timestamps from log output.
- Added fully flake-less setup support via the Nixpkgs overlay and NixOS module import.
- Added a statically linked package output for Linux.

### Changed

- Removed the systemd environment detection for hiding log timestamps. The `--no-log-timestamp` option is now set explicitly in the NixOS module's systemd service.

### Fixed

- Fixed a panic in NAR info resolution that occurred when a NAR info actor was evicted while in use. The resolution now returns an error instead.
- Fixed statically linked build failure for `aarch64-darwin`.
- Fixed the `configureSubstituter` option to align with the code. [@mio-19]

## [0.2.0] - 2026-05-06

### Added

- Added a command-line interface:
  - `--config-file <PATH>` to specify the configuration file.
  - `--log-level <LEVEL>` to set the log verbosity level.
  - `serve` to start the server (default).
  - `check` to validate the configuration file.
- Added the `proxy.rewrite_to_target` configuration option to control how the NAR info `URL` field is rewritten.
- Added the `logLevel` option to the NixOS module.
- The NixOS module can now prepend to or rewrite the substituter list.
- **Breaking:** The NixOS module now validates the generated configuration file automatically. Invalid configurations can no longer build successfully.

## [0.1.1] - 2026-05-03

### Added

- Added a Nixpkgs overlay as an alternative way to build and install `selector4nix` on NixOS.

### Fixed

- Fixed a panic in NAR info query tasks when no substituter returned a result.

## [0.1.0] - 2026-05-03

This is the first release of `selector4nix`, a Nix substituter proxy with parallel cache queries and latency-aware selection.

### Added

- Queries all configured substituters in parallel for NAR info lookups and selects the fastest responding one based on latency and priority.
- Automatically detects and skips unavailable substituters, retrying them with exponential backoff.
- Streams NAR files directly from the best substituter without local caching, while caching NAR info files for better responsiveness.
- Provides a NixOS module for declarative setup and configuration.

<!-- Versions -->

[Unreleased]: https://github.com/StarryReverie/selector4nix/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/StarryReverie/selector4nix/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/StarryReverie/selector4nix/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/StarryReverie/selector4nix/compare/v0.6.1...v0.7.0
[0.6.1]: https://github.com/StarryReverie/selector4nix/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/StarryReverie/selector4nix/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/StarryReverie/selector4nix/compare/v0.4.2...v0.5.0
[0.4.2]: https://github.com/StarryReverie/selector4nix/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/StarryReverie/selector4nix/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/StarryReverie/selector4nix/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/StarryReverie/selector4nix/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/StarryReverie/selector4nix/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/StarryReverie/selector4nix/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/StarryReverie/selector4nix/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/StarryReverie/selector4nix/commits/v0.1.0

<!-- Contributors -->

[@XYenon]: https://github.com/XYenon
[@lxl66566]: https://github.com/lxl66566
[@mio-19]: https://github.com/mio-19
