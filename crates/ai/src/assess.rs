//! The qualitative half of the tailoring panel: relevance, tone, flow, semantic keyword coverage,
//! and rewrite suggestions — judgment a deterministic pass can't make
//! (`hoskinator_core::tailoring` handles the keyword-overlap half; see
//! `docs/decisions/tailoring.md`).

use serde::{Deserialize, Serialize};

use crate::transport::{Transport, TransportError};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assessment {
    pub relevance: Score,
    pub tone: Score,
    pub flow: Score,
    pub semantic_coverage: Vec<SemanticMatch>,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub score: u8,
    pub reason: String,
}

/// Whether one of the deterministic pass's missing keywords is covered by the resume some other
/// way — a different word for the same thing, not a fabricated connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticMatch {
    pub keyword: String,
    pub covered: bool,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Suggestion {
    pub on: String,
    pub suggestion: String,
    pub why: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AssessError {
    #[error("could not reach Claude")]
    Transport(#[from] TransportError),
    #[error("Claude's response was not the expected JSON shape")]
    MalformedResponse(#[source] serde_json::Error),
}

/// Scores `resume_yaml` against `jd_text` for relevance, tone, and flow; checks whether each of
/// `missing_keywords` (the deterministic pass's literal misses) is covered some other way; and
/// suggests rewrites.
pub async fn assess(
    transport: &dyn Transport,
    model: &str,
    resume_yaml: &str,
    jd_text: &str,
    missing_keywords: &[String],
) -> Result<Assessment, AssessError> {
    let prompt = prompt(resume_yaml, jd_text, missing_keywords);
    let reply = transport.complete(model, &prompt).await?;
    serde_json::from_str(strip_code_fence(&reply)).map_err(AssessError::MalformedResponse)
}

fn prompt(resume_yaml: &str, jd_text: &str, missing_keywords: &[String]) -> String {
    let missing_list = if missing_keywords.is_empty() {
        "(none)".to_string()
    } else {
        missing_keywords.join(", ")
    };
    let shape = serde_json::json!({
        "relevance": {"score": 0, "reason": ""},
        "tone": {"score": 0, "reason": ""},
        "flow": {"score": 0, "reason": ""},
        "semantic_coverage": [{"keyword": "", "covered": false, "evidence": null}],
        "suggestions": [{"on": "", "suggestion": "", "why": ""}],
    });
    format!(
        "You are assessing a resume against a job description for relevance, tone, and flow. \
         Keyword/ATS matching is scored separately — ignore it for those three.\n\n\
         Score relevance, tone, and flow from 0-100, each with a one-sentence reason.\n\n\
         Separately, a literal keyword pass already ran and could not find these terms anywhere \
         in the resume: {missing_list}. For each one, decide whether the resume actually covers \
         it in different words — a real equivalent, not a stretch. If covered, quote the resume \
         line that covers it as evidence. If the list is (none), answer with an empty array.\n\n\
         Then list up to 5 specific rewrite suggestions for lines that are vague, generic, or \
         weakly phrased, each naming what to change and why.\n\n\
         Respond with ONLY this JSON shape and nothing else:\n{shape}\n\n\
         Job description:\n{jd_text}\n\n\
         Resume (rendercv YAML):\n{resume_yaml}"
    )
}

/// Strips a ```json ... ``` or ``` ... ``` fence if the model wrapped its JSON in one.
fn strip_code_fence(reply: &str) -> &str {
    let trimmed = reply.trim();
    let Some(without_open) = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
    else {
        return trimmed;
    };
    without_open
        .strip_suffix("```")
        .unwrap_or(without_open)
        .trim()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    const CANNED_REPLY: &str = r#"{"relevance":{"score":80,"reason":"Strong Rust overlap"},"tone":{"score":70,"reason":"Confident"},"flow":{"score":60,"reason":"Uneven bullet lengths"},"semantic_coverage":[{"keyword":"kubernetes","covered":true,"evidence":"Orchestrated services across three EKS clusters"},{"keyword":"aws","covered":false,"evidence":null}],"suggestions":[{"on":"Worked on backend","suggestion":"Built the payments backend","why":"Names what was built"}]}"#;

    /// Returns `reply` on `complete`, or `TransportError::EmptyResponse` if none was given.
    struct StubTransport {
        reply: Option<String>,
        last_prompt: Mutex<Option<String>>,
    }

    impl StubTransport {
        fn new(reply: &str) -> Self {
            Self {
                reply: Some(reply.to_string()),
                last_prompt: Mutex::new(None),
            }
        }

        fn failing() -> Self {
            Self {
                reply: None,
                last_prompt: Mutex::new(None),
            }
        }
    }

    #[async_trait::async_trait]
    impl Transport for StubTransport {
        async fn complete(&self, _model: &str, prompt: &str) -> Result<String, TransportError> {
            *self.last_prompt.lock().unwrap() = Some(prompt.to_string());
            self.reply.clone().ok_or(TransportError::EmptyResponse)
        }
    }

    fn missing(terms: &[&str]) -> Vec<String> {
        terms.iter().map(|t| t.to_string()).collect()
    }

    #[tokio::test]
    async fn a_well_formed_reply_parses_and_every_input_reaches_the_transport() {
        let transport = StubTransport::new(CANNED_REPLY);
        let assessment = assess(
            &transport,
            "model",
            "UNIQUE_RESUME_TEXT",
            "UNIQUE_JD_TEXT",
            &missing(&["kubernetes", "aws"]),
        )
        .await
        .unwrap();
        assert_eq!(assessment.relevance.score, 80);
        assert_eq!(assessment.suggestions.len(), 1);
        assert_eq!(assessment.suggestions[0].on, "Worked on backend");
        assert_eq!(assessment.semantic_coverage.len(), 2);
        assert!(assessment.semantic_coverage[0].covered);
        assert_eq!(
            assessment.semantic_coverage[0].evidence.as_deref(),
            Some("Orchestrated services across three EKS clusters")
        );
        assert!(!assessment.semantic_coverage[1].covered);

        let prompt = transport.last_prompt.lock().unwrap().clone().unwrap();
        assert!(prompt.contains("UNIQUE_RESUME_TEXT"));
        assert!(prompt.contains("UNIQUE_JD_TEXT"));
        assert!(prompt.contains("kubernetes"));
    }

    #[tokio::test]
    async fn a_reply_wrapped_in_a_json_code_fence_still_parses() {
        let fenced = format!("```json\n{CANNED_REPLY}\n```");
        let transport = StubTransport::new(&fenced);
        let assessment = assess(&transport, "model", "resume", "jd", &missing(&[]))
            .await
            .unwrap();
        assert_eq!(assessment.tone.score, 70);
    }

    #[tokio::test]
    async fn no_missing_keywords_tells_the_model_there_is_nothing_to_check() {
        let transport = StubTransport::new(CANNED_REPLY);
        assess(&transport, "model", "resume", "jd", &missing(&[]))
            .await
            .unwrap();
        let prompt = transport.last_prompt.lock().unwrap().clone().unwrap();
        assert!(prompt.contains("(none)"));
    }

    #[tokio::test]
    async fn a_non_json_reply_is_a_malformed_response_error() {
        let transport = StubTransport::new("not json at all");
        let error = assess(&transport, "model", "resume", "jd", &missing(&[]))
            .await
            .unwrap_err();
        assert!(matches!(error, AssessError::MalformedResponse(_)));
    }

    #[tokio::test]
    async fn a_transport_failure_propagates() {
        let transport = StubTransport::failing();
        let error = assess(&transport, "model", "resume", "jd", &missing(&[]))
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            AssessError::Transport(TransportError::EmptyResponse)
        ));
    }
}
