//! Checks that a serialised [`Profile`] is something rendercv will accept.

use hoskinator_core::entry::EntryFields;
use hoskinator_core::profile::{
    CustomConnection, OneOrMany, Profile, SocialNetwork, SocialNetworkName,
};
use hoskinator_core::section::EntryType;
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

/// A minimal entry of the given type, carrying only rendercv's required fields.
fn entry(entry_type: EntryType) -> Value {
    match entry_type {
        EntryType::Text => json!("A line of prose."),
        EntryType::OneLine => json!({ "label": "Languages", "details": "Rust, Python" }),
        EntryType::Normal => json!({ "name": "Hoskinator" }),
        EntryType::Experience => json!({ "company": "Acme", "position": "Engineer" }),
        EntryType::Education => json!({ "institution": "Cornell", "area": "CS" }),
        EntryType::Publication => json!({ "title": "A Paper", "authors": ["Ada"] }),
        EntryType::Bullet => json!({ "bullet": "Shipped it." }),
        EntryType::Numbered => json!({ "number": "Shipped it." }),
        EntryType::ReversedNumbered => json!({ "reversed_number": "Shipped it." }),
    }
}

/// Wraps a section's entries as a rendercv document.
fn with_section(entries: Value) -> Value {
    json!({ "cv": { "sections": { "Stuff": entries } } })
}

#[test]
fn every_entry_type_is_an_arm_rendercv_accepts() {
    for entry_type in EntryType::ALL {
        let document = with_section(json!([entry(entry_type), entry(entry_type)]));

        let errors = errors(&document);

        assert!(errors.is_empty(), "{entry_type} rejected: {errors:#?}");
    }
}

#[test]
fn a_section_mixing_entry_types_is_rejected() {
    for entry_type in EntryType::ALL {
        for other in EntryType::ALL {
            if entry_type == other {
                continue;
            }

            let document = with_section(json!([entry(entry_type), entry(other)]));

            assert!(
                !errors(&document).is_empty(),
                "{entry_type} mixed with {other} should not validate"
            );
        }
    }
}

/// Every field the store holds for the given type.
fn populated_entry(entry_type: EntryType) -> Value {
    match entry_type {
        EntryType::Text => json!("A line of prose."),
        EntryType::OneLine => json!({ "label": "Languages", "details": "Rust, Python" }),
        EntryType::Normal => json!({
            "name": "Hoskinator",
            "location": "Ithaca, NY",
            "date": "2025",
            "start_date": "2025-01",
            "end_date": "present",
            "summary": "A resume manager.",
        }),
        EntryType::Experience => json!({
            "company": "Acme",
            "position": "Engineer",
            "location": "Remote",
            "date": "2021",
            "start_date": "2021-06",
            "end_date": "present",
            "summary": "Built services.",
        }),
        EntryType::Education => json!({
            "institution": "Cornell",
            "area": "Computer Science",
            "degree": "BS",
            "location": "Ithaca, NY",
            "date": "2024",
            "start_date": "2020-08",
            "end_date": "2024-05",
            "summary": "Systems coursework.",
        }),
        EntryType::Publication => json!({
            "title": "A Paper",
            "authors": ["Ada", "**Cam**"],
            "doi": "10.1000/example",
            "url": "https://example.com/paper",
            "journal": "Nature",
            "date": "2023-04",
            "summary": "A summary.",
        }),
        EntryType::Bullet => json!({ "bullet": "Shipped it." }),
        EntryType::Numbered => json!({ "number": "Shipped it." }),
        EntryType::ReversedNumbered => json!({ "reversed_number": "Shipped it." }),
    }
}

#[test]
fn stored_fields_render_as_an_entry_rendercv_accepts() {
    for entry_type in EntryType::ALL {
        for input in [entry(entry_type), populated_entry(entry_type)] {
            let fields = EntryFields::parse(entry_type, input)
                .unwrap_or_else(|error| panic!("{entry_type} fields rejected: {error}"));
            let document = with_section(json!([serde_json::to_value(&fields).unwrap()]));

            let errors = errors(&document);

            assert!(errors.is_empty(), "{entry_type} rejected: {errors:#?}");
        }
    }
}

/// The name rendercv's schema gives the entry type's definition, if it has one.
fn definition_of(entry_type: EntryType) -> Option<&'static str> {
    match entry_type {
        // A text entry is a bare string, so rendercv defines no object for it.
        EntryType::Text => None,
        EntryType::OneLine => Some("rendercv__schema__models__cv__entries__one_line__OneLineEntry"),
        EntryType::Normal => Some("rendercv__schema__models__cv__entries__normal__NormalEntry"),
        EntryType::Experience => {
            Some("rendercv__schema__models__cv__entries__experience__ExperienceEntry")
        }
        EntryType::Education => {
            Some("rendercv__schema__models__cv__entries__education__EducationEntry")
        }
        EntryType::Publication => {
            Some("rendercv__schema__models__cv__entries__publication__PublicationEntry")
        }
        EntryType::Bullet => Some("BulletEntry"),
        EntryType::Numbered => Some("NumberedEntry"),
        EntryType::ReversedNumbered => Some("ReversedNumberedEntry"),
    }
}

#[test]
fn bullets_are_carried_by_exactly_the_types_rendercv_gives_highlights() {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();

    for entry_type in EntryType::ALL {
        let has_highlights = definition_of(entry_type).is_some_and(|definition| {
            schema["$defs"][definition]["properties"]
                .get("highlights")
                .is_some()
        });

        assert_eq!(
            entry_type.carries_bullets(),
            has_highlights,
            "{entry_type} disagrees with rendercv about highlights"
        );
    }
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
