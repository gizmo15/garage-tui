# Changelog

All notable changes to this project are documented here.
Version numbers match the `version` field in `Cargo.toml` and the corresponding git tag.

---

## [Unreleased]

### Added
- `GARAGE_ADMIN_TOKEN` / `GARAGE_ADMIN_URL` environment variables override `admin_token` / `admin_url` from the config file (precedence env > file); the config file is optional when both are set (`-c` still requires it). The Configuration tab shows whether the token comes from the file or the environment

### Changed
- `admin_token` and `admin_url` may be omitted from the config file when provided by the environment; the "readable by group/others" warning is now only shown when the file contains an `admin_token`
- A missing default config file is no longer an error when the environment provides the url and token

---

## [0.2.0] - 2026-10-19

### Added
- `insecure_skip_tls_verify: true` config key to talk to a Garage behind a self-signed certificate (certificate verification is disabled: use only on a trusted network). A warning is shown while it is active, and the Configuration tab shows the state
- Warning when `admin_url` is `http://` to a non-loopback host, since the admin token would travel in clear
- `quota_warn_percent` (default 85) and `disk_warn_percent` (default 90, yellow 15 points below) config keys replace the hardcoded thresholds; values must be in ]0, 100]
- Configuration tab (`4`) showing the effective settings (the token is never displayed)
- Object-count limit shown in the Buckets tab (`42 / 100`, with its percentage in the detail pane) when a `maxObjects` quota is set; it is highlighted in red above the quota threshold like the size quota. Counters come from `GetBucketInfo` and can lag by a few seconds
- Buckets whose size-quota usage exceeds 85% are shown in bold red in the list, and flagged in the detail pane
- Token badge in the status bar (`admin`, `read-only`, `custom: N endpoints`, `expired`) from `GET /v2/GetCurrentAdminTokenInfo`; hidden when the lookup fails (static config token, older Garage)

### Changed
- Secret keys can only be revealed (`S`) with an admin token (scope `*`) or the static config token; for any other token the key is disabled and the help line hides it. This is a UI safeguard: to truly prevent access, leave `GetKeyInfo` out of the token scope
- `admin_token` and key secrets are masked in `Debug` output (new `Secret` type) so they cannot leak through logs or panics
- YAML parsing moved from the deprecated `serde_yaml` to `serde-saphyr`, which removes the unmaintained `unsafe-libyaml` dependency (RUSTSEC-2024-0449); the config file format is unchanged
- More unit tests and a build job in the GitLab CI pipeline

---

## [0.1.0] - 2026-10-18

### Added
- Initial terminal UI (ratatui + crossterm) for Garage, read-only, based on the Garage Admin API v2 (Garage >= 2.0)
- Buckets tab: name, size, object count and size-quota percentage, with a detail pane (id, aliases, creation date, quotas, pending uploads, keys with their R/W/O permissions)
- Keys tab: name, access key id, number of buckets, expiration state, with a detail pane listing per-bucket permissions; secret keys are masked and can be revealed with `S`
- Cluster tab: per-node status (up/down, zone, draining) with a data-disk usage gauge, plus total buckets, objects and stored size
- Filtering (`/`), bucket sorting by name/size/objects (`s`), manual refresh (`r`) and automatic refresh (`refresh_secs`, default 5s) without blocking the UI
- YAML configuration file holding `admin_url` and `admin_token` (see `config.example.yaml`); default path `~/.config/garage-tui/config.yaml`, overridable with `-c, --config <PATH>`; a warning is shown when the file is readable by group/others
- Admin API error responses now show Garage's own message in the status bar (e.g. `HTTP 403 - Invalid bearer token`) instead of only the HTTP status code
- Unit tests (JSON fixtures in `tests/fixtures/`, config parsing, error formatting) and GitLab CI pipeline (fmt, clippy, audit, coverage, nextest with junit report)
