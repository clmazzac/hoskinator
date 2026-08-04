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
}

/// A configuration file exists but could not be used.
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
}
