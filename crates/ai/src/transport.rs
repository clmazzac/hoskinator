//! The seam to Anthropic. Stubbed in tests (ADR-0005, PRD testing decisions).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

const ANTHROPIC_API: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 2048;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("could not reach Anthropic")]
    Request(#[from] reqwest::Error),
    #[error("Anthropic answered with {status}: {body}")]
    Http { status: u16, body: String },
    #[error("Anthropic's response had no text content")]
    EmptyResponse,
}

/// One call to a model: a prompt in, its text reply out.
#[async_trait]
pub trait Transport: Send + Sync {
    async fn complete(&self, model: &str, prompt: &str) -> Result<String, TransportError>;
}

/// Calls the real Anthropic Messages API.
pub struct AnthropicTransport {
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicTransport {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: [Message<'a>; 1],
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[async_trait]
impl Transport for AnthropicTransport {
    async fn complete(&self, model: &str, prompt: &str) -> Result<String, TransportError> {
        let request = MessagesRequest {
            model,
            max_tokens: MAX_TOKENS,
            messages: [Message {
                role: "user",
                content: prompt,
            }],
        };

        let response = self
            .client
            .post(ANTHROPIC_API)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(TransportError::Http {
                status: status.as_u16(),
                body,
            });
        }

        let parsed: MessagesResponse = response.json().await?;
        parsed
            .content
            .into_iter()
            .find(|block| block.kind == "text")
            .and_then(|block| block.text)
            .ok_or(TransportError::EmptyResponse)
    }
}
