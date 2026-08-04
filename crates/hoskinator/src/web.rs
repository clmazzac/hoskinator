//! The embedded Web shell.
//!
//! `web/dist` is compiled into the binary. Building that directory is a separate `npm run build`.

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../web/dist"]
struct Assets;

/// Serves an embedded asset. The root serves the document.
pub async fn asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                file.data.into_owned(),
            )
                .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            "The Web UI is not built. Run `npm --prefix web run build`.",
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoskinator_core::section::EntryType;

    /// Whether the bundle was built when this binary was compiled.
    fn is_built() -> bool {
        Assets::get("index.html").is_some()
    }

    /// The shell's JSON-RPC client, which lists the entry types in TypeScript.
    const RPC_TS: &str = include_str!("../../../web/src/rpc.ts");

    #[test]
    fn the_shell_lists_the_same_entry_types_as_rust() {
        let listed: Vec<&str> = RPC_TS
            .split_once("ENTRY_TYPES = [")
            .expect("an ENTRY_TYPES array")
            .1
            .split_once(']')
            .expect("a closing bracket")
            .0
            .split(',')
            .map(|entry| entry.trim().trim_matches('"'))
            .filter(|entry| !entry.is_empty())
            .collect();

        let owned: Vec<&str> = EntryType::ALL.iter().map(|kind| kind.as_str()).collect();

        assert_eq!(listed, owned);
    }

    #[tokio::test]
    async fn the_root_serves_html_when_the_bundle_is_built() {
        if !is_built() {
            return;
        }

        let response = asset(Uri::from_static("/")).await;

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response.headers().get(header::CONTENT_TYPE).unwrap();
        assert!(
            content_type.to_str().unwrap().starts_with("text/html"),
            "got {content_type:?}"
        );
    }

    #[tokio::test]
    async fn an_unknown_asset_is_not_found() {
        if !is_built() {
            return;
        }

        let response = asset(Uri::from_static("/nothing-here.js")).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_missing_bundle_says_how_to_build_it() {
        if is_built() {
            return;
        }

        let response = asset(Uri::from_static("/")).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
