//! The on-disk configuration file.
//!
//! Every field is optional. Hoskinator runs with no configuration file at all, so absence is the
//! ordinary state rather than an error.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// User settings, read from `config.toml` in the platform configuration directory.
///
/// Unknown keys are rejected rather than ignored.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Where Hoskinator keeps its data. Overridden by `HOSKINATOR_HOME` env var.
    pub home: Option<PathBuf>,
    /// The configured standard Git worktree for the user's resume.
    pub resume_repo: Option<PathBuf>,
    /// The id of the Google Sheet the application tracker syncs from.
    pub applications_sheet: Option<String>,
    /// Overridden by `ANTHROPIC_API_KEY` env var. Plaintext on disk, readable only by the file's
    /// owner (`remember_anthropic_api_key` sets the file to mode 0600 on Unix).
    pub anthropic_api_key: Option<String>,
}

/// A configuration file exists but could not be used, or could not be written.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read the config file at {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("the config file at {path} is not valid TOML")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("could not write the config file at {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl Config {
    /// Reads configuration from `path`.
    ///
    /// A file that does not exist yields [`Config::default`]. Reports any other read failure.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(source) => {
                return Err(ConfigError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }
}

/// Rewrites `key`'s line in `config_path` to `value`, or removes the line when `value` is
/// `None`. Keeps every other line as is, and restricts the file to its owner on Unix.
pub(crate) fn remember_key(
    config_path: &Path,
    key: &str,
    value: Option<&str>,
) -> std::io::Result<()> {
    let existing = std::fs::read_to_string(config_path).unwrap_or_default();
    let kept: Vec<&str> = existing
        .lines()
        .filter(|line| !line.trim_start().starts_with(key))
        .collect();

    let mut written = kept.join("\n");
    if !written.is_empty() && !written.ends_with('\n') {
        written.push('\n');
    }
    if let Some(value) = value {
        written.push_str(&format!("{key} = \"{value}\"\n"));
    }

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(config_path, &written)?;
    restrict_to_owner(config_path);
    Ok(())
}

/// Writes or clears `anthropic_api_key` in the config file, keeping anything else already set.
pub fn remember_anthropic_api_key(
    config_path: &Path,
    key: Option<&str>,
) -> Result<(), ConfigError> {
    remember_key(config_path, "anthropic_api_key", key).map_err(|source| ConfigError::Write {
        path: config_path.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn restrict_to_owner(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = std::fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_config(dir: &TempDir, contents: &str) -> PathBuf {
        let path = dir.path().join("config.toml");
        std::fs::write(&path, contents).expect("writing the test config");
        path
    }

    #[test]
    fn a_missing_file_yields_the_default() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            Config::load(&dir.path().join("does-not-exist.toml")).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn an_empty_file_yields_the_default() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            Config::load(&write_config(&dir, "")).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn reads_configured_paths() {
        let dir = TempDir::new().unwrap();
        let config = Config::load(&write_config(
            &dir,
            "home = \"/srv/hoskinator\"\nresume_repo = \"/srv/resume\"\n",
        ))
        .unwrap();
        assert_eq!(config.home, Some(PathBuf::from("/srv/hoskinator")));
        assert_eq!(config.resume_repo, Some(PathBuf::from("/srv/resume")));
    }

    #[test]
    fn an_unknown_key_is_rejected() {
        let dir = TempDir::new().unwrap();
        assert!(matches!(
            Config::load(&write_config(&dir, "hom = \"/srv/hoskinator\"\n")).unwrap_err(),
            ConfigError::Parse { .. }
        ));
    }

    #[test]
    fn invalid_toml_is_reported_with_its_path() {
        let dir = TempDir::new().unwrap();
        let path = write_config(&dir, "home = \n");
        match Config::load(&path).unwrap_err() {
            ConfigError::Parse { path: reported, .. } => assert_eq!(reported, path),
            other => panic!("expected a parse error, got {other:?}"),
        }
    }

    #[test]
    fn remembering_the_key_writes_it_and_keeps_other_keys() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");
        std::fs::write(&config, "resume_repo = \"/srv/resume\"\n").unwrap();

        remember_anthropic_api_key(&config, Some("sk-ant-test")).unwrap();

        let loaded = Config::load(&config).unwrap();
        assert_eq!(loaded.resume_repo, Some(PathBuf::from("/srv/resume")));
        assert_eq!(loaded.anthropic_api_key, Some("sk-ant-test".to_string()));
    }

    #[test]
    fn remembering_again_replaces_the_previous_key() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");

        remember_anthropic_api_key(&config, Some("first")).unwrap();
        remember_anthropic_api_key(&config, Some("second")).unwrap();

        let written = std::fs::read_to_string(&config).unwrap();
        assert_eq!(written.matches("anthropic_api_key").count(), 1);
        assert_eq!(
            Config::load(&config).unwrap().anthropic_api_key,
            Some("second".to_string())
        );
    }

    #[test]
    fn remembering_none_clears_the_key() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");
        remember_anthropic_api_key(&config, Some("sk-ant-test")).unwrap();

        remember_anthropic_api_key(&config, None).unwrap();

        assert_eq!(Config::load(&config).unwrap().anthropic_api_key, None);
    }

    #[cfg(unix)]
    #[test]
    fn remembering_restricts_the_file_to_its_owner() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");

        remember_anthropic_api_key(&config, Some("sk-ant-test")).unwrap();

        let mode = std::fs::metadata(&config).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
