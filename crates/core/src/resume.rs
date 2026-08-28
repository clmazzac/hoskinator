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

/// The document key holding how the resume looks (ADR-0006, as amended).
const DESIGN_KEY: &str = "design";

/// The `design:` key naming the theme.
const THEME_KEY: &str = "theme";

/// The `design:` key holding page options.
const PAGE_KEY: &str = "page";

/// The `design.page` key showing the "Last updated in …" note.
const TOP_NOTE_KEY: &str = "show_top_note";

/// The themes rendercv ships. The picker offers these and nothing else, so every document the
/// engine writes stays inside the closed union the schema models (ADR-0006, as amended).
pub const THEMES: &[&str] = &[
    "classic",
    "ember",
    "engineeringclassic",
    "engineeringresumes",
    "harvard",
    "ink",
    "moderncv",
    "opal",
    "sb2nov",
];

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
    #[error("no section called {section}")]
    NoSuchSection { section: String },
    #[error("rendercv has no theme called {theme}")]
    UnknownTheme { theme: String },
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

    apply(repository_path, &document, &[patch], profile)
}

/// Builds the patches adding `section: value` under `cv.sections`, creating `sections:` as a
/// block mapping if the resume has none.
fn place_in_sections<'a>(
    document: &yamlpath::Document,
    sections_route: yamlpath::Route<'a>,
    section: &'a str,
    value: yaml_serde::Value,
) -> Vec<yamlpatch::Patch<'a>> {
    if document.query_exists(&sections_route) {
        vec![yamlpatch::Patch {
            route: sections_route,
            operation: yamlpatch::Op::Add {
                key: section.to_string(),
                value,
            },
        }]
    } else {
        // A key `Add` renders its value inline: without the `as_block` replace below, every
        // later route into any key nested inside `sections:` would fail to query at all, not
        // just an `Append` into the one section just added.
        let mut sections = yaml_serde::Mapping::new();
        sections.insert(yaml_serde::Value::String(section.to_string()), value);
        let create = yamlpatch::Patch {
            route: yamlpath::Route::default().with_key(CV_KEY),
            operation: yamlpatch::Op::Add {
                key: SECTIONS_KEY.to_string(),
                value: yaml_serde::Value::Mapping(sections.clone()),
            },
        };
        let as_block = yamlpatch::Patch {
            route: sections_route,
            operation: yamlpatch::Op::Replace(yaml_serde::Value::Mapping(sections)),
        };
        vec![create, as_block]
    }
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
    let (path, document) = load(repository_path)?;
    let entry = yaml_serde::to_value(&fields).map_err(ResumeError::EncodeProfile)?;

    let sections_route = yamlpath::Route::default()
        .with_key(CV_KEY)
        .with_key(SECTIONS_KEY);
    let section_route = sections_route.with_key(section);

    let parsed: yaml_serde::Value =
        yaml_serde::from_str(document.source()).map_err(|source| ResumeError::Decode {
            path: path.clone(),
            source,
        })?;
    let has_entries = parsed
        .get(CV_KEY)
        .and_then(|cv| cv.get(SECTIONS_KEY))
        .and_then(|sections| sections.get(section))
        .and_then(|value| value.as_sequence())
        .is_some_and(|entries| !entries.is_empty());

    if has_entries {
        let patch = yamlpatch::Patch {
            route: section_route,
            operation: yamlpatch::Op::Append { value: entry },
        };
        return apply(repository_path, &document, &[patch], profile);
    }

    // Whichever brought the section here — missing entirely, or placed empty by
    // `place_section` — a key `Add` and an empty sequence both render inline, and `yamlpatch`
    // refuses to `Append` into a flow sequence. Replacing it re-emits it as a block sequence,
    // which is what a hand-editable file wants and what later field writes need.
    let entries = yaml_serde::Value::Sequence(vec![entry]);
    let mut patches = Vec::new();
    if !document.query_exists(&section_route) {
        patches.extend(place_in_sections(
            &document,
            sections_route,
            section,
            entries.clone(),
        ));
    }
    patches.push(yamlpatch::Patch {
        route: section_route,
        operation: yamlpatch::Op::Replace(entries),
    });

    apply(repository_path, &document, &patches, profile)
}

/// How the resume looks: everything under `design:` the picker can set.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Design {
    /// The theme it renders with, if it names one.
    pub theme: Option<String>,
    /// Whether the "Last updated in …" note prints. rendercv shows it unless told not to.
    pub show_top_note: bool,
}

/// Reads the `design:` block.
pub fn design(repository_path: &Path) -> Result<Design, ResumeError> {
    let path = repository_path.join(FILENAME);
    let text = read(repository_path)?;
    let document: yaml_serde::Value =
        yaml_serde::from_str(&text).map_err(|source| ResumeError::Decode { path, source })?;
    let block = document.get(DESIGN_KEY);

    Ok(Design {
        theme: block
            .and_then(|design| design.get(THEME_KEY))
            .and_then(|theme| theme.as_str())
            .map(str::to_string),
        show_top_note: block
            .and_then(|design| design.get(PAGE_KEY))
            .and_then(|page| page.get(TOP_NOTE_KEY))
            .and_then(|shown| shown.as_bool())
            .unwrap_or(true),
    })
}

/// Shows or hides the "Last updated in …" note.
pub fn set_top_note(
    repository_path: &Path,
    show: bool,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let (_, document) = load(repository_path)?;
    let design_route = yamlpath::Route::default().with_key(DESIGN_KEY);
    let page_route = design_route.clone().with_key(PAGE_KEY);
    let note_route = page_route.clone().with_key(TOP_NOTE_KEY);
    let shown = yaml_serde::Value::Bool(show);

    let patch = if document.query_exists(&note_route) {
        yamlpatch::Patch {
            route: note_route,
            operation: yamlpatch::Op::Replace(shown),
        }
    } else if document.query_exists(&page_route) {
        yamlpatch::Patch {
            route: page_route,
            operation: yamlpatch::Op::Add {
                key: TOP_NOTE_KEY.to_string(),
                value: shown,
            },
        }
    } else {
        let mut page = yaml_serde::Mapping::new();
        page.insert(yaml_serde::Value::String(TOP_NOTE_KEY.to_string()), shown);
        let page = yaml_serde::Value::Mapping(page);

        if document.query_exists(&design_route) {
            yamlpatch::Patch {
                route: design_route,
                operation: yamlpatch::Op::Add {
                    key: PAGE_KEY.to_string(),
                    value: page,
                },
            }
        } else {
            let mut block = yaml_serde::Mapping::new();
            block.insert(yaml_serde::Value::String(PAGE_KEY.to_string()), page);
            yamlpatch::Patch {
                route: yamlpath::Route::default(),
                operation: yamlpatch::Op::Add {
                    key: DESIGN_KEY.to_string(),
                    value: yaml_serde::Value::Mapping(block),
                },
            }
        }
    };

    apply(repository_path, &document, &[patch], profile)
}

/// Sets the resume's theme, adding a `design:` block if it has none.
pub fn set_theme(
    repository_path: &Path,
    theme: &str,
    profile: &Profile,
) -> Result<(), ResumeError> {
    if !THEMES.contains(&theme) {
        return Err(ResumeError::UnknownTheme {
            theme: theme.to_string(),
        });
    }

    let (_, document) = load(repository_path)?;
    let design_route = yamlpath::Route::default().with_key(DESIGN_KEY);
    let theme_route = design_route.clone().with_key(THEME_KEY);
    let named = yaml_serde::Value::String(theme.to_string());

    let patch = if document.query_exists(&theme_route) {
        yamlpatch::Patch {
            route: theme_route,
            operation: yamlpatch::Op::Replace(named),
        }
    } else if document.query_exists(&design_route) {
        yamlpatch::Patch {
            route: design_route,
            operation: yamlpatch::Op::Add {
                key: THEME_KEY.to_string(),
                value: named,
            },
        }
    } else {
        let mut design = yaml_serde::Mapping::new();
        design.insert(yaml_serde::Value::String(THEME_KEY.to_string()), named);
        yamlpatch::Patch {
            route: yamlpath::Route::default(),
            operation: yamlpatch::Op::Add {
                key: DESIGN_KEY.to_string(),
                value: yaml_serde::Value::Mapping(design),
            },
        }
    };

    apply(repository_path, &document, &[patch], profile)
}

/// Adds an empty section to the resume, leaving one that already exists alone.
///
/// A section placed on its own gives Entries somewhere to land and fixes where it sits in the
/// order. rendercv accepts an empty section.
pub fn place_section(
    repository_path: &Path,
    section: &str,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let (_, document) = load(repository_path)?;

    let sections_route = yamlpath::Route::default()
        .with_key(CV_KEY)
        .with_key(SECTIONS_KEY);
    let section_route = sections_route.with_key(section);
    if document.query_exists(&section_route) {
        return Ok(());
    }

    // An empty sequence has no block form to re-emit as (unlike `place_entry`'s non-empty case),
    // so it stays a flow `[]` regardless. `place_entry` checks for that and replaces it outright
    // rather than trying to `Append` into it.
    let empty = yaml_serde::Value::Sequence(Vec::new());
    let patches = place_in_sections(&document, sections_route, section, empty);

    apply(repository_path, &document, &patches, profile)
}

/// Removes one section, with everything it holds.
pub fn remove_section(
    repository_path: &Path,
    section: &str,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let (_, document) = load(repository_path)?;
    let route = yamlpath::Route::default()
        .with_key(CV_KEY)
        .with_key(SECTIONS_KEY)
        .with_key(section);
    if !document.query_exists(&route) {
        return Err(ResumeError::NoSuchSection {
            section: section.to_string(),
        });
    }

    apply(
        repository_path,
        &document,
        &[yamlpatch::Patch {
            route,
            operation: yamlpatch::Op::Remove,
        }],
        profile,
    )
}

/// Removes one entry from a section, with everything it holds. The section itself is left in
/// place, even once empty.
pub fn remove_entry(
    repository_path: &Path,
    section: &str,
    entry_index: usize,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let (path, document) = load(repository_path)?;
    let route = entry_route(section, entry_index);
    require(&document, &route, section, entry_index)?;

    let sections_route = yamlpath::Route::default()
        .with_key(CV_KEY)
        .with_key(SECTIONS_KEY);
    let section_route = sections_route.with_key(section);

    let parsed: yaml_serde::Value =
        yaml_serde::from_str(document.source()).map_err(|source| ResumeError::Decode {
            path: path.clone(),
            source,
        })?;
    let is_last_entry = parsed
        .get(CV_KEY)
        .and_then(|cv| cv.get(SECTIONS_KEY))
        .and_then(|sections| sections.get(section))
        .and_then(|value| value.as_sequence())
        .is_some_and(|entries| entries.len() == 1);

    // Removing the only entry would leave the section an empty sequence — but `Remove` doesn't
    // stop there: an empty result cascades into removing the key that held it, and, `sections:`
    // then holding nothing either, that key too. Replacing the section with `[]` outright avoids
    // the cascade; the section itself is meant to survive down to empty, same as `place_section`.
    let patch = if is_last_entry {
        yamlpatch::Patch {
            route: section_route,
            operation: yamlpatch::Op::Replace(yaml_serde::Value::Sequence(Vec::new())),
        }
    } else {
        yamlpatch::Patch {
            route,
            operation: yamlpatch::Op::Remove,
        }
    };

    apply(repository_path, &document, &[patch], profile)
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
        &[yamlpatch::Patch {
            route,
            operation: yamlpatch::Op::Remove,
        }],
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

    apply(repository_path, &document, &[patch], profile)
}

/// Moves an entry within its section, or a wording within an entry.
///
/// Rewrites the whole list rather than editing it in place: `yamlpatch` has `Append` and `Remove`
/// but nothing that inserts at a position. Comments inside the list do not survive.
fn reorder(
    repository_path: &Path,
    section: &str,
    route: yamlpath::Route<'_>,
    locate: impl Fn(&yaml_serde::Value) -> Option<&yaml_serde::Value>,
    from: usize,
    to: usize,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let (path, document) = load(repository_path)?;
    if !document.query_exists(&route) {
        return Err(ResumeError::NoSuchEntry {
            section: section.to_string(),
            index: from,
        });
    }

    let parsed: yaml_serde::Value = yaml_serde::from_str(document.source())
        .map_err(|source| ResumeError::Decode { path, source })?;
    let mut items = locate(&parsed)
        .and_then(|value| value.as_sequence())
        .cloned()
        .ok_or_else(|| ResumeError::NoSuchEntry {
            section: section.to_string(),
            index: from,
        })?;

    if from >= items.len() || to > items.len() {
        return Err(ResumeError::NoSuchEntry {
            section: section.to_string(),
            index: from.max(to),
        });
    }

    let moved = items.remove(from);
    items.insert(if to > from { to - 1 } else { to }, moved);

    apply(
        repository_path,
        &document,
        &[yamlpatch::Patch {
            route,
            operation: yamlpatch::Op::Replace(yaml_serde::Value::Sequence(items)),
        }],
        profile,
    )
}

/// Moves an entry to another position in its section. `to` is the index it lands before.
pub fn move_entry(
    repository_path: &Path,
    section: &str,
    from: usize,
    to: usize,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let route = yamlpath::Route::default()
        .with_key(CV_KEY)
        .with_key(SECTIONS_KEY)
        .with_key(section);
    let owned = section.to_string();
    reorder(
        repository_path,
        section,
        route,
        move |parsed| {
            parsed
                .get(CV_KEY)
                .and_then(|cv| cv.get(SECTIONS_KEY))
                .and_then(|sections| sections.get(&owned))
        },
        from,
        to,
        profile,
    )
}

/// Moves a wording to another position within one entry's `highlights`.
pub fn move_bullet(
    repository_path: &Path,
    section: &str,
    entry_index: usize,
    from: usize,
    to: usize,
    profile: &Profile,
) -> Result<(), ResumeError> {
    let route = entry_route(section, entry_index).with_key(HIGHLIGHTS_KEY);
    let owned = section.to_string();
    reorder(
        repository_path,
        section,
        route,
        move |parsed| {
            parsed
                .get(CV_KEY)
                .and_then(|cv| cv.get(SECTIONS_KEY))
                .and_then(|sections| sections.get(&owned))
                .and_then(|entries| entries.get(entry_index))
                .and_then(|entry| entry.get(HIGHLIGHTS_KEY))
        },
        from,
        to,
        profile,
    )
}

/// Moves a section so it sits at final index `to` (the same contract as [`move_entry`]).
///
/// The section's lines move whole — comments directly above a section's key travel with it —
/// because patching could only replace the sections mapping's value, which would flatten every
/// comment inside it.
pub fn move_section(repository_path: &Path, from: usize, to: usize) -> Result<(), ResumeError> {
    let (path, document) = load(repository_path)?;
    let missing = || ResumeError::NoSuchEntry {
        section: SECTIONS_KEY.to_string(),
        index: from.max(to),
    };
    let sections_route = yamlpath::Route::default()
        .with_key(CV_KEY)
        .with_key(SECTIONS_KEY);

    // Ordered names, straight from the file's mapping order.
    let parsed: yaml_serde::Value =
        yaml_serde::from_str(document.source()).map_err(|source| ResumeError::Decode {
            path: path.clone(),
            source,
        })?;
    let names: Vec<String> = parsed
        .get(CV_KEY)
        .and_then(|cv| cv.get(SECTIONS_KEY))
        .and_then(|sections| sections.as_mapping())
        .map(|sections| {
            sections
                .iter()
                .filter_map(|(name, _)| name.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if from >= names.len() || to > names.len() || from == to {
        return Err(missing());
    }

    let source = document.source();
    let start_of_line = |at: usize| source[..at].rfind('\n').map_or(0, |newline| newline + 1);

    // Each section occupies whole lines from its key to the next key; the last runs to the
    // end of the sections value.
    let mut starts = Vec::with_capacity(names.len());
    for name in &names {
        let feature = document
            .query_key_only(&sections_route.with_key(name.as_str()))
            .map_err(|_| missing())?;
        starts.push(start_of_line(feature.location.byte_span.0));
    }
    let end = match document.query_exact(&sections_route).ok().flatten() {
        Some(feature) if source[feature.location.byte_span.1..].contains('\n') => {
            source[feature.location.byte_span.1..].find('\n').unwrap()
                + feature.location.byte_span.1
                + 1
        }
        Some(feature) => feature.location.byte_span.1,
        None => return Err(missing()),
    };

    // A comment directly above the moved key belongs to that section: shift the boundary
    // between it and the previous block, so the comment lines travel with their section.
    while let Some(newline) = source[..starts[from]].rfind('\n') {
        let line = &source[newline + 1..starts[from]];
        if line.trim_start().is_empty() || !line.trim_start().starts_with('#') {
            break;
        }
        starts[from] = newline + 1;
    }

    let blocks: Vec<String> = (0..names.len())
        .map(|at| {
            let stop = starts.get(at + 1).copied().unwrap_or(end);
            source[starts[at]..stop].to_owned()
        })
        .collect();

    let mut order: Vec<usize> = (0..names.len()).collect();
    order.remove(from);
    order.insert(if to > from { to - 1 } else { to }, from);

    let mut rebuilt = String::new();
    for at in &order {
        rebuilt.push_str(&blocks[*at]);
    }
    if !rebuilt.ends_with('\n') {
        rebuilt.push('\n');
    }
    let rest = source[end..].strip_prefix('\n').unwrap_or(&source[end..]);

    let updated = format!("{}{}{}", &source[..starts[0]], rebuilt, rest);

    let as_json: Value = yaml_serde::from_str(&updated).map_err(|source| ResumeError::Decode {
        path: path.clone(),
        source,
    })?;
    validate(&as_json).map_err(ResumeError::Invalid)?;
    std::fs::write(&path, updated).map_err(|source| ResumeError::Write { path, source })
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
    patches: &[yamlpatch::Patch],
    profile: &Profile,
) -> Result<(), ResumeError> {
    let patched =
        yamlpatch::apply_yaml_patches(document, patches).map_err(|source| ResumeError::Patch {
            path: repository_path.join(FILENAME),
            source,
        })?;

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
    fn moving_a_section_reorders_the_file_and_keeps_its_comment() {
        // `to` names the index in the original ordering the moved section lands before,
        // matching [`move_entry`] — so 3 appends Experience to the end.
        let text = "\
cv:
  sections:
    # The experience block, hand placed.
    Experience:
      - company: Helio
        position: Engineer
        start_date: 2019-08
        end_date: present
    Education:
      - institution: College
        area: Computer Science
        degree: BS
        start_date: 2015-09
        end_date: 2019-05
    Skills:
      - label: Languages
        details: Rust, Go
design:
  theme: classic
"
        .to_string();
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(FILENAME), &text).unwrap();

        move_section(dir.path(), 0, 3).unwrap();
        let moved = read(dir.path()).unwrap();

        let order = |text: &str| {
            yaml_serde::from_str::<yaml_serde::Value>(text)
                .unwrap()
                .get("cv")
                .unwrap()
                .get("sections")
                .unwrap()
                .as_mapping()
                .unwrap()
                .iter()
                .map(|(name, _)| name.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(order(&moved), vec!["Education", "Skills", "Experience"]);
        // The comment rides with the section it describes.
        let experience_at = moved.find("Experience").unwrap();
        assert!(moved[..experience_at].contains("# The experience block"));
        assert!(moved.contains("- company: Helio"));
        // And it still validates against rendercv's schema.
        let as_json: Value = yaml_serde::from_str(&moved).unwrap();
        assert_eq!(validate(&as_json), Ok(()));

        // Moving it back restores the original order.
        move_section(dir.path(), 2, 0).unwrap();
        assert_eq!(
            order(&read(dir.path()).unwrap()),
            vec!["Experience", "Education", "Skills"]
        );
    }

    #[test]
    fn moving_a_section_out_of_range_is_rejected() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join(FILENAME),
            "cv:\n  sections:\n    Experience: []\n",
        )
        .unwrap();

        assert!(matches!(
            move_section(dir.path(), 3, 0),
            Err(ResumeError::NoSuchEntry { .. })
        ));
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
    fn removing_a_freshly_created_highlight_takes_only_that_highlight() {
        let dir = seeded(SAMPLE);
        let profile = populated_profile();

        // Entry 1 (Ravensmoor) has no `highlights:` yet, so this Adds it fresh.
        place_bullet(dir.path(), "Experience", 1, "First one.".into(), &profile).unwrap();
        remove_bullet(dir.path(), "Experience", 1, 0, &profile).unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(outline[0].name, "Experience");
        assert_eq!(outline[0].entries.len(), 2);
        assert_eq!(outline[0].entries[1].fields["company"], "Ravensmoor");
        assert_eq!(outline[0].entries[1].highlights, Vec::<String>::new());
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
    fn an_entry_can_be_placed_after_several_sections_share_one_cold_start() {
        // Reproduces the live failure: several sections placed empty in a row, from a resume
        // with no `sections:` key yet, then an entry placed into one of them.
        let dir = seeded("cv:\n  name: Cam\n");
        let profile = populated_profile();

        for section in ["Education", "Experience", "Projects", "Skills"] {
            place_section(dir.path(), section, &profile).unwrap();
        }
        place_entry(
            dir.path(),
            "Experience",
            json!({ "company": "Knightscope", "position": "SWE Intern" }),
            &profile,
        )
        .unwrap();

        let outline = outline(dir.path()).unwrap();
        let experience = outline.iter().find(|s| s.name == "Experience").unwrap();
        assert_eq!(experience.entries[0].fields["company"], "Knightscope");
    }

    #[test]
    fn an_entry_can_be_placed_into_a_section_placed_empty() {
        let dir = seeded("cv:\n  name: Someone\n");
        let profile = populated_profile();

        place_section(dir.path(), "Experience", &profile).unwrap();
        place_entry(
            dir.path(),
            "Experience",
            json!({ "company": "Acme", "position": "Engineer" }),
            &profile,
        )
        .unwrap();

        let outline = outline(dir.path()).unwrap();
        let experience = outline.iter().find(|s| s.name == "Experience").unwrap();
        assert_eq!(experience.entries[0].fields["company"], "Acme");
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
    fn removing_the_only_highlight_leaves_the_entry() {
        let dir = seeded(
            "cv:\n  sections:\n    Experience:\n      - company: Knightscope\n        position: SWE Intern\n        highlights:\n          - Did a thing.\n",
        );

        remove_bullet(dir.path(), "Experience", 0, 0, &populated_profile()).unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(outline[0].entries.len(), 1);
        assert_eq!(outline[0].entries[0].fields["company"], "Knightscope");
        assert_eq!(outline[0].entries[0].highlights, Vec::<String>::new());
    }

    #[test]
    fn removing_the_only_entry_leaves_the_section_empty_not_gone() {
        let dir = seeded(
            "cv:\n  sections:\n    Experience:\n      - company: Knightscope\n        position: SWE Intern\n",
        );

        remove_entry(dir.path(), "Experience", 0, &populated_profile()).unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].name, "Experience");
        assert_eq!(outline[0].entries.len(), 0);
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
    fn a_wording_can_be_placed_after_the_last_one_was_removed() {
        let dir = seeded(SAMPLE);
        let profile = populated_profile();

        remove_bullet(dir.path(), "Experience", 0, 0, &profile).unwrap();
        place_bullet(
            dir.path(),
            "Experience",
            0,
            "Did another thing.".into(),
            &profile,
        )
        .unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(outline[0].entries[0].highlights, ["Did another thing."]);
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
    fn removing_a_section_takes_only_that_section() {
        let dir = seeded(
            "cv:\n  sections:\n    Experience:\n      - company: Helio\n        position: Engineer\n    Education:\n      - institution: College\n        area: Computer Science\n        degree: BS\n",
        );

        remove_section(dir.path(), "Experience", &populated_profile()).unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].name, "Education");
    }

    #[test]
    fn removing_a_section_that_is_not_there_is_rejected() {
        let dir = seeded(SAMPLE);

        assert!(matches!(
            remove_section(dir.path(), "Nonexistent", &populated_profile()),
            Err(ResumeError::NoSuchSection { .. })
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

    #[test]
    fn a_created_section_is_block_style_and_takes_field_writes() {
        let dir = seeded("cv:\n  name: Someone\n");
        let profile = populated_profile();

        place_entry(
            dir.path(),
            "Skills",
            json!({ "label": "Languages", "details": "Rust, Go" }),
            &profile,
        )
        .unwrap();

        let written = read(dir.path()).unwrap();
        assert!(
            written.contains("\n      - "),
            "not a block sequence: {written}"
        );
        assert!(!written.contains("[{"), "still a flow mapping: {written}");

        // The write that used to split a flow mapping into further keys.
        set_entry_field(
            dir.path(),
            "Skills",
            0,
            "details",
            json!("Go, Rust, Python"),
            &profile,
        )
        .unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(outline[0].entries[0].fields["details"], "Go, Rust, Python");
        assert_eq!(outline[0].entries[0].fields["label"], "Languages");
    }

    #[test]
    fn moving_an_entry_reorders_the_section() {
        let dir = seeded(SAMPLE);

        move_entry(dir.path(), "Experience", 1, 0, &populated_profile()).unwrap();

        let outline = outline(dir.path()).unwrap();
        let companies: Vec<&str> = outline[0]
            .entries
            .iter()
            .map(|entry| entry.fields["company"].as_str().unwrap())
            .collect();
        assert_eq!(companies, ["Ravensmoor", "Helio"]);
    }

    #[test]
    fn moving_a_wording_reorders_the_entry() {
        let dir = seeded(SAMPLE);
        let profile = populated_profile();
        place_bullet(dir.path(), "Experience", 0, "Second.".into(), &profile).unwrap();

        move_bullet(dir.path(), "Experience", 0, 1, 0, &profile).unwrap();

        let outline = outline(dir.path()).unwrap();
        assert_eq!(outline[0].entries[0].highlights[0], "Second.");
    }

    #[test]
    fn moving_past_the_end_is_rejected() {
        let dir = seeded(SAMPLE);

        assert!(matches!(
            move_entry(dir.path(), "Experience", 5, 0, &populated_profile()),
            Err(ResumeError::NoSuchEntry { .. })
        ));
    }
}
