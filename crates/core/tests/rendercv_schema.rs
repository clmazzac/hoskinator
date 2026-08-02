//! Checks that a serialised [`Profile`] is something rendercv will accept.

use hoskinator_core::profile::{
    CustomConnection, OneOrMany, Profile, SocialNetwork, SocialNetworkName,
};
use serde_json::{Value, json};

/// rendercv's own emitted JSON Schema.
const SCHEMA: &str = include_str!("fixtures/rendercv-2.8-schema.json");

/// The rendercv version the vendored schema came from.
const SCHEMA_VERSION: &str = "2.8";

fn validator() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(SCHEMA).expect("the vendored schema is valid JSON");
    jsonschema::validator_for(&schema).expect("the vendored schema compiles")
}

/// Wraps a Profile as the `cv:` block of a rendercv document.
fn as_document(profile: &Profile) -> Value {
    json!({ "cv": serde_json::to_value(profile).unwrap() })
}

fn errors(document: &Value) -> Vec<String> {
    validator()
        .iter_errors(document)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect()
}

fn populated() -> Profile {
    Profile {
        name: Some("Ada Lovelace".into()),
        headline: Some("Mathematician".into()),
        location: Some("London".into()),
        photo: None,
        email: Some(OneOrMany::One("ada@example.com".into())),
        phone: Some(OneOrMany::One("+12125550143".into())),
        website: Some(OneOrMany::One("https://example.com".into())),
        social_networks: vec![SocialNetwork {
            network: SocialNetworkName::GitHub,
            username: "ada".into(),
        }],
        custom_connections: vec![CustomConnection {
            fontawesome_icon: "fa-brands fa-discord".into(),
            placeholder: "ada#0001".into(),
            url: None,
        }],
    }
}

#[test]
fn a_populated_profile_is_accepted() {
    let document = as_document(&populated());

    let errors = errors(&document);

    assert!(errors.is_empty(), "rejected: {errors:#?}");
}

#[test]
fn an_empty_profile_is_accepted() {
    let document = as_document(&Profile::default());

    let errors = errors(&document);

    assert!(errors.is_empty(), "rejected: {errors:#?}");
}

#[test]
fn several_emails_are_accepted() {
    let profile = Profile {
        email: Some(OneOrMany::Many(vec![
            "ada@example.com".into(),
            "lovelace@example.org".into(),
        ])),
        ..Profile::default()
    };

    let errors = errors(&as_document(&profile));

    assert!(errors.is_empty(), "rejected: {errors:#?}");
}

#[test]
fn every_network_name_is_one_rendercv_knows() {
    for network in NETWORKS {
        let profile = Profile {
            social_networks: vec![SocialNetwork {
                network,
                username: "someone".into(),
            }],
            ..Profile::default()
        };

        let errors = errors(&as_document(&profile));

        assert!(errors.is_empty(), "{network:?} rejected: {errors:#?}");
    }
}

#[test]
fn a_network_rendercv_does_not_know_is_rejected() {
    let mut document = as_document(&Profile::default());
    document["cv"]["social_networks"] = json!([{ "network": "Friendster", "username": "ada" }]);

    let errors = errors(&document);

    assert!(!errors.is_empty(), "an unknown network should not validate");
}

/// Fails if the installed rendercv is not the version the fixture was taken from.
///
/// Ignored by default; run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn the_vendored_schema_matches_the_installed_rendercv() {
    let output = std::process::Command::new("rendercv")
        .arg("--version")
        .output()
        .expect("rendercv is not on PATH");
    let reported = String::from_utf8_lossy(&output.stdout);

    assert!(
        reported.contains(SCHEMA_VERSION),
        "fixture is from rendercv {SCHEMA_VERSION}, but `rendercv --version` said {reported:?}"
    );
}

const NETWORKS: [SocialNetworkName; 17] = [
    SocialNetworkName::LinkedIn,
    SocialNetworkName::GitHub,
    SocialNetworkName::GitLab,
    SocialNetworkName::Imdb,
    SocialNetworkName::Instagram,
    SocialNetworkName::Orcid,
    SocialNetworkName::Mastodon,
    SocialNetworkName::StackOverflow,
    SocialNetworkName::ResearchGate,
    SocialNetworkName::YouTube,
    SocialNetworkName::GoogleScholar,
    SocialNetworkName::Telegram,
    SocialNetworkName::WhatsApp,
    SocialNetworkName::Leetcode,
    SocialNetworkName::X,
    SocialNetworkName::Bluesky,
    SocialNetworkName::Reddit,
];
