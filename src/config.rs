use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const DEFAULT_REFRESH_SECS: u64 = 5;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub admin_url: String,
    pub admin_token: String,
    #[serde(default)]
    pub refresh_secs: Option<u64>,
}

impl Config {
    pub fn refresh_secs(&self) -> u64 {
        self.refresh_secs.unwrap_or(DEFAULT_REFRESH_SECS).max(1)
    }

    pub fn from_yaml(content: &str) -> Result<Self> {
        let config: Config = serde_yaml::from_str(content).context("invalid YAML config")?;
        if config.admin_url.trim().is_empty() || config.admin_token.trim().is_empty() {
            bail!("admin_url and admin_token must not be empty");
        }
        Ok(config)
    }
}

pub fn default_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("garage-tui").join("config.yaml"))
}

/// Loads the config; returns it with an optional warning (e.g. loose permissions).
pub fn load(explicit: Option<&Path>) -> Result<(Config, Option<String>)> {
    let path = match explicit {
        Some(path) => path.to_path_buf(),
        None => default_path().context("cannot determine the default config directory")?,
    };
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read config file {}", path.display()))?;
    let config = Config::from_yaml(&content)
        .with_context(|| format!("in config file {}", path.display()))?;
    Ok((config, permissions_warning(&path)))
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
        std::fs::write(&path, "admin_url: http://h:3903\nadmin_token: tok\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        (dir, path)
    }

    #[test]
    fn loads_explicit_path_without_warning() {
        let (_dir, path) = write_config(0o600);
        let (config, warning) = load(Some(&path)).unwrap();
        assert_eq!(config.admin_token, "tok");
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
        assert!(Config::from_yaml("admin_url: ''\nadmin_token: t\n").is_err());
        assert!(Config::from_yaml("admin_url: u\n").is_err());
    }
}
