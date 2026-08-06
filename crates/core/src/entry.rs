//! Entries: one accumulated fact, typed to the rendercv entry shape that renders it.

use diesel::prelude::*;
use serde::{Deserialize, Deserializer, Serialize};

use crate::section::{EntryType, SectionError};
use crate::store::schema::entry as entry_table;
use crate::store::{Store, StoreError};

/// An entry was rejected before it reached the store.
#[derive(Debug, thiserror::Error)]
pub enum EntryError {
    #[error("the fields given are not a {entry_type} entry")]
    Fields {
        entry_type: EntryType,
        #[source]
        source: serde_json::Error,
    },

    #[error("no entry with id {id}")]
    NoSuchEntry { id: i64 },
}

/// A rendercv `one-line` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OneLineFields {
    pub label: String,
    pub details: String,
}

/// A rendercv `normal` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalFields {
    pub name: String,
    pub location: Option<String>,
    pub date: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub summary: Option<String>,
    pub highlights: Option<Vec<String>>,
}

/// A rendercv `experience` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExperienceFields {
    pub company: String,
    pub position: String,
    pub location: Option<String>,
    pub date: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub summary: Option<String>,
    pub highlights: Option<Vec<String>>,
}

/// A rendercv `education` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EducationFields {
    pub institution: String,
    pub area: String,
    pub degree: Option<String>,
    pub location: Option<String>,
    pub date: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub summary: Option<String>,
    pub highlights: Option<Vec<String>>,
}

/// A rendercv `publication` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationFields {
    pub title: String,
    pub authors: Vec<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub journal: Option<String>,
    pub date: Option<String>,
    pub summary: Option<String>,
}

/// A rendercv `bullet` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BulletFields {
    pub bullet: String,
}

/// A rendercv `numbered` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumberedFields {
    pub number: String,
}

/// A rendercv `reversed-numbered` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReversedNumberedFields {
    pub reversed_number: String,
}

/// What an entry holds, in the shape rendercv reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum EntryFields {
    Text(String),
    OneLine(OneLineFields),
    Normal(NormalFields),
    Experience(ExperienceFields),
    Education(EducationFields),
    Publication(PublicationFields),
    Bullet(BulletFields),
    Numbered(NumberedFields),
    ReversedNumbered(ReversedNumberedFields),
}

impl EntryFields {
    /// The entry type these fields are the shape of.
    pub fn entry_type(&self) -> EntryType {
        match self {
            EntryFields::Text(_) => EntryType::Text,
            EntryFields::OneLine(_) => EntryType::OneLine,
            EntryFields::Normal(_) => EntryType::Normal,
            EntryFields::Experience(_) => EntryType::Experience,
            EntryFields::Education(_) => EntryType::Education,
            EntryFields::Publication(_) => EntryType::Publication,
            EntryFields::Bullet(_) => EntryType::Bullet,
            EntryFields::Numbered(_) => EntryType::Numbered,
            EntryFields::ReversedNumbered(_) => EntryType::ReversedNumbered,
        }
    }

    /// Reads fields of `entry_type` from the JSON rendercv would accept for it.
    pub fn parse(entry_type: EntryType, fields: serde_json::Value) -> Result<Self, EntryError> {
        fn read<T: serde::de::DeserializeOwned>(
            entry_type: EntryType,
            fields: serde_json::Value,
        ) -> Result<T, EntryError> {
            serde_json::from_value(fields)
                .map_err(|source| EntryError::Fields { entry_type, source })
        }

        Ok(match entry_type {
            EntryType::Text => EntryFields::Text(read(entry_type, fields)?),
            EntryType::OneLine => EntryFields::OneLine(read(entry_type, fields)?),
            EntryType::Normal => EntryFields::Normal(read(entry_type, fields)?),
            EntryType::Experience => EntryFields::Experience(read(entry_type, fields)?),
            EntryType::Education => EntryFields::Education(read(entry_type, fields)?),
            EntryType::Publication => EntryFields::Publication(read(entry_type, fields)?),
            EntryType::Bullet => EntryFields::Bullet(read(entry_type, fields)?),
            EntryType::Numbered => EntryFields::Numbered(read(entry_type, fields)?),
            EntryType::ReversedNumbered => EntryFields::ReversedNumbered(read(entry_type, fields)?),
        })
    }

    /// The dates the fields carry, if the type has any.
    fn dates(&self) -> Dates<'_> {
        match self {
            EntryFields::Normal(fields) => Dates {
                date: fields.date.as_deref(),
                start_date: fields.start_date.as_deref(),
                end_date: fields.end_date.as_deref(),
            },
            EntryFields::Experience(fields) => Dates {
                date: fields.date.as_deref(),
                start_date: fields.start_date.as_deref(),
                end_date: fields.end_date.as_deref(),
            },
            EntryFields::Education(fields) => Dates {
                date: fields.date.as_deref(),
                start_date: fields.start_date.as_deref(),
                end_date: fields.end_date.as_deref(),
            },
            EntryFields::Publication(fields) => Dates {
                date: fields.date.as_deref(),
                start_date: None,
                end_date: None,
            },
            EntryFields::Text(_)
            | EntryFields::OneLine(_)
            | EntryFields::Bullet(_)
            | EntryFields::Numbered(_)
            | EntryFields::ReversedNumbered(_) => Dates::NONE,
        }
    }
}

/// The date fields promoted out of [`EntryFields`] into their own columns.
struct Dates<'fields> {
    date: Option<&'fields str>,
    start_date: Option<&'fields str>,
    end_date: Option<&'fields str>,
}

impl Dates<'_> {
    const NONE: Self = Dates {
        date: None,
        start_date: None,
        end_date: None,
    };
}

/// One stored entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub id: i64,
    pub entry_type: EntryType,
    pub fields: EntryFields,
    pub created_at: String,
}

impl<'de> Deserialize<'de> for Entry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // `fields` is untagged, so its shape is read against the `entry_type` beside it.
        #[derive(Deserialize)]
        struct Wire {
            id: i64,
            entry_type: EntryType,
            fields: serde_json::Value,
            created_at: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        let fields =
            EntryFields::parse(wire.entry_type, wire.fields).map_err(serde::de::Error::custom)?;

        Ok(Entry {
            id: wire.id,
            entry_type: wire.entry_type,
            fields,
            created_at: wire.created_at,
        })
    }
}

/// A stored entry, before its type and fields are parsed.
#[derive(Queryable, Selectable)]
#[diesel(table_name = entry_table)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct EntryRow {
    id: i64,
    entry_type: String,
    fields: String,
    created_at: String,
}

impl EntryRow {
    fn decode(self) -> Result<Entry, StoreError> {
        let entry_type: EntryType = self.entry_type.parse()?;
        let id = self.id;
        let fields = decode_fields(entry_type, &self.fields)
            .map_err(|source| StoreError::DecodeEntry { id, source })?;

        Ok(Entry {
            id,
            entry_type,
            fields,
            created_at: self.created_at,
        })
    }
}

/// The columns an entry's fields are written to, the promoted dates among them.
#[derive(Insertable, AsChangeset)]
#[diesel(table_name = entry_table)]
#[diesel(treat_none_as_null = true)]
struct FieldsRow {
    entry_type: String,
    fields: String,
    date: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
}

impl FieldsRow {
    fn encode(fields: &EntryFields) -> Result<Self, StoreError> {
        let dates = fields.dates();

        Ok(Self {
            entry_type: fields.entry_type().to_string(),
            fields: serde_json::to_string(fields).map_err(StoreError::EncodeEntry)?,
            date: dates.date.map(str::to_owned),
            start_date: dates.start_date.map(str::to_owned),
            end_date: dates.end_date.map(str::to_owned),
        })
    }
}

/// Reads stored field JSON back into the shape `entry_type` names.
fn decode_fields(entry_type: EntryType, fields: &str) -> Result<EntryFields, EntryError> {
    let value =
        serde_json::from_str(fields).map_err(|source| EntryError::Fields { entry_type, source })?;

    EntryFields::parse(entry_type, value)
}

impl Store {
    /// Creates an entry.
    pub async fn create_entry(&self, fields: &EntryFields) -> Result<Entry, StoreError> {
        let row = FieldsRow::encode(fields)?;

        self.with_connection(move |connection| {
            diesel::insert_into(entry_table::table)
                .values(row)
                .returning(EntryRow::as_returning())
                .get_result(connection)
                .map_err(StoreError::WriteEntry)
        })
        .await?
        .decode()
    }

    /// Reads one entry by id.
    pub async fn entry(&self, id: i64) -> Result<Option<Entry>, StoreError> {
        let row = self
            .with_connection(move |connection| {
                entry_table::table
                    .find(id)
                    .select(EntryRow::as_select())
                    .first(connection)
                    .optional()
                    .map_err(StoreError::ReadEntry)
            })
            .await?;

        row.map(EntryRow::decode).transpose()
    }

    /// Every entry of `entry_type`, or every entry at all, oldest first.
    pub async fn entries(&self, entry_type: Option<EntryType>) -> Result<Vec<Entry>, StoreError> {
        let entry_type = entry_type.map(|entry_type| entry_type.to_string());

        let rows = self
            .with_connection(move |connection| {
                let mut query = entry_table::table.into_boxed();

                if let Some(entry_type) = entry_type {
                    query = query.filter(entry_table::entry_type.eq(entry_type));
                }

                query
                    .order(entry_table::id)
                    .select(EntryRow::as_select())
                    .load(connection)
                    .map_err(StoreError::ReadEntry)
            })
            .await?;

        rows.into_iter().map(EntryRow::decode).collect()
    }

    /// Every entry a section is eligible to hold.
    pub async fn entries_for_section(&self, section: &str) -> Result<Vec<Entry>, StoreError> {
        let section = self
            .section(section)
            .await?
            .ok_or_else(|| SectionError::NoSuchSection {
                name: section.to_string(),
            })?;

        self.entries(Some(section.entry_type)).await
    }

    /// Replaces an entry's fields, which are read against the type it already has.
    pub async fn update_entry(
        &self,
        id: i64,
        fields: serde_json::Value,
    ) -> Result<Entry, StoreError> {
        let current = self
            .entry(id)
            .await?
            .ok_or(EntryError::NoSuchEntry { id })?;
        let fields = EntryFields::parse(current.entry_type, fields)?;
        let row = FieldsRow::encode(&fields)?;

        self.with_connection(move |connection| {
            diesel::update(entry_table::table.find(id))
                .set(row)
                .returning(EntryRow::as_returning())
                .get_result(connection)
                .map_err(StoreError::WriteEntry)
        })
        .await?
        .decode()
    }

    /// Deletes an entry.
    pub async fn delete_entry(&self, id: i64) -> Result<(), StoreError> {
        let deleted = self
            .with_connection(move |connection| {
                diesel::delete(entry_table::table.find(id))
                    .execute(connection)
                    .map_err(StoreError::WriteEntry)
            })
            .await?;

        if deleted == 0 {
            return Err(EntryError::NoSuchEntry { id }.into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;
    use tempfile::TempDir;

    async fn open_temp_store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("store").join("hoskinator.db"))
            .await
            .expect("opening the store");
        (dir, store)
    }

    fn entry_error(error: StoreError) -> EntryError {
        match error {
            StoreError::Entry(error) => error,
            other => panic!("expected an EntryError, got {other:?}"),
        }
    }

    /// A minimal set of fields for each type, carrying only what rendercv requires.
    fn minimal(entry_type: EntryType) -> serde_json::Value {
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

    fn fields(entry_type: EntryType) -> EntryFields {
        EntryFields::parse(entry_type, minimal(entry_type)).expect("minimal fields parse")
    }

    fn experience() -> EntryFields {
        EntryFields::Experience(ExperienceFields {
            company: "Acme".into(),
            position: "Engineer".into(),
            location: Some("Ithaca, NY".into()),
            date: None,
            start_date: Some("2021-06".into()),
            end_date: Some("present".into()),
            summary: None,
            highlights: Some(vec!["Shipped the thing.".into()]),
        })
    }

    #[test]
    fn every_entry_type_has_a_fields_shape_that_answers_to_it() {
        for entry_type in EntryType::ALL {
            assert_eq!(fields(entry_type).entry_type(), entry_type);
        }
    }

    #[test]
    fn a_missing_required_field_is_rejected() {
        let missing =
            EntryFields::parse(EntryType::Experience, json!({ "company": "Acme" })).unwrap_err();

        assert!(
            matches!(missing, EntryError::Fields { entry_type, .. } if entry_type == EntryType::Experience)
        );
    }

    #[test]
    fn an_omitted_optional_field_reads_as_unset() {
        let EntryFields::Experience(fields) = fields(EntryType::Experience) else {
            panic!("expected experience fields");
        };

        assert_eq!(fields.start_date, None);
        assert_eq!(fields.highlights, None);
    }

    #[test]
    fn a_field_rendercv_does_not_have_is_rejected() {
        let unknown = EntryFields::parse(
            EntryType::Bullet,
            json!({ "bullet": "Shipped it.", "tags": ["rust"] }),
        )
        .unwrap_err();

        assert!(matches!(unknown, EntryError::Fields { .. }));
    }

    #[test]
    fn fields_of_another_type_are_rejected() {
        let mismatched =
            EntryFields::parse(EntryType::Education, minimal(EntryType::Experience)).unwrap_err();

        assert!(matches!(mismatched, EntryError::Fields { .. }));
    }

    #[test]
    fn fields_serialise_as_the_bare_shape_rendercv_reads() {
        assert_eq!(
            serde_json::to_value(fields(EntryType::Bullet)).unwrap(),
            json!({ "bullet": "Shipped it." })
        );
        assert_eq!(
            serde_json::to_value(fields(EntryType::Text)).unwrap(),
            json!("A line of prose.")
        );
    }

    #[tokio::test]
    async fn an_entry_of_every_type_reads_back() {
        let (_dir, store) = open_temp_store().await;

        for entry_type in EntryType::ALL {
            let created = store.create_entry(&fields(entry_type)).await.unwrap();

            assert_eq!(created.entry_type, entry_type);
            assert!(!created.created_at.is_empty());
            assert_eq!(store.entry(created.id).await.unwrap(), Some(created));
        }
    }

    #[tokio::test]
    async fn an_absent_entry_reads_as_none() {
        let (_dir, store) = open_temp_store().await;

        assert_eq!(store.entry(404).await.unwrap(), None);
    }

    #[tokio::test]
    async fn an_entry_survives_reopening_the_store() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");
        let created = Store::open(&path)
            .await
            .unwrap()
            .create_entry(&experience())
            .await
            .unwrap();

        let reopened = Store::open(&path).await.unwrap();

        assert_eq!(reopened.entry(created.id).await.unwrap(), Some(created));
    }

    #[tokio::test]
    async fn the_dates_the_fields_carry_are_promoted_to_their_own_columns() {
        let (_dir, store) = open_temp_store().await;
        let created = store.create_entry(&experience()).await.unwrap();

        let promoted = store
            .with_connection(move |connection| {
                entry_table::table
                    .find(created.id)
                    .select((
                        entry_table::date,
                        entry_table::start_date,
                        entry_table::end_date,
                    ))
                    .first::<(Option<String>, Option<String>, Option<String>)>(connection)
                    .unwrap()
            })
            .await;

        assert_eq!(
            promoted,
            (None, Some("2021-06".into()), Some("present".into()))
        );
    }

    #[tokio::test]
    async fn listing_filters_by_type_and_is_ordered_by_id() {
        let (_dir, store) = open_temp_store().await;
        let bullet = store
            .create_entry(&fields(EntryType::Bullet))
            .await
            .unwrap();
        store
            .create_entry(&fields(EntryType::Education))
            .await
            .unwrap();
        let second = store
            .create_entry(&fields(EntryType::Bullet))
            .await
            .unwrap();

        assert_eq!(store.entries(None).await.unwrap().len(), 3);
        assert_eq!(
            store.entries(Some(EntryType::Bullet)).await.unwrap(),
            vec![bullet, second]
        );
    }

    #[tokio::test]
    async fn a_section_is_eligible_for_the_entries_of_its_type() {
        let (_dir, store) = open_temp_store().await;
        store
            .create_section("Experience", EntryType::Experience)
            .await
            .unwrap();
        let matching = store
            .create_entry(&fields(EntryType::Experience))
            .await
            .unwrap();
        store
            .create_entry(&fields(EntryType::Education))
            .await
            .unwrap();

        assert_eq!(
            store.entries_for_section("Experience").await.unwrap(),
            vec![matching]
        );
    }

    #[tokio::test]
    async fn retyping_a_section_changes_which_entries_it_is_eligible_for() {
        let (_dir, store) = open_temp_store().await;
        store
            .create_section("Writing", EntryType::Normal)
            .await
            .unwrap();
        let publication = store
            .create_entry(&fields(EntryType::Publication))
            .await
            .unwrap();

        assert!(
            store
                .entries_for_section("Writing")
                .await
                .unwrap()
                .is_empty()
        );

        store
            .update_section("Writing", None, Some(EntryType::Publication))
            .await
            .unwrap();

        assert_eq!(
            store.entries_for_section("Writing").await.unwrap(),
            vec![publication]
        );
    }

    #[tokio::test]
    async fn listing_for_an_absent_section_is_rejected() {
        let (_dir, store) = open_temp_store().await;

        let missing = store.entries_for_section("Nowhere").await.unwrap_err();

        assert!(matches!(
            missing,
            StoreError::Section(SectionError::NoSuchSection { .. })
        ));
    }

    #[tokio::test]
    async fn updating_replaces_every_field_and_the_promoted_dates() {
        let (_dir, store) = open_temp_store().await;
        let created = store.create_entry(&experience()).await.unwrap();

        let updated = store
            .update_entry(
                created.id,
                json!({ "company": "Acme", "position": "Staff Engineer" }),
            )
            .await
            .unwrap();

        assert_eq!(
            updated.fields,
            EntryFields::parse(
                EntryType::Experience,
                json!({ "company": "Acme", "position": "Staff Engineer" })
            )
            .unwrap()
        );
        assert_eq!(store.entry(created.id).await.unwrap(), Some(updated));
    }

    #[tokio::test]
    async fn fields_of_another_type_do_not_update_an_entry() {
        let (_dir, store) = open_temp_store().await;
        let created = store.create_entry(&experience()).await.unwrap();

        let rejected = store
            .update_entry(created.id, minimal(EntryType::Education))
            .await
            .unwrap_err();

        assert!(matches!(entry_error(rejected), EntryError::Fields { .. }));
        assert_eq!(store.entry(created.id).await.unwrap(), Some(created));
    }

    #[tokio::test]
    async fn updating_an_absent_entry_is_rejected() {
        let (_dir, store) = open_temp_store().await;

        let missing = store
            .update_entry(404, minimal(EntryType::Bullet))
            .await
            .unwrap_err();

        assert!(matches!(
            entry_error(missing),
            EntryError::NoSuchEntry { id: 404 }
        ));
    }

    #[tokio::test]
    async fn a_deleted_entry_is_gone() {
        let (_dir, store) = open_temp_store().await;
        let created = store
            .create_entry(&fields(EntryType::Bullet))
            .await
            .unwrap();

        store.delete_entry(created.id).await.unwrap();

        assert_eq!(store.entry(created.id).await.unwrap(), None);
    }

    #[tokio::test]
    async fn deleting_an_absent_entry_is_rejected() {
        let (_dir, store) = open_temp_store().await;

        let missing = store.delete_entry(404).await.unwrap_err();

        assert!(matches!(
            entry_error(missing),
            EntryError::NoSuchEntry { id: 404 }
        ));
    }

    #[tokio::test]
    async fn an_entry_round_trips_through_its_wire_form() {
        let (_dir, store) = open_temp_store().await;
        let created = store.create_entry(&experience()).await.unwrap();

        let wire = serde_json::to_string(&created).unwrap();

        assert_eq!(serde_json::from_str::<Entry>(&wire).unwrap(), created);
    }
}
