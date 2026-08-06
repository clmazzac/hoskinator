//! Bullets: one accomplishment inside an Entry, and the Variants wording it.

use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::entry::EntryError;
use crate::section::EntryType;
use crate::store::schema::{bullet as bullet_table, variant as variant_table};
use crate::store::{Store, StoreError};

/// A bullet or variant was rejected before it reached the store.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BulletError {
    #[error("an entry of type {entry_type} does not carry bullets")]
    EntryCarriesNoBullets { entry_type: EntryType },

    #[error("no bullet with id {id}")]
    NoSuchBullet { id: i64 },

    #[error("no variant with id {id}")]
    NoSuchVariant { id: i64 },

    #[error("a variant's wording cannot be blank")]
    BlankText,

    #[error("bullet {bullet_id} would be left with no wording")]
    LastVariant { bullet_id: i64 },
}

/// One wording of an accomplishment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name = variant_table)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Variant {
    pub id: i64,
    pub bullet_id: i64,
    pub text: String,
    pub note: Option<String>,
    pub is_default: bool,
}

/// One accomplishment inside an Entry, with every wording it has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bullet {
    pub id: i64,
    pub entry_id: i64,
    pub position: i32,
    pub variants: Vec<Variant>,
}

/// A stored bullet, before its variants are attached.
#[derive(Queryable, Selectable)]
#[diesel(table_name = bullet_table)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct BulletRow {
    id: i64,
    entry_id: i64,
    position: i32,
}

#[derive(Insertable)]
#[diesel(table_name = bullet_table)]
struct NewBullet {
    entry_id: i64,
    position: i32,
}

#[derive(Insertable)]
#[diesel(table_name = variant_table)]
struct NewVariant {
    bullet_id: i64,
    text: String,
    note: Option<String>,
    is_default: bool,
}

impl Store {
    /// Creates a bullet at the end of its entry, worded by one default variant.
    pub async fn create_bullet(
        &self,
        entry_id: i64,
        text: &str,
        note: Option<&str>,
    ) -> Result<Bullet, StoreError> {
        let entry = self
            .entry(entry_id)
            .await?
            .ok_or(EntryError::NoSuchEntry { id: entry_id })?;

        if !entry.entry_type.carries_bullets() {
            return Err(BulletError::EntryCarriesNoBullets {
                entry_type: entry.entry_type,
            }
            .into());
        }

        let text = clean_text(text)?;
        let note = clean_note(note);

        self.with_connection(move |connection| {
            connection
                .transaction(|connection| {
                    let last: Option<i32> = bullet_table::table
                        .filter(bullet_table::entry_id.eq(entry_id))
                        .select(diesel::dsl::max(bullet_table::position))
                        .first(connection)?;

                    let row: BulletRow = diesel::insert_into(bullet_table::table)
                        .values(NewBullet {
                            entry_id,
                            position: last.map_or(0, |last| last + 1),
                        })
                        .returning(BulletRow::as_returning())
                        .get_result(connection)?;

                    let variant = diesel::insert_into(variant_table::table)
                        .values(NewVariant {
                            bullet_id: row.id,
                            text,
                            note,
                            is_default: true,
                        })
                        .returning(Variant::as_returning())
                        .get_result(connection)?;

                    Ok(Bullet {
                        id: row.id,
                        entry_id: row.entry_id,
                        position: row.position,
                        variants: vec![variant],
                    })
                })
                .map_err(StoreError::WriteBullet)
        })
        .await
    }

    /// Every bullet of an entry, in order, each with its variants.
    pub async fn bullets(&self, entry_id: i64) -> Result<Vec<Bullet>, StoreError> {
        self.with_connection(move |connection| {
            let rows = bullet_table::table
                .filter(bullet_table::entry_id.eq(entry_id))
                .order(bullet_table::position)
                .select(BulletRow::as_select())
                .load(connection)
                .map_err(StoreError::ReadBullet)?;

            attach_variants(connection, rows)
        })
        .await
    }

    /// Reads one bullet by id.
    pub async fn bullet(&self, id: i64) -> Result<Option<Bullet>, StoreError> {
        self.with_connection(move |connection| {
            let row = bullet_table::table
                .find(id)
                .select(BulletRow::as_select())
                .first(connection)
                .optional()
                .map_err(StoreError::ReadBullet)?;

            match row {
                Some(row) => Ok(attach_variants(connection, vec![row])?.pop()),
                None => Ok(None),
            }
        })
        .await
    }

    /// Moves a bullet to `position` within its entry and renumbers the rest.
    pub async fn move_bullet(&self, id: i64, position: i32) -> Result<Vec<Bullet>, StoreError> {
        let entry_id = self
            .with_connection(move |connection| {
                bullet_table::table
                    .find(id)
                    .select(bullet_table::entry_id)
                    .first::<i64>(connection)
                    .optional()
                    .map_err(StoreError::ReadBullet)
            })
            .await?
            .ok_or(BulletError::NoSuchBullet { id })?;

        self.with_connection(move |connection| {
            connection
                .transaction(|connection| {
                    let mut order: Vec<i64> = bullet_table::table
                        .filter(bullet_table::entry_id.eq(entry_id))
                        .order(bullet_table::position)
                        .select(bullet_table::id)
                        .load(connection)?;

                    let from = order.iter().position(|held| *held == id).unwrap_or(0);
                    order.remove(from);
                    let to = (position.max(0) as usize).min(order.len());
                    order.insert(to, id);

                    for (position, id) in order.into_iter().enumerate() {
                        diesel::update(bullet_table::table.find(id))
                            .set(bullet_table::position.eq(position as i32))
                            .execute(connection)?;
                    }

                    Ok(())
                })
                .map_err(StoreError::WriteBullet)
        })
        .await?;

        self.bullets(entry_id).await
    }

    /// Deletes a bullet and every variant of it.
    pub async fn delete_bullet(&self, id: i64) -> Result<(), StoreError> {
        let deleted = self
            .with_connection(move |connection| {
                diesel::delete(bullet_table::table.find(id))
                    .execute(connection)
                    .map_err(StoreError::WriteBullet)
            })
            .await?;

        if deleted == 0 {
            return Err(BulletError::NoSuchBullet { id }.into());
        }

        Ok(())
    }

    /// Adds another wording to a bullet, which does not become the default.
    pub async fn add_variant(
        &self,
        bullet_id: i64,
        text: &str,
        note: Option<&str>,
    ) -> Result<Variant, StoreError> {
        let text = clean_text(text)?;
        let note = clean_note(note);

        self.with_connection(move |connection| {
            if !bullet_exists(connection, bullet_id)? {
                return Err(BulletError::NoSuchBullet { id: bullet_id }.into());
            }

            diesel::insert_into(variant_table::table)
                .values(NewVariant {
                    bullet_id,
                    text,
                    note,
                    is_default: false,
                })
                .returning(Variant::as_returning())
                .get_result(connection)
                .map_err(StoreError::WriteVariant)
        })
        .await
    }

    /// Rewords a variant, renotes it, or both. An empty note clears it.
    pub async fn update_variant(
        &self,
        id: i64,
        text: Option<&str>,
        note: Option<&str>,
    ) -> Result<Variant, StoreError> {
        let text = text.map(clean_text).transpose()?;
        let note = note.map(|note| clean_note(Some(note)));

        self.with_connection(move |connection| {
            let current = read_variant(connection, id)?;

            diesel::update(variant_table::table.find(id))
                .set((
                    variant_table::text.eq(text.unwrap_or(current.text)),
                    variant_table::note.eq(note.unwrap_or(current.note)),
                ))
                .returning(Variant::as_returning())
                .get_result(connection)
                .map_err(StoreError::WriteVariant)
        })
        .await
    }

    /// Makes a variant the default wording of its bullet.
    pub async fn set_default_variant(&self, id: i64) -> Result<Variant, StoreError> {
        self.with_connection(move |connection| {
            let current = read_variant(connection, id)?;

            connection
                .transaction(|connection| {
                    clear_default(connection, current.bullet_id)?;

                    diesel::update(variant_table::table.find(id))
                        .set(variant_table::is_default.eq(true))
                        .returning(Variant::as_returning())
                        .get_result(connection)
                })
                .map_err(StoreError::WriteVariant)
        })
        .await
    }

    /// Deletes a variant, unless it is the only wording its bullet has.
    ///
    /// Deleting the default promotes the lowest remaining variant id.
    pub async fn delete_variant(&self, id: i64) -> Result<(), StoreError> {
        self.with_connection(move |connection| {
            let current = read_variant(connection, id)?;

            let siblings: Vec<i64> = variant_table::table
                .filter(variant_table::bullet_id.eq(current.bullet_id))
                .order(variant_table::id)
                .select(variant_table::id)
                .load(connection)
                .map_err(StoreError::ReadVariant)?;

            if siblings.len() == 1 {
                return Err(BulletError::LastVariant {
                    bullet_id: current.bullet_id,
                }
                .into());
            }

            connection
                .transaction(|connection| {
                    diesel::delete(variant_table::table.find(id)).execute(connection)?;

                    if current.is_default {
                        let promoted = siblings
                            .into_iter()
                            .find(|sibling| *sibling != id)
                            .expect("a sibling remains");

                        diesel::update(variant_table::table.find(promoted))
                            .set(variant_table::is_default.eq(true))
                            .execute(connection)?;
                    }

                    Ok(())
                })
                .map_err(StoreError::WriteVariant)
        })
        .await
    }
}

/// Loads the variants of each bullet, ordered by id with the default first.
fn attach_variants(
    connection: &mut SqliteConnection,
    rows: Vec<BulletRow>,
) -> Result<Vec<Bullet>, StoreError> {
    let ids: Vec<i64> = rows.iter().map(|row| row.id).collect();

    let variants = variant_table::table
        .filter(variant_table::bullet_id.eq_any(&ids))
        .order((
            variant_table::bullet_id,
            variant_table::is_default.desc(),
            variant_table::id,
        ))
        .select(Variant::as_select())
        .load::<Variant>(connection)
        .map_err(StoreError::ReadVariant)?;

    Ok(rows
        .into_iter()
        .map(|row| Bullet {
            variants: variants
                .iter()
                .filter(|variant| variant.bullet_id == row.id)
                .cloned()
                .collect(),
            id: row.id,
            entry_id: row.entry_id,
            position: row.position,
        })
        .collect())
}

fn bullet_exists(connection: &mut SqliteConnection, id: i64) -> Result<bool, StoreError> {
    bullet_table::table
        .find(id)
        .select(bullet_table::id)
        .first::<i64>(connection)
        .optional()
        .map(|found| found.is_some())
        .map_err(StoreError::ReadBullet)
}

fn read_variant(connection: &mut SqliteConnection, id: i64) -> Result<Variant, StoreError> {
    variant_table::table
        .find(id)
        .select(Variant::as_select())
        .first(connection)
        .optional()
        .map_err(StoreError::ReadVariant)?
        .ok_or_else(|| BulletError::NoSuchVariant { id }.into())
}

fn clear_default(
    connection: &mut SqliteConnection,
    bullet_id: i64,
) -> Result<(), diesel::result::Error> {
    diesel::update(variant_table::table.filter(variant_table::bullet_id.eq(bullet_id)))
        .set(variant_table::is_default.eq(false))
        .execute(connection)?;

    Ok(())
}

/// Trims a wording and rejects it if nothing is left.
fn clean_text(text: &str) -> Result<String, BulletError> {
    let text = text.trim();

    if text.is_empty() {
        return Err(BulletError::BlankText);
    }

    Ok(text.to_string())
}

/// Trims a note, treating an empty one as no note.
fn clean_note(note: Option<&str>) -> Option<String> {
    note.map(str::trim)
        .filter(|note| !note.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::entry::EntryFields;
    use serde_json::json;
    use tempfile::TempDir;

    async fn open_temp_store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("store").join("hoskinator.db"))
            .await
            .expect("opening the store");
        (dir, store)
    }

    /// An entry of a type that carries bullets.
    async fn entry_with_bullets(store: &Store) -> i64 {
        let fields = EntryFields::parse(
            EntryType::Experience,
            json!({ "company": "Acme", "position": "Engineer" }),
        )
        .unwrap();

        store.create_entry(&fields).await.unwrap().id
    }

    fn bullet_error(error: StoreError) -> BulletError {
        match error {
            StoreError::Bullet(error) => error,
            other => panic!("expected a BulletError, got {other:?}"),
        }
    }

    fn default_of(bullet: &Bullet) -> &Variant {
        bullet
            .variants
            .iter()
            .find(|variant| variant.is_default)
            .expect("a default variant")
    }

    #[tokio::test]
    async fn a_created_bullet_owns_its_wording_as_the_default() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;

        let bullet = store
            .create_bullet(entry_id, "Cut latency in half.", Some("from the H1 review"))
            .await
            .unwrap();

        assert_eq!(bullet.entry_id, entry_id);
        assert_eq!(bullet.position, 0);
        assert_eq!(bullet.variants.len(), 1);
        assert_eq!(default_of(&bullet).text, "Cut latency in half.");
        assert_eq!(
            default_of(&bullet).note.as_deref(),
            Some("from the H1 review")
        );
        assert_eq!(store.bullet(bullet.id).await.unwrap(), Some(bullet));
    }

    #[tokio::test]
    async fn a_bullet_on_an_entry_type_without_highlights_is_rejected() {
        let (_dir, store) = open_temp_store().await;
        let fields =
            EntryFields::parse(EntryType::Bullet, json!({ "bullet": "Shipped it." })).unwrap();
        let entry_id = store.create_entry(&fields).await.unwrap().id;

        let rejected = store
            .create_bullet(entry_id, "Cut latency in half.", None)
            .await
            .unwrap_err();

        assert_eq!(
            bullet_error(rejected),
            BulletError::EntryCarriesNoBullets {
                entry_type: EntryType::Bullet
            }
        );
    }

    #[tokio::test]
    async fn a_bullet_on_an_absent_entry_is_rejected() {
        let (_dir, store) = open_temp_store().await;

        let rejected = store
            .create_bullet(404, "Shipped it.", None)
            .await
            .unwrap_err();

        assert!(matches!(
            rejected,
            StoreError::Entry(crate::entry::EntryError::NoSuchEntry { id: 404 })
        ));
    }

    #[tokio::test]
    async fn blank_wording_is_rejected() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;

        let rejected = store
            .create_bullet(entry_id, "   ", None)
            .await
            .unwrap_err();

        assert_eq!(bullet_error(rejected), BulletError::BlankText);
    }

    #[tokio::test]
    async fn an_empty_note_is_no_note() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;

        let bullet = store
            .create_bullet(entry_id, "Shipped it.", Some("  "))
            .await
            .unwrap();

        assert_eq!(default_of(&bullet).note, None);
    }

    #[tokio::test]
    async fn bullets_come_back_in_position_order() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;
        let first = store.create_bullet(entry_id, "One.", None).await.unwrap();
        let second = store.create_bullet(entry_id, "Two.", None).await.unwrap();

        assert_eq!(second.position, 1);
        assert_eq!(
            store.bullets(entry_id).await.unwrap(),
            vec![first.clone(), second.clone()]
        );

        let moved = store.move_bullet(second.id, 0).await.unwrap();

        assert_eq!(
            moved.iter().map(|bullet| bullet.id).collect::<Vec<_>>(),
            vec![second.id, first.id]
        );
        assert_eq!(
            moved
                .iter()
                .map(|bullet| bullet.position)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[tokio::test]
    async fn moving_past_the_end_lands_last() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;
        let first = store.create_bullet(entry_id, "One.", None).await.unwrap();
        store.create_bullet(entry_id, "Two.", None).await.unwrap();

        let moved = store.move_bullet(first.id, 99).await.unwrap();

        assert_eq!(moved.last().unwrap().id, first.id);
    }

    #[tokio::test]
    async fn a_second_variant_does_not_take_the_default() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;
        let bullet = store
            .create_bullet(entry_id, "Cut latency in half.", None)
            .await
            .unwrap();

        let added = store
            .add_variant(bullet.id, "Halved p99 latency.", Some("for the SRE role"))
            .await
            .unwrap();

        assert!(!added.is_default);
        let reread = store.bullet(bullet.id).await.unwrap().unwrap();
        assert_eq!(reread.variants.len(), 2);
        assert_eq!(default_of(&reread).text, "Cut latency in half.");
    }

    #[tokio::test]
    async fn the_default_moves_to_whichever_variant_is_set() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;
        let bullet = store.create_bullet(entry_id, "One.", None).await.unwrap();
        let added = store.add_variant(bullet.id, "Two.", None).await.unwrap();

        let promoted = store.set_default_variant(added.id).await.unwrap();

        assert!(promoted.is_default);
        let reread = store.bullet(bullet.id).await.unwrap().unwrap();
        assert_eq!(default_of(&reread).id, added.id);
        assert_eq!(
            reread.variants.iter().filter(|v| v.is_default).count(),
            1,
            "exactly one default"
        );
    }

    #[tokio::test]
    async fn the_default_variant_is_listed_first() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;
        let bullet = store.create_bullet(entry_id, "One.", None).await.unwrap();
        let added = store.add_variant(bullet.id, "Two.", None).await.unwrap();
        store.set_default_variant(added.id).await.unwrap();

        let reread = store.bullet(bullet.id).await.unwrap().unwrap();

        assert_eq!(reread.variants.first().unwrap().id, added.id);
    }

    #[tokio::test]
    async fn a_reworded_variant_keeps_its_note_and_the_other_way_round() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;
        let bullet = store
            .create_bullet(entry_id, "One.", Some("a note"))
            .await
            .unwrap();
        let id = default_of(&bullet).id;

        let reworded = store.update_variant(id, Some("Two."), None).await.unwrap();

        assert_eq!(reworded.text, "Two.");
        assert_eq!(reworded.note.as_deref(), Some("a note"));

        let renoted = store
            .update_variant(id, None, Some("another"))
            .await
            .unwrap();

        assert_eq!(renoted.text, "Two.");
        assert_eq!(renoted.note.as_deref(), Some("another"));
    }

    #[tokio::test]
    async fn an_empty_note_clears_it() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;
        let bullet = store
            .create_bullet(entry_id, "One.", Some("a note"))
            .await
            .unwrap();

        let cleared = store
            .update_variant(default_of(&bullet).id, None, Some(""))
            .await
            .unwrap();

        assert_eq!(cleared.note, None);
    }

    #[tokio::test]
    async fn deleting_the_default_promotes_the_lowest_remaining_id() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;
        let bullet = store.create_bullet(entry_id, "One.", None).await.unwrap();
        let second = store.add_variant(bullet.id, "Two.", None).await.unwrap();
        let third = store.add_variant(bullet.id, "Three.", None).await.unwrap();

        store.delete_variant(default_of(&bullet).id).await.unwrap();

        let reread = store.bullet(bullet.id).await.unwrap().unwrap();
        assert_eq!(reread.variants.len(), 2);
        assert_eq!(default_of(&reread).id, second.id.min(third.id));
    }

    #[tokio::test]
    async fn deleting_the_last_variant_is_rejected() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;
        let bullet = store.create_bullet(entry_id, "One.", None).await.unwrap();

        let rejected = store
            .delete_variant(default_of(&bullet).id)
            .await
            .unwrap_err();

        assert_eq!(
            bullet_error(rejected),
            BulletError::LastVariant {
                bullet_id: bullet.id
            }
        );
        assert_eq!(
            store
                .bullet(bullet.id)
                .await
                .unwrap()
                .unwrap()
                .variants
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn deleting_a_bullet_takes_its_variants_with_it() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;
        let bullet = store.create_bullet(entry_id, "One.", None).await.unwrap();
        let added = store.add_variant(bullet.id, "Two.", None).await.unwrap();

        store.delete_bullet(bullet.id).await.unwrap();

        assert_eq!(store.bullet(bullet.id).await.unwrap(), None);
        assert_eq!(
            bullet_error(store.delete_variant(added.id).await.unwrap_err()),
            BulletError::NoSuchVariant { id: added.id }
        );
    }

    #[tokio::test]
    async fn deleting_an_entry_takes_its_bullets_with_it() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;
        let bullet = store.create_bullet(entry_id, "One.", None).await.unwrap();

        store.delete_entry(entry_id).await.unwrap();

        assert_eq!(store.bullet(bullet.id).await.unwrap(), None);
        assert!(store.bullets(entry_id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn absent_bullets_and_variants_are_rejected() {
        let (_dir, store) = open_temp_store().await;

        assert_eq!(store.bullet(404).await.unwrap(), None);
        assert_eq!(
            bullet_error(store.delete_bullet(404).await.unwrap_err()),
            BulletError::NoSuchBullet { id: 404 }
        );
        assert_eq!(
            bullet_error(store.move_bullet(404, 0).await.unwrap_err()),
            BulletError::NoSuchBullet { id: 404 }
        );
        assert_eq!(
            bullet_error(store.add_variant(404, "One.", None).await.unwrap_err()),
            BulletError::NoSuchBullet { id: 404 }
        );
        assert_eq!(
            bullet_error(
                store
                    .update_variant(404, Some("One."), None)
                    .await
                    .unwrap_err()
            ),
            BulletError::NoSuchVariant { id: 404 }
        );
        assert_eq!(
            bullet_error(store.set_default_variant(404).await.unwrap_err()),
            BulletError::NoSuchVariant { id: 404 }
        );
    }

    #[tokio::test]
    async fn sqlite_refuses_a_second_default_for_one_bullet() {
        let (_dir, store) = open_temp_store().await;
        let entry_id = entry_with_bullets(&store).await;
        let bullet = store.create_bullet(entry_id, "One.", None).await.unwrap();
        let added = store.add_variant(bullet.id, "Two.", None).await.unwrap();

        let rejected = store
            .with_connection(move |connection| {
                diesel::update(variant_table::table.find(added.id))
                    .set(variant_table::is_default.eq(true))
                    .execute(connection)
            })
            .await;

        assert!(
            rejected.is_err(),
            "the partial unique index should reject it"
        );
    }

    #[tokio::test]
    async fn a_bullet_survives_reopening_the_store() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");
        let store = Store::open(&path).await.unwrap();
        let entry_id = entry_with_bullets(&store).await;
        let bullet = store
            .create_bullet(entry_id, "Cut latency in half.", None)
            .await
            .unwrap();
        drop(store);

        let reopened = Store::open(&path).await.unwrap();

        assert_eq!(reopened.bullet(bullet.id).await.unwrap(), Some(bullet));
    }
}
