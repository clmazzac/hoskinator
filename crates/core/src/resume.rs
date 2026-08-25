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

/// The `cv:` key holding the resume's sections.
const SECTIONS_KEY: &str = "sections";

/// The entry key holding an entry's placed wordings.
const HIGHLIGHTS_KEY: &str = "highlights";

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
    #[error("section {section} has no entry at index {index}")]
    NoSuchEntry { section: String, index: usize },
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

/// One entry of a resume section, at the position it sits in the file.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResumeEntry {
    /// Where the entry sits in its section's list, and how a write addresses it.
    pub index: usize,
    /// The entry's own keys, as rendercv holds them, minus `highlights`.
    pub fields: Value,
    /// The wordings placed on this entry.
    pub highlights: Vec<String>,
}

/// One section of a resume, named as the file names it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResumeSection {
    pub name: String,
    pub entries: Vec<ResumeEntry>,
}

/// The `cv.sections` key, read as structure rather than text.
///
/// Sections a human wrote by hand appear alongside ones the engine placed; the file is the only
/// record of what a resume holds (ADR-0001).
pub fn outline(repository_path: &Path) -> Result<Vec<ResumeSection>, ResumeError> {
    let path = repository_path.join(FILENAME);
    let text = read(repository_path)?;
    // Walked as YAML rather than JSON: `yaml_serde::Mapping` keeps insertion order, and a
    // resume's sections render in the order the file lists them.
    let document: yaml_serde::Value =
        yaml_serde::from_str(&text).map_err(|source| ResumeError::Decode { path, source })?;

    let Some(sections) = document
        .get(CV_KEY)
        .and_then(|cv| cv.get(SECTIONS_KEY))
        .and_then(|sections| sections.as_mapping())
    else {
        return Ok(Vec::new());
    };

    Ok(sections
        .iter()
        .filter_map(|(name, entries)| {
            Some(ResumeSection {
                name: name.as_str()?.to_string(),
                entries: entries
                    .as_sequence()
                    .map(|entries| {
                        entries
                            .iter()
                            .enumerate()
                            .map(|(index, entry)| read_entry(index, entry))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect())
}

fn read_entry(index: usize, entry: &yaml_serde::Value) -> ResumeEntry {
    let highlights = entry
        .get(HIGHLIGHTS_KEY)
        .and_then(|value| value.as_sequence())
        .map(|items| {
            items
                .iter()
                .map(|item| match item.as_str() {
                    Some(text) => text.to_string(),
                    None => yaml_serde::to_string(item)
                        .unwrap_or_default()
                        .trim()
                        .into(),
                })
                .collect()
        })
        .unwrap_or_default();

    let mut fields = entry.clone();
    if let Some(mapping) = fields.as_mapping_mut() {
        mapping.remove(HIGHLIGHTS_KEY);
    }

    ResumeEntry {
        index,
        fields: serde_json::to_value(&fields).unwrap_or(Value::Null),
        highlights,
    }
}

/// Appends one wording to an entry's `highlights`, creating the list if the entry has none.
///
/// Writes only the wording given. Nothing is matched against the Master Store, deduplicated, or
/// reconciled (ADR-0001) — placing a wording already present adds it again.
pub fn place_bullet(
    repository_path: &Path,
    section: &str,
    entry_index: usize,
    text: String,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let (_, document) = load(repository_path)?;
    let entry_route = entry_route(section, entry_index);
    require(&document, &entry_route, section, entry_index)?;

    let highlights_route = entry_route.clone().with_key(HIGHLIGHTS_KEY);
    let patch = if document.query_exists(&highlights_route) {
        yamlpatch::Patch {
            route: highlights_route,
            operation: yamlpatch::Op::Append {
                value: yaml_serde::Value::String(text),
            },
        }
    } else {
        yamlpatch::Patch {
            route: entry_route,
            operation: yamlpatch::Op::Add {
                key: HIGHLIGHTS_KEY.to_string(),
                value: yaml_serde::Value::Sequence(vec![yaml_serde::Value::String(text)]),
            },
        }
    };

    apply(repository_path, &document, patch, profile)
}

/// Places an Entry's fields at the end of a section, creating the section if the resume has none.
///
/// Drop order is resume order: an Entry lands after the ones already placed. Its fields are copied
/// from the Master Store, and no reference back to it is written (ADR-0001).
pub fn place_entry(
    repository_path: &Path,
    section: &str,
    fields: Value,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let (_, document) = load(repository_path)?;
    let entry = yaml_serde::to_value(&fields).map_err(ResumeError::EncodeProfile)?;

    let sections_route = yamlpath::Route::default()
        .with_key(CV_KEY)
        .with_key(SECTIONS_KEY);
    let section_route = sections_route.with_key(section);

    let patch = if document.query_exists(&section_route) {
        yamlpatch::Patch {
            route: section_route,
            operation: yamlpatch::Op::Append { value: entry },
        }
    } else if document.query_exists(&sections_route) {
        yamlpatch::Patch {
            route: sections_route,
            operation: yamlpatch::Op::Add {
                key: section.to_string(),
                value: yaml_serde::Value::Sequence(vec![entry]),
            },
        }
    } else {
        let mut sections = yaml_serde::Mapping::new();
        sections.insert(
            yaml_serde::Value::String(section.to_string()),
            yaml_serde::Value::Sequence(vec![entry]),
        );
        yamlpatch::Patch {
            route: yamlpath::Route::default().with_key(CV_KEY),
            operation: yamlpatch::Op::Add {
                key: SECTIONS_KEY.to_string(),
                value: yaml_serde::Value::Mapping(sections),
            },
        }
    };

    apply(repository_path, &document, patch, profile)
}

/// Removes one entry from a section, with everything it holds.
pub fn remove_entry(
    repository_path: &Path,
    section: &str,
    entry_index: usize,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let (_, document) = load(repository_path)?;
    let route = entry_route(section, entry_index);
    require(&document, &route, section, entry_index)?;

    apply(
        repository_path,
        &document,
        yamlpatch::Patch {
            route,
            operation: yamlpatch::Op::Remove,
        },
        profile,
    )
}

/// Removes one wording from an entry's `highlights`.
pub fn remove_bullet(
    repository_path: &Path,
    section: &str,
    entry_index: usize,
    highlight_index: usize,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let (_, document) = load(repository_path)?;
    let route = entry_route(section, entry_index)
        .with_key(HIGHLIGHTS_KEY)
        .with_key(highlight_index);
    require(&document, &route, section, entry_index)?;

    apply(
        repository_path,
        &document,
        yamlpatch::Patch {
            route,
            operation: yamlpatch::Op::Remove,
        },
        profile,
    )
}

/// Replaces one field of a resume entry, adding it when the entry has no such key.
///
/// The write shape a one-line entry needs: its elements live inside a single comma-separated
/// string, so removing or reordering one rewrites the field rather than editing a list.
///
/// **Only sound against a block-style entry.** A section the engine created renders as a flow
/// mapping, where a replacement value holding a comma splits the mapping into further keys. The
/// schema check in [`write`] catches that and rejects the write, so the file is never corrupted,
/// but the edit fails. See `docs/decisions/resume.md`.
pub fn set_entry_field(
    repository_path: &Path,
    section: &str,
    entry_index: usize,
    key: &str,
    value: Value,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let (_, document) = load(repository_path)?;
    let route = entry_route(section, entry_index);
    require(&document, &route, section, entry_index)?;

    let encoded = yaml_serde::to_value(&value).map_err(ResumeError::EncodeProfile)?;
    let field_route = route.clone().with_key(key);
    let patch = if document.query_exists(&field_route) {
        yamlpatch::Patch {
            route: field_route,
            operation: yamlpatch::Op::Replace(encoded),
        }
    } else {
        yamlpatch::Patch {
            route,
            operation: yamlpatch::Op::Add {
                key: key.to_string(),
                value: encoded,
            },
        }
    };

    apply(repository_path, &document, patch, profile)
}

fn entry_route<'a>(section: &'a str, entry_index: usize) -> yamlpath::Route<'a> {
    yamlpath::Route::default()
        .with_key(CV_KEY)
        .with_key(SECTIONS_KEY)
        .with_key(section)
        .with_key(entry_index)
}

fn require(
    document: &yamlpath::Document,
    route: &yamlpath::Route,
    section: &str,
    index: usize,
) -> Result<(), ResumeError> {
    if document.query_exists(route) {
        return Ok(());
    }
    Err(ResumeError::NoSuchEntry {
        section: section.to_string(),
        index,
    })
}

fn load(repository_path: &Path) -> Result<(PathBuf, yamlpath::Document), ResumeError> {
    let path = repository_path.join(FILENAME);
    let current = read(repository_path)?;
    let document = yamlpath::Document::new(current).map_err(|source| ResumeError::Parse {
        path: path.clone(),
        source,
    })?;
    Ok((path, document))
}

/// Applies one patch and writes the result back through [`write`], so every edit is validated.
fn apply(
    repository_path: &Path,
    document: &yamlpath::Document,
    patch: yamlpatch::Patch,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let patched = yamlpatch::apply_yaml_patches(document, std::slice::from_ref(&patch)).map_err(
        |source| ResumeError::Patch {
            path: repository_path.join(FILENAME),
            source,
        },
    )?;

    write(repository_path, patched.source().to_string(), profile)
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

    const SAMPLE: &str = "cv:\n  name: Someone\n  sections:\n    # kept\n    Experience:\n      - company: Helio\n        position: Engineer\n        highlights:\n          - Did a thing.\n      - company: Ravensmoor\n        position: Engineer\n";

    fn seeded(text: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(FILENAME), text).unwrap();
        dir
    }

    #[test]
    fn the_outline_reads_sections_entries_and_highlights() {
        let dir = seeded(SAMPLE);

        let outline = outline(dir.path()).unwrap();

        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].name, "Experience");
        assert_eq!(outline[0].entries.len(), 2);
        assert_eq!(outline[0].entries[0].index, 0);
        assert_eq!(outline[0].entries[0].highlights, ["Did a thing."]);
        assert_eq!(outline[0].entries[1].highlights, Vec::<String>::new());
    }

    #[test]
    fn the_outline_keeps_highlights_out_of_the_fields() {
        let dir = seeded(SAMPLE);

        let outline = outline(dir.path()).unwrap();

        assert_eq!(outline[0].entries[0].fields["company"], "Helio");
        assert!(outline[0].entries[0].fields.get("highlights").is_none());
    }

    #[test]
    fn the_outline_keeps_the_files_section_order() {
        let dir = seeded(
            "cv:\n  sections:\n    Experience:\n      - company: A\n    Education:\n      - institution: B\n    Awards:\n      - name: C\n",
        );

        let names: Vec<String> = outline(dir.path())
            .unwrap()
            .into_iter()
            .map(|section| section.name)
            .collect();

        assert_eq!(names, ["Experience", "Education", "Awards"]);
    }

    #[test]
    fn a_document_with_no_sections_has_an_empty_outline() {
        let dir = seeded("cv:\n  name: Someone\n");

        assert_eq!(outline(dir.path()).unwrap(), Vec::new());
    }

    #[test]
    fn placing_appends_to_an_existing_highlights_list() {
        let dir = seeded(SAMPLE);

        place_bullet(
            dir.path(),
            "Experience",
            0,
            "Did another.".into(),
            &populated_profile(),
        )
        .unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(
            outline[0].entries[0].highlights,
            ["Did a thing.", "Did another."]
        );
    }

    #[test]
    fn placing_creates_a_highlights_list_when_the_entry_has_none() {
        let dir = seeded(SAMPLE);

        place_bullet(
            dir.path(),
            "Experience",
            1,
            "First one.".into(),
            &populated_profile(),
        )
        .unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(outline[0].entries[1].highlights, ["First one."]);
    }

    #[test]
    fn placing_the_same_wording_twice_adds_it_twice() {
        let dir = seeded(SAMPLE);
        let profile = populated_profile();

        place_bullet(dir.path(), "Experience", 0, "Repeated.".into(), &profile).unwrap();
        place_bullet(dir.path(), "Experience", 0, "Repeated.".into(), &profile).unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(
            outline[0].entries[0]
                .highlights
                .iter()
                .filter(|h| *h == "Repeated.")
                .count(),
            2
        );
    }

    #[test]
    fn placing_leaves_the_rest_of_the_document_alone() {
        let dir = seeded(SAMPLE);

        place_bullet(
            dir.path(),
            "Experience",
            0,
            "Did another.".into(),
            &populated_profile(),
        )
        .unwrap();

        let written = read(dir.path()).unwrap();
        assert!(written.contains("# kept"));
        assert!(written.contains("company: Ravensmoor"));
    }

    #[test]
    fn placing_into_a_missing_entry_is_rejected() {
        let dir = seeded(SAMPLE);

        assert!(matches!(
            place_bullet(
                dir.path(),
                "Experience",
                9,
                "Nope.".into(),
                &populated_profile()
            ),
            Err(ResumeError::NoSuchEntry { .. })
        ));
    }

    #[test]
    fn placing_an_entry_appends_it_to_the_section() {
        let dir = seeded(SAMPLE);

        place_entry(
            dir.path(),
            "Experience",
            json!({ "company": "Quillfeather", "position": "Intern" }),
            &populated_profile(),
        )
        .unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(outline[0].entries.len(), 3);
        assert_eq!(outline[0].entries[2].fields["company"], "Quillfeather");
    }

    #[test]
    fn placing_an_entry_creates_a_section_the_resume_lacks() {
        let dir = seeded(SAMPLE);

        place_entry(
            dir.path(),
            "Awards",
            json!({ "name": "Best in Show" }),
            &populated_profile(),
        )
        .unwrap();

        let outline = outline(dir.path()).unwrap();
        let awards = outline.iter().find(|s| s.name == "Awards").unwrap();
        assert_eq!(awards.entries[0].fields["name"], "Best in Show");
    }

    #[test]
    fn drop_order_is_resume_order() {
        let dir = seeded("cv:\n  name: Someone\n");
        let profile = populated_profile();

        for company in ["First", "Second", "Third"] {
            place_entry(
                dir.path(),
                "Experience",
                json!({ "company": company, "position": "Engineer" }),
                &profile,
            )
            .unwrap();
        }

        let placed: Vec<String> = outline(dir.path()).unwrap()[0]
            .entries
            .iter()
            .map(|entry| entry.fields["company"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(placed, ["First", "Second", "Third"]);
    }

    #[test]
    fn removing_an_entry_takes_only_that_entry() {
        let dir = seeded(SAMPLE);

        remove_entry(dir.path(), "Experience", 0, &populated_profile()).unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(outline[0].entries.len(), 1);
        assert_eq!(outline[0].entries[0].fields["company"], "Ravensmoor");
    }

    #[test]
    fn removing_a_wording_leaves_the_entry() {
        let dir = seeded(SAMPLE);

        remove_bullet(dir.path(), "Experience", 0, 0, &populated_profile()).unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(outline[0].entries[0].highlights, Vec::<String>::new());
        assert_eq!(outline[0].entries[0].fields["company"], "Helio");
    }

    #[test]
    fn removing_keeps_the_rest_of_the_document() {
        let dir = seeded(SAMPLE);

        remove_bullet(dir.path(), "Experience", 0, 0, &populated_profile()).unwrap();

        let written = read(dir.path()).unwrap();
        assert!(written.contains("# kept"));
        assert!(written.contains("company: Ravensmoor"));
    }

    #[test]
    fn removing_an_entry_that_is_not_there_is_rejected() {
        let dir = seeded(SAMPLE);

        assert!(matches!(
            remove_entry(dir.path(), "Experience", 9, &populated_profile()),
            Err(ResumeError::NoSuchEntry { .. })
        ));
    }

    #[test]
    fn setting_a_field_rewrites_it_in_place() {
        let dir = seeded(
            "cv:\n  sections:\n    Skills:\n      - label: Languages\n        details: Rust, Go\n",
        );

        set_entry_field(
            dir.path(),
            "Skills",
            0,
            "details",
            json!("Go, Rust, Python"),
            &populated_profile(),
        )
        .unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(outline[0].entries[0].fields["details"], "Go, Rust, Python");
        assert_eq!(outline[0].entries[0].fields["label"], "Languages");
    }
}
