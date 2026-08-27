//! Setting up the resume repository, and the GitHub account it lives on.
//!
//! GitHub is reached entirely through the `gh` CLI (docs/decisions/workspace.md): it sets up the
//! repository and configures git's own credential helper for pushes.

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
    #[error("could not create {path}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
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
    /// Where a new repository is cloned by default, absent a location of the user's choosing.
    pub default_repository_root: PathBuf,
}

/// Reads how the workspace stands.
pub fn status(
    repository_path: Option<&Path>,
    applications_sheet: Option<&str>,
    default_repository_root: &Path,
) -> WorkspaceStatus {
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
        default_repository_root: default_repository_root.to_path_buf(),
    }
}

/// Creates a private GitHub repository and clones it to `destination`.
pub fn create_github(name: &str, destination: &Path) -> Result<PathBuf, WorkspaceError> {
    require_gh()?;
    ensure_free(destination)?;
    let parent = ensure_parent(destination)?;

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
        &parent,
    )?;

    Ok(destination.to_path_buf())
}

/// Clones an existing repository to `destination`. `source` is anything `gh repo clone` accepts.
///
/// A no-op if `destination` already holds a clone of `source`.
pub fn connect_github(source: &str, destination: &Path) -> Result<PathBuf, WorkspaceError> {
    require_gh()?;

    if already_connected(destination, source) {
        return Ok(destination.to_path_buf());
    }

    ensure_free(destination)?;
    let parent = ensure_parent(destination)?;

    run(
        GH,
        &["repo", "clone", source, &destination.to_string_lossy()],
        &parent,
    )?;

    Ok(destination.to_path_buf())
}

/// Whether `destination` already holds a clone of `source`.
fn already_connected(destination: &Path, source: &str) -> bool {
    if !destination.join(".git").exists() {
        return false;
    }
    remote(destination).is_some_and(|origin| same_repository(&origin, source))
}

/// Whether a remote URL and a `gh repo clone`-style source name the same repository, ignoring
/// scheme, host, and a trailing `.git`.
fn same_repository(remote_url: &str, source: &str) -> bool {
    owner_and_name(remote_url) == owner_and_name(source)
}

/// The trailing `owner/name` of a repository reference — a bare `owner/name`, or a `.git`-suffixed
/// HTTPS or SSH remote URL.
fn owner_and_name(reference: &str) -> String {
    let trimmed = reference.trim_end_matches('/').trim_end_matches(".git");
    let mut segments: Vec<&str> = trimmed.rsplit(['/', ':']).take(2).collect();
    segments.reverse();
    segments.join("/").to_lowercase()
}

/// Pushes the current branch to `origin`, setting upstream the first time.
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

fn push_refs(repository_path: &Path, arguments: &[String]) -> Result<(), WorkspaceError> {
    let mut command = Command::new("git");
    // `gh` configures itself as git's credential helper for github.com on `gh auth login`
    // (docs/decisions/workspace.md), so a plain push already authenticates.
    command
        .current_dir(repository_path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(arguments);

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

/// `destination`'s parent, creating it if it does not exist yet.
fn ensure_parent(destination: &Path) -> Result<PathBuf, WorkspaceError> {
    let parent = destination.parent().unwrap_or(Path::new(".")).to_path_buf();
    std::fs::create_dir_all(&parent).map_err(|source| WorkspaceError::CreateDirectory {
        path: parent.clone(),
        source,
    })?;
    Ok(parent)
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

/// The `owner/name` a GitHub remote URL names, or `None` if it doesn't look like one. Used to
/// scope data that belongs to one resume repository — applications, primarily.
pub fn repository_slug(remote_url: &str) -> Option<String> {
    let normalized = remote_url.trim().trim_end_matches(".git").replace(':', "/");
    let mut parts: Vec<&str> = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    let name = parts.pop()?;
    let owner = parts.pop()?;
    Some(format!("{owner}/{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_https_remote_names_owner_and_repository() {
        assert_eq!(
            repository_slug("https://github.com/clmazzac/resume-store.git"),
            Some("clmazzac/resume-store".to_string())
        );
    }

    #[test]
    fn an_ssh_remote_names_owner_and_repository() {
        assert_eq!(
            repository_slug("git@github.com:clmazzac/resume-store.git"),
            Some("clmazzac/resume-store".to_string())
        );
    }

    #[test]
    fn a_url_with_no_owner_segment_is_not_a_slug() {
        assert_eq!(repository_slug("resume-store"), None);
    }

    #[test]
    fn a_bare_owner_and_name_matches_itself() {
        assert!(same_repository("clmazzac/resume-store", "clmazzac/resume-store"));
    }

    #[test]
    fn an_https_remote_matches_the_bare_source_it_was_cloned_from() {
        assert!(same_repository(
            "https://github.com/clmazzac/resume-store.git",
            "clmazzac/resume-store",
        ));
    }

    #[test]
    fn an_ssh_remote_matches_the_bare_source_it_was_cloned_from() {
        assert!(same_repository(
            "git@github.com:clmazzac/resume-store.git",
            "clmazzac/resume-store",
        ));
    }

    #[test]
    fn comparison_is_case_insensitive() {
        assert!(same_repository("Clmazzac/Resume-Store", "clmazzac/resume-store"));
    }

    #[test]
    fn a_different_repository_does_not_match() {
        assert!(!same_repository(
            "clmazzac/resume-store",
            "clmazzac/hoskinator",
        ));
    }
}
