//! Per-task model selection and where the API key comes from.

/// Used for `assess` unless overridden.
pub const DEFAULT_ASSESS_MODEL: &str = "claude-haiku-4-5-20251001";

/// Used for `suggest_bullets` unless overridden.
pub const DEFAULT_SUGGEST_MODEL: &str = "claude-haiku-4-5-20251001";

/// What `assess` and `suggest_bullets` need to call Anthropic.
pub struct Config {
    pub api_key: String,
    pub assess_model: String,
    pub suggest_model: String,
}

impl Config {
    /// Resolves the API key from `configured_key` (Hoskinator's own settings) if given, else
    /// `ANTHROPIC_API_KEY`; and `HOSKINATOR_AI_ASSESS_MODEL` / `HOSKINATOR_AI_SUGGEST_MODEL`
    /// overrides from the environment. `None` when no key is found either way — the caller
    /// reports AI as unconfigured rather than erroring.
    pub fn resolve(configured_key: Option<&str>) -> Option<Self> {
        let api_key = configured_key
            .map(str::to_owned)
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
            .filter(|key| !key.is_empty())?;
        let assess_model = std::env::var("HOSKINATOR_AI_ASSESS_MODEL")
            .unwrap_or_else(|_| DEFAULT_ASSESS_MODEL.to_string());
        let suggest_model = std::env::var("HOSKINATOR_AI_SUGGEST_MODEL")
            .unwrap_or_else(|_| DEFAULT_SUGGEST_MODEL.to_string());
        Some(Self {
            api_key,
            assess_model,
            suggest_model,
        })
    }
}
