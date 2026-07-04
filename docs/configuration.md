# Configuration Reference

`selector4nix` reads a TOML configuration file from the first of these locations:

1. The path specified by the `--config-file` command line argument
2. The path specified by the `SELECTOR4NIX_CONFIG_FILE` environment variable
3. `./selector4nix.toml` in the current directory
4. `/etc/selector4nix/selector4nix.toml`

## `server`

Server listen address.

### `server.ip`

- Type: IP Address

The IP address that `selector4nix` listens on.

### `server.port`

- Type: Port
- Default: `5496`

The port that `selector4nix` listens on.

## `network`

Network request settings.

### `network.nar_info_timeout_secs`

- Type: Natural
- Default: `30`
- Minimum: `1`

Timeout in seconds for NAR info lookup requests.

### `network.nar_timeout_secs`

- Type: Natural
- Default: `30`
- Minimum: `1`

Timeout in seconds for NAR file downloads, also used as connect timeout.

### `network.max_concurrent_requests`

- Type: Natural
- Default: `12`

Maximum number of concurrent outgoing NAR file streaming requests, applied per distinct substituter host. The overall ceiling across the proxy is `max_concurrent_requests` multiplied by the number of distinct substituter hosts.

### `network.tolerance_msecs`

- Type: Natural
- Default: `50`
- Minimum: `1`

Latency tolerance window in milliseconds. The preference of a substituter is calculated as `-tolerance * priority - latency`. After the fastest substituter responds, other substituters have additional milliseconds equal to the difference between their preference and the current best before being pruned.

### `network.ignore_nar_info_error`

- Type: Boolean
- Default: `false`

When enabled, NAR info lookup errors from substituters are treated as not-found instead of infrastructure errors.

> **Warning:** This may cause incorrect judgments about whether a NAR info actually exists. A substituter returning an error will be interpreted as "not found", which may not be the case.

### `network.periodic_probing`

- Type: Boolean
- Default: `true`

When enabled, `selector4nix` continuously probes substituters every 30 seconds to detect failures early. Probing during retry recovery always occurs regardless of this setting.

## `download`

Optional segmented NAR download settings. When enabled, large NAR files may be fetched over multiple HTTP `Range` connections from the winning substituter when overall download load is low.

### `download.segmented`

- Type: Boolean
- Default: `false`

When enabled, eligible NAR downloads use multiple parallel range requests instead of a single streaming connection. Falls back to single-stream download when ineligible or when the substituter does not support byte ranges.

### `download.segmented_min_file_bytes`

- Type: Natural
- Default: `8388608` (8 MiB)

Minimum uncompressed-on-the-wire object size required before segmented download is considered.

### `download.segmented_max_connections`

- Type: Natural
- Default: `4`
- Minimum: `2`

Maximum number of parallel HTTP connections used for a single segmented NAR download.

### `download.segmented_load_threshold`

- Type: Natural
- Default: `3`

Segmented download is only used while the number of in-flight NAR downloads is less than or equal to this threshold. Additional range workers may also be spawned mid-download when load drops.

### `download.segmented_buffer_bytes`

- Type: Natural
- Default: `16777216` (16 MiB)

Maximum in-memory buffer budget for out-of-order segment reassembly. Applies backpressure to workers that run ahead of the client.

### HTTP Range support (substituter compatibility)

Segmented download requires the winning upstream substituter to advertise `Accept-Ranges: bytes` on the initial `200 OK` NAR response and to honour `Range` requests with `206 Partial Content` plus a `Content-Range` header. If a substituter does not meet these requirements, `selector4nix` transparently falls back to a single-stream download.

Spot checks performed in July 2026 against a `zlib` store path (`dbz6pb9g67kpgpl95k8d85kzpxm1c32p`, `.nar.zst` object) using `HEAD` and `Range: bytes=0-1023`:

| Substituter | Segmented download | Notes |
|-------------|-------------------|-------|
| `https://cache.nixos.org/` | Supported | `Accept-Ranges: bytes`; range probe returned `206` with `Content-Range` (Amazon S3 backend) |
| `https://mirrors.tuna.tsinghua.edu.cn/nix-channels/store/` | Supported | `Accept-Ranges: bytes`; range probe returned `206` with `Content-Range` |
| `https://mirrors.ustc.edu.cn/nix-channels/store/` | Supported | `Accept-Ranges: bytes`; range probe returned `206` with `Content-Range` |
| `https://mirror.sjtu.edu.cn/nix-channels/store/` | Not supported | No `Accept-Ranges` on NAR; range probe returned `200` without `Content-Range` |
| `https://mirrors.bfsu.edu.cn/nix-channels/store/` | Redirects to Tsinghua | Returns `302` to the Tsinghua mirror; inherits its range behaviour after redirect |

These results are illustrative only. Substituter CDNs, mirror sync state, and object availability can change over time. Private caches (for example Attic) depend on the storage backend in use.

## `proxy`

Proxy behavior settings.

### `proxy.rewrite_nar_url`

- Type: Boolean
- Default: `true`

When enabled, the `URL` field in NAR info responses is rewritten according to `rewrite_to_target`. When disabled, the original full URL or relative path from the upstream substituter is preserved as-is and `rewrite_to_target` is ignored.

### `proxy.rewrite_to_target`

- Type: String of `"self"` or `"upstream"`
- Default: `"self"`

Controls how the `URL` field is rewritten when `rewrite_nar_url` is enabled. Only effective when `rewrite_nar_url = true`.

- `"self"`: Rewrite to a relative path (e.g. `URL: nar/<hash>.nar.xz`) so that NAR file requests go through `selector4nix`. This allows transparent fallback to other substituters when the original one becomes unavailable.
- `"upstream"`: Rewrite to the winning upstream substituter's storage URL (e.g. `URL: https://cache.nixos.org/nar/<hash>.nar.xz`). This normalizes URLs to a consistent upstream address rather than preserving whatever format each substituter returns. NAR file requests will go directly to the upstream substituter, bypassing `selector4nix`.

Note that the `URL` field in NAR info is opaque and varies across substituters: a given store path may map to different NAR URLs on different substituters, so fallback is not guaranteed to succeed when the NAR files are not identical across substituters.

## `cache_info`

Cache info exposed via `/nix-cache-info` endpoint.

### `cache_info.store_dir`

- Type: String
- Default: `"/nix/store"`

Nix store directory path. Must be an absolute path.

### `cache_info.want_mass_query`

- Type: Boolean
- Default: `true`

Whether to advertise support for mass queries.

### `cache_info.priority`

- Type: Natural
- Default: `40`

Substituter priority advertised to Nix clients.

## `cache`

Internal LRU cache settings for NAR info and NAR location data.

NAR info cache stores the NAR info content for each store path hash. NAR location cache stores the reverse mapping from NAR file names back to their corresponding NAR info, used to locate the correct upstream substituter when proxying NAR file downloads.

### `cache.nar_info_lookup_capacity`

- Type: Natural
- Default: `4096`

Maximum number of cached NAR info entries.

### `cache.nar_info_lookup_ttl_secs`

- Type: Natural
- Default: `14400`
- Minimum: `1`

Time-to-live in seconds for cached NAR info entries.

### `cache.nar_location_capacity`

- Type: Natural
- Default: `4096`

Maximum number of cached NAR location entries.

### `cache.nar_location_ttl_secs`

- Type: Natural
- Default: `14400`
- Minimum: `1`

Time-to-live in seconds for cached NAR location entries.

## `substituters`

Upstream substituter list. At least one entry is required.

### `substituters[].url`

- Type: URL

Base URL of the upstream substituter.

### `substituters[].storage_url`

- Type: URL
- Default: `"{substituters[].url}/nar/""`

Override the base URL used for NAR file downloads.

### `substituters[].priority`

- Type: Natural
- Default: `40`

Priority of this substituter. Higher values mean lower priority.

### `substituters[].nar_info_timeout_secs`

- Type: Natural | None
- Default: none

Per-substituter override for NAR info lookup timeout in seconds. When unset, falls back to `network.nar_info_timeout_secs`.

### `substituters[].nar_timeout_secs`

- Type: Natural | None
- Default: none

Per-substituter override for NAR file download timeout in seconds. When unset, falls back to `network.nar_timeout_secs`.
