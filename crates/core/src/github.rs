//! The GitHub account a resume repository syncs to, authorized by a personal access token.
//!
//! The token lives in one file inside Home and never leaves this machine except inside
//! requests to GitHub's API. Pushes carry it through a per-invocation credential helper, so
//! it is never written into the repository's own configuration.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

const API: &str = "https://api.github.com";
const TOKEN_FILE: &str = "github-token";

#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    #[error("that token was not accepted by GitHub")]
    Unauthorized,
    #[error("GitHub refused the request: {0}")]
    Api(String),
    #[error("Home could not be resolved")]
    Home(#[source] crate::home::HomeError),
    #[error("could not read or write the stored token")]
    Io(#[source] std::io::Error),
}

/// The account the token belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub login: String,
}

/// One repository the account can reach.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    /// The repository as `owner/name`.
    pub name_with_owner: String,
    pub private: bool,
}

fn token_path() -> Result<PathBuf, GithubError> {
    let home = crate::home::Home::resolve().map_err(GithubError::Home)?;
    Ok(home.root().join(TOKEN_FILE))
}

/// The stored token, if one is.
pub fn read_token() -> Result<Option<String>, GithubError> {
    match fs::read_to_string(token_path()?) {
        Ok(text) => {
            let token = text.trim().to_string();
            Ok((!token.is_empty()).then_some(token))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(GithubError::Io(source)),
    }
}

fn store_token(token: &str) -> Result<(), GithubError> {
    let path = token_path()?;
    fs::write(&path, format!("{token}\n")).map_err(GithubError::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(GithubError::Io)?;
    }
    Ok(())
}

/// Forgets the stored token; forgetting twice is fine.
pub fn clear_token() -> Result<(), GithubError> {
    match fs::remove_file(token_path()?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(GithubError::Io(source)),
    }
}

fn client() -> Client {
    Client::builder()
        .user_agent("hoskinator")
        .timeout(Duration::from_secs(30))
        .build()
        .expect("reqwest builds with its default configuration")
}

fn get(path: &str, token: &str) -> Result<reqwest::blocking::Response, GithubError> {
    client()
        .get(format!("{API}{path}"))
        .bearer_auth(token)
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|source| GithubError::Api(source.to_string()))
}

fn post(
    path: &str,
    token: &str,
    body: &serde_json::Value,
) -> Result<reqwest::blocking::Response, GithubError> {
    client()
        .post(format!("{API}{path}"))
        .bearer_auth(token)
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(body)
        .send()
        .map_err(|source| GithubError::Api(source.to_string()))
}

fn check(
    response: reqwest::blocking::Response,
) -> Result<reqwest::blocking::Response, GithubError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let message = response
        .json::<serde_json::Value>()
        .ok()
        .and_then(|body| body["message"].as_str().map(str::to_owned))
        .unwrap_or_else(|| status.to_string());
    if status.as_u16() == 401 || status.as_u16() == 403 {
        Err(GithubError::Unauthorized)
    } else {
        Err(GithubError::Api(message))
    }
}

/// The login the token belongs to, or an error if GitHub rejects it.
pub fn verify(token: &str) -> Result<String, GithubError> {
    check(get("/user", token)?)?
        .json::<Account>()
        .map(|account| account.login)
        .map_err(|source| GithubError::Api(source.to_string()))
}

/// The login of the stored token; `None` when no token is stored, an error when it stopped
/// working (so a stale token is visible rather than silently ignored).
pub fn status() -> Result<Option<String>, GithubError> {
    match read_token()? {
        None => Ok(None),
        Some(token) => verify(&token).map(Some),
    }
}

/// Remembers a token after checking GitHub accepts it, and returns the login it belongs to.
pub fn authorize(token: &str) -> Result<String, GithubError> {
    let login = verify(token)?;
    store_token(token)?;
    Ok(login)
}

/// Every repository the account owns, newest push first.
pub fn repositories(token: &str) -> Result<Vec<Repository>, GithubError> {
    let owned = check(get(
        "/user/repos?per_page=100&sort=pushed&affiliation=owner",
        token,
    )?)?
    .json::<Vec<serde_json::Value>>()
    .map_err(|source| GithubError::Api(source.to_string()))?;
    Ok(owned
        .into_iter()
        .filter_map(|repository| {
            Some(Repository {
                name_with_owner: repository["full_name"].as_str()?.to_string(),
                private: repository["private"].as_bool().unwrap_or(false),
            })
        })
        .collect())
}

/// Creates a private repository under the account and returns it as `owner/name`.
pub fn create_repository(token: &str, name: &str) -> Result<String, GithubError> {
    let created = check(post(
        "/user/repos",
        token,
        &serde_json::json!({ "name": name, "private": true }),
    )?)?
    .json::<serde_json::Value>()
    .map_err(|source| GithubError::Api(source.to_string()))?;
    created["full_name"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| GithubError::Api("the created repository had no name".into()))
}
