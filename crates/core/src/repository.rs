//! Typed, synchronous access to the configured resume Git repository.

use std::path::{Path, PathBuf};

use git2::{BranchType, ErrorCode, Repository, Status, StatusOptions};
use serde::{Deserialize, Serialize};

const LOG_LIMIT: usize = 50;

pub struct ResumeRepository {
    repository: Repository,
}

#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("no resume repository is configured")]
    MissingConfiguration,
    #[error("could not open or initialize the repository at {path}")]
    OpenOrInitialize {
        path: PathBuf,
        #[source]
        source: git2::Error,
    },
    #[error("repository branch or revision was not found: {name}")]
    NotFound { name: String },
    #[error("repository precondition failed: {message}")]
    Conflict { message: String },
    #[error("invalid repository request: {message}")]
    InvalidRequest { message: String },
    #[error("Git identity is unavailable")]
    IdentityUnavailable(#[source] git2::Error),
    #[error("repository operation failed")]
    Operation(#[source] git2::Error),
    #[error("could not write {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryState {
    pub head: Option<Head>,
    pub branches: Vec<Branch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Head {
    pub branch: Option<String>,
    pub commit_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub commit_id: Option<String>,
    pub is_head: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateBranchRequest {
    pub name: String,
    /// The branch it starts from. HEAD when absent.
    #[serde(default)]
    pub from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutRequest {
    pub branch: String,
}

/// What a merge did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeOutcome {
    /// The branch merged into.
    pub branch: String,
    /// The branch merged from.
    pub from: String,
    /// One of `already-current`, `fast-forward`, or `merged`.
    pub kind: String,
    pub commit_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitRequest {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitRecord {
    pub id: String,
    pub message: String,
    pub author: CommitAuthor,
    pub committed_at: GitTime,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitAuthor {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitTime {
    pub seconds_since_epoch: i64,
    pub offset_minutes: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryStatus {
    pub head: Option<Head>,
    pub entries: Vec<StatusEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEntry {
    pub path: String,
    pub old_path: Option<String>,
    pub index: Option<FileChange>,
    pub worktree: Option<FileChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChange {
    Added,
    Modified,
    Deleted,
    Renamed,
    Typechange,
    Conflicted,
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryDiff {
    pub files: Vec<FileDiff>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiff {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub status: FileChange,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryLog {
    pub commits: Vec<CommitRecord>,
}

impl ResumeRepository {
    pub fn open_or_init(path: &Path) -> Result<Self, RepositoryError> {
        let repository = match Repository::open(path) {
            Ok(repository) => Ok(repository),
            Err(error) if error.code() == ErrorCode::NotFound => Repository::init(path),
            Err(error) => Err(error),
        }
        .map_err(|source| RepositoryError::OpenOrInitialize {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self { repository })
    }

    pub fn state(&self) -> Result<RepositoryState, RepositoryError> {
        let head = self.head()?;
        let mut branches = Vec::new();
        for branch in self
            .repository
            .branches(Some(BranchType::Local))
            .map_err(RepositoryError::Operation)?
        {
            let (branch, _) = branch.map_err(RepositoryError::Operation)?;
            let name = branch
                .name()
                .map_err(RepositoryError::Operation)?
                .ok_or_else(non_utf8_error)?
                .to_owned();
            branches.push(Branch {
                is_head: head.as_ref().and_then(|head| head.branch.as_ref()) == Some(&name),
                commit_id: branch.get().target().map(|id| id.to_string()),
                name,
            });
        }
        branches.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(RepositoryState { head, branches })
    }

    pub fn create_branch(&self, request: CreateBranchRequest) -> Result<Branch, RepositoryError> {
        if !git2::Reference::is_valid_name(&format!("refs/heads/{}", request.name)) {
            return Err(RepositoryError::InvalidRequest {
                message: "branch name is invalid".into(),
            });
        }
        if self
            .repository
            .find_branch(&request.name, BranchType::Local)
            .is_ok()
        {
            return Err(RepositoryError::Conflict {
                message: "branch already exists".into(),
            });
        }
        let commit = match &request.from {
            Some(from) => self
                .repository
                .find_branch(from, BranchType::Local)
                .map_err(|_| RepositoryError::NotFound { name: from.clone() })?
                .get()
                .peel_to_commit()
                .map_err(|_| precondition_error())?,
            None => {
                let head = self.local_head()?;
                head.peel_to_commit().map_err(|_| precondition_error())?
            }
        };
        self.repository
            .branch(&request.name, &commit, false)
            .map_err(RepositoryError::Operation)?;
        Ok(Branch {
            name: request.name,
            commit_id: Some(commit.id().to_string()),
            is_head: false,
        })
    }

    /// Deletes a local branch. The open branch cannot be deleted, and neither can a
    /// branch whose commits no other local branch reaches.
    pub fn delete_branch(&self, name: &str) -> Result<(), RepositoryError> {
        let mut branch = self
            .repository
            .find_branch(name, BranchType::Local)
            .map_err(|_| RepositoryError::NotFound {
                name: name.to_owned(),
            })?;
        if branch.is_head() {
            return Err(precondition_error());
        }
        let tip = branch
            .get()
            .peel_to_commit()
            .map_err(|_| precondition_error())?;
        let reached_elsewhere = self
            .repository
            .branches(Some(BranchType::Local))
            .map_err(RepositoryError::Operation)?
            .filter_map(Result::ok)
            .filter(|(other, _)| !matches!(other.name(), Ok(Some(existing)) if existing == name))
            .filter_map(|(other, _)| other.get().peel_to_commit().ok())
            .any(|commit| {
                commit.id() == tip.id()
                    || self
                        .repository
                        .graph_descendant_of(commit.id(), tip.id())
                        .unwrap_or(false)
            });
        if !reached_elsewhere {
            return Err(RepositoryError::Conflict {
                message: "branch has commits no other branch reaches".into(),
            });
        }
        branch.delete().map_err(RepositoryError::Operation)
    }

    pub fn checkout(&self, request: CheckoutRequest) -> Result<RepositoryState, RepositoryError> {
        let branch = self
            .repository
            .find_branch(&request.branch, BranchType::Local)
            .map_err(|error| {
                if error.code() == ErrorCode::NotFound {
                    RepositoryError::NotFound {
                        name: request.branch.clone(),
                    }
                } else {
                    RepositoryError::Operation(error)
                }
            })?;
        let name = branch.get().name().ok_or_else(non_utf8_error)?.to_owned();
        let target = branch
            .get()
            .peel(git2::ObjectType::Tree)
            .map_err(RepositoryError::Operation)?;
        self.repository
            .checkout_tree(&target, Some(git2::build::CheckoutBuilder::new().safe()))
            .map_err(|error| match error.code() {
                ErrorCode::Conflict => RepositoryError::Conflict {
                    message: "safe checkout would overwrite worktree changes".into(),
                },
                _ => RepositoryError::Operation(error),
            })?;
        self.repository
            .set_head(&name)
            .map_err(RepositoryError::Operation)?;
        self.state()
    }

    pub fn commit(&self, request: CommitRequest) -> Result<CommitRecord, RepositoryError> {
        if request.message.trim().is_empty() {
            return Err(RepositoryError::InvalidRequest {
                message: "commit message must not be empty".into(),
            });
        }
        let parent = self
            .local_head()?
            .peel_to_commit()
            .map_err(|_| precondition_error())?;
        let statuses = self
            .repository
            .statuses(None)
            .map_err(RepositoryError::Operation)?;
        let mut index = self
            .repository
            .index()
            .map_err(RepositoryError::Operation)?;
        for entry in statuses.iter() {
            let status = entry.status();
            if status.contains(Status::WT_NEW) {
                continue;
            }
            let path = entry.path().ok_or_else(non_utf8_error)?;
            if status.intersects(Status::WT_DELETED | Status::INDEX_DELETED) {
                index
                    .remove_path(Path::new(path))
                    .map_err(RepositoryError::Operation)?;
            } else if status.intersects(
                Status::WT_MODIFIED
                    | Status::WT_TYPECHANGE
                    | Status::INDEX_MODIFIED
                    | Status::INDEX_TYPECHANGE,
            ) {
                index
                    .add_path(Path::new(path))
                    .map_err(RepositoryError::Operation)?;
            }
        }
        index.write().map_err(RepositoryError::Operation)?;
        let tree_id = index.write_tree().map_err(RepositoryError::Operation)?;
        if tree_id == parent.tree_id() {
            return Err(RepositoryError::Conflict {
                message: "there are no tracked changes to commit".into(),
            });
        }
        let tree = self
            .repository
            .find_tree(tree_id)
            .map_err(RepositoryError::Operation)?;
        let signature = self
            .repository
            .signature()
            .map_err(RepositoryError::IdentityUnavailable)?;
        let id = self
            .repository
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                &request.message,
                &tree,
                &[&parent],
            )
            .map_err(RepositoryError::Operation)?;
        self.record(id)
    }

    /// Writes `contents` to `relative_path` within the repository and stages it, so the next
    /// `commit` picks it up alongside whatever else changed.
    pub fn write_staged(&self, relative_path: &str, contents: &str) -> Result<(), RepositoryError> {
        let workdir = self.repository.workdir().ok_or_else(|| {
            RepositoryError::Operation(git2::Error::from_str("repository has no working directory"))
        })?;
        let path = workdir.join(relative_path);
        std::fs::write(&path, contents).map_err(|source| RepositoryError::Io { path, source })?;

        let mut index = self.repository.index().map_err(RepositoryError::Operation)?;
        index
            .add_path(Path::new(relative_path))
            .map_err(RepositoryError::Operation)?;
        index.write().map_err(RepositoryError::Operation)
    }

    pub fn status(&self) -> Result<RepositoryStatus, RepositoryError> {
        let mut options = StatusOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .renames_index_to_workdir(true)
            .renames_head_to_index(true);
        let mut entries = self
            .repository
            .statuses(Some(&mut options))
            .map_err(RepositoryError::Operation)?
            .iter()
            .map(|entry| {
                let status = entry.status();
                let index = index_change(status);
                let worktree = worktree_change(status);
                let rename = if index == Some(FileChange::Renamed) {
                    entry.head_to_index()
                } else if worktree == Some(FileChange::Renamed) {
                    entry.index_to_workdir()
                } else {
                    None
                };
                let path = if let Some(delta) = rename.as_ref() {
                    delta
                        .new_file()
                        .path()
                        .map(utf8_path)
                        .transpose()?
                        .ok_or_else(non_utf8_error)?
                } else {
                    entry.path().map(str::to_owned).ok_or_else(non_utf8_error)?
                };
                let old_path = rename
                    .as_ref()
                    .and_then(|delta| delta.old_file().path())
                    .map(utf8_path)
                    .transpose()?;
                Ok(StatusEntry {
                    path,
                    old_path,
                    index,
                    worktree,
                })
            })
            .collect::<Result<Vec<_>, RepositoryError>>()?;
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(RepositoryStatus {
            head: self.head()?,
            entries,
        })
    }

    pub fn diff(&self) -> Result<RepositoryDiff, RepositoryError> {
        let head_tree = self
            .repository
            .head()
            .ok()
            .and_then(|head| head.peel_to_tree().ok());
        let mut options = git2::DiffOptions::new();
        options
            .include_untracked(false)
            .include_typechange(true)
            .recurse_untracked_dirs(false);
        let mut diff = self
            .repository
            .diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut options))
            .map_err(RepositoryError::Operation)?;
        diff.find_similar(None)
            .map_err(RepositoryError::Operation)?;
        let mut files = Vec::new();
        for index in 0..diff.deltas().len() {
            let delta = diff.get_delta(index).ok_or_else(|| {
                RepositoryError::Operation(git2::Error::from_str("missing diff delta"))
            })?;
            let old_path = delta.old_file().path().map(utf8_path).transpose()?;
            let new_path = delta.new_file().path().map(utf8_path).transpose()?;
            let mut file = FileDiff {
                old_path,
                new_path,
                status: delta_change(delta.status()),
                hunks: Vec::new(),
            };
            if !delta.flags().contains(git2::DiffFlags::BINARY)
                && let Some(patch) =
                    git2::Patch::from_diff(&diff, index).map_err(RepositoryError::Operation)?
            {
                for hunk_index in 0..patch.num_hunks() {
                    let (hunk, lines) =
                        patch.hunk(hunk_index).map_err(RepositoryError::Operation)?;
                    let mut output = DiffHunk {
                        old_start: hunk.old_start(),
                        old_lines: hunk.old_lines(),
                        new_start: hunk.new_start(),
                        new_lines: hunk.new_lines(),
                        lines: Vec::new(),
                    };
                    for line_index in 0..lines {
                        let line = patch
                            .line_in_hunk(hunk_index, line_index)
                            .map_err(RepositoryError::Operation)?;
                        let kind = match line.origin() {
                            ' ' => DiffLineKind::Context,
                            '+' => DiffLineKind::Addition,
                            '-' => DiffLineKind::Deletion,
                            _ => continue,
                        };
                        output.lines.push(DiffLine {
                            kind,
                            old_line: line.old_lineno(),
                            new_line: line.new_lineno(),
                            content: std::str::from_utf8(line.content())
                                .map_err(|_| {
                                    RepositoryError::Operation(git2::Error::from_str(
                                        "diff content is not UTF-8",
                                    ))
                                })?
                                .to_owned(),
                        });
                    }
                    file.hunks.push(output);
                }
            }
            files.push(file);
        }
        Ok(RepositoryDiff { files })
    }

    pub fn log(&self) -> Result<RepositoryLog, RepositoryError> {
        let mut walk = self
            .repository
            .revwalk()
            .map_err(RepositoryError::Operation)?;
        if self.repository.head().is_ok() {
            walk.push_head().map_err(RepositoryError::Operation)?;
        }
        walk.set_sorting(git2::Sort::TIME)
            .map_err(RepositoryError::Operation)?;
        Ok(RepositoryLog {
            commits: walk
                .take(LOG_LIMIT)
                .map(|id| {
                    id.map_err(RepositoryError::Operation)
                        .and_then(|id| self.record(id))
                })
                .collect::<Result<_, _>>()?,
        })
    }

    /// Merges `from` into the checked-out branch.
    ///
    /// Fast-forwards where it can. A merge that would leave conflicts is refused and nothing is
    /// written, because a half-merged resume.yaml is worse than no merge.
    pub fn merge(&self, from: &str) -> Result<MergeOutcome, RepositoryError> {
        let source = self
            .repository
            .find_branch(from, BranchType::Local)
            .map_err(|_| RepositoryError::NotFound {
                name: from.to_string(),
            })?;
        let source_commit = source
            .get()
            .peel_to_commit()
            .map_err(|_| precondition_error())?;

        let head = self.local_head()?;
        let head_commit = head.peel_to_commit().map_err(|_| precondition_error())?;
        let head_name = head.shorthand().unwrap_or_default().to_string();

        if head_commit.id() == source_commit.id() {
            return Ok(MergeOutcome {
                branch: head_name,
                from: from.to_string(),
                kind: "already-current".into(),
                commit_id: Some(head_commit.id().to_string()),
            });
        }

        let annotated = self
            .repository
            .find_annotated_commit(source_commit.id())
            .map_err(RepositoryError::Operation)?;
        let (analysis, _) = self
            .repository
            .merge_analysis(&[&annotated])
            .map_err(RepositoryError::Operation)?;

        if analysis.is_up_to_date() {
            return Ok(MergeOutcome {
                branch: head_name,
                from: from.to_string(),
                kind: "already-current".into(),
                commit_id: Some(head_commit.id().to_string()),
            });
        }

        if analysis.is_fast_forward() {
            let mut reference = self
                .repository
                .find_reference(head.name().ok_or_else(non_utf8_error)?)
                .map_err(RepositoryError::Operation)?;
            reference
                .set_target(source_commit.id(), "merge: fast-forward")
                .map_err(RepositoryError::Operation)?;
            self.repository
                .set_head(reference.name().ok_or_else(non_utf8_error)?)
                .map_err(RepositoryError::Operation)?;
            self.repository
                .checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                .map_err(RepositoryError::Operation)?;
            return Ok(MergeOutcome {
                branch: head_name,
                from: from.to_string(),
                kind: "fast-forward".into(),
                commit_id: Some(source_commit.id().to_string()),
            });
        }

        let mut index = self
            .repository
            .merge_commits(&head_commit, &source_commit, None)
            .map_err(RepositoryError::Operation)?;

        if index.has_conflicts() {
            let paths: Vec<String> = index
                .conflicts()
                .map_err(RepositoryError::Operation)?
                .filter_map(|conflict| conflict.ok())
                .filter_map(|conflict| {
                    conflict
                        .our
                        .or(conflict.their)
                        .and_then(|entry| String::from_utf8(entry.path).ok())
                })
                .collect();
            return Err(RepositoryError::Conflict {
                message: format!("merge would conflict in {}", paths.join(", ")),
            });
        }

        let tree_id = index
            .write_tree_to(&self.repository)
            .map_err(RepositoryError::Operation)?;
        let tree = self
            .repository
            .find_tree(tree_id)
            .map_err(RepositoryError::Operation)?;
        let signature = self
            .repository
            .signature()
            .map_err(RepositoryError::IdentityUnavailable)?;
        let id = self
            .repository
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                &format!("Merge {from} into {head_name}"),
                &tree,
                &[&head_commit, &source_commit],
            )
            .map_err(RepositoryError::Operation)?;
        self.repository
            .checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(RepositoryError::Operation)?;

        Ok(MergeOutcome {
            branch: head_name,
            from: from.to_string(),
            kind: "merged".into(),
            commit_id: Some(id.to_string()),
        })
    }

    fn local_head(&self) -> Result<git2::Reference<'_>, RepositoryError> {
        let head = self.repository.head().map_err(|_| precondition_error())?;
        if head.is_branch() {
            Ok(head)
        } else {
            Err(precondition_error())
        }
    }

    fn head(&self) -> Result<Option<Head>, RepositoryError> {
        match self.repository.head() {
            Ok(head) => Ok(Some(Head {
                branch: head
                    .shorthand()
                    .filter(|_| head.is_branch())
                    .map(str::to_owned),
                commit_id: head.target().map(|id| id.to_string()),
            })),
            Err(error) if matches!(error.code(), ErrorCode::UnbornBranch | ErrorCode::NotFound) => {
                Ok(Some(Head {
                    branch: None,
                    commit_id: None,
                }))
            }
            Err(error) => Err(RepositoryError::Operation(error)),
        }
    }

    fn record(&self, id: git2::Oid) -> Result<CommitRecord, RepositoryError> {
        let commit = self
            .repository
            .find_commit(id)
            .map_err(RepositoryError::Operation)?;
        let author = commit.author();
        let time = author.when();
        Ok(CommitRecord {
            id: id.to_string(),
            message: commit.message().ok_or_else(non_utf8_error)?.to_owned(),
            author: CommitAuthor {
                name: author.name().ok_or_else(non_utf8_error)?.to_owned(),
                email: author.email().ok_or_else(non_utf8_error)?.to_owned(),
            },
            committed_at: GitTime {
                seconds_since_epoch: time.seconds(),
                offset_minutes: time.offset_minutes(),
            },
            parents: commit.parent_ids().map(|id| id.to_string()).collect(),
        })
    }
}

fn precondition_error() -> RepositoryError {
    RepositoryError::Conflict {
        message: "current HEAD must name a commit on a local branch".into(),
    }
}

fn non_utf8_error() -> RepositoryError {
    RepositoryError::Operation(git2::Error::from_str("repository data is not UTF-8"))
}

fn utf8_path(path: &Path) -> Result<String, RepositoryError> {
    path.to_str().map(str::to_owned).ok_or_else(non_utf8_error)
}

fn index_change(status: Status) -> Option<FileChange> {
    if status.contains(Status::CONFLICTED) {
        Some(FileChange::Conflicted)
    } else if status.contains(Status::INDEX_NEW) {
        Some(FileChange::Added)
    } else if status.contains(Status::INDEX_MODIFIED) {
        Some(FileChange::Modified)
    } else if status.contains(Status::INDEX_DELETED) {
        Some(FileChange::Deleted)
    } else if status.contains(Status::INDEX_RENAMED) {
        Some(FileChange::Renamed)
    } else if status.contains(Status::INDEX_TYPECHANGE) {
        Some(FileChange::Typechange)
    } else {
        None
    }
}

fn worktree_change(status: Status) -> Option<FileChange> {
    if status.contains(Status::CONFLICTED) {
        Some(FileChange::Conflicted)
    } else if status.contains(Status::WT_NEW) {
        Some(FileChange::Untracked)
    } else if status.contains(Status::WT_MODIFIED) {
        Some(FileChange::Modified)
    } else if status.contains(Status::WT_DELETED) {
        Some(FileChange::Deleted)
    } else if status.contains(Status::WT_RENAMED) {
        Some(FileChange::Renamed)
    } else if status.contains(Status::WT_TYPECHANGE) {
        Some(FileChange::Typechange)
    } else {
        None
    }
}

fn delta_change(status: git2::Delta) -> FileChange {
    match status {
        git2::Delta::Added => FileChange::Added,
        git2::Delta::Modified => FileChange::Modified,
        git2::Delta::Deleted => FileChange::Deleted,
        git2::Delta::Renamed => FileChange::Renamed,
        git2::Delta::Typechange => FileChange::Typechange,
        git2::Delta::Conflicted => FileChange::Conflicted,
        _ => FileChange::Modified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A repository with one commit on its initial branch.
    fn seeded() -> (TempDir, ResumeRepository) {
        let (dir, repository) = repo();
        initial_commit(&repository, &dir);
        (dir, repository)
    }

    fn repo() -> (TempDir, ResumeRepository) {
        let dir = TempDir::new().unwrap();
        let repository = ResumeRepository::open_or_init(dir.path()).unwrap();
        repository
            .repository
            .config()
            .unwrap()
            .set_str("user.name", "Ada Lovelace")
            .unwrap();
        repository
            .repository
            .config()
            .unwrap()
            .set_str("user.email", "ada@example.com")
            .unwrap();
        (dir, repository)
    }

    fn initial_commit(repository: &ResumeRepository, dir: &TempDir) -> CommitRecord {
        std::fs::write(dir.path().join("resume.yaml"), "name: Ada\n").unwrap();
        let mut index = repository.repository.index().unwrap();
        index.add_path(Path::new("resume.yaml")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repository.repository.find_tree(tree_id).unwrap();
        let signature = repository.repository.signature().unwrap();
        let id = repository
            .repository
            .commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
            .unwrap();
        repository.record(id).unwrap()
    }

    #[test]
    fn initializes_standard_worktree_and_observes_unborn_state() {
        let dir = TempDir::new().unwrap();
        let repository = ResumeRepository::open_or_init(dir.path()).unwrap();
        let reopened = ResumeRepository::open_or_init(dir.path()).unwrap();

        assert_eq!(repository.state().unwrap().head.unwrap().commit_id, None);
        assert!(reopened.log().unwrap().commits.is_empty());
        assert!(matches!(
            reopened.create_branch(CreateBranchRequest {
                name: "next".into(),
                from: None,
            }),
            Err(RepositoryError::Conflict { .. })
        ));
        assert!(matches!(
            reopened.commit(CommitRequest {
                message: "x".into()
            }),
            Err(RepositoryError::Conflict { .. })
        ));
        let files = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(files, vec![".git"]);
    }

    #[test]
    fn commits_tracked_changes_and_leaves_untracked_files() {
        let (dir, repository) = repo();
        let initial = initial_commit(&repository, &dir);
        std::fs::write(dir.path().join("resume.yaml"), "name: Grace\n").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "untracked\n").unwrap();

        let status = repository.status().unwrap();
        assert_eq!(status.entries[0].path, "notes.txt");
        assert_eq!(status.entries[0].worktree, Some(FileChange::Untracked));
        assert_eq!(status.entries[1].path, "resume.yaml");
        let record = repository
            .commit(CommitRequest {
                message: "update resume".into(),
            })
            .unwrap();
        assert_eq!(record.parents, vec![initial.id]);
        assert_eq!(record.author.name, "Ada Lovelace");
        assert_eq!(record.author.email, "ada@example.com");
        assert_eq!(record.id.len(), 40);
        assert_eq!(repository.status().unwrap().entries.len(), 1);
        assert_eq!(repository.status().unwrap().entries[0].path, "notes.txt");
    }

    #[test]
    fn commits_an_already_staged_deletion() {
        let (dir, repository) = repo();
        initial_commit(&repository, &dir);
        std::fs::remove_file(dir.path().join("resume.yaml")).unwrap();
        let mut index = repository.repository.index().unwrap();
        index.remove_path(Path::new("resume.yaml")).unwrap();
        index.write().unwrap();

        let commit = repository
            .commit(CommitRequest {
                message: "remove resume".into(),
            })
            .unwrap();
        assert_eq!(commit.message, "remove resume");
        assert!(repository.status().unwrap().entries.is_empty());
    }

    #[test]
    fn a_staged_write_lands_in_the_next_commit() {
        let (dir, repository) = seeded();

        repository
            .write_staged("applications.csv", "Company,Position\nAcme,Engineer\n")
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join("applications.csv")).unwrap(),
            "Company,Position\nAcme,Engineer\n"
        );
        let record = repository
            .commit(CommitRequest {
                message: "sync applications".into(),
            })
            .unwrap();
        assert_eq!(record.message, "sync applications");
        assert!(repository.status().unwrap().entries.is_empty());
    }

    #[test]
    fn a_staged_write_replaces_the_previous_contents() {
        let (dir, repository) = seeded();
        repository.write_staged("applications.csv", "first\n").unwrap();
        repository
            .commit(CommitRequest {
                message: "first sync".into(),
            })
            .unwrap();

        repository.write_staged("applications.csv", "second\n").unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join("applications.csv")).unwrap(),
            "second\n"
        );
        assert_eq!(repository.status().unwrap().entries[0].path, "applications.csv");
    }

    #[test]
    fn creates_and_checks_out_local_branches() {
        let (dir, repository) = repo();
        let initial = initial_commit(&repository, &dir);
        let branch = repository
            .create_branch(CreateBranchRequest {
                name: "revision".into(),
                from: None,
            })
            .unwrap();
        assert_eq!(branch.commit_id, Some(initial.id));
        let state = repository
            .checkout(CheckoutRequest {
                branch: "revision".into(),
            })
            .unwrap();
        assert_eq!(state.head.unwrap().branch.as_deref(), Some("revision"));
        assert!(matches!(
            repository.create_branch(CreateBranchRequest {
                name: "revision".into(),
                from: None,
            }),
            Err(RepositoryError::Conflict { .. })
        ));
        assert!(matches!(
            repository.checkout(CheckoutRequest {
                branch: "missing".into()
            }),
            Err(RepositoryError::NotFound { .. })
        ));
    }

    #[test]
    fn deletes_a_merged_branch_and_refuses_the_open_and_unreachable_ones() {
        let (dir, repository) = repo();
        initial_commit(&repository, &dir);
        repository
            .create_branch(CreateBranchRequest {
                name: "spare".into(),
                from: None,
            })
            .unwrap();
        repository
            .create_branch(CreateBranchRequest {
                name: "divergent".into(),
                from: None,
            })
            .unwrap();
        repository
            .checkout(CheckoutRequest {
                branch: "divergent".into(),
            })
            .unwrap();
        std::fs::write(dir.path().join("resume.yaml"), "cv:\n  name: X\n").unwrap();
        repository
            .commit(CommitRequest {
                message: "Diverge".into(),
            })
            .unwrap();

        assert!(repository.delete_branch("missing").is_err());
        // The open branch is refused, whatever it is.
        assert!(repository.delete_branch("divergent").is_err());
        // A branch at a commit another branch reaches goes quietly.
        repository.delete_branch("spare").unwrap();
        assert!(
            repository
                .state()
                .unwrap()
                .branches
                .iter()
                .all(|branch| branch.name != "spare")
        );
        // A branch whose commits nothing else reaches is refused.
        assert!(matches!(
            repository.delete_branch("divergent"),
            Err(RepositoryError::Conflict { .. })
        ));
    }

    #[test]
    fn failed_safe_checkout_keeps_the_current_branch() {
        let (dir, repository) = repo();
        initial_commit(&repository, &dir);
        let main = repository.state().unwrap().head.unwrap().branch.unwrap();
        repository
            .create_branch(CreateBranchRequest {
                name: "revision".into(),
                from: None,
            })
            .unwrap();
        repository
            .checkout(CheckoutRequest {
                branch: "revision".into(),
            })
            .unwrap();
        std::fs::write(dir.path().join("resume.yaml"), "name: Grace\n").unwrap();
        repository
            .commit(CommitRequest {
                message: "revision change".into(),
            })
            .unwrap();
        repository
            .checkout(CheckoutRequest {
                branch: main.clone(),
            })
            .unwrap();

        std::fs::write(dir.path().join("resume.yaml"), "name: Local\n").unwrap();
        assert!(matches!(
            repository.checkout(CheckoutRequest {
                branch: "revision".into(),
            }),
            Err(RepositoryError::Conflict { .. })
        ));
        assert_eq!(repository.state().unwrap().head.unwrap().branch, Some(main));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("resume.yaml")).unwrap(),
            "name: Local\n"
        );
    }

    #[test]
    fn status_only_includes_old_paths_for_renames() {
        let (dir, repository) = repo();
        initial_commit(&repository, &dir);
        std::fs::write(dir.path().join("resume.yaml"), "name: Grace\n").unwrap();
        let modified = repository.status().unwrap();
        assert_eq!(modified.entries[0].path, "resume.yaml");
        assert_eq!(modified.entries[0].old_path, None);

        std::fs::write(dir.path().join("resume.yaml"), "name: Ada\n").unwrap();
        std::fs::rename(dir.path().join("resume.yaml"), dir.path().join("cv.yaml")).unwrap();
        let mut index = repository.repository.index().unwrap();
        index.add_path(Path::new("cv.yaml")).unwrap();
        index.remove_path(Path::new("resume.yaml")).unwrap();
        index.write().unwrap();
        let renamed = repository.status().unwrap();
        assert_eq!(renamed.entries.len(), 1);
        assert_eq!(renamed.entries[0].path, "cv.yaml");
        assert_eq!(renamed.entries[0].old_path.as_deref(), Some("resume.yaml"));
        assert_eq!(renamed.entries[0].index, Some(FileChange::Renamed));
    }

    #[test]
    fn returns_structured_diffs_and_limits_log() {
        let (dir, repository) = repo();
        initial_commit(&repository, &dir);
        std::fs::write(
            dir.path().join("resume.yaml"),
            "name: Ada\nsummary: Engineer\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("asset.bin"), [0_u8, 1, 2, 0]).unwrap();
        let mut index = repository.repository.index().unwrap();
        index.add_path(Path::new("asset.bin")).unwrap();
        index.write().unwrap();
        let diff = repository.diff().unwrap();
        assert!(diff.files.iter().any(|file| {
            file.new_path.as_deref() == Some("resume.yaml")
                && file.hunks.iter().flat_map(|hunk| &hunk.lines).any(|line| {
                    line.kind == DiffLineKind::Addition && line.content == "summary: Engineer\n"
                })
        }));
        assert!(
            diff.files
                .iter()
                .any(|file| file.new_path.as_deref() == Some("asset.bin") && file.hunks.is_empty())
        );
        repository
            .commit(CommitRequest {
                message: "update resume".into(),
            })
            .unwrap();
        let log = repository.log().unwrap();
        assert_eq!(log.commits[0].message, "update resume");
        assert!(log.commits.len() <= LOG_LIMIT);
        assert!(matches!(
            repository.commit(CommitRequest {
                message: " ".into()
            }),
            Err(RepositoryError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn a_branch_can_start_from_another_branch() {
        let (_dir, repository) = seeded();
        repository
            .create_branch(CreateBranchRequest {
                name: "archetype/systems".into(),
                from: None,
            })
            .unwrap();

        let child = repository
            .create_branch(CreateBranchRequest {
                name: "apply/systems/acme".into(),
                from: Some("archetype/systems".into()),
            })
            .unwrap();

        assert_eq!(child.name, "apply/systems/acme");
    }

    #[test]
    fn branching_from_a_branch_that_is_not_there_is_rejected() {
        let (_dir, repository) = seeded();

        assert!(matches!(
            repository.create_branch(CreateBranchRequest {
                name: "child".into(),
                from: Some("nowhere".into()),
            }),
            Err(RepositoryError::NotFound { .. })
        ));
    }

    #[test]
    fn merging_a_branch_that_has_not_moved_changes_nothing() {
        let (_dir, repository) = seeded();
        repository
            .create_branch(CreateBranchRequest {
                name: "sibling".into(),
                from: None,
            })
            .unwrap();

        let outcome = repository.merge("sibling").unwrap();

        assert_eq!(outcome.kind, "already-current");
    }

    #[test]
    fn merging_a_branch_that_is_not_there_is_rejected() {
        let (_dir, repository) = seeded();

        assert!(matches!(
            repository.merge("nowhere"),
            Err(RepositoryError::NotFound { .. })
        ));
    }
}
