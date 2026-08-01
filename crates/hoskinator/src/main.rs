//! The Hoskinator binary.
//!
//! Per ADR-0005 this single executable is simultaneously the HTTP daemon, the CLI, and the
//! host serving the embedded Web UI. The `serve` daemon and the CLI subcommands arrive in
//! later PRs of this stack; for now this is scaffolding that proves the workspace and the
//! `hoskinator` -> `hoskinator-core` dependency boundary build.

fn main() {
    println!("hoskinator {}", env!("CARGO_PKG_VERSION"));
}
