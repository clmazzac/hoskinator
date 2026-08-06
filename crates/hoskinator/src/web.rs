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

    use hoskinator_core::entry::EntryFields;
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

    /// The shell's field lists, which the form is generated from.
    const FIELDS_TS: &str = include_str!("../../../web/src/entryFields.ts");

    /// The fields the shell says `entry_type` holds.
    fn listed_fields(entry_type: EntryType) -> Vec<String> {
        FIELDS_TS
            .split_once(&format!("\"{}\": [", entry_type.as_str()))
            .unwrap_or_else(|| panic!("{entry_type} is missing from ENTRY_FIELDS"))
            .1
            .split_once(']')
            .expect("a closing bracket")
            .0
            .split(',')
            .map(|field| field.trim().trim_matches('"').to_string())
            .filter(|field| !field.is_empty())
            .collect()
    }

    /// Fields carrying enough to make every list field an array once serialised.
    fn populated(entry_type: EntryType) -> serde_json::Value {
        match entry_type {
            EntryType::Text => serde_json::json!("A line of prose."),
            EntryType::OneLine => serde_json::json!({ "label": "Languages", "details": "Rust" }),
            EntryType::Normal => serde_json::json!({ "name": "Hoskinator", "highlights": ["It"] }),
            EntryType::Experience => serde_json::json!({
                "company": "Acme", "position": "Engineer", "highlights": ["It"]
            }),
            EntryType::Education => serde_json::json!({
                "institution": "Cornell", "area": "CS", "highlights": ["It"]
            }),
            EntryType::Publication => {
                serde_json::json!({ "title": "A Paper", "authors": ["Ada"] })
            }
            EntryType::Bullet => serde_json::json!({ "bullet": "Shipped it." }),
            EntryType::Numbered => serde_json::json!({ "number": "Shipped it." }),
            EntryType::ReversedNumbered => serde_json::json!({ "reversed_number": "Shipped it." }),
        }
    }

    /// What `entry_type` serialises to, as a field name and whether it holds a list.
    fn owned_fields(entry_type: EntryType) -> Vec<(String, bool)> {
        let fields = EntryFields::parse(entry_type, populated(entry_type))
            .unwrap_or_else(|error| panic!("{entry_type} fixture rejected: {error}"));

        match serde_json::to_value(&fields).unwrap() {
            serde_json::Value::Object(held) => held
                .into_iter()
                .map(|(name, value)| (name, value.is_array()))
                .collect(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn the_shell_lists_the_same_entry_fields_as_rust() {
        for entry_type in EntryType::ALL {
            let owned: Vec<String> = owned_fields(entry_type)
                .into_iter()
                .map(|(name, _)| name)
                .collect();
            let mut listed = listed_fields(entry_type);
            listed.sort();

            assert_eq!(listed, owned, "{entry_type} fields have drifted");
        }
    }

    #[test]
    fn the_shell_writes_a_list_for_every_field_rust_holds_as_one() {
        let listed: Vec<&str> = FIELDS_TS
            .split_once("LIST_FIELDS = [")
            .expect("a LIST_FIELDS array")
            .1
            .split_once(']')
            .expect("a closing bracket")
            .0
            .split(',')
            .map(|field| field.trim().trim_matches('"'))
            .filter(|field| !field.is_empty())
            .collect();

        let mut owned: Vec<String> = EntryType::ALL
            .into_iter()
            .flat_map(owned_fields)
            .filter(|(_, is_list)| *is_list)
            .map(|(name, _)| name)
            .collect();
        owned.sort();
        owned.dedup();

        let mut listed: Vec<String> = listed.into_iter().map(str::to_string).collect();
        listed.sort();

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
