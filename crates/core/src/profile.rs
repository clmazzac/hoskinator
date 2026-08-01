//! The singleton Profile: rendercv's `cv:` header.
//!
//! Mirrors every `cv:` field except `sections`. Every field is optional.

use serde::{Deserialize, Serialize};

use crate::store::{Store, StoreError};

/// A field rendercv accepts as either a single value or a list of them.
///
/// The form the user wrote is preserved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

/// A social network rendercv can render a connection for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SocialNetworkName {
    LinkedIn,
    GitHub,
    GitLab,
    #[serde(rename = "IMDB")]
    Imdb,
    Instagram,
    #[serde(rename = "ORCID")]
    Orcid,
    Mastodon,
    StackOverflow,
    ResearchGate,
    YouTube,
    #[serde(rename = "Google Scholar")]
    GoogleScholar,
    Telegram,
    WhatsApp,
    Leetcode,
    X,
    Bluesky,
    Reddit,
}

/// A username on a known social network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialNetwork {
    pub network: SocialNetworkName,
    pub username: String,
}

/// A connection rendercv has no built-in icon for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomConnection {
    pub fontawesome_icon: String,
    pub placeholder: String,
    pub url: Option<String>,
}

/// The singleton record of who the resumes belong to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: Option<String>,
    pub headline: Option<String>,
    pub location: Option<String>,
    pub photo: Option<String>,
    pub email: Option<OneOrMany<String>>,
    pub phone: Option<OneOrMany<String>>,
    pub website: Option<OneOrMany<String>>,
    pub social_networks: Vec<SocialNetwork>,
    pub custom_connections: Vec<CustomConnection>,
}

impl Store {
    /// Reads the Profile, yielding [`Profile::default`] when none has been written yet.
    pub async fn profile(&self) -> Result<Profile, StoreError> {
        let mut rows = self
            .connection()
            .query(
                "SELECT name, headline, location, photo, email, phone, website, \
                 social_networks, custom_connections FROM profile WHERE id = 1",
                (),
            )
            .await
            .map_err(StoreError::ReadProfile)?;

        let Some(row) = rows.next().await.map_err(StoreError::ReadProfile)? else {
            return Ok(Profile::default());
        };

        Ok(Profile {
            name: column(&row, 0)?,
            headline: column(&row, 1)?,
            location: column(&row, 2)?,
            photo: column(&row, 3)?,
            email: json_column(&row, 4, "email")?,
            phone: json_column(&row, 5, "phone")?,
            website: json_column(&row, 6, "website")?,
            social_networks: json_column(&row, 7, "social_networks")?.unwrap_or_default(),
            custom_connections: json_column(&row, 8, "custom_connections")?.unwrap_or_default(),
        })
    }

    /// Replaces the Profile wholesale.
    pub async fn set_profile(&self, profile: &Profile) -> Result<(), StoreError> {
        let params = vec![
            text(profile.name.as_deref()),
            text(profile.headline.as_deref()),
            text(profile.location.as_deref()),
            text(profile.photo.as_deref()),
            json(profile.email.as_ref(), "email")?,
            json(profile.phone.as_ref(), "phone")?,
            json(profile.website.as_ref(), "website")?,
            json_list(&profile.social_networks, "social_networks")?,
            json_list(&profile.custom_connections, "custom_connections")?,
        ];

        self.connection()
            .execute(
                "INSERT INTO profile \
                 (id, name, headline, location, photo, email, phone, website, \
                 social_networks, custom_connections) \
                 VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
                 ON CONFLICT(id) DO UPDATE SET \
                 name = excluded.name, headline = excluded.headline, \
                 location = excluded.location, photo = excluded.photo, \
                 email = excluded.email, phone = excluded.phone, \
                 website = excluded.website, social_networks = excluded.social_networks, \
                 custom_connections = excluded.custom_connections",
                params,
            )
            .await
            .map_err(StoreError::WriteProfile)?;

        Ok(())
    }
}

fn column(row: &libsql::Row, index: i32) -> Result<Option<String>, StoreError> {
    row.get::<Option<String>>(index)
        .map_err(StoreError::ReadProfile)
}

fn json_column<T: for<'de> Deserialize<'de>>(
    row: &libsql::Row,
    index: i32,
    name: &'static str,
) -> Result<Option<T>, StoreError> {
    let Some(text) = column(row, index)? else {
        return Ok(None);
    };

    serde_json::from_str(&text)
        .map(Some)
        .map_err(|source| StoreError::DecodeProfile {
            column: name,
            source,
        })
}

fn text(value: Option<&str>) -> libsql::Value {
    value.map_or(libsql::Value::Null, |value| {
        libsql::Value::Text(value.to_string())
    })
}

fn json<T: Serialize>(value: Option<&T>, name: &'static str) -> Result<libsql::Value, StoreError> {
    let Some(value) = value else {
        return Ok(libsql::Value::Null);
    };

    encode(value, name)
}

/// Encodes a list, storing SQL `NULL` for an empty one.
fn json_list<T: Serialize>(values: &[T], name: &'static str) -> Result<libsql::Value, StoreError> {
    if values.is_empty() {
        return Ok(libsql::Value::Null);
    }

    encode(values, name)
}

fn encode<T: Serialize + ?Sized>(
    value: &T,
    name: &'static str,
) -> Result<libsql::Value, StoreError> {
    serde_json::to_string(value)
        .map(libsql::Value::Text)
        .map_err(|source| StoreError::EncodeProfile {
            column: name,
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    async fn open_temp_store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");
        let store = Store::open(&path).await.expect("opening the store");
        (dir, store)
    }

    fn populated() -> Profile {
        Profile {
            name: Some("Ada Lovelace".into()),
            headline: Some("Mathematician".into()),
            location: Some("London".into()),
            photo: Some("photo.jpg".into()),
            email: Some(OneOrMany::One("ada@example.com".into())),
            phone: Some(OneOrMany::One("+12125550143".into())),
            website: Some(OneOrMany::Many(vec!["https://example.com".into()])),
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

    /// Every network name paired with the string rendercv expects on the wire.
    const WIRE_NAMES: &[(SocialNetworkName, &str)] = &[
        (SocialNetworkName::LinkedIn, "LinkedIn"),
        (SocialNetworkName::GitHub, "GitHub"),
        (SocialNetworkName::GitLab, "GitLab"),
        (SocialNetworkName::Imdb, "IMDB"),
        (SocialNetworkName::Instagram, "Instagram"),
        (SocialNetworkName::Orcid, "ORCID"),
        (SocialNetworkName::Mastodon, "Mastodon"),
        (SocialNetworkName::StackOverflow, "StackOverflow"),
        (SocialNetworkName::ResearchGate, "ResearchGate"),
        (SocialNetworkName::YouTube, "YouTube"),
        (SocialNetworkName::GoogleScholar, "Google Scholar"),
        (SocialNetworkName::Telegram, "Telegram"),
        (SocialNetworkName::WhatsApp, "WhatsApp"),
        (SocialNetworkName::Leetcode, "Leetcode"),
        (SocialNetworkName::X, "X"),
        (SocialNetworkName::Bluesky, "Bluesky"),
        (SocialNetworkName::Reddit, "Reddit"),
    ];

    #[test]
    fn every_network_serialises_to_the_name_rendercv_expects() {
        for (network, expected) in WIRE_NAMES {
            let encoded = serde_json::to_string(network).unwrap();

            assert_eq!(encoded, format!("\"{expected}\""));
            assert_eq!(
                serde_json::from_str::<SocialNetworkName>(&encoded).unwrap(),
                *network
            );
        }
    }

    #[test]
    fn a_scalar_stays_a_scalar_and_a_list_stays_a_list() {
        let one: OneOrMany<String> = serde_json::from_str("\"ada@example.com\"").unwrap();
        let many: OneOrMany<String> = serde_json::from_str("[\"ada@example.com\"]").unwrap();

        assert_eq!(serde_json::to_string(&one).unwrap(), "\"ada@example.com\"");
        assert_eq!(
            serde_json::to_string(&many).unwrap(),
            "[\"ada@example.com\"]"
        );
    }

    #[tokio::test]
    async fn an_unwritten_profile_reads_as_the_default() {
        let (_dir, store) = open_temp_store().await;

        assert_eq!(store.profile().await.unwrap(), Profile::default());
    }

    #[tokio::test]
    async fn a_written_profile_reads_back_unchanged() {
        let (_dir, store) = open_temp_store().await;
        let profile = populated();

        store.set_profile(&profile).await.unwrap();

        assert_eq!(store.profile().await.unwrap(), profile);
    }

    #[tokio::test]
    async fn writing_twice_replaces_rather_than_appends() {
        let (_dir, store) = open_temp_store().await;

        store.set_profile(&populated()).await.unwrap();
        let second = Profile {
            name: Some("Grace Hopper".into()),
            ..Profile::default()
        };
        store.set_profile(&second).await.unwrap();

        assert_eq!(store.profile().await.unwrap(), second);
    }

    #[tokio::test]
    async fn an_empty_profile_round_trips() {
        let (_dir, store) = open_temp_store().await;

        store.set_profile(&Profile::default()).await.unwrap();

        assert_eq!(store.profile().await.unwrap(), Profile::default());
    }

    #[tokio::test]
    async fn a_profile_survives_reopening_the_store() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");
        let profile = populated();

        let store = Store::open(&path).await.unwrap();
        store.set_profile(&profile).await.unwrap();
        drop(store);

        let reopened = Store::open(&path).await.unwrap();

        assert_eq!(reopened.profile().await.unwrap(), profile);
    }

    #[tokio::test]
    async fn multiple_emails_round_trip() {
        let (_dir, store) = open_temp_store().await;
        let profile = Profile {
            email: Some(OneOrMany::Many(vec![
                "ada@example.com".into(),
                "lovelace@example.org".into(),
            ])),
            ..Profile::default()
        };

        store.set_profile(&profile).await.unwrap();

        assert_eq!(store.profile().await.unwrap(), profile);
    }

    #[tokio::test]
    async fn a_corrupt_json_column_is_reported_with_its_name() {
        let (_dir, store) = open_temp_store().await;
        store.set_profile(&populated()).await.unwrap();
        store
            .connection()
            .execute(
                "UPDATE profile SET social_networks = 'not json' WHERE id = 1",
                (),
            )
            .await
            .unwrap();

        let error = store.profile().await.unwrap_err();

        assert!(
            matches!(
                error,
                StoreError::DecodeProfile {
                    column: "social_networks",
                    ..
                }
            ),
            "got {error:?}"
        );
    }
}
