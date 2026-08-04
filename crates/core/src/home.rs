//! Resolving Hoskinator's home directory.
//!
//! Hoskinator is not a per-directory tool the way `git` is. A command run from anywhere must reach
//! the same data, so the home directory is resolved from the environment and the user's
//! configuration — never from the current working directory.
//!
//! Precedence, highest first:
//!
//! 1. the `HOSKINATOR_HOME` environment variable
//! 2. the `home` key in the configuration file
//! 3. the platform data directory (`~/.local/share/hoskinator` on Linux)
//!
//! Only the Master Store lives here. The rendercv git repository is located separately.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::config::{Config, ConfigError};

/// Environment variable that overrides the home directory.
pub const HOME_ENV: &str = "HOSKINATOR_HOME";

/// Directory within Home holding the store and anything written beside it.
const STORE_DIR: &str = "store";

/// Name of the libSQL database within the store directory.
const STORE_FILE: &str = "hoskinator.db";

/// Name of the configuration file within the platform configuration directory.
const CONFIG_FILE: &str = "config.toml";

/// The home directory could not be determined.
#[derive(Debug, thiserror::Error)]
pub enum HomeError {
    #[error("{HOME_ENV} is set but empty; either unset it or give it a path")]
    EmptyEnvVar,

    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error("no platform data directory is available; set {HOME_ENV} to choose one")]
    NoPlatformDataDir,
}

/// The directory holding Hoskinator's data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    root: PathBuf,
}

impl Home {
    /// Reads configuration from its platform-defined location.
    pub fn config() -> Result<Config, ConfigError> {
        match config_file_path() {
            Some(path) => Config::load(&path),
            None => Ok(Config::default()),
        }
    }

    /// Resolves the home directory from the environment and supplied configuration.
    pub fn resolve_with_config(config: &Config) -> Result<Self, HomeError> {
        Self::resolve_from(std::env::var_os(HOME_ENV), config, platform_data_dir())
    }

    /// Resolves the home directory from the environment and the configuration file.
    pub fn resolve() -> Result<Self, HomeError> {
        let config = Self::config()?;
        Self::resolve_with_config(&config)
    }

    /// The resolution rule itself, with every input supplied by the caller.
    ///
    /// Separate from [`Home::resolve`] so it can be tested without mutating the process
    /// environment, which is global and would make tests race one another.
    fn resolve_from(
        env: Option<OsString>,
        config: &Config,
        platform_data_dir: Option<PathBuf>,
    ) -> Result<Self, HomeError> {
        if let Some(from_env) = env {
            // An empty value is a mistake rather than a request for the default: silently falling
            // through would send data somewhere the user did not intend.
            if from_env.is_empty() {
                return Err(HomeError::EmptyEnvVar);
            }
            return Ok(Self {
                root: PathBuf::from(from_env),
            });
        }

        if let Some(from_config) = &config.home {
            return Ok(Self {
                root: from_config.clone(),
            });
        }

        platform_data_dir
            .map(|root| Self { root })
            .ok_or(HomeError::NoPlatformDataDir)
    }

    /// The home directory itself.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Directory holding the store.
    ///
    /// The database is not alone in here: SQLite writes `-wal` and `-shm` files beside it, and
    /// backups or a second database may join them later.
    pub fn store_dir(&self) -> PathBuf {
        self.root.join(STORE_DIR)
    }

    /// Path to the libSQL database. Resolution does not create it; the store does.
    pub fn store_path(&self) -> PathBuf {
        self.store_dir().join(STORE_FILE)
    }
}

/// Hoskinator's data directory on this platform, if the OS provides one.
fn platform_data_dir() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.data_dir().to_path_buf())
}

/// The configuration file path, if the OS provides a configuration directory.
fn config_file_path() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.config_dir().join(CONFIG_FILE))
}

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", "hoskinator")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_naming(path: &str) -> Config {
        Config {
            home: Some(PathBuf::from(path)),
            resume_repo: None,
        }
    }

    fn platform(path: &str) -> Option<PathBuf> {
        Some(PathBuf::from(path))
    }

    #[test]
    fn the_env_var_outranks_everything_else() {
        let home = Home::resolve_from(
            Some(OsString::from("/from/env")),
            &config_naming("/from/config"),
            platform("/from/platform"),
        )
        .unwrap();

        assert_eq!(home.root(), Path::new("/from/env"));
    }

    #[test]
    fn the_config_outranks_the_platform_default() {
        let home = Home::resolve_from(
            None,
            &config_naming("/from/config"),
            platform("/from/platform"),
        )
        .unwrap();

        assert_eq!(home.root(), Path::new("/from/config"));
    }

    #[test]
    fn falls_back_to_the_platform_default() {
        let home =
            Home::resolve_from(None, &Config::default(), platform("/from/platform")).unwrap();

        assert_eq!(home.root(), Path::new("/from/platform"));
    }

    #[test]
    fn an_empty_env_var_is_an_error_rather_than_a_fallback() {
        let error = Home::resolve_from(
            Some(OsString::new()),
            &config_naming("/from/config"),
            platform("/from/platform"),
        )
        .unwrap_err();

        assert!(matches!(error, HomeError::EmptyEnvVar), "got {error:?}");
    }

    #[test]
    fn reports_when_no_source_yields_a_directory() {
        let error = Home::resolve_from(None, &Config::default(), None).unwrap_err();

        assert!(
            matches!(error, HomeError::NoPlatformDataDir),
            "got {error:?}"
        );
    }

    #[test]
    fn the_platform_default_is_an_absolute_hoskinator_directory() {
        let Some(dir) = platform_data_dir() else {
            return; // No home directory on this machine; nothing to assert.
        };

        assert!(dir.is_absolute(), "{dir:?} should be absolute");
        assert_eq!(dir.file_name().unwrap(), "hoskinator", "in {dir:?}");
    }

    #[test]
    fn the_store_lives_in_its_own_directory_inside_home() {
        let home =
            Home::resolve_from(None, &Config::default(), platform("/data/hoskinator")).unwrap();

        assert_eq!(home.store_dir(), Path::new("/data/hoskinator/store"));
        assert_eq!(
            home.store_path(),
            Path::new("/data/hoskinator/store/hoskinator.db")
        );
    }
}
