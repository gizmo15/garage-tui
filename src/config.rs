use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::secret::Secret;

const DEFAULT_REFRESH_SECS: u64 = 5;
const DEFAULT_QUOTA_WARN_PERCENT: f64 = 85.0;
const DEFAULT_DISK_WARN_PERCENT: f64 = 90.0;
/// Gap between the yellow (caution) and red (warning) disk gauge thresholds.
pub const DISK_CAUTION_MARGIN: f64 = 15.0;

const ENV_ADMIN_URL: &str = "GARAGE_ADMIN_URL";
const ENV_ADMIN_TOKEN: &str = "GARAGE_ADMIN_TOKEN";
const TOKEN_SOURCE_FILE: &str = "config file";
const TOKEN_SOURCE_ENV: &str = "environment (GARAGE_ADMIN_TOKEN)";

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub admin_url: String,
    #[serde(default)]
    pub admin_token: Secret,
    #[serde(default)]
    pub refresh_secs: Option<u64>,
    /// Bucket quota usage (%) above which a bucket is highlighted.
    #[serde(default)]
    pub quota_warn_percent: Option<f64>,
    /// Data disk usage (%) above which the gauge turns red.
    #[serde(default)]
    pub disk_warn_percent: Option<f64>,
    /// Accept any TLS certificate (self-signed). Unsafe: exposes the token to MITM.
    #[serde(default)]
    pub insecure_skip_tls_verify: Option<bool>,
    /// A config file was read (as opposed to a setup made of environment variables only).
    #[serde(skip)]
    has_file: bool,
    #[serde(skip)]
    token_from_env: bool,
}

/// Effective settings shown in the Configuration tab and used by the UI (no secrets).
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub admin_url: String,
    pub config_path: Option<String>,
    pub token_source: &'static str,
    pub refresh_secs: u64,
    pub quota_warn_percent: f64,
    pub disk_warn_percent: f64,
    pub insecure_skip_tls_verify: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            admin_url: String::new(),
            config_path: None,
            token_source: TOKEN_SOURCE_FILE,
            refresh_secs: DEFAULT_REFRESH_SECS,
            quota_warn_percent: DEFAULT_QUOTA_WARN_PERCENT,
            disk_warn_percent: DEFAULT_DISK_WARN_PERCENT,
            insecure_skip_tls_verify: false,
        }
    }
}

impl Config {
    pub fn refresh_secs(&self) -> u64 {
        self.refresh_secs.unwrap_or(DEFAULT_REFRESH_SECS).max(1)
    }

    pub fn quota_warn_percent(&self) -> f64 {
        self.quota_warn_percent
            .unwrap_or(DEFAULT_QUOTA_WARN_PERCENT)
    }

    pub fn disk_warn_percent(&self) -> f64 {
        self.disk_warn_percent.unwrap_or(DEFAULT_DISK_WARN_PERCENT)
    }

    pub fn insecure_skip_tls_verify(&self) -> bool {
        self.insecure_skip_tls_verify.unwrap_or(false)
    }

    /// Warnings about risky transport settings.
    pub fn security_warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if is_cleartext_remote(&self.admin_url) {
            warnings.push(
                "admin token sent in clear over http to a remote host, use https".to_string(),
            );
        }
        if self.insecure_skip_tls_verify() {
            warnings.push("TLS certificate verification is disabled".to_string());
        }
        warnings
    }

    pub fn settings(&self, config_path: Option<&Path>) -> Settings {
        Settings {
            admin_url: self.admin_url.clone(),
            config_path: self
                .has_file
                .then(|| config_path.map(Path::to_path_buf).or_else(default_path))
                .flatten()
                .map(|p| p.display().to_string()),
            token_source: if self.token_from_env {
                TOKEN_SOURCE_ENV
            } else {
                TOKEN_SOURCE_FILE
            },
            refresh_secs: self.refresh_secs(),
            quota_warn_percent: self.quota_warn_percent(),
            disk_warn_percent: self.disk_warn_percent(),
            insecure_skip_tls_verify: self.insecure_skip_tls_verify(),
        }
    }

    pub fn from_yaml(content: &str) -> Result<Self> {
        let mut config: Config = serde_saphyr::from_str(content).context("invalid YAML config")?;
        config.has_file = true;
        for (name, value) in [
            ("quota_warn_percent", config.quota_warn_percent),
            ("disk_warn_percent", config.disk_warn_percent),
        ] {
            if value.is_some_and(|v| !(v > 0.0 && v <= 100.0)) {
                bail!("{name} must be in the range ]0, 100]");
            }
        }
        Ok(config)
    }

    /// Environment variables take precedence over the config file.
    fn apply_env(&mut self, get: impl Fn(&str) -> Option<String>) {
        let non_empty = |key: &str| {
            get(key)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        if let Some(url) = non_empty(ENV_ADMIN_URL) {
            self.admin_url = url;
        }
        if let Some(token) = non_empty(ENV_ADMIN_TOKEN) {
            self.admin_token = Secret::from(token);
            self.token_from_env = true;
        }
    }

    /// Checked after the file and the environment are merged.
    fn check_required(&self) -> Result<()> {
        if self.admin_url.trim().is_empty() {
            bail!("admin_url is missing: set it in the config file or with {ENV_ADMIN_URL}");
        }
        if self.admin_token.expose().trim().is_empty() {
            bail!("admin_token is missing: set it in the config file or with {ENV_ADMIN_TOKEN}");
        }
        Ok(())
    }
}

pub fn default_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("garage-tui").join("config.yaml"))
}

/// True for an `http://` URL whose host is not loopback.
fn is_cleartext_remote(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    if url.scheme() != "http" {
        return false;
    }
    match url.host_str().map(|h| h.trim_matches(['[', ']'])) {
        Some(host) if host.eq_ignore_ascii_case("localhost") => false,
        Some(host) => host
            .parse::<std::net::IpAddr>()
            .map_or(true, |ip| !ip.is_loopback()),
        None => false,
    }
}

/// Loads the config; returns it with an optional warning (loose permissions, risky transport).
///
/// An explicit path must exist. The default path may be absent when the environment provides
/// the settings (`GARAGE_ADMIN_URL`, `GARAGE_ADMIN_TOKEN`).
pub fn load(explicit: Option<&Path>) -> Result<(Config, Option<String>)> {
    load_with(explicit, default_path(), |key| std::env::var(key).ok())
}

fn read_file(path: &Path) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read config file {}", path.display()))?;
    Config::from_yaml(&content).with_context(|| format!("in config file {}", path.display()))
}

fn load_with(
    explicit: Option<&Path>,
    default: Option<PathBuf>,
    env: impl Fn(&str) -> Option<String>,
) -> Result<(Config, Option<String>)> {
    let path = explicit
        .map(Path::to_path_buf)
        .or_else(|| default.filter(|p| p.exists()));
    let mut config = match &path {
        Some(path) => read_file(path)?,
        None => Config::default(),
    };
    // Read before the environment override: only a secret stored in the file needs chmod 600.
    let token_in_file = !config.admin_token.expose().trim().is_empty();
    config.apply_env(env);
    config.check_required()?;
    let permissions = path
        .as_deref()
        .filter(|_| token_in_file)
        .and_then(permissions_warning);
    let warnings: Vec<String> = permissions
        .into_iter()
        .chain(config.security_warnings())
        .collect();
    let warning = (!warnings.is_empty()).then(|| warnings.join("; "));
    Ok((config, warning))
}

#[cfg(unix)]
fn permissions_warning(path: &Path) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).ok()?.permissions().mode();
    (mode & 0o077 != 0).then(|| {
        format!(
            "{} is readable by others (mode {:o}), run chmod 600",
            path.display(),
            mode & 0o777
        )
    })
}

#[cfg(not(unix))]
fn permissions_warning(_path: &Path) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_yaml() {
        let c = Config::from_yaml("admin_url: http://x:3903\nadmin_token: t\nrefresh_secs: 2\n")
            .unwrap();
        assert_eq!(c.admin_url, "http://x:3903");
        assert_eq!(c.refresh_secs(), 2);
    }

    fn write_config(mode: u32) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        std::fs::write(&path, "admin_url: https://h:3903\nadmin_token: tok\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        (dir, path)
    }

    #[test]
    fn loads_explicit_path_without_warning() {
        let (_dir, path) = write_config(0o600);
        let (config, warning) = load(Some(&path)).unwrap();
        assert_eq!(config.admin_token.expose(), "tok");
        assert!(warning.is_none());
    }

    #[test]
    fn warns_when_file_is_world_readable() {
        let (_dir, path) = write_config(0o644);
        let (_, warning) = load(Some(&path)).unwrap();
        assert!(warning.unwrap().contains("chmod 600"));
    }

    #[test]
    fn missing_or_invalid_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = load(Some(&dir.path().join("nope.yaml"))).unwrap_err();
        assert!(err.to_string().contains("cannot read config file"));

        let bad = dir.path().join("bad.yaml");
        std::fs::write(&bad, "admin_url: [").unwrap();
        let err = load(Some(&bad)).unwrap_err();
        assert!(format!("{err:#}").contains("in config file"));
    }

    #[test]
    fn default_path_ends_with_app_config() {
        if let Some(path) = default_path() {
            assert!(path.ends_with("garage-tui/config.yaml"));
        }
    }

    #[test]
    fn default_refresh_and_validation() {
        let c = Config::from_yaml("admin_url: u\nadmin_token: t\n").unwrap();
        assert_eq!(c.refresh_secs(), DEFAULT_REFRESH_SECS);
        assert!(
            Config::from_yaml("admin_url: ''\nadmin_token: t\n")
                .unwrap()
                .check_required()
                .is_err()
        );
        assert!(
            Config::from_yaml("admin_url: u\n")
                .unwrap()
                .check_required()
                .is_err()
        );
    }

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            vars.iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    fn write_yaml(dir: &tempfile::TempDir, yaml: &str, mode: u32) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.path().join("config.yaml");
        std::fs::write(&path, yaml).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    #[test]
    fn env_token_overrides_the_file() {
        let (_dir, path) = write_config(0o600);
        let vars = [(ENV_ADMIN_TOKEN, "from-env")];
        let (config, warning) = load_with(Some(&path), None, env(&vars)).unwrap();
        assert_eq!(config.admin_token.expose(), "from-env");
        assert_eq!(config.admin_url, "https://h:3903");
        assert_eq!(config.settings(None).token_source, TOKEN_SOURCE_ENV);
        assert!(warning.is_none());
        assert!(!format!("{config:?}").contains("from-env"));
    }

    #[test]
    fn env_token_with_url_from_a_file_without_secret() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_yaml(&dir, "admin_url: https://h:3903\n", 0o644);
        let vars = [(ENV_ADMIN_TOKEN, "from-env")];
        let (config, warning) = load_with(Some(&path), None, env(&vars)).unwrap();
        assert_eq!(config.admin_url, "https://h:3903");
        assert_eq!(config.admin_token.expose(), "from-env");
        assert!(warning.is_none(), "no secret in the file, no chmod warning");
    }

    #[test]
    fn world_readable_file_with_token_still_warns_despite_env() {
        let (_dir, path) = write_config(0o644);
        let vars = [(ENV_ADMIN_TOKEN, "from-env")];
        let (_, warning) = load_with(Some(&path), None, env(&vars)).unwrap();
        assert!(warning.unwrap().contains("chmod 600"));
    }

    #[test]
    fn env_alone_replaces_a_missing_default_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.yaml");
        let vars = [
            (ENV_ADMIN_URL, "http://127.0.0.1:3903"),
            (ENV_ADMIN_TOKEN, "t"),
        ];
        let (config, warning) = load_with(None, Some(missing), env(&vars)).unwrap();
        assert_eq!(config.admin_url, "http://127.0.0.1:3903");
        assert!(warning.is_none());
        assert_eq!(config.settings(None).config_path, None);
    }

    #[test]
    fn missing_explicit_file_is_an_error_even_with_env() {
        let dir = tempfile::tempdir().unwrap();
        let vars = [(ENV_ADMIN_URL, "u"), (ENV_ADMIN_TOKEN, "t")];
        let err = load_with(Some(&dir.path().join("nope.yaml")), None, env(&vars)).unwrap_err();
        assert!(err.to_string().contains("cannot read config file"));
    }

    #[test]
    fn missing_token_or_url_is_reported_and_blank_env_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_yaml(&dir, "admin_url: u\n", 0o600);
        let vars = [(ENV_ADMIN_TOKEN, "  ")];
        let err = load_with(Some(&path), None, env(&vars)).unwrap_err();
        assert!(err.to_string().contains("GARAGE_ADMIN_TOKEN"));

        let err = load_with(None, None, env(&[(ENV_ADMIN_TOKEN, "t")])).unwrap_err();
        assert!(err.to_string().contains("GARAGE_ADMIN_URL"));
    }

    #[test]
    fn warns_on_cleartext_remote_and_disabled_tls_verification() {
        let warnings = |url: &str, extra: &str| {
            Config::from_yaml(&format!("admin_url: {url}\nadmin_token: t\n{extra}"))
                .unwrap()
                .security_warnings()
        };
        for ok in [
            "http://127.0.0.1:3903",
            "http://localhost:3903",
            "http://[::1]:3903",
            "https://garage.example:3903",
        ] {
            assert!(warnings(ok, "").is_empty(), "{ok}");
        }
        assert_eq!(warnings("http://10.0.0.5:3903", "").len(), 1);
        assert_eq!(warnings("http://garage.lan:3903", "").len(), 1);

        let w = warnings("https://garage:3903", "insecure_skip_tls_verify: true\n");
        assert_eq!(w, ["TLS certificate verification is disabled"]);
        let c = Config::from_yaml("admin_url: u\nadmin_token: t\ninsecure_skip_tls_verify: true\n")
            .unwrap();
        assert!(c.settings(None).insecure_skip_tls_verify);
        assert!(!format!("{c:?}").contains("admin_token: \"t\""));
    }

    #[test]
    fn warn_thresholds_default_custom_and_invalid() {
        let c = Config::from_yaml("admin_url: u\nadmin_token: t\n").unwrap();
        assert_eq!(c.quota_warn_percent(), 85.0);
        assert_eq!(c.disk_warn_percent(), 90.0);

        let c = Config::from_yaml(
            "admin_url: u\nadmin_token: t\nquota_warn_percent: 50\ndisk_warn_percent: 80.5\n",
        )
        .unwrap();
        assert_eq!(c.quota_warn_percent(), 50.0);
        assert_eq!(c.settings(None).disk_warn_percent, 80.5);

        for bad in ["0", "101", "-3"] {
            let yaml = format!("admin_url: u\nadmin_token: t\nquota_warn_percent: {bad}\n");
            assert!(
                Config::from_yaml(&yaml).is_err(),
                "{bad} should be rejected"
            );
        }
    }
}
