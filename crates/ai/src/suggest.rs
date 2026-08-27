//! Drafts resume bullets from a Braindump — the free-write notes an Entry carries
//! (`hoskinator_core::entry::Entry::braindump`; see `docs/decisions/braindump.md`).

use serde::{Deserialize, Serialize};

use crate::transport::{Transport, TransportError, strip_code_fence};

/// A candidate bullet, with the phrase in the braindump it is grounded in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftBullet {
    pub text: String,
    pub why: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SuggestError {
    #[error("could not reach Claude")]
    Transport(#[from] TransportError),
    #[error("Claude's response was not the expected JSON shape")]
    MalformedResponse(#[source] serde_json::Error),
}

/// Drafts up to 5 bullets from `braindump`, skipping anything `existing` already covers.
pub async fn suggest_bullets(
    transport: &dyn Transport,
    model: &str,
    braindump: &str,
    existing: &[String],
) -> Result<Vec<DraftBullet>, SuggestError> {
    let prompt = prompt(braindump, existing);
    let reply = transport.complete(model, &prompt).await?;
    serde_json::from_str(strip_code_fence(&reply)).map_err(SuggestError::MalformedResponse)
}

fn prompt(braindump: &str, existing: &[String]) -> String {
    let existing_list = if existing.is_empty() {
        "(none yet)".to_string()
    } else {
        existing
            .iter()
            .map(|bullet| format!("- {bullet}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let shape = serde_json::json!([{"text": "", "why": ""}]);
    format!(
        "You are drafting resume bullet points from a candidate's own free-write notes about a \
         job or project.\n\n\
         Use only what the notes say. Do not invent a number, tool, or outcome the notes do not \
         mention. Open with a strong action verb, and quantify the result where the notes give you \
         a number to quantify it with. One line each.\n\n\
         The role already has these bullets — do not repeat them:\n{existing_list}\n\n\
         Suggest up to 5 new bullets the notes support. For each, quote the phrase in the notes it \
         comes from as `why`, so it can be checked against the source.\n\n\
         Respond with ONLY this JSON shape and nothing else:\n{shape}\n\n\
         Notes:\n{braindump}"
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    const CANNED_REPLY: &str = r#"[{"text":"Cut p99 latency in half by moving the billing pipeline off cron onto a queue","why":"Migrated the billing pipeline off cron to a queue. Cut p99 latency from 4s to 300ms."},{"text":"Led a 3-person team through the rewrite","why":"Led a team of 3."}]"#;

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

    fn existing(bullets: &[&str]) -> Vec<String> {
        bullets.iter().map(|b| b.to_string()).collect()
    }

    #[tokio::test]
    async fn a_well_formed_reply_parses_and_every_input_reaches_the_transport() {
        let transport = StubTransport::new(CANNED_REPLY);
        let drafts = suggest_bullets(
            &transport,
            "model",
            "UNIQUE_BRAINDUMP_TEXT",
            &existing(&["Shipped the payments backend"]),
        )
        .await
        .unwrap();

        assert_eq!(drafts.len(), 2);
        assert!(drafts[0].text.contains("Cut p99 latency"));
        assert!(drafts[0].why.contains("Migrated the billing pipeline"));

        let prompt = transport.last_prompt.lock().unwrap().clone().unwrap();
        assert!(prompt.contains("UNIQUE_BRAINDUMP_TEXT"));
        assert!(prompt.contains("Shipped the payments backend"));
    }

    #[tokio::test]
    async fn a_reply_wrapped_in_a_json_code_fence_still_parses() {
        let fenced = format!("```json\n{CANNED_REPLY}\n```");
        let transport = StubTransport::new(&fenced);
        let drafts = suggest_bullets(&transport, "model", "notes", &existing(&[]))
            .await
            .unwrap();
        assert_eq!(drafts.len(), 2);
    }

    #[tokio::test]
    async fn no_existing_bullets_tells_the_model_there_is_nothing_to_avoid_repeating() {
        let transport = StubTransport::new(CANNED_REPLY);
        suggest_bullets(&transport, "model", "notes", &existing(&[]))
            .await
            .unwrap();
        let prompt = transport.last_prompt.lock().unwrap().clone().unwrap();
        assert!(prompt.contains("(none yet)"));
    }

    #[tokio::test]
    async fn a_non_json_reply_is_a_malformed_response_error() {
        let transport = StubTransport::new("not json at all");
        let error = suggest_bullets(&transport, "model", "notes", &existing(&[]))
            .await
            .unwrap_err();
        assert!(matches!(error, SuggestError::MalformedResponse(_)));
    }

    #[tokio::test]
    async fn a_transport_failure_propagates() {
        let transport = StubTransport::failing();
        let error = suggest_bullets(&transport, "model", "notes", &existing(&[]))
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            SuggestError::Transport(TransportError::EmptyResponse)
        ));
    }
}
