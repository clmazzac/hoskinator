//! Hoskinator: daemon, CLI, and Web UI host in one binary (ADR-0005).

use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
}

#[derive(Subcommand)]
enum ProfileAction {
    /// Prints the stored Profile as JSON.
    Get,

    /// Replaces the stored Profile with the JSON on standard input.
    Set,
}

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = Cli::parse();
    let port = arguments.port;

    let result: Result<(), Box<dyn std::error::Error>> = match arguments.command {
        Command::Serve => serve::run(port).await.map_err(Into::into),
        Command::Profile { action } => match action {
            ProfileAction::Get => cli::profile_get(port).await.map_err(Into::into),
            ProfileAction::Set => cli::profile_set(port).await.map_err(Into::into),
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
}
