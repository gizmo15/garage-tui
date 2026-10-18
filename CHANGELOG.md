# Changelog

All notable changes to this project are documented here.
Version numbers match the `version` field in `Cargo.toml` and the corresponding git tag.

---

## [Unreleased]

### Added
- Buckets whose size-quota usage exceeds 85% are shown in bold red in the list, and flagged in the detail pane
- Token badge in the status bar (`admin`, `read-only`, `custom: N endpoints`, `expired`) from `GET /v2/GetCurrentAdminTokenInfo`; hidden when the lookup fails (static config token, older Garage)
- Initial terminal UI (ratatui + crossterm) for Garage, read-only, based on the Garage Admin API v2 (Garage >= 2.0)
- Buckets tab: name, size, object count and size-quota percentage, with a detail pane (id, aliases, creation date, quotas, pending uploads, keys with their R/W/O permissions)
- Keys tab: name, access key id, number of buckets, expiration state, with a detail pane listing per-bucket permissions; secret keys are masked and can be revealed with `S`
- Cluster tab: per-node status (up/down, zone, draining) with a data-disk usage gauge, plus total buckets, objects and stored size
- Filtering (`/`), bucket sorting by name/size/objects (`s`), manual refresh (`r`) and automatic refresh (`refresh_secs`, default 5s) without blocking the UI
- YAML configuration file holding `admin_url` and `admin_token` (see `config.example.yaml`); default path `~/.config/garage-tui/config.yaml`, overridable with `-c, --config <PATH>`; a warning is shown when the file is readable by group/others
- Admin API error responses now show Garage's own message in the status bar (e.g. `HTTP 403 - Invalid bearer token`) instead of only the HTTP status code
- Unit tests (JSON fixtures in `tests/fixtures/`, config parsing, error formatting) and GitLab CI pipeline (fmt, clippy, audit, coverage, nextest with junit report)
