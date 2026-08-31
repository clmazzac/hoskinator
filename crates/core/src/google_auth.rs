//! Google OAuth: the "installed app" loopback flow (RFC 8252) with PKCE, used to authorize
//! Sheets access.
//!
//! The client id and secret are the user's own Google Cloud project's, entered through
//! `google.set_credentials` — never baked into this binary (`docs/decisions/workspace.md`).

use std::path::Path;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

const AUTHORIZE_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const USERINFO_ENDPOINT: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const SPREADSHEETS_SCOPE: &str = "https://www.googleapis.com/auth/spreadsheets";

#[derive(Debug, thiserror::Error)]
pub enum GoogleAuthError {
    #[error("could not reach Google")]
    Request(#[source] reqwest::Error),
    #[error("Google rejected the request: {message}")]
    Denied { message: String },
    #[error("could not write the configuration")]
    Config(#[source] std::io::Error),
}

/// A PKCE verifier/challenge pair (RFC 7636, `S256`).
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
}

/// Generates a fresh PKCE pair.
pub fn generate_pkce() -> PkcePair {
    let verifier = random_token(32);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    PkcePair {
        verifier,
        challenge,
    }
}

/// Generates a fresh anti-CSRF `state` token.
pub fn generate_state() -> String {
    random_token(16)
}

fn random_token(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut buffer);
    URL_SAFE_NO_PAD.encode(buffer)
}

/// The URL to send the user's browser to, to start Google's consent screen.
pub fn authorize_url(client_id: &str, redirect_uri: &str, state: &str, challenge: &str) -> String {
    Url::parse_with_params(
        AUTHORIZE_ENDPOINT,
        &[
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("response_type", "code"),
            ("scope", SPREADSHEETS_SCOPE),
            ("state", state),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ],
    )
    .expect("the authorize endpoint is a valid base URL")
    .into()
}

/// A successful token response from Google's token endpoint.
#[derive(Debug)]
pub struct TokenResponse {
    pub access_token: String,
    /// Only present the first time a user consents, or when `prompt=consent` forces it again.
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

#[derive(Deserialize)]
struct TokenPayload {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

/// Exchanges an authorization code (from the loopback callback) for tokens.
pub fn exchange_code(
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<TokenResponse, GoogleAuthError> {
    token_request(
        TOKEN_ENDPOINT,
        &[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", verifier),
        ],
    )
}

/// Trades a stored refresh token for a fresh access token.
pub fn refresh_access_token(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<TokenResponse, GoogleAuthError> {
    token_request(
        TOKEN_ENDPOINT,
        &[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ],
    )
}

fn token_request(endpoint: &str, form: &[(&str, &str)]) -> Result<TokenResponse, GoogleAuthError> {
    let response = client()?
        .post(endpoint)
        .form(form)
        .send()
        .map_err(GoogleAuthError::Request)?;

    if !response.status().is_success() {
        return Err(GoogleAuthError::Denied {
            message: response.text().unwrap_or_default(),
        });
    }
    let payload: TokenPayload = response.json().map_err(GoogleAuthError::Request)?;
    Ok(TokenResponse {
        access_token: payload.access_token,
        refresh_token: payload.refresh_token,
        expires_in: payload.expires_in,
    })
}

#[derive(Deserialize)]
struct UserInfo {
    email: Option<String>,
}

/// The signed-in account's email, for a "Signed in as {email}" display. `None` on any failure.
pub fn account_email(access_token: &str) -> Option<String> {
    account_email_at(USERINFO_ENDPOINT, access_token)
}

fn account_email_at(endpoint: &str, access_token: &str) -> Option<String> {
    let response = client()
        .ok()?
        .get(endpoint)
        .bearer_auth(access_token)
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json::<UserInfo>().ok()?.email
}

fn client() -> Result<reqwest::blocking::Client, GoogleAuthError> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(GoogleAuthError::Request)
}

/// Writes `google_client_id`/`google_client_secret` into the config file, keeping anything else
/// already set.
pub fn remember_credentials(
    config_path: &Path,
    client_id: Option<&str>,
    client_secret: Option<&str>,
) -> Result<(), GoogleAuthError> {
    crate::config::remember_key(config_path, "google_client_id", client_id)
        .and_then(|()| {
            crate::config::remember_key(config_path, "google_client_secret", client_secret)
        })
        .map_err(GoogleAuthError::Config)
}

/// Writes `google_refresh_token` into the config file, keeping anything else already set.
pub fn remember_refresh_token(
    config_path: &Path,
    token: Option<&str>,
) -> Result<(), GoogleAuthError> {
    crate::config::remember_key(config_path, "google_refresh_token", token)
        .map_err(GoogleAuthError::Config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn pkce_pairs_are_url_safe_and_vary_each_call() {
        let first = generate_pkce();
        let second = generate_pkce();

        let is_url_safe = |value: &str| {
            value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        };
        assert!(is_url_safe(&first.verifier));
        assert!(is_url_safe(&first.challenge));
        assert_ne!(first.verifier, second.verifier);
        assert_ne!(first.challenge, second.challenge);
    }

    #[test]
    fn the_challenge_is_the_sha256_of_the_verifier() {
        let pair = generate_pkce();
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(pair.verifier.as_bytes()));
        assert_eq!(pair.challenge, expected);
    }

    #[test]
    fn state_tokens_vary_each_call() {
        assert_ne!(generate_state(), generate_state());
    }

    #[test]
    fn the_authorize_url_carries_pkce_and_state() {
        let url = authorize_url(
            "client-123",
            "http://127.0.0.1:8737/oauth/google/callback",
            "the-state",
            "the-challenge",
        );
        let parsed = Url::parse(&url).unwrap();
        let params: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();

        assert_eq!(parsed.host_str(), Some("accounts.google.com"));
        assert_eq!(params.get("client_id"), Some(&"client-123".to_string()));
        assert_eq!(
            params.get("redirect_uri"),
            Some(&"http://127.0.0.1:8737/oauth/google/callback".to_string())
        );
        assert_eq!(params.get("response_type"), Some(&"code".to_string()));
        assert_eq!(
            params.get("scope"),
            Some(&"https://www.googleapis.com/auth/spreadsheets".to_string())
        );
        assert_eq!(params.get("state"), Some(&"the-state".to_string()));
        assert_eq!(
            params.get("code_challenge"),
            Some(&"the-challenge".to_string())
        );
        assert_eq!(
            params.get("code_challenge_method"),
            Some(&"S256".to_string())
        );
        assert_eq!(params.get("access_type"), Some(&"offline".to_string()));
        assert_eq!(params.get("prompt"), Some(&"consent".to_string()));
    }

    #[tokio::test]
    async fn a_successful_token_response_is_parsed() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "at-1",
                "refresh_token": "rt-1",
                "expires_in": 3599,
            })))
            .mount(&server)
            .await;

        let endpoint = format!("{}/token", server.uri());
        let response = tokio::task::spawn_blocking(move || {
            token_request(&endpoint, &[("grant_type", "authorization_code")])
        })
        .await
        .unwrap()
        .unwrap();

        assert_eq!(response.access_token, "at-1");
        assert_eq!(response.refresh_token.as_deref(), Some("rt-1"));
        assert_eq!(response.expires_in, 3599);
    }

    #[tokio::test]
    async fn a_refresh_response_may_omit_a_new_refresh_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "at-2",
                "expires_in": 3599,
            })))
            .mount(&server)
            .await;

        let endpoint = format!("{}/token", server.uri());
        let response = tokio::task::spawn_blocking(move || {
            token_request(&endpoint, &[("grant_type", "refresh_token")])
        })
        .await
        .unwrap()
        .unwrap();

        assert_eq!(response.access_token, "at-2");
        assert_eq!(response.refresh_token, None);
    }

    #[tokio::test]
    async fn a_denied_request_reports_googles_error_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(400).set_body_string("invalid_grant"))
            .mount(&server)
            .await;

        let endpoint = format!("{}/token", server.uri());
        let error = tokio::task::spawn_blocking(move || token_request(&endpoint, &[]))
            .await
            .unwrap()
            .unwrap_err();

        assert!(matches!(error, GoogleAuthError::Denied { message } if message == "invalid_grant"));
    }

    #[tokio::test]
    async fn the_account_email_is_read_from_userinfo() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/userinfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "email": "cam@example.com",
            })))
            .mount(&server)
            .await;

        let endpoint = format!("{}/userinfo", server.uri());
        let email = tokio::task::spawn_blocking(move || account_email_at(&endpoint, "at-1"))
            .await
            .unwrap();

        assert_eq!(email.as_deref(), Some("cam@example.com"));
    }

    #[tokio::test]
    async fn a_failed_userinfo_lookup_yields_no_email_rather_than_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/userinfo"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let endpoint = format!("{}/userinfo", server.uri());
        let email = tokio::task::spawn_blocking(move || account_email_at(&endpoint, "bad-token"))
            .await
            .unwrap();

        assert_eq!(email, None);
    }

    #[test]
    fn remembering_credentials_writes_both_keys_and_keeps_others() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");
        std::fs::write(&config, "resume_repo = \"/srv/resume\"\n").unwrap();

        remember_credentials(&config, Some("client-id"), Some("client-secret")).unwrap();

        let written = std::fs::read_to_string(&config).unwrap();
        assert!(written.contains("resume_repo = \"/srv/resume\""));
        assert!(written.contains("google_client_id = \"client-id\""));
        assert!(written.contains("google_client_secret = \"client-secret\""));
    }

    #[test]
    fn remembering_none_clears_the_refresh_token() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");
        remember_refresh_token(&config, Some("rt-1")).unwrap();

        remember_refresh_token(&config, None).unwrap();

        let written = std::fs::read_to_string(&config).unwrap();
        assert!(!written.contains("google_refresh_token"));
    }
}
