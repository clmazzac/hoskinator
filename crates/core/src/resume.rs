//! Reading, writing, and validating the per-branch `resume.yaml` (ADR-0002).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use indexmap::IndexMap;
use serde_json::Value;

use crate::profile::Profile;

/// The resume file's fixed name within the repository working directory.
pub const FILENAME: &str = "resume.yaml";

/// The document key the Profile is injected under (rendercv's `cv:` header).
const CV_KEY: &str = "cv";

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
    #[error("could not write {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not valid YAML")]
    Parse {
        path: PathBuf,
        #[source]
        source: yamlpath::QueryError,
    },
    #[error("could not encode the Profile as YAML")]
    EncodeProfile(#[source] yaml_serde::Error),
    #[error("could not inject the Profile into {path}")]
    Patch {
        path: PathBuf,
        #[source]
        source: yamlpatch::Error,
    },
    #[error("the patched YAML at {path} could not be read back")]
    Decode {
        path: PathBuf,
        #[source]
        source: yaml_serde::Error,
    },
    #[error("the resulting resume.yaml would not validate against rendercv's schema: {0:?}")]
    Invalid(Vec<String>),
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

/// Writes `text` as `resume.yaml`, injecting the Profile into its `cv:` header and rejecting
/// the result if it would not validate against rendercv's schema. `text` need not already
/// exist on disk.
pub fn write(repository_path: &Path, text: String, profile: &Profile) -> Result<(), ResumeError> {
    let path = repository_path.join(FILENAME);
    // A blank document has no node yamlpath can route into at all, so an empty write is seeded
    // with a bare `cv:` key rather than left to fail inside the patch step.
    let seed = if text.trim().is_empty() {
        format!("{CV_KEY}: {{}}\n")
    } else {
        text
    };
    let document = yamlpath::Document::new(seed).map_err(|source| ResumeError::Parse {
        path: path.clone(),
        source,
    })?;

    let updates = profile_updates(profile)?;

    let patch = yamlpatch::Patch {
        route: yamlpath::Route::default(),
        operation: yamlpatch::Op::MergeInto {
            key: CV_KEY.to_string(),
            updates,
        },
    };
    let patched = yamlpatch::apply_yaml_patches(&document, std::slice::from_ref(&patch)).map_err(
        |source| ResumeError::Patch {
            path: path.clone(),
            source,
        },
    )?;

    let as_json: Value =
        yaml_serde::from_str(patched.source()).map_err(|source| ResumeError::Decode {
            path: path.clone(),
            source,
        })?;
    validate(&as_json).map_err(ResumeError::Invalid)?;

    std::fs::write(&path, patched.source()).map_err(|source| ResumeError::Write { path, source })
}

/// The Profile's fields the way rendercv expects them, with unset fields left out.
fn profile_updates(profile: &Profile) -> Result<IndexMap<String, yaml_serde::Value>, ResumeError> {
    let encoded = yaml_serde::to_value(profile).map_err(ResumeError::EncodeProfile)?;
    let yaml_serde::Value::Mapping(fields) = encoded else {
        unreachable!("Profile serialises as a mapping")
    };

    Ok(fields
        .into_iter()
        .filter(|(_, value)| !is_unset(value))
        .map(|(key, value)| {
            let yaml_serde::Value::String(key) = key else {
                unreachable!("Profile's field names are strings")
            };
            (key, value)
        })
        .collect())
}

/// Whether a Profile field carries nothing worth injecting.
fn is_unset(value: &yaml_serde::Value) -> bool {
    matches!(value, yaml_serde::Value::Null)
        || matches!(value, yaml_serde::Value::Sequence(items) if items.is_empty())
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
    use crate::profile::{SocialNetwork, SocialNetworkName};
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

    fn populated_profile() -> Profile {
        Profile {
            name: Some("Ada Lovelace".into()),
            headline: Some("Mathematician".into()),
            ..Profile::default()
        }
    }

    #[test]
    fn writing_injects_the_profile_and_keeps_the_rest_untouched() {
        let dir = TempDir::new().unwrap();
        let text = "cv:\n  name: Old Name # kept comment\n  sections:\n    Experience: []\n";

        write(dir.path(), text.into(), &populated_profile()).unwrap();

        let written = read(dir.path()).unwrap();
        assert!(written.contains("name: Ada Lovelace"));
        assert!(written.contains("headline: Mathematician"));
        assert!(written.contains("# kept comment"));
        assert!(written.contains("Experience: []"));
    }

    #[test]
    fn writing_creates_a_missing_cv_header() {
        let dir = TempDir::new().unwrap();

        write(
            dir.path(),
            "design:\n  theme: sb2nov\n".into(),
            &populated_profile(),
        )
        .unwrap();

        let written = read(dir.path()).unwrap();
        assert!(written.contains("name: Ada Lovelace"));
        assert!(written.contains("theme: sb2nov"));
    }

    #[test]
    fn writing_an_unset_profile_adds_no_null_fields() {
        let dir = TempDir::new().unwrap();

        write(
            dir.path(),
            "cv:\n  name: Someone\n".into(),
            &Profile::default(),
        )
        .unwrap();

        let written = read(dir.path()).unwrap();
        assert!(!written.contains("null"));
    }

    #[test]
    fn writing_a_valid_document_round_trips_through_validation() {
        let dir = TempDir::new().unwrap();

        write(
            dir.path(),
            "cv:\n  name: Someone\n".into(),
            &populated_profile(),
        )
        .unwrap();

        let written = read(dir.path()).unwrap();
        let document: Value = yaml_serde::from_str(&written).unwrap();
        assert_eq!(validate(&document), Ok(()));
    }

    #[test]
    fn writing_bootstraps_a_missing_resume_yaml_from_blank_text() {
        let dir = TempDir::new().unwrap();
        assert!(matches!(
            read(dir.path()),
            Err(ResumeError::NotFound { .. })
        ));

        write(dir.path(), "".into(), &populated_profile()).unwrap();

        let written = read(dir.path()).unwrap();
        let document: Value = yaml_serde::from_str(&written).unwrap();
        assert_eq!(document["cv"]["name"], "Ada Lovelace");
        assert_eq!(validate(&document), Ok(()));
    }

    #[test]
    fn writing_twice_keeps_a_sequence_valued_profile_field_intact() {
        let dir = TempDir::new().unwrap();
        let profile = Profile {
            social_networks: vec![SocialNetwork {
                network: SocialNetworkName::GitHub,
                username: "ada".into(),
            }],
            ..populated_profile()
        };

        write(dir.path(), "cv:\n  name: Someone\n".into(), &profile).unwrap();
        let once = read(dir.path()).unwrap();

        write(dir.path(), once, &profile).unwrap();

        let twice = read(dir.path()).unwrap();
        let document: Value = yaml_serde::from_str(&twice).unwrap();
        assert_eq!(document["cv"]["social_networks"][0]["username"], "ada");
        assert_eq!(validate(&document), Ok(()));
    }
}
