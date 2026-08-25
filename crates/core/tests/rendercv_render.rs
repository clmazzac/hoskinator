//! Checks that a render really goes through the installed rendercv.
//!
//! Ignored by default; run with `cargo test -- --ignored`.

use std::path::Path;

use hoskinator_core::render;
use hoskinator_core::resume;
use tempfile::TempDir;

const RESUME: &str = "\
cv:
  name: Ada Lovelace
  sections:
    experience:
      - company: Analytical Engines
        position: Engineer
        start_date: 1842-01
        end_date: present
        highlights:
          - Wrote the first program.
design:
  theme: classic
";

fn repository(text: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(resume::FILENAME), text).unwrap();
    dir
}

#[test]
#[ignore]
fn the_installed_rendercv_is_available() {
    assert!(render::is_available(), "rendercv is not on PATH");
}

#[test]
#[ignore]
fn a_resume_renders_to_the_named_pdf_and_leaves_nothing_else_behind() {
    let dir = repository(RESUME);
    let output = TempDir::new().unwrap();

    let rendered = render::pdf(dir.path(), output.path(), "Ada").unwrap();

    assert_eq!(rendered.path, output.path().join("Ada.pdf"));
    assert!(std::fs::read(&rendered.path).unwrap().starts_with(b"%PDF"));
    let produced: Vec<_> = std::fs::read_dir(output.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(produced, ["Ada.pdf"]);
    let repository_files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(repository_files, [resume::FILENAME]);
}

#[test]
#[ignore]
fn a_relative_directory_lands_inside_the_repository() {
    let dir = repository(RESUME);

    let rendered = render::pdf(dir.path(), Path::new("out"), "Ada").unwrap();

    assert_eq!(rendered.path, dir.path().join("out").join("Ada.pdf"));
}

#[test]
#[ignore]
fn a_resume_rendercv_rejects_carries_its_complaint_back() {
    let dir = repository("cv:\n  name: Ada\n  sections:\n    experience:\n      - nonsense: 1\n");
    let output = TempDir::new().unwrap();

    let error = render::pdf(dir.path(), output.path(), "Ada").unwrap_err();

    let render::RenderError::Failed { diagnostics, .. } = &error else {
        panic!("expected a failure, got {error:?}")
    };
    assert!(
        diagnostics.contains("experience"),
        "no complaint about the bad section: {diagnostics}"
    );
}
