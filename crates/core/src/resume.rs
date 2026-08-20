//! Reading, writing, and validating the per-branch `resume.yaml` (ADR-0002).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::Value;

/// The resume file's fixed name within the repository working directory.
pub const FILENAME: &str = "resume.yaml";

#[derive(Debug, thiserror::Error)]
pub enum ResumeError {
    #[error("no resume.yaml at {path}")]
    NotFound { path: PathBuf },
    #[error("could not read {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Reads the raw text of `resume.yaml` from a repository's working directory.
pub fn read(repository_path: &Path) -> Result<String, ResumeError> {
    let path = repository_path.join(FILENAME);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Err(ResumeError::NotFound { path })
        }
        Err(source) => Err(ResumeError::Read { path, source }),
    }
}

/// rendercv's own emitted JSON Schema, vendored at [`SCHEMA_VERSION`].
pub const SCHEMA: &str = include_str!("../schema/rendercv-2.8-schema.json");

/// The rendercv version [`SCHEMA`] was vendored from.
pub const SCHEMA_VERSION: &str = "2.8";

fn validator() -> &'static jsonschema::Validator {
    static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| {
        let schema: Value =
            serde_json::from_str(SCHEMA).expect("the vendored schema is valid JSON");
        jsonschema::validator_for(&schema).expect("the vendored schema compiles")
    })
}

/// Validates a rendercv document (a `{ "cv": { ... } }` value) against [`SCHEMA`].
pub fn validate(document: &Value) -> Result<(), Vec<String>> {
    let errors: Vec<String> = validator()
        .iter_errors(document)
        .map(|error| format!("{error} at {}", error.instance_path()))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn an_empty_cv_document_is_accepted() {
        assert_eq!(validate(&json!({ "cv": {} })), Ok(()));
    }

    #[test]
    fn an_unknown_social_network_is_rejected() {
        let document = json!({
            "cv": { "social_networks": [{ "network": "Friendster", "username": "ada" }] }
        });

        assert!(validate(&document).is_err());
    }

    #[test]
    fn reading_returns_the_files_exact_text() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(FILENAME), "cv:\n  name: Ada # kept\n").unwrap();

        assert_eq!(read(dir.path()).unwrap(), "cv:\n  name: Ada # kept\n");
    }

    #[test]
    fn reading_a_missing_file_is_not_found() {
        let dir = TempDir::new().unwrap();

        assert!(matches!(
            read(dir.path()),
            Err(ResumeError::NotFound { .. })
        ));
    }
}
