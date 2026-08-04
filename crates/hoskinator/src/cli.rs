//! The command-line frontend.
//!
//! Speaks to the daemon over HTTP like any other frontend. There is no in-process path to the
//! store (ADR-0003).

use hoskinator_core::profile::Profile;
use hoskinator_core::section::{EntryType, Section};
use jsonrpsee::core::client::Error as ClientError;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};

use crate::rpc::{ProfileRpcClient, SectionRpcClient};

/// A command could not be carried out.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("no Hoskinator daemon answered on port {port}; start one with `hoskinator serve`")]
    NoDaemon {
        port: u16,
        #[source]
        source: ClientError,
    },

    #[error("the daemon rejected the request")]
    Request(#[source] ClientError),

    #[error("could not read a Profile from standard input")]
    ReadInput(#[source] std::io::Error),

    #[error("standard input is not a valid Profile")]
    ParseInput(#[source] serde_json::Error),

    #[error("could not render the Profile")]
    Render(#[source] serde_json::Error),
}

/// Prints the stored Profile as JSON.
pub async fn profile_get(port: u16) -> Result<(), CliError> {
    let profile = client(port)?
        .profile_get()
        .await
        .map_err(|source| classify(source, port))?;

    println!(
        "{}",
        serde_json::to_string_pretty(&profile).map_err(CliError::Render)?
    );
    Ok(())
}

/// Replaces the stored Profile with the JSON on standard input.
pub async fn profile_set(port: u16) -> Result<(), CliError> {
    let profile = read_profile(std::io::stdin().lock())?;

    client(port)?
        .profile_set(profile)
        .await
        .map_err(|source| classify(source, port))
}

/// Creates a section and prints the stored record.
pub async fn section_create(port: u16, name: &str, entry_type: EntryType) -> Result<(), CliError> {
    let section = client(port)?
        .section_create(name.to_owned(), entry_type)
        .await
        .map_err(|source| classify(source, port))?;

    print!("{}", table(&[section]));
    Ok(())
}

/// Prints every section.
pub async fn section_list(port: u16) -> Result<(), CliError> {
    let sections = client(port)?
        .section_list()
        .await
        .map_err(|source| classify(source, port))?;

    print!("{}", table(&sections));
    Ok(())
}

/// Renames a section and prints the stored record.
pub async fn section_rename(port: u16, name: &str, new_name: &str) -> Result<(), CliError> {
    let section = client(port)?
        .section_update(name.to_owned(), Some(new_name.to_owned()), None)
        .await
        .map_err(|source| classify(source, port))?;

    print!("{}", table(&[section]));
    Ok(())
}

/// Changes a section's entry type and prints the stored record.
pub async fn section_retype(port: u16, name: &str, entry_type: EntryType) -> Result<(), CliError> {
    let section = client(port)?
        .section_update(name.to_owned(), None, Some(entry_type))
        .await
        .map_err(|source| classify(source, port))?;

    print!("{}", table(&[section]));
    Ok(())
}

/// Deletes a section.
pub async fn section_delete(port: u16, name: &str) -> Result<(), CliError> {
    client(port)?
        .section_delete(name.to_owned())
        .await
        .map_err(|source| classify(source, port))
}

/// Renders sections as a two-column table, or the empty string when there are none.
fn table(sections: &[Section]) -> String {
    if sections.is_empty() {
        return String::new();
    }

    let width = sections
        .iter()
        .map(|section| section.name.chars().count())
        .chain(std::iter::once(NAME_HEADING.len()))
        .max()
        .unwrap_or(NAME_HEADING.len());

    let mut rendered = format!("{NAME_HEADING:<width$}  {TYPE_HEADING}\n");
    for section in sections {
        rendered.push_str(&format!(
            "{:<width$}  {}\n",
            section.name, section.entry_type
        ));
    }

    rendered
}

const NAME_HEADING: &str = "NAME";
const TYPE_HEADING: &str = "ENTRY TYPE";

fn read_profile(mut input: impl std::io::Read) -> Result<Profile, CliError> {
    let mut text = String::new();
    input
        .read_to_string(&mut text)
        .map_err(CliError::ReadInput)?;

    serde_json::from_str(&text).map_err(CliError::ParseInput)
}

fn client(port: u16) -> Result<HttpClient, CliError> {
    HttpClientBuilder::default()
        .build(format!("http://127.0.0.1:{port}/rpc"))
        .map_err(|source| CliError::NoDaemon { port, source })
}

/// Separates "nothing is listening" from an error the daemon actually sent back.
fn classify(error: ClientError, port: u16) -> CliError {
    if matches!(error, ClientError::Transport(_)) {
        CliError::NoDaemon {
            port,
            source: error,
        }
    } else {
        CliError::Request(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoskinator_core::profile::OneOrMany;

    #[test]
    fn a_profile_is_read_from_json_on_standard_input() {
        let input = r#"{"name":"Ada Lovelace","email":"ada@example.com"}"#;

        let profile = read_profile(input.as_bytes()).unwrap();

        assert_eq!(profile.name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(
            profile.email,
            Some(OneOrMany::One("ada@example.com".into()))
        );
    }

    #[test]
    fn omitted_fields_read_as_unset() {
        let profile = read_profile("{}".as_bytes()).unwrap();

        assert_eq!(profile, Profile::default());
    }

    #[test]
    fn input_that_is_not_a_profile_is_rejected() {
        let error = read_profile("not json".as_bytes()).unwrap_err();

        assert!(matches!(error, CliError::ParseInput(_)), "got {error:?}");
    }

    fn section(name: &str, entry_type: EntryType) -> Section {
        Section {
            name: name.into(),
            entry_type,
        }
    }

    #[test]
    fn the_table_heads_each_column() {
        let rendered = table(&[section("Experience", EntryType::Experience)]);

        assert_eq!(rendered.lines().next(), Some("NAME        ENTRY TYPE"));
    }

    #[test]
    fn the_name_column_fits_the_longest_name() {
        let rendered = table(&[
            section("Experience", EntryType::Experience),
            section("Selected Projects", EntryType::Normal),
        ]);

        let starts: Vec<usize> = rendered
            .lines()
            .map(|line| {
                line.find("ENTRY TYPE")
                    .or_else(|| line.rfind("  ").map(|at| at + 2))
            })
            .map(|at| at.expect("a second column"))
            .collect();

        assert!(
            starts.windows(2).all(|pair| pair[0] == pair[1]),
            "columns are ragged: {rendered:?}"
        );
    }

    #[test]
    fn a_name_shorter_than_the_heading_still_lines_up() {
        let rendered = table(&[section("X", EntryType::Text)]);

        let mut lines = rendered.lines();
        let heading = lines.next().unwrap();
        let row = lines.next().unwrap();

        assert_eq!(heading.find("ENTRY TYPE"), row.find("text"));
    }

    #[test]
    fn no_sections_render_as_nothing() {
        assert_eq!(table(&[]), "");
    }

    #[test]
    fn an_unreachable_daemon_names_the_port_and_the_fix() {
        let error = classify(ClientError::Transport("refused".into()), 8737);

        let rendered = error.to_string();
        assert!(rendered.contains("8737"), "got {rendered}");
        assert!(rendered.contains("hoskinator serve"), "got {rendered}");
    }
}
