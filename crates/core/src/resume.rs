//! Reading, writing, and validating the per-branch `resume.yaml` (ADR-0002).

use std::sync::OnceLock;

use serde_json::Value;

/// rendercv's own emitted JSON Schema, vendored at [`SCHEMA_VERSION`].
pub const SCHEMA: &str = include_str!("../schema/rendercv-2.8-schema.json");

/// The rendercv version [`SCHEMA`] was vendored from.
pub const SCHEMA_VERSION: &str = "2.8";

fn validator() -> &'static jsonschema::Validator {
    static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| {
        let schema: Value =
            serde_json::from_str(SCHEMA).expect("the vendored schema is valid JSON");
        jsonschema::validator_for(&schema).expect("the vendored schema compiles")
    })
}

/// Validates a rendercv document (a `{ "cv": { ... } }` value) against [`SCHEMA`].
pub fn validate(document: &Value) -> Result<(), Vec<String>> {
    let errors: Vec<String> = validator()
        .iter_errors(document)
        .map(|error| format!("{error} at {}", error.instance_path()))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_empty_cv_document_is_accepted() {
        assert_eq!(validate(&json!({ "cv": {} })), Ok(()));
    }

    #[test]
    fn an_unknown_social_network_is_rejected() {
        let document = json!({
            "cv": { "social_networks": [{ "network": "Friendster", "username": "ada" }] }
        });

        assert!(validate(&document).is_err());
    }
}
