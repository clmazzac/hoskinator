//! Hoskinator: daemon, CLI, and Web UI host in one binary (ADR-0005).

use std::net::IpAddr;
use std::process::ExitCode;

use clap::builder::TypedValueParser;
use clap::{Parser, Subcommand};
use hoskinator_core::section::EntryType;

mod cli;
mod rpc;
mod serve;
mod web;

#[derive(Parser)]
#[command(name = "hoskinator", version, about, long_about = None)]
struct Cli {
    /// Port the daemon serves on.
    #[arg(long, global = true, default_value_t = serve::DEFAULT_PORT)]
    port: u16,
    /// Address the daemon binds. Change from loopback only behind your own auth/network
    /// boundary (e.g. Identity-Aware Proxy) — the daemon itself ships no TLS and no auth.
    #[arg(long, global = true, default_value_t = serve::DEFAULT_BIND)]
    bind: IpAddr,
    /// The URL a browser reaches this daemon at, e.g. `https://hoskinator.example.com`. Google's
    /// OAuth consent screen redirects here, so behind a reverse proxy this must be the proxy's
    /// address, not `--bind`. Defaults to `http://127.0.0.1:<port>`.
    #[arg(long, global = true)]
    public_url: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Runs the daemon until interrupted.
    Serve,
    /// Reads and writes the Profile.
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Creates, lists, and edits Sections.
    Section {
        #[command(subcommand)]
        action: SectionAction,
    },

    /// Creates, lists, and edits Entries.
    Entry {
        #[command(subcommand)]
        action: EntryAction,
    },

    /// Creates, lists, and orders the Bullets of an Entry.
    Bullet {
        #[command(subcommand)]
        action: BulletAction,
    },

    /// Words a Bullet, and chooses which wording is the default.
    Variant {
        #[command(subcommand)]
        action: VariantAction,
    },

    /// Searches the Master Store.
    Search { query: String },

    /// Creates and manages standalone Job Descriptions.
    #[command(name = "jd")]
    JobDescription {
        #[command(subcommand)]
        action: JobDescriptionAction,
    },

    /// Operates on the configured resume repository.
    Repository {
        #[command(subcommand)]
        action: RepositoryAction,
    },
}

#[derive(Subcommand)]
enum SectionAction {
    /// Creates a section.
    Create {
        name: String,

        /// Entry shape the section holds.
        #[arg(long = "type", value_name = "ENTRY_TYPE", value_parser = entry_type_parser())]
        entry_type: EntryType,
    },

    /// Prints every section.
    List,

    /// Renames a section.
    Rename { name: String, new_name: String },

    /// Changes a section's entry type.
    Retype {
        name: String,

        #[arg(value_name = "ENTRY_TYPE", value_parser = entry_type_parser())]
        entry_type: EntryType,
    },

    /// Deletes a section.
    Delete { name: String },
}

#[derive(Subcommand)]
enum EntryAction {
    /// Creates an entry from the fields in JSON on standard input.
    Create {
        /// Entry shape the fields are read as.
        #[arg(long = "type", value_name = "ENTRY_TYPE", value_parser = entry_type_parser())]
        entry_type: EntryType,
    },

    /// Prints one entry as JSON.
    Get { id: i64 },

    /// Prints entries, optionally of one type only.
    List {
        #[arg(long = "type", value_name = "ENTRY_TYPE", value_parser = entry_type_parser())]
        entry_type: Option<EntryType>,
    },

    /// Prints the entries a section is eligible to hold.
    Eligible { section: String },

    /// Replaces an entry's fields with the JSON on standard input.
    Update { id: i64 },

    /// Deletes one entry.
    Delete { id: i64 },
}

#[derive(Subcommand)]
enum BulletAction {
    /// Creates a bullet on an entry, worded by its first variant.
    Create {
        entry_id: i64,
        text: String,

        #[arg(long)]
        note: Option<String>,
    },

    /// Prints one bullet with its variants.
    Get { id: i64 },

    /// Prints every bullet of an entry.
    List { entry_id: i64 },

    /// Moves a bullet within its entry.
    Move { id: i64, position: i32 },

    /// Deletes a bullet and its variants.
    Delete { id: i64 },
}

#[derive(Subcommand)]
enum VariantAction {
    /// Adds another wording to a bullet.
    Create {
        bullet_id: i64,
        text: String,

        #[arg(long)]
        note: Option<String>,
    },

    /// Rewords a variant, renotes it, or both.
    Update {
        id: i64,

        #[arg(long)]
        text: Option<String>,

        /// An empty note clears it.
        #[arg(long)]
        note: Option<String>,
    },

    /// Makes a variant the default wording of its bullet.
    SetDefault { id: i64 },

    /// Deletes a variant, unless it is the last one.
    Delete { id: i64 },
}

#[derive(Subcommand)]
enum JobDescriptionAction {
    /// Creates a Job Description from JSON on standard input.
    Create,

    /// Prints one Job Description as JSON.
    Get { id: i64 },

    /// Prints Job Descriptions, optionally matching a full-text query.
    List { query: Option<String> },

    /// Deletes one Job Description.
    Delete { id: i64 },
}

#[derive(Subcommand)]
enum ProfileAction {
    /// Prints the stored Profile as JSON.
    Get,
    /// Replaces the stored Profile with the JSON on standard input.
    Set,
}

#[derive(Subcommand)]
enum RepositoryAction {
    Init,
    Branch { name: String },
    Checkout { branch: String },
    Delete { branch: String },
    Commit { message: String },
    Status,
    Diff,
    Log,
}

/// Awaits a fallible action and boxes its error for `main`'s uniform result type.
async fn run<E: std::error::Error + 'static>(
    action: impl std::future::Future<Output = Result<(), E>>,
) -> Result<(), Box<dyn std::error::Error>> {
    action.await.map_err(Into::into)
}

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = Cli::parse();
    let port = arguments.port;
    let bind = arguments.bind;
    let public_url = arguments.public_url;
    let result: Result<(), Box<dyn std::error::Error>> = match arguments.command {
        Command::Serve => run(serve::run(port, bind, public_url)).await,
        Command::Profile { action } => match action {
            ProfileAction::Get => run(cli::profile_get(port)).await,
            ProfileAction::Set => run(cli::profile_set(port)).await,
        },
        Command::Section { action } => match action {
            SectionAction::Create { name, entry_type } => {
                run(cli::section_create(port, &name, entry_type)).await
            }
            SectionAction::List => run(cli::section_list(port)).await,
            SectionAction::Rename { name, new_name } => {
                run(cli::section_rename(port, &name, &new_name)).await
            }
            SectionAction::Retype { name, entry_type } => {
                run(cli::section_retype(port, &name, entry_type)).await
            }
            SectionAction::Delete { name } => run(cli::section_delete(port, &name)).await,
        },
        Command::Entry { action } => match action {
            EntryAction::Create { entry_type } => run(cli::entry_create(port, entry_type)).await,
            EntryAction::Get { id } => run(cli::entry_get(port, id)).await,
            EntryAction::List { entry_type } => run(cli::entry_list(port, entry_type)).await,
            EntryAction::Eligible { section } => run(cli::entry_eligible(port, &section)).await,
            EntryAction::Update { id } => run(cli::entry_update(port, id)).await,
            EntryAction::Delete { id } => run(cli::entry_delete(port, id)).await,
        },
        Command::Bullet { action } => match action {
            BulletAction::Create {
                entry_id,
                text,
                note,
            } => run(cli::bullet_create(port, entry_id, &text, note)).await,
            BulletAction::Get { id } => run(cli::bullet_get(port, id)).await,
            BulletAction::List { entry_id } => run(cli::bullet_list(port, entry_id)).await,
            BulletAction::Move { id, position } => run(cli::bullet_move(port, id, position)).await,
            BulletAction::Delete { id } => run(cli::bullet_delete(port, id)).await,
        },
        Command::Variant { action } => match action {
            VariantAction::Create {
                bullet_id,
                text,
                note,
            } => run(cli::variant_create(port, bullet_id, &text, note)).await,
            VariantAction::Update { id, text, note } => {
                run(cli::variant_update(port, id, text, note)).await
            }
            VariantAction::SetDefault { id } => run(cli::variant_set_default(port, id)).await,
            VariantAction::Delete { id } => run(cli::variant_delete(port, id)).await,
        },
        Command::Search { query } => run(cli::search(port, &query)).await,
        Command::JobDescription { action } => match action {
            JobDescriptionAction::Create => run(cli::jd_create(port)).await,
            JobDescriptionAction::Get { id } => run(cli::jd_get(port, id)).await,
            JobDescriptionAction::List { query } => run(cli::jd_list(port, query)).await,
            JobDescriptionAction::Delete { id } => run(cli::jd_delete(port, id)).await,
        },
        Command::Repository { action } => match action {
            RepositoryAction::Init => run(cli::repository_init(port)).await,
            RepositoryAction::Branch { name } => run(cli::repository_branch(port, name)).await,
            RepositoryAction::Checkout { branch } => {
                run(cli::repository_checkout(port, branch)).await
            }
            RepositoryAction::Delete { branch } => run(cli::repository_delete(port, branch)).await,
            RepositoryAction::Commit { message } => {
                run(cli::repository_commit(port, message)).await
            }
            RepositoryAction::Status => run(cli::repository_status(port)).await,
            RepositoryAction::Diff => run(cli::repository_diff(port)).await,
            RepositoryAction::Log => run(cli::repository_log(port)).await,
        },
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(error.as_ref());
            ExitCode::FAILURE
        }
    }
}

/// Accepts the nine entry types by name, and lists them in `--help`.
fn entry_type_parser() -> impl TypedValueParser<Value = EntryType> {
    clap::builder::PossibleValuesParser::new(EntryType::ALL.map(EntryType::as_str))
        .map(|value| value.parse::<EntryType>().expect("clap checked the value"))
}

/// Prints an error with everything that caused it, innermost last.
fn report(error: &dyn std::error::Error) {
    eprintln!("error: {error}");
    let mut source = error.source();
    while let Some(cause) = source {
        eprintln!("  caused by: {cause}");
        source = cause.source();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_command_line_is_internally_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn a_section_is_created_with_its_type_behind_a_flag() {
        let arguments = Cli::parse_from([
            "hoskinator",
            "section",
            "create",
            "Selected Projects",
            "--type",
            "normal",
        ]);

        let Command::Section {
            action: SectionAction::Create { name, entry_type },
        } = arguments.command
        else {
            panic!("expected a section create");
        };
        assert_eq!(name, "Selected Projects");
        assert_eq!(entry_type, EntryType::Normal);
    }

    #[test]
    fn renaming_takes_the_old_name_then_the_new() {
        let arguments =
            Cli::parse_from(["hoskinator", "section", "rename", "Projects", "Selected"]);

        let Command::Section {
            action: SectionAction::Rename { name, new_name },
        } = arguments.command
        else {
            panic!("expected a section rename");
        };
        assert_eq!((name.as_str(), new_name.as_str()), ("Projects", "Selected"));
    }

    #[test]
    fn every_entry_type_is_accepted_on_the_command_line() {
        for entry_type in EntryType::ALL {
            let arguments = Cli::parse_from([
                "hoskinator",
                "section",
                "retype",
                "Writing",
                entry_type.as_str(),
            ]);

            let Command::Section {
                action:
                    SectionAction::Retype {
                        entry_type: parsed, ..
                    },
            } = arguments.command
            else {
                panic!("expected a section retype");
            };
            assert_eq!(parsed, entry_type);
        }
    }

    #[test]
    fn an_entry_type_rendercv_does_not_have_is_refused() {
        let refused =
            Cli::try_parse_from(["hoskinator", "section", "retype", "Writing", "timeline"]);

        assert!(refused.is_err(), "an unknown entry type should not parse");
    }

    #[test]
    fn the_port_defaults_to_the_documented_one() {
        let arguments = Cli::parse_from(["hoskinator", "serve"]);

        assert_eq!(arguments.port, serve::DEFAULT_PORT);
    }

    #[test]
    fn the_port_can_be_given_after_the_subcommand() {
        let arguments = Cli::parse_from(["hoskinator", "profile", "get", "--port", "9000"]);

        assert_eq!(arguments.port, 9000);
        assert!(matches!(
            arguments.command,
            Command::Profile {
                action: ProfileAction::Get
            }
        ));
    }

    #[test]
    fn profile_set_is_its_own_action() {
        let arguments = Cli::parse_from(["hoskinator", "profile", "set"]);

        assert!(matches!(
            arguments.command,
            Command::Profile {
                action: ProfileAction::Set
            }
        ));
    }

    #[test]
    fn an_entry_is_created_with_its_type_behind_a_flag() {
        let arguments = Cli::parse_from(["hoskinator", "entry", "create", "--type", "experience"]);

        let Command::Entry {
            action: EntryAction::Create { entry_type },
        } = arguments.command
        else {
            panic!("expected an entry create");
        };
        assert_eq!(entry_type, EntryType::Experience);
    }

    #[test]
    fn listing_entries_without_a_type_is_every_type() {
        let arguments = Cli::parse_from(["hoskinator", "entry", "list"]);

        assert!(matches!(
            arguments.command,
            Command::Entry {
                action: EntryAction::List { entry_type: None }
            }
        ));
    }

    #[test]
    fn entry_actions_have_their_own_subcommands() {
        for arguments in [
            ["hoskinator", "entry", "get", "17"].as_slice(),
            ["hoskinator", "entry", "list", "--type", "bullet"].as_slice(),
            ["hoskinator", "entry", "eligible", "Experience"].as_slice(),
            ["hoskinator", "entry", "update", "17"].as_slice(),
            ["hoskinator", "entry", "delete", "17"].as_slice(),
        ] {
            assert!(Cli::try_parse_from(arguments).is_ok(), "{arguments:?}");
        }
    }

    #[test]
    fn a_bullet_is_created_with_its_wording_and_an_optional_note() {
        let arguments = Cli::parse_from([
            "hoskinator",
            "bullet",
            "create",
            "17",
            "Cut latency in half.",
            "--note",
            "from the H1 review",
        ]);

        let Command::Bullet {
            action:
                BulletAction::Create {
                    entry_id,
                    text,
                    note,
                },
        } = arguments.command
        else {
            panic!("expected a bullet create");
        };
        assert_eq!(entry_id, 17);
        assert_eq!(text, "Cut latency in half.");
        assert_eq!(note.as_deref(), Some("from the H1 review"));
    }

    #[test]
    fn a_bullet_without_a_note_parses() {
        let arguments = Cli::parse_from(["hoskinator", "bullet", "create", "17", "One."]);

        assert!(matches!(
            arguments.command,
            Command::Bullet {
                action: BulletAction::Create { note: None, .. }
            }
        ));
    }

    #[test]
    fn bullet_and_variant_actions_have_their_own_subcommands() {
        for arguments in [
            ["hoskinator", "bullet", "get", "17"].as_slice(),
            ["hoskinator", "bullet", "list", "17"].as_slice(),
            ["hoskinator", "bullet", "move", "17", "0"].as_slice(),
            ["hoskinator", "bullet", "delete", "17"].as_slice(),
            ["hoskinator", "variant", "create", "17", "One."].as_slice(),
            ["hoskinator", "variant", "update", "17", "--text", "Two."].as_slice(),
            ["hoskinator", "variant", "update", "17", "--note", ""].as_slice(),
            ["hoskinator", "variant", "set-default", "17"].as_slice(),
            ["hoskinator", "variant", "delete", "17"].as_slice(),
        ] {
            assert!(Cli::try_parse_from(arguments).is_ok(), "{arguments:?}");
        }
    }

    #[test]
    fn search_takes_the_query_as_one_argument() {
        let arguments = Cli::parse_from(["hoskinator", "search", "p99 latency"]);

        let Command::Search { query } = arguments.command else {
            panic!("expected a search");
        };
        assert_eq!(query, "p99 latency");
    }

    #[test]
    fn job_description_actions_have_their_own_subcommands() {
        let create = Cli::parse_from(["hoskinator", "jd", "create"]);
        let get = Cli::parse_from(["hoskinator", "jd", "get", "17"]);
        let list = Cli::parse_from(["hoskinator", "jd", "list", "Rust"]);
        let delete = Cli::parse_from(["hoskinator", "jd", "delete", "17"]);

        assert!(matches!(
            create.command,
            Command::JobDescription {
                action: JobDescriptionAction::Create
            }
        ));
        assert!(matches!(
            get.command,
            Command::JobDescription {
                action: JobDescriptionAction::Get { id: 17 }
            }
        ));
        assert!(matches!(
            list.command,
            Command::JobDescription {
                action: JobDescriptionAction::List {
                    query: Some(query)
                }
            } if query == "Rust"
        ));
        assert!(matches!(
            delete.command,
            Command::JobDescription {
                action: JobDescriptionAction::Delete { id: 17 }
            }
        ));
    }

    #[test]
    fn repository_commands_accept_the_documented_forms() {
        for arguments in [
            ["hoskinator", "repository", "init"].as_slice(),
            ["hoskinator", "repository", "branch", "revision"].as_slice(),
            ["hoskinator", "repository", "checkout", "revision"].as_slice(),
            ["hoskinator", "repository", "commit", "message"].as_slice(),
            ["hoskinator", "repository", "status"].as_slice(),
            ["hoskinator", "repository", "diff"].as_slice(),
            ["hoskinator", "repository", "log"].as_slice(),
        ] {
            assert!(Cli::try_parse_from(arguments).is_ok());
        }
    }
}
