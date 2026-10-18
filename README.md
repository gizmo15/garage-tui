# garage-tui

Terminal UI (ratatui) for [Garage](https://garagehq.deuxfleurs.fr): buckets with size/objects/quota, access keys with their per-bucket permissions, and disk usage per cluster node. Read-only, uses the Garage Admin API v2 (Garage >= 2.0).

## Config

YAML file with the admin URL and token (copy `config.example.yaml`, then `chmod 600`):

```yaml
admin_url: http://127.0.0.1:3903
admin_token: xxxxxxxx
refresh_secs: 5   # optional
```

Default path: `~/.config/garage-tui/config.yaml`. Use another file with `-c, --config <PATH>`.

A read-only admin token is enough (the status bar shows its kind):

```
garage admin-token create garage-tui-ro --expires-in 90d \
  --scope ListBuckets,GetBucketInfo,ListKeys,GetKeyInfo,GetClusterStatus,GetCurrentAdminTokenInfo
```

## Run

```
cargo run -- --config ./config.yaml
```

## Keys

| Key | Action |
|---|---|
| `Tab`, `1`-`3` | Buckets / Keys / Cluster |
| `j` `k` / arrows | move |
| `/` | filter (Enter/Esc to stop, Esc again to clear) |
| `s` | sort buckets (name, size, objects) |
| `S` | reveal / hide key secrets |
| `r` | refresh now |
| `q` | quit |
