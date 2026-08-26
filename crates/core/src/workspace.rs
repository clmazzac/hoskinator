//! Setting up the resume repository, and the GitHub account it lives on.
//!
//! GitHub is reached through the `gh` CLI when setting the repository up, and through a
//! personal access token once one is authorized: pushes authenticate with the stored token,
//! while setup keeps using the account the user is already signed in with.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

const GH: &str = "gh";

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("the GitHub CLI (gh) is not installed")]
    GhMissing,
    #[error("not signed in to GitHub; run `gh auth login`")]
    GhSignedOut,
    #[error("git or gh failed: {0}")]
    Command(String),
    #[error("could not run {program}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} already exists and is not an empty directory")]
    Occupied { path: PathBuf },
    #[error("could not write the configuration")]
    Config(#[source] std::io::Error),
}

/// Who is signed in to GitHub, and whether a repository is set up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceStatus {
    /// Whether the `gh` CLI is on PATH.
    pub gh_installed: bool,
    /// The signed-in GitHub login, if there is one.
    pub github_login: Option<String>,
    /// The configured resume repository, if one is configured.
    pub repository_path: Option<PathBuf>,
    /// Whether that path holds a git repository.
    pub repository_ready: bool,
    /// The `origin` remote's URL, if the repository has one.
    pub remote_url: Option<String>,
    /// The id of the Google Sheet the application tracker syncs from, if one is linked.
    pub applications_sheet: Option<String>,
}

/// Reads how the workspace stands.
pub fn status(repository_path: Option<&Path>, applications_sheet: Option<&str>) -> WorkspaceStatus {
    let gh_installed = which(GH);
    let github_login = if gh_installed { login() } else { None };
    let repository_ready = repository_path.is_some_and(|path| path.join(".git").exists());
    let remote_url = if repository_ready {
        repository_path.and_then(remote)
    } else {
        None
    };

    WorkspaceStatus {
        gh_installed,
        github_login,
        repository_path: repository_path.map(Path::to_path_buf),
        repository_ready,
        remote_url,
        applications_sheet: applications_sheet.map(str::to_string),
    }
}

/// Creates a private GitHub repository and clones it to `destination`.
pub fn create_github(name: &str, destination: &Path) -> Result<PathBuf, WorkspaceError> {
    require_gh()?;
    ensure_free(destination)?;

    run(
        GH,
        &[
            "repo",
            "create",
            name,
            "--private",
            "--clone",
            "--description",
            "Resumes, managed by Hoskinator",
        ],
        destination.parent().unwrap_or(Path::new(".")),
    )?;

    Ok(destination.to_path_buf())
}

/// Clones an existing repository to `destination`. `source` is anything `gh repo clone` accepts.
pub fn connect_github(source: &str, destination: &Path) -> Result<PathBuf, WorkspaceError> {
    require_gh()?;
    ensure_free(destination)?;

    run(
        GH,
        &["repo", "clone", source, &destination.to_string_lossy()],
        destination.parent().unwrap_or(Path::new(".")),
    )?;

    Ok(destination.to_path_buf())
}

/// Pushes the current branch to `origin`, setting upstream the first time.
///
/// When a GitHub token is stored, it rides in a per-invocation credential helper, so pushes
/// over HTTPS authenticate without the token landing in the repository's configuration.
pub fn push(repository_path: &Path, branch: &str) -> Result<(), WorkspaceError> {
    push_refs(
        repository_path,
        &[
            "push".into(),
            "--set-upstream".into(),
            "origin".into(),
            branch.into(),
        ],
    )
}

/// Pushes every local branch, so a newly connected repository receives the whole resume.
pub fn push_all(repository_path: &Path) -> Result<(), WorkspaceError> {
    push_refs(
        repository_path,
        &[
            "push".into(),
            "origin".into(),
            "refs/heads/*:refs/heads/*".into(),
        ],
    )
}

fn push_refs(repository_path: &Path, arguments: &[String]) -> Result<(), WorkspaceError> {
    let mut command = Command::new("git");
    command
        .current_dir(repository_path)
        .env("GIT_TERMINAL_PROMPT", "0");

    // The `-c` option must come before the subcommand. The helper reads the token from the
    // environment, so the token never appears on the command line or in the repository's own
    // configuration.
    if let Ok(Some(token)) = crate::github::read_token() {
        command.arg("-c").arg(
            "credential.helper=!f() { printf 'username=x-access-token\\npassword=%s\\n' \"$HOSKINATOR_GITHUB_TOKEN\"; }; f",
        );
        command.env("HOSKINATOR_GITHUB_TOKEN", token);
    }
    command.args(arguments);

    let output = command.output().map_err(|source| WorkspaceError::Spawn {
        program: "git".to_string(),
        source,
    })?;

    if output.status.success() {
        return Ok(());
    }
    let mut reported = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if reported.is_empty() {
        reported = String::from_utf8_lossy(&output.stdout).trim().to_string();
    }
    Err(WorkspaceError::Command(reported))
}

/// Points `origin` at `url`, whether or not a remote is already set.
pub fn connect_remote(repository_path: &Path, url: &str) -> Result<(), WorkspaceError> {
    if remote(repository_path).is_some() {
        run(
            "git",
            &["remote", "set-url", "origin", url],
            repository_path,
        )
        .map(|_| ())
    } else {
        run("git", &["remote", "add", "origin", url], repository_path).map(|_| ())
    }
}

/// Every repository the signed-in account owns, newest first.
pub fn owned_repositories() -> Result<Vec<String>, WorkspaceError> {
    require_gh()?;
    let listed = run(
        GH,
        &[
            "repo",
            "list",
            "--limit",
            "100",
            "--json",
            "nameWithOwner",
            "--jq",
            ".[].nameWithOwner",
        ],
        Path::new("."),
    )?;
    Ok(listed.lines().map(str::to_string).collect())
}

fn require_gh() -> Result<(), WorkspaceError> {
    if !which(GH) {
        return Err(WorkspaceError::GhMissing);
    }
    if login().is_none() {
        return Err(WorkspaceError::GhSignedOut);
    }
    Ok(())
}

fn ensure_free(destination: &Path) -> Result<(), WorkspaceError> {
    if !destination.exists() {
        return Ok(());
    }
    let empty = std::fs::read_dir(destination)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false);
    if empty {
        return Ok(());
    }
    Err(WorkspaceError::Occupied {
        path: destination.to_path_buf(),
    })
}

fn which(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn login() -> Option<String> {
    let output = Command::new(GH)
        .args(["api", "user", "--jq", ".login"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let login = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!login.is_empty()).then_some(login)
}

fn remote(repository_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(repository_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!url.is_empty()).then_some(url)
}

fn run(program: &str, args: &[&str], directory: &Path) -> Result<String, WorkspaceError> {
    let output = Command::new(program)
        .args(args)
        .current_dir(directory)
        .output()
        .map_err(|source| WorkspaceError::Spawn {
            program: program.to_string(),
            source,
        })?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }

    let mut reported = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if reported.is_empty() {
        reported = String::from_utf8_lossy(&output.stdout).trim().to_string();
    }
    Err(WorkspaceError::Command(reported))
}

/// Writes `resume_repo` into the config file, keeping any `home` already set.
pub fn remember_repository(config_path: &Path, repository: &Path) -> Result<(), WorkspaceError> {
    let existing = std::fs::read_to_string(config_path).unwrap_or_default();
    let kept: Vec<&str> = existing
        .lines()
        .filter(|line| !line.trim_start().starts_with("resume_repo"))
        .collect();

    let mut written = kept.join("\n");
    if !written.is_empty() && !written.ends_with('\n') {
        written.push('\n');
    }
    written.push_str(&format!(
        "resume_repo = \"{}\"\n",
        repository.to_string_lossy()
    ));

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(WorkspaceError::Config)?;
    }
    std::fs::write(config_path, written).map_err(WorkspaceError::Config)
}
