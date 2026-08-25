//! Rendering the per-branch `resume.yaml` to a PDF by shelling out to rendercv (ADR-0005).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::resume;

/// The renderer, looked up on PATH.
const PROGRAM: &str = "rendercv";

/// Extension every rendered file carries.
const EXTENSION: &str = ".pdf";

/// Where a render landed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedPdf {
    pub path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("no resume.yaml at {path}")]
    ResumeNotFound { path: PathBuf },
    #[error("rendercv is not on PATH")]
    ProgramMissing,
    #[error("could not run rendercv")]
    Spawn(#[source] std::io::Error),
    #[error("could not create a scratch directory for rendercv's intermediate files")]
    Scratch(#[source] std::io::Error),
    #[error("rendercv exited unsuccessfully: {diagnostics}")]
    Failed {
        code: Option<i32>,
        diagnostics: String,
    },
    #[error("rendercv reported success but wrote no PDF to {path}")]
    NoOutput { path: PathBuf },
}

/// Whether rendercv can be run.
pub fn is_available() -> bool {
    available(PROGRAM)
}

/// Renders a repository's `resume.yaml` to `file_name` within `directory`.
///
/// A relative `directory` resolves against the repository. `.pdf` is appended to `file_name`
/// unless it is already there.
pub fn pdf(
    repository: &Path,
    directory: &Path,
    file_name: &str,
) -> Result<RenderedPdf, RenderError> {
    render(PROGRAM, repository, directory, file_name)
}

/// Whether `program` answers `--version` successfully.
fn available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The render itself, with the renderer named by the caller.
fn render(
    program: &str,
    repository: &Path,
    directory: &Path,
    file_name: &str,
) -> Result<RenderedPdf, RenderError> {
    let input = repository.join(resume::FILENAME);
    if !input.is_file() {
        return Err(RenderError::ResumeNotFound { path: input });
    }
    let path = repository.join(directory).join(named_pdf(file_name));

    // rendercv cannot make a PDF without also writing Typst, so the intermediate is sent
    // somewhere other than the directory the caller asked for.
    let scratch = tempfile::TempDir::new().map_err(RenderError::Scratch)?;

    let output = Command::new(program)
        .current_dir(repository)
        .arg("render")
        .arg(&input)
        .arg("--output-folder")
        .arg(scratch.path())
        .arg("--pdf-path")
        .arg(&path)
        .args(["-nomd", "-nohtml", "-nopng"])
        .output()
        .map_err(|source| match source.kind() {
            std::io::ErrorKind::NotFound => RenderError::ProgramMissing,
            _ => RenderError::Spawn(source),
        })?;

    if !output.status.success() {
        return Err(RenderError::Failed {
            code: output.status.code(),
            diagnostics: diagnostics(&output.stderr, &output.stdout),
        });
    }
    if !path.is_file() {
        return Err(RenderError::NoOutput { path });
    }
    Ok(RenderedPdf { path })
}

/// `file_name` with the PDF extension, added only if it is not already there.
fn named_pdf(file_name: &str) -> String {
    if file_name.to_ascii_lowercase().ends_with(EXTENSION) {
        file_name.to_owned()
    } else {
        format!("{file_name}{EXTENSION}")
    }
}

/// What the renderer said, from both streams.
fn diagnostics(stderr: &[u8], stdout: &[u8]) -> String {
    [stderr, stdout]
        .into_iter()
        .map(|stream| String::from_utf8_lossy(stream).trim().to_owned())
        .filter(|said| !said.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const NOT_INSTALLED: &str = "hoskinator-no-such-renderer";

    fn repository() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(resume::FILENAME), "cv:\n  name: Ada\n").unwrap();
        dir
    }

    #[test]
    fn a_renderer_that_is_not_installed_is_unavailable() {
        assert!(!available(NOT_INSTALLED));
    }

    #[test]
    fn a_repository_without_a_resume_is_reported_before_the_renderer_runs() {
        let dir = TempDir::new().unwrap();

        assert!(matches!(
            render(NOT_INSTALLED, dir.path(), Path::new("out"), "Resume"),
            Err(RenderError::ResumeNotFound { .. })
        ));
    }

    #[test]
    fn a_renderer_that_is_not_on_path_is_reported_as_missing() {
        let dir = repository();

        assert!(matches!(
            render(NOT_INSTALLED, dir.path(), Path::new("out"), "Resume"),
            Err(RenderError::ProgramMissing)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_relative_directory_resolves_against_the_repository() {
        let dir = repository();

        let error = render("true", dir.path(), Path::new("out"), "Resume").unwrap_err();

        assert!(
            matches!(&error, RenderError::NoOutput { path } if *path == dir.path().join("out").join("Resume.pdf")),
            "got {error:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_absolute_directory_is_used_as_given() {
        let dir = repository();
        let elsewhere = TempDir::new().unwrap();

        let error = render("true", dir.path(), elsewhere.path(), "Resume.pdf").unwrap_err();

        assert!(
            matches!(&error, RenderError::NoOutput { path } if *path == elsewhere.path().join("Resume.pdf")),
            "got {error:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_renderer_that_exits_unsuccessfully_is_a_failure() {
        let dir = repository();

        assert!(matches!(
            render("false", dir.path(), Path::new("out"), "Resume"),
            Err(RenderError::Failed { code: Some(1), .. })
        ));
    }

    #[test]
    fn the_extension_is_added_once() {
        assert_eq!(named_pdf("Resume"), "Resume.pdf");
        assert_eq!(named_pdf("Resume.pdf"), "Resume.pdf");
        assert_eq!(named_pdf("Resume.PDF"), "Resume.PDF");
        assert_eq!(named_pdf("Ada.Lovelace"), "Ada.Lovelace.pdf");
    }

    #[test]
    fn diagnostics_carry_both_streams_and_skip_the_silent_one() {
        assert_eq!(diagnostics(b"broke\n", b"why\n"), "broke\nwhy");
        assert_eq!(
            diagnostics(b"", b"  validation failed  "),
            "validation failed"
        );
        assert_eq!(diagnostics(b"", b""), "");
    }
}
