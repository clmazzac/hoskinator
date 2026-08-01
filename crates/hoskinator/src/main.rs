//! Hoskinator: daemon, CLI, and Web UI host in one binary (ADR-0005).

use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod rpc;
mod serve;

#[derive(Parser)]
#[command(name = "hoskinator", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Runs the daemon until interrupted.
    Serve {
        /// Port to bind on loopback.
        #[arg(long, default_value_t = serve::DEFAULT_PORT)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Serve { port } => serve::run(port).await,
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&error);
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
    fn serve_defaults_to_the_documented_port() {
        let cli = Cli::parse_from(["hoskinator", "serve"]);

        let Command::Serve { port } = cli.command;
        assert_eq!(port, serve::DEFAULT_PORT);
    }

    #[test]
    fn serve_accepts_an_explicit_port() {
        let cli = Cli::parse_from(["hoskinator", "serve", "--port", "9000"]);

        let Command::Serve { port } = cli.command;
        assert_eq!(port, 9000);
    }
}
