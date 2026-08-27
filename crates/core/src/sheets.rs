//! Reading a linked Google Sheet as CSV, over its public export link.
//!
//! No OAuth and no API key: the sheet must be shared "anyone with the link can view", the same
//! requirement a pasted copy already carried (`docs/decisions/workspace.md`). This only reads —
//! writing back still goes through the existing CSV export.

use std::path::Path;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum SheetError {
    #[error("that does not look like a Google Sheets link or id")]
    InvalidLink,
    #[error("no sheet is linked")]
    NotLinked,
    #[error("could not reach the sheet")]
    Request(#[source] reqwest::Error),
    #[error("the sheet answered {status}; is it shared with \"anyone with the link\" as a viewer?")]
    Denied { status: u16 },
    #[error("could not write the configuration")]
    Config(#[source] std::io::Error),
}

/// Pulls the spreadsheet id out of a Google Sheets URL, or accepts a bare id.
///
/// A bare id is accepted so a stored id round-trips through [`link`] and back.
pub fn id_from(input: &str) -> Result<String, SheetError> {
    let trimmed = input.trim();
    let after_d = trimmed
        .split_once("docs.google.com/spreadsheets/d/")
        .map(|(_, rest)| rest);

    let candidate = match after_d {
        Some(rest) => rest.split('/').next().unwrap_or(""),
        None => trimmed,
    };

    if !candidate.is_empty()
        && candidate
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Ok(candidate.to_string())
    } else {
        Err(SheetError::InvalidLink)
    }
}

/// Fetches the linked sheet's first tab as CSV.
pub fn csv(id: &str) -> Result<String, SheetError> {
    let url = format!("https://docs.google.com/spreadsheets/d/{id}/export?format=csv");
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(SheetError::Request)?
        .get(&url)
        .send()
        .map_err(SheetError::Request)?;

    if !response.status().is_success() {
        return Err(SheetError::Denied {
            status: response.status().as_u16(),
        });
    }
    response.text().map_err(SheetError::Request)
}

/// Writes `applications_sheet` into the config file, keeping anything else already set.
pub fn remember(config_path: &Path, id: &str) -> Result<(), SheetError> {
    crate::config::remember_key(config_path, "applications_sheet", Some(id))
        .map_err(SheetError::Config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn a_url_yields_the_id_between_d_and_the_next_slash() {
        assert_eq!(
            id_from("https://docs.google.com/spreadsheets/d/1JBEs-5vL26unGDYkwf_5NwZGBXycfosfuceq0M93a1w/edit?usp=sharing").unwrap(),
            "1JBEs-5vL26unGDYkwf_5NwZGBXycfosfuceq0M93a1w"
        );
    }

    #[test]
    fn a_bare_id_is_accepted_as_is() {
        assert_eq!(id_from("abc-123_XYZ").unwrap(), "abc-123_XYZ");
    }

    #[test]
    fn whitespace_around_a_pasted_link_is_trimmed() {
        assert_eq!(id_from("  abc123  ").unwrap(), "abc123");
    }

    #[test]
    fn nonsense_is_rejected() {
        assert!(matches!(id_from(""), Err(SheetError::InvalidLink)));
        assert!(matches!(
            id_from("not a link at all"),
            Err(SheetError::InvalidLink)
        ));
    }

    #[test]
    fn remembering_writes_the_id_and_keeps_other_keys() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");
        std::fs::write(&config, "resume_repo = \"/srv/resume\"\n").unwrap();

        remember(&config, "abc123").unwrap();

        let written = std::fs::read_to_string(&config).unwrap();
        assert!(written.contains("resume_repo = \"/srv/resume\""));
        assert!(written.contains("applications_sheet = \"abc123\""));
    }

    #[test]
    fn remembering_again_replaces_the_previous_id() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");

        remember(&config, "first").unwrap();
        remember(&config, "second").unwrap();

        let written = std::fs::read_to_string(&config).unwrap();
        assert_eq!(written.matches("applications_sheet").count(), 1);
        assert!(written.contains("applications_sheet = \"second\""));
    }
}
