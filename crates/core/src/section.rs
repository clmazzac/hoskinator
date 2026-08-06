//! Sections: the managed `{ name, entry_type }` vocabulary of the Master Store.

use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::store::schema::section as section_table;
use crate::store::{Store, StoreError};

/// A section was rejected before it reached the store.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SectionError {
    #[error("a section name cannot be blank")]
    BlankName,

    #[error("a section named {name:?} already exists")]
    DuplicateName { name: String },

    #[error("no section named {name:?}")]
    NoSuchSection { name: String },

    #[error("{0:?} is not an entry type rendercv knows")]
    UnknownEntryType(String),
}

/// One of the entry shapes rendercv accepts in a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryType {
    Text,
    OneLine,
    Normal,
    Experience,
    Education,
    Publication,
    Bullet,
    Numbered,
    ReversedNumbered,
}

impl EntryType {
    /// Every variant, in the order rendercv lists its arms.
    pub const ALL: [EntryType; 9] = [
        EntryType::Text,
        EntryType::OneLine,
        EntryType::Normal,
        EntryType::Experience,
        EntryType::Education,
        EntryType::Publication,
        EntryType::Bullet,
        EntryType::Numbered,
        EntryType::ReversedNumbered,
    ];

    /// Whether entries of this type hold accomplishments as Bullets.
    pub fn carries_bullets(self) -> bool {
        matches!(
            self,
            EntryType::Normal | EntryType::Experience | EntryType::Education
        )
    }

    /// How the variant is spelled in the store and over JSON-RPC.
    pub fn as_str(self) -> &'static str {
        match self {
            EntryType::Text => "text",
            EntryType::OneLine => "one-line",
            EntryType::Normal => "normal",
            EntryType::Experience => "experience",
            EntryType::Education => "education",
            EntryType::Publication => "publication",
            EntryType::Bullet => "bullet",
            EntryType::Numbered => "numbered",
            EntryType::ReversedNumbered => "reversed-numbered",
        }
    }
}

impl std::str::FromStr for EntryType {
    type Err = SectionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        EntryType::ALL
            .into_iter()
            .find(|entry_type| entry_type.as_str() == value)
            .ok_or_else(|| SectionError::UnknownEntryType(value.to_string()))
    }
}

impl std::fmt::Display for EntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A section of a resume: a user-chosen name and the entry shape it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub entry_type: EntryType,
}

/// A stored section, before its `entry_type` is parsed.
#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = section_table)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct SectionRow {
    name: String,
    entry_type: String,
}

impl TryFrom<SectionRow> for Section {
    type Error = StoreError;

    fn try_from(row: SectionRow) -> Result<Self, Self::Error> {
        Ok(Section {
            name: row.name,
            entry_type: row.entry_type.parse()?,
        })
    }
}

impl Store {
    /// Creates a section.
    pub async fn create_section(
        &self,
        name: &str,
        entry_type: EntryType,
    ) -> Result<Section, StoreError> {
        let name = clean_name(name)?;

        if self.section(&name).await?.is_some() {
            return Err(SectionError::DuplicateName { name }.into());
        }

        let row = SectionRow {
            name: name.clone(),
            entry_type: entry_type.to_string(),
        };

        self.with_connection(move |connection| {
            diesel::insert_into(section_table::table)
                .values(row)
                .execute(connection)
                .map_err(StoreError::WriteSection)
        })
        .await?;

        Ok(Section { name, entry_type })
    }

    /// Reads one section by name.
    pub async fn section(&self, name: &str) -> Result<Option<Section>, StoreError> {
        let name = name.to_string();

        let row = self
            .with_connection(move |connection| {
                section_table::table
                    .find(name)
                    .select(SectionRow::as_select())
                    .first(connection)
                    .optional()
                    .map_err(StoreError::ReadSection)
            })
            .await?;

        row.map(Section::try_from).transpose()
    }

    /// Every section, ordered by name.
    pub async fn sections(&self) -> Result<Vec<Section>, StoreError> {
        let rows = self
            .with_connection(|connection| {
                section_table::table
                    .order(section_table::name)
                    .select(SectionRow::as_select())
                    .load(connection)
                    .map_err(StoreError::ReadSection)
            })
            .await?;

        rows.into_iter().map(Section::try_from).collect()
    }

    /// Renames a section, retypes it, or both.
    pub async fn update_section(
        &self,
        name: &str,
        new_name: Option<&str>,
        entry_type: Option<EntryType>,
    ) -> Result<Section, StoreError> {
        let current = self
            .section(name)
            .await?
            .ok_or_else(|| SectionError::NoSuchSection {
                name: name.to_string(),
            })?;

        let updated = Section {
            name: match new_name {
                Some(new_name) => clean_name(new_name)?,
                None => current.name.clone(),
            },
            entry_type: entry_type.unwrap_or(current.entry_type),
        };

        if updated.name != current.name && self.section(&updated.name).await?.is_some() {
            return Err(SectionError::DuplicateName { name: updated.name }.into());
        }

        let new_name = updated.name.clone();
        let new_entry_type = updated.entry_type.to_string();

        // Set column by column rather than from a changeset struct: `name` is the primary key,
        // which `AsChangeset` skips, and a rename is exactly a change to it.
        self.with_connection(move |connection| {
            diesel::update(section_table::table.find(current.name))
                .set((
                    section_table::name.eq(new_name),
                    section_table::entry_type.eq(new_entry_type),
                ))
                .execute(connection)
                .map_err(StoreError::WriteSection)
        })
        .await?;

        Ok(updated)
    }

    /// Deletes a section.
    pub async fn delete_section(&self, name: &str) -> Result<(), StoreError> {
        let target = name.to_string();

        let deleted = self
            .with_connection(move |connection| {
                diesel::delete(section_table::table.find(target))
                    .execute(connection)
                    .map_err(StoreError::WriteSection)
            })
            .await?;

        if deleted == 0 {
            return Err(SectionError::NoSuchSection {
                name: name.to_string(),
            }
            .into());
        }

        Ok(())
    }
}

/// Trims a name and rejects it if nothing is left.
fn clean_name(name: &str) -> Result<String, SectionError> {
    let name = name.trim();

    if name.is_empty() {
        return Err(SectionError::BlankName);
    }

    Ok(name.to_string())
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

    fn section_error(error: StoreError) -> SectionError {
        match error {
            StoreError::Section(error) => error,
            other => panic!("expected a SectionError, got {other:?}"),
        }
    }

    #[test]
    fn every_entry_type_round_trips_through_its_wire_string() {
        for entry_type in EntryType::ALL {
            assert_eq!(entry_type.as_str().parse(), Ok(entry_type));
        }
    }

    #[test]
    fn the_wire_string_is_what_serde_emits() {
        for entry_type in EntryType::ALL {
            let serialised = serde_json::to_string(&entry_type).unwrap();

            assert_eq!(serialised, format!("\"{}\"", entry_type.as_str()));
        }
    }

    #[test]
    fn an_entry_type_rendercv_does_not_have_is_rejected() {
        assert_eq!(
            "timeline".parse::<EntryType>(),
            Err(SectionError::UnknownEntryType("timeline".into()))
        );
    }

    #[tokio::test]
    async fn a_created_section_reads_back() {
        let (_dir, store) = open_temp_store().await;

        let created = store
            .create_section("Experience", EntryType::Experience)
            .await
            .unwrap();

        assert_eq!(store.section("Experience").await.unwrap(), Some(created));
    }

    #[tokio::test]
    async fn an_absent_section_reads_as_none() {
        let (_dir, store) = open_temp_store().await;

        assert_eq!(store.section("Experience").await.unwrap(), None);
    }

    #[tokio::test]
    async fn sections_come_back_ordered_by_name() {
        let (_dir, store) = open_temp_store().await;
        store
            .create_section("Projects", EntryType::Normal)
            .await
            .unwrap();
        store
            .create_section("Education", EntryType::Education)
            .await
            .unwrap();

        let names: Vec<String> = store
            .sections()
            .await
            .unwrap()
            .into_iter()
            .map(|section| section.name)
            .collect();

        assert_eq!(names, ["Education", "Projects"]);
    }

    #[tokio::test]
    async fn a_duplicate_name_is_rejected() {
        let (_dir, store) = open_temp_store().await;
        store
            .create_section("Experience", EntryType::Experience)
            .await
            .unwrap();

        let second = store
            .create_section("Experience", EntryType::Normal)
            .await
            .unwrap_err();

        assert_eq!(
            section_error(second),
            SectionError::DuplicateName {
                name: "Experience".into()
            }
        );
    }

    #[tokio::test]
    async fn a_blank_name_is_rejected() {
        let (_dir, store) = open_temp_store().await;

        let created = store
            .create_section("   ", EntryType::Normal)
            .await
            .unwrap_err();

        assert_eq!(section_error(created), SectionError::BlankName);
    }

    #[tokio::test]
    async fn a_name_is_stored_trimmed() {
        let (_dir, store) = open_temp_store().await;

        store
            .create_section("  Open Source  ", EntryType::Normal)
            .await
            .unwrap();

        assert!(store.section("Open Source").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn renaming_moves_the_section() {
        let (_dir, store) = open_temp_store().await;
        store
            .create_section("Projects", EntryType::Normal)
            .await
            .unwrap();

        let updated = store
            .update_section("Projects", Some("Selected Projects"), None)
            .await
            .unwrap();

        assert_eq!(updated.name, "Selected Projects");
        assert_eq!(updated.entry_type, EntryType::Normal);
        assert_eq!(store.section("Projects").await.unwrap(), None);
    }

    #[tokio::test]
    async fn retyping_keeps_the_name() {
        let (_dir, store) = open_temp_store().await;
        store
            .create_section("Writing", EntryType::Normal)
            .await
            .unwrap();

        let updated = store
            .update_section("Writing", None, Some(EntryType::Publication))
            .await
            .unwrap();

        assert_eq!(updated.name, "Writing");
        assert_eq!(updated.entry_type, EntryType::Publication);
    }

    #[tokio::test]
    async fn renaming_onto_an_existing_name_is_rejected() {
        let (_dir, store) = open_temp_store().await;
        store
            .create_section("Projects", EntryType::Normal)
            .await
            .unwrap();
        store
            .create_section("Experience", EntryType::Experience)
            .await
            .unwrap();

        let clash = store
            .update_section("Projects", Some("Experience"), None)
            .await
            .unwrap_err();

        assert_eq!(
            section_error(clash),
            SectionError::DuplicateName {
                name: "Experience".into()
            }
        );
    }

    #[tokio::test]
    async fn renaming_a_section_to_itself_is_allowed() {
        let (_dir, store) = open_temp_store().await;
        store
            .create_section("Projects", EntryType::Normal)
            .await
            .unwrap();

        let updated = store
            .update_section("Projects", Some("Projects"), None)
            .await
            .unwrap();

        assert_eq!(updated.name, "Projects");
    }

    #[tokio::test]
    async fn updating_an_absent_section_is_rejected() {
        let (_dir, store) = open_temp_store().await;

        let missing = store
            .update_section("Nowhere", None, Some(EntryType::Normal))
            .await
            .unwrap_err();

        assert_eq!(
            section_error(missing),
            SectionError::NoSuchSection {
                name: "Nowhere".into()
            }
        );
    }

    #[tokio::test]
    async fn a_deleted_section_is_gone() {
        let (_dir, store) = open_temp_store().await;
        store
            .create_section("Projects", EntryType::Normal)
            .await
            .unwrap();

        store.delete_section("Projects").await.unwrap();

        assert_eq!(store.section("Projects").await.unwrap(), None);
    }

    #[tokio::test]
    async fn deleting_an_absent_section_is_rejected() {
        let (_dir, store) = open_temp_store().await;

        let missing = store.delete_section("Nowhere").await.unwrap_err();

        assert_eq!(
            section_error(missing),
            SectionError::NoSuchSection {
                name: "Nowhere".into()
            }
        );
    }
}
