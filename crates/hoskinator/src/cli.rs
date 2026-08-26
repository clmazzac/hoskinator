//! The command-line frontend.
//!
//! Speaks to the daemon over HTTP like any other frontend. There is no in-process path to the
//! store (ADR-0003).

use hoskinator_core::job_description::NewJobDescription;
use hoskinator_core::profile::Profile;
use hoskinator_core::repository::{CheckoutRequest, CommitRequest, CreateBranchRequest};
use hoskinator_core::section::{EntryType, Section};
use jsonrpsee::core::client::Error as ClientError;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};

use crate::rpc::{
    BulletRpcClient, EntryRpcClient, JobDescriptionRpcClient, ProfileRpcClient,
    RepositoryRpcClient, SearchRpcClient, SectionRpcClient,
};

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
    #[error("could not render the response")]
    Render(#[source] serde_json::Error),

    #[error("could not read entry fields from standard input")]
    ReadEntryInput(#[source] std::io::Error),

    #[error("standard input is not JSON")]
    ParseEntryInput(#[source] serde_json::Error),

    #[error("could not read a Job Description from standard input")]
    ReadJobDescriptionInput(#[source] std::io::Error),

    #[error("standard input is not a valid Job Description")]
    ParseJobDescriptionInput(#[source] serde_json::Error),

    #[error("could not render a Job Description")]
    RenderJobDescription(#[source] serde_json::Error),
}

pub async fn profile_get(port: u16) -> Result<(), CliError> {
    render(
        client(port)?
            .profile_get()
            .await
            .map_err(|source| classify(source, port))?,
    )
}

pub async fn profile_set(port: u16) -> Result<(), CliError> {
    let profile = read_profile(std::io::stdin().lock())?;
    client(port)?
        .profile_set(profile)
        .await
        .map_err(|source| classify(source, port))
}

pub async fn repository_init(port: u16) -> Result<(), CliError> {
    render(
        client(port)?
            .repository_init()
            .await
            .map_err(|source| classify(source, port))?,
    )
}

pub async fn repository_branch(port: u16, name: String) -> Result<(), CliError> {
    render(
        client(port)?
            .repository_branch_create(CreateBranchRequest { name, from: None })
            .await
            .map_err(|source| classify(source, port))?,
    )
}

pub async fn repository_checkout(port: u16, branch: String) -> Result<(), CliError> {
    render(
        client(port)?
            .repository_checkout(CheckoutRequest { branch })
            .await
            .map_err(|source| classify(source, port))?,
    )
}

pub async fn repository_delete(port: u16, branch: String) -> Result<(), CliError> {
    render(
        client(port)?
            .repository_branch_delete(branch)
            .await
            .map_err(|source| classify(source, port))?,
    )
}

pub async fn repository_commit(port: u16, message: String) -> Result<(), CliError> {
    render(
        client(port)?
            .repository_commit(CommitRequest { message })
            .await
            .map_err(|source| classify(source, port))?,
    )
}

pub async fn repository_status(port: u16) -> Result<(), CliError> {
    render(
        client(port)?
            .repository_status()
            .await
            .map_err(|source| classify(source, port))?,
    )
}

pub async fn repository_diff(port: u16) -> Result<(), CliError> {
    render(
        client(port)?
            .repository_diff()
            .await
            .map_err(|source| classify(source, port))?,
    )
}

pub async fn repository_log(port: u16) -> Result<(), CliError> {
    render(
        client(port)?
            .repository_log()
            .await
            .map_err(|source| classify(source, port))?,
    )
}

fn render(value: impl serde::Serialize) -> Result<(), CliError> {
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(CliError::Render)?
    );
    Ok(())
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

/// Creates an entry of `entry_type` from the fields on standard input and prints it.
pub async fn entry_create(port: u16, entry_type: EntryType) -> Result<(), CliError> {
    let fields = read_fields(std::io::stdin().lock())?;

    render(
        client(port)?
            .entry_create(entry_type, fields)
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Prints the entry with `id`, or `null` when it is absent.
pub async fn entry_get(port: u16, id: i64) -> Result<(), CliError> {
    render(
        client(port)?
            .entry_get(id)
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Prints every entry, or only those of one type.
pub async fn entry_list(port: u16, entry_type: Option<EntryType>) -> Result<(), CliError> {
    render(
        client(port)?
            .entry_list(entry_type)
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Prints the entries a section is eligible to hold.
pub async fn entry_eligible(port: u16, section: &str) -> Result<(), CliError> {
    render(
        client(port)?
            .entry_eligible(section.to_owned())
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Replaces an entry's fields with those on standard input and prints it.
pub async fn entry_update(port: u16, id: i64) -> Result<(), CliError> {
    let fields = read_fields(std::io::stdin().lock())?;

    render(
        client(port)?
            .entry_update(id, fields)
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Deletes an entry.
pub async fn entry_delete(port: u16, id: i64) -> Result<(), CliError> {
    client(port)?
        .entry_delete(id)
        .await
        .map_err(|source| classify(source, port))
}

fn read_fields(mut input: impl std::io::Read) -> Result<serde_json::Value, CliError> {
    let mut text = String::new();
    input
        .read_to_string(&mut text)
        .map_err(CliError::ReadEntryInput)?;

    serde_json::from_str(&text).map_err(CliError::ParseEntryInput)
}

/// Creates a bullet on an entry, worded by its first variant.
pub async fn bullet_create(
    port: u16,
    entry_id: i64,
    text: &str,
    note: Option<String>,
) -> Result<(), CliError> {
    render(
        client(port)?
            .bullet_create(entry_id, text.to_owned(), note)
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Prints one bullet with its variants.
pub async fn bullet_get(port: u16, id: i64) -> Result<(), CliError> {
    render(
        client(port)?
            .bullet_get(id)
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Prints every bullet of an entry, in order.
pub async fn bullet_list(port: u16, entry_id: i64) -> Result<(), CliError> {
    render(
        client(port)?
            .bullet_list(entry_id)
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Moves a bullet within its entry and prints the new order.
pub async fn bullet_move(port: u16, id: i64, position: i32) -> Result<(), CliError> {
    render(
        client(port)?
            .bullet_move(id, position)
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Deletes a bullet and its variants.
pub async fn bullet_delete(port: u16, id: i64) -> Result<(), CliError> {
    client(port)?
        .bullet_delete(id)
        .await
        .map_err(|source| classify(source, port))
}

/// Adds another wording to a bullet.
pub async fn variant_create(
    port: u16,
    bullet_id: i64,
    text: &str,
    note: Option<String>,
) -> Result<(), CliError> {
    render(
        client(port)?
            .variant_create(bullet_id, text.to_owned(), note)
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Rewords a variant, renotes it, or both.
pub async fn variant_update(
    port: u16,
    id: i64,
    text: Option<String>,
    note: Option<String>,
) -> Result<(), CliError> {
    render(
        client(port)?
            .variant_update(id, text, note)
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Makes a variant the default wording of its bullet.
pub async fn variant_set_default(port: u16, id: i64) -> Result<(), CliError> {
    render(
        client(port)?
            .variant_set_default(id)
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Deletes a variant.
pub async fn variant_delete(port: u16, id: i64) -> Result<(), CliError> {
    client(port)?
        .variant_delete(id)
        .await
        .map_err(|source| classify(source, port))
}

/// Prints what a query matches, best first.
pub async fn search(port: u16, query: &str) -> Result<(), CliError> {
    render(
        client(port)?
            .search_query(query.to_owned())
            .await
            .map_err(|source| classify(source, port))?,
    )
}

/// Creates a Job Description from JSON on standard input and prints its record.
pub async fn jd_create(port: u16) -> Result<(), CliError> {
    let job_description = read_job_description(std::io::stdin().lock())?;
    let created = client(port)?
        .jd_create(job_description)
        .await
        .map_err(|source| classify(source, port))?;

    print_job_description(&created)
}

/// Prints the Job Description with `id`, or `null` when it is absent.
pub async fn jd_get(port: u16, id: i64) -> Result<(), CliError> {
    let job_description = client(port)?
        .jd_get(id)
        .await
        .map_err(|source| classify(source, port))?;

    print_job_description(&job_description)
}

/// Prints every Job Description matching an optional full-text query.
pub async fn jd_list(port: u16, query: Option<String>) -> Result<(), CliError> {
    let job_descriptions = client(port)?
        .jd_list(query)
        .await
        .map_err(|source| classify(source, port))?;

    print_job_description(&job_descriptions)
}

/// Deletes the Job Description with `id` and prints whether it existed.
pub async fn jd_delete(port: u16, id: i64) -> Result<(), CliError> {
    let deleted = client(port)?
        .jd_delete(id)
        .await
        .map_err(|source| classify(source, port))?;

    print_job_description(&deleted)
}

fn read_job_description(mut input: impl std::io::Read) -> Result<NewJobDescription, CliError> {
    let mut text = String::new();
    input
        .read_to_string(&mut text)
        .map_err(CliError::ReadJobDescriptionInput)?;

    serde_json::from_str(&text).map_err(CliError::ParseJobDescriptionInput)
}

fn print_job_description(value: &impl serde::Serialize) -> Result<(), CliError> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(CliError::RenderJobDescription)?
    );
    Ok(())
}
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
    #[test]
    fn entry_fields_are_read_from_json_on_standard_input() {
        let input = r#"{"company":"Acme","position":"Engineer"}"#;

        let fields = read_fields(input.as_bytes()).unwrap();

        assert_eq!(fields["company"], "Acme");
    }

    #[test]
    fn entry_fields_that_are_not_json_are_rejected() {
        let error = read_fields("not json".as_bytes()).unwrap_err();

        assert!(
            matches!(error, CliError::ParseEntryInput(_)),
            "got {error:?}"
        );
    }

    #[test]
    fn a_job_description_is_read_from_json_on_standard_input() {
        let input = r#"{"title":"Systems engineer","text":"Build Rust services."}"#;

        let job_description = read_job_description(input.as_bytes()).unwrap();

        assert_eq!(
            job_description,
            NewJobDescription {
                title: Some("Systems engineer".into()),
                text: "Build Rust services.".into(),
            }
        );
    }

    #[test]
    fn input_that_is_not_a_job_description_is_rejected() {
        let error = read_job_description("not json".as_bytes()).unwrap_err();

        assert!(
            matches!(error, CliError::ParseJobDescriptionInput(_)),
            "got {error:?}"
        );
    }
}
