//! Deterministic resume-to-job-description matching.
//!
//! Keyword overlap only — no embeddings, no LLM call, no API key (ADR-0005; embeddings are out of
//! scope for v1 per the PRD). Real ATS platforms still filter mostly this way: Taleo does literal
//! keyword matching, Lever's is stemming-based, and neither uses semantic ML. See
//! `docs/decisions/tailoring.md` for the research this leans on.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Common English words excluded from keyword weighting.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "been", "being", "but", "by", "can", "do", "does",
    "for", "from", "had", "has", "have", "how", "if", "in", "into", "is", "it", "its", "of", "on",
    "or", "our", "over", "per", "some", "such", "than", "that", "the", "their", "then", "there",
    "these", "they", "this", "to", "up", "us", "was", "we", "were", "what", "when", "where",
    "which", "while", "who", "will", "with", "within", "would", "you", "your",
];

/// Openers that read as passive or deflect credit, rather than naming what was done.
const WEAK_OPENERS: &[&str] = &[
    "responsible",
    "worked",
    "helped",
    "assisted",
    "involved",
    "participated",
    "duties",
    "was",
    "tasked",
];

/// Keywords from a job description matched against a resume's own wording, plus a few writing
/// signals pulled from the resume alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchReport {
    /// 0-100: the share of the JD's weighted keywords found in the resume.
    pub score: u8,
    pub matched: Vec<Keyword>,
    pub missing: Vec<Keyword>,
    pub writing_notes: Vec<WritingNote>,
}

/// A candidate keyword extracted from the JD, and how much it counts for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyword {
    pub term: String,
    pub weight: u32,
}

/// A resume line that reads as a bullet, flagged for a specific writing issue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WritingNote {
    pub line: String,
    pub kind: WritingNoteKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WritingNoteKind {
    /// No digit or `%` anywhere in the line — likely an unquantified claim.
    Unquantified,
    /// Opens with a passive or credit-deflecting verb.
    WeakOpener,
}

/// Caps how many missing keywords `match_report` returns.
const MAX_MISSING: usize = 15;
/// Caps how many writing notes `match_report` returns.
const MAX_WRITING_NOTES: usize = 10;

/// Scores `resume_yaml` against `jd_text` by weighted keyword overlap.
pub fn match_report(resume_yaml: &str, jd_text: &str) -> MatchReport {
    let resume_stems = stemmed_token_set(resume_yaml);
    let mut keywords: Vec<Keyword> = keyword_weights(jd_text)
        .into_iter()
        .map(|(term, weight)| Keyword { term, weight })
        .collect();
    keywords.sort_by(|a, b| b.weight.cmp(&a.weight).then_with(|| a.term.cmp(&b.term)));

    let total_weight: u32 = keywords.iter().map(|k| k.weight).sum();
    let (matched, missing): (Vec<_>, Vec<_>) = keywords
        .into_iter()
        .partition(|keyword| resume_stems.contains(&stem(&keyword.term)));
    let matched_weight: u32 = matched.iter().map(|k| k.weight).sum();
    // Nothing to match against is treated as a pass, not a failure — there is no missed keyword.
    let score = if total_weight == 0 {
        100
    } else {
        ((matched_weight as f64 / total_weight as f64) * 100.0).round() as u8
    };

    MatchReport {
        score,
        matched,
        missing: missing.into_iter().take(MAX_MISSING).collect(),
        writing_notes: writing_notes(resume_yaml),
    }
}

/// Weighted keyword candidates from `text`: word frequency, doubled for tokens capitalized where
/// they appeared.
fn keyword_weights(text: &str) -> HashMap<String, u32> {
    let mut weights = HashMap::new();
    for token in tokenize(text) {
        let lower = token.to_lowercase();
        if lower.len() < 2 || STOPWORDS.contains(&lower.as_str()) {
            continue;
        }
        let is_capitalized = token.chars().next().is_some_and(char::is_uppercase);
        *weights.entry(lower).or_insert(0) += if is_capitalized { 2 } else { 1 };
    }
    weights
}

/// The distinct stems present anywhere in `text`.
fn stemmed_token_set(text: &str) -> std::collections::HashSet<String> {
    tokenize(text)
        .into_iter()
        .map(|token| stem(&token.to_lowercase()))
        .collect()
}

/// Splits `text` into word-like tokens. A token may hold a trailing `+` or `#` (`c++`, `c#`);
/// every other separator ends a token.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ((ch == '+' || ch == '#') && !current.is_empty()) {
            current.push(ch);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// A light suffix strip, not a real stemmer — enough to match "managing" against "managed".
fn stem(word: &str) -> String {
    for suffix in ["ing", "edly", "ed", "es", "s"] {
        if let Some(stripped) = word.strip_suffix(suffix)
            && stripped.len() >= 3
        {
            return stripped.to_string();
        }
    }
    word.to_string()
}

/// Flags each highlight in `resume_yaml` for missing quantification or a weak opening verb.
fn writing_notes(resume_yaml: &str) -> Vec<WritingNote> {
    let mut notes = Vec::new();
    for content in highlights(resume_yaml) {
        if !content.chars().any(|c| c.is_ascii_digit() || c == '%') {
            notes.push(WritingNote {
                line: content.clone(),
                kind: WritingNoteKind::Unquantified,
            });
        }
        let opener = content
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();
        if WEAK_OPENERS.contains(&opener.as_str()) {
            notes.push(WritingNote {
                line: content,
                kind: WritingNoteKind::WeakOpener,
            });
        }
    }
    notes.truncate(MAX_WRITING_NOTES);
    notes
}

/// Every highlight string under `cv.sections.*[].highlights`, in file order. An entry's other
/// fields (`company:`, `position:`, ...) are never included.
fn highlights(resume_yaml: &str) -> Vec<String> {
    let Ok(document) = yaml_serde::from_str::<yaml_serde::Value>(resume_yaml) else {
        return Vec::new();
    };
    let Some(sections) = document
        .get("cv")
        .and_then(|cv| cv.get("sections"))
        .and_then(|sections| sections.as_mapping())
    else {
        return Vec::new();
    };
    sections
        .values()
        .filter_map(|entries| entries.as_sequence())
        .flatten()
        .filter_map(|entry| entry.get("highlights"))
        .filter_map(|value| value.as_sequence())
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_term(keywords: &[Keyword], term: &str) -> bool {
        keywords.iter().any(|k| k.term == term)
    }

    #[test]
    fn an_exact_keyword_shared_by_both_is_matched() {
        let report = match_report("- Built services in Rust", "Looking for a Rust engineer");
        assert!(has_term(&report.matched, "rust"));
    }

    #[test]
    fn a_keyword_absent_from_the_resume_is_missing() {
        let report = match_report(
            "- Built services in Rust",
            "Looking for a Kubernetes expert",
        );
        assert!(has_term(&report.missing, "kubernetes"));
    }

    #[test]
    fn stemming_matches_a_different_inflection() {
        let report = match_report("- Managed a team of five", "Experience managing engineers");
        assert!(has_term(&report.matched, "managing"));
    }

    #[test]
    fn stopwords_never_become_keywords() {
        let report = match_report("", "This is that of the and");
        assert!(report.matched.is_empty() && report.missing.is_empty());
    }

    #[test]
    fn a_capitalized_term_outweighs_a_lowercase_one_of_equal_frequency() {
        let report = match_report("", "Kubernetes and cadence both matter here");
        let kubernetes = report
            .missing
            .iter()
            .find(|k| k.term == "kubernetes")
            .unwrap();
        let cadence = report.missing.iter().find(|k| k.term == "cadence").unwrap();
        assert!(kubernetes.weight > cadence.weight);
    }

    #[test]
    fn a_jd_with_no_real_keywords_scores_full_marks() {
        let report = match_report("- Anything at all", "This is that of the and");
        assert_eq!(report.score, 100);
    }

    #[test]
    fn full_keyword_coverage_scores_full_marks() {
        let report = match_report("- Rust and Kubernetes experience", "Rust and Kubernetes");
        assert_eq!(report.score, 100);
    }

    #[test]
    fn zero_keyword_coverage_scores_zero() {
        let report = match_report("- Painting and pottery", "Rust and Kubernetes role");
        assert_eq!(report.score, 0);
    }

    fn entry_with_highlight(highlight: &str) -> String {
        format!("cv:\n  sections:\n    experience:\n      - highlights:\n          - {highlight}\n")
    }

    #[test]
    fn an_unquantified_bullet_is_flagged() {
        let report = match_report(&entry_with_highlight("Improved system performance"), "");
        assert!(
            report
                .writing_notes
                .iter()
                .any(|n| n.kind == WritingNoteKind::Unquantified)
        );
    }

    #[test]
    fn a_quantified_bullet_is_not_flagged_as_unquantified() {
        let report = match_report(&entry_with_highlight("Improved performance by 40%"), "");
        assert!(
            !report
                .writing_notes
                .iter()
                .any(|n| n.kind == WritingNoteKind::Unquantified)
        );
    }

    #[test]
    fn a_weak_opener_is_flagged() {
        let report = match_report(&entry_with_highlight("Responsible for the migration"), "");
        assert!(
            report
                .writing_notes
                .iter()
                .any(|n| n.kind == WritingNoteKind::WeakOpener)
        );
    }

    #[test]
    fn a_resume_with_no_sections_has_no_writing_notes() {
        let report = match_report("cv:\n  name: Cam", "");
        assert!(report.writing_notes.is_empty());
    }

    #[test]
    fn a_quoted_highlight_starting_with_a_word_and_a_colon_is_still_scanned() {
        // A text-scraping heuristic once stripped the quotes off a line like this before checking
        // its shape, saw "Word:" at the start, and mistook it for a YAML mapping key — silently
        // dropping the whole line. This highlight has no digit, so it must be flagged, not skipped.
        let report = match_report(&entry_with_highlight("\"Impact: improved things\""), "");
        assert!(
            report
                .writing_notes
                .iter()
                .any(|n| n.kind == WritingNoteKind::Unquantified)
        );
    }

    #[test]
    fn an_entry_field_outside_highlights_is_never_a_writing_note() {
        let report = match_report(
            "cv:\n  sections:\n    experience:\n      - company: Acme\n        highlights:\n          - Shipped things\n",
            "",
        );
        assert!(!report.writing_notes.iter().any(|n| n.line.contains("Acme")));
    }
}
