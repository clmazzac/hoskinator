//! The singleton Profile: rendercv's `cv:` header.
//!
//! Mirrors every `cv:` field except `sections`. Every field is optional.

use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::store::schema::profile as profile_table;
use crate::store::{Store, StoreError};

/// The `profile` row's fixed primary key.
const PROFILE_ID: i32 = 1;

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
///
/// Every field is optional on the wire: an omitted field reads as unset.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
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

/// The stored Profile: its JSON columns still encoded.
#[derive(Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = profile_table)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true, treat_none_as_default_value = false)]
struct ProfileRow {
    name: Option<String>,
    headline: Option<String>,
    location: Option<String>,
    photo: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    website: Option<String>,
    social_networks: Option<String>,
    custom_connections: Option<String>,
}

impl ProfileRow {
    fn encode(profile: &Profile) -> Result<Self, StoreError> {
        Ok(Self {
            name: profile.name.clone(),
            headline: profile.headline.clone(),
            location: profile.location.clone(),
            photo: profile.photo.clone(),
            email: encode(profile.email.as_ref(), "email")?,
            phone: encode(profile.phone.as_ref(), "phone")?,
            website: encode(profile.website.as_ref(), "website")?,
            social_networks: encode_list(&profile.social_networks, "social_networks")?,
            custom_connections: encode_list(&profile.custom_connections, "custom_connections")?,
        })
    }

    fn decode(self) -> Result<Profile, StoreError> {
        Ok(Profile {
            name: self.name,
            headline: self.headline,
            location: self.location,
            photo: self.photo,
            email: decode(self.email, "email")?,
            phone: decode(self.phone, "phone")?,
            website: decode(self.website, "website")?,
            social_networks: decode(self.social_networks, "social_networks")?.unwrap_or_default(),
            custom_connections: decode(self.custom_connections, "custom_connections")?
                .unwrap_or_default(),
        })
    }
}

impl Store {
    /// Reads the Profile, yielding [`Profile::default`] when none has been written yet.
    pub async fn profile(&self) -> Result<Profile, StoreError> {
        let row = self
            .with_connection(|connection| {
                profile_table::table
                    .find(PROFILE_ID)
                    .select(ProfileRow::as_select())
                    .first(connection)
                    .optional()
                    .map_err(StoreError::ReadProfile)
            })
            .await?;

        match row {
            Some(row) => row.decode(),
            None => Ok(Profile::default()),
        }
    }

    /// Replaces the Profile wholesale.
    pub async fn set_profile(&self, profile: &Profile) -> Result<(), StoreError> {
        let row = ProfileRow::encode(profile)?;

        self.with_connection(move |connection| {
            diesel::insert_into(profile_table::table)
                .values((profile_table::id.eq(PROFILE_ID), &row))
                .on_conflict(profile_table::id)
                .do_update()
                .set(&row)
                .execute(connection)
                .map_err(StoreError::WriteProfile)
        })
        .await?;

        Ok(())
    }
}

fn decode<T: for<'de> Deserialize<'de>>(
    stored: Option<String>,
    column: &'static str,
) -> Result<Option<T>, StoreError> {
    let Some(stored) = stored else {
        return Ok(None);
    };

    serde_json::from_str(&stored)
        .map(Some)
        .map_err(|source| StoreError::DecodeProfile { column, source })
}

fn encode<T: Serialize>(
    value: Option<&T>,
    column: &'static str,
) -> Result<Option<String>, StoreError> {
    value.map(|value| to_json(value, column)).transpose()
}

/// Encodes a list, storing SQL `NULL` for an empty one.
fn encode_list<T: Serialize>(
    values: &[T],
    column: &'static str,
) -> Result<Option<String>, StoreError> {
    if values.is_empty() {
        return Ok(None);
    }

    to_json(values, column).map(Some)
}

fn to_json<T: Serialize + ?Sized>(value: &T, column: &'static str) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(|source| StoreError::EncodeProfile { column, source })
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
            .with_connection(|connection| {
                diesel::update(profile_table::table.find(PROFILE_ID))
                    .set(profile_table::social_networks.eq("not json"))
                    .execute(connection)
                    .unwrap()
            })
            .await;

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
