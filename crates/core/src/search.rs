//! Full-text search across the Master Store.

use diesel::prelude::*;
use diesel::sql_types::{Double, Text};
use serde::{Deserialize, Serialize};

use crate::bullet::Variant;
use crate::entry::Entry;
use crate::store::schema::{
    bullet as bullet_table, entry as entry_table, variant as variant_table,
};
use crate::store::{Store, StoreError};

/// One thing a query matched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SearchHit {
    /// An entry matched on its own fields.
    Entry { entry: Entry, rank: f64 },

    /// A bullet matched on one of its wordings.
    Bullet {
        entry: Entry,
        bullet_id: i64,
        matched_variant: Variant,
        other_variants: usize,
        rank: f64,
    },
}

impl SearchHit {
    /// How well the hit matched. Lower is better, as FTS5 ranks it.
    pub fn rank(&self) -> f64 {
        match self {
            SearchHit::Entry { rank, .. } | SearchHit::Bullet { rank, .. } => *rank,
        }
    }
}

/// One row of the index, as the query reads it back.
#[derive(QueryableByName)]
struct Match {
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    r#ref: i64,
    #[diesel(sql_type = Double)]
    rank: f64,
}

impl Store {
    /// Searches Entries and Variants, best match first.
    ///
    /// Variant matches roll up to their Bullet, keeping the best-ranked wording.
    pub async fn search(&self, query: &str) -> Result<Vec<SearchHit>, StoreError> {
        let query = query.trim().to_owned();

        if query.is_empty() {
            return Ok(Vec::new());
        }

        let matches: Vec<Match> = self
            .with_connection(move |connection| {
                // FTS5 is a virtual table Diesel's schema cannot describe, so the query that
                // reaches it is written out and decoded by column name.
                diesel::sql_query(
                    "SELECT kind, ref, rank FROM search_fts \
                     WHERE search_fts MATCH ? ORDER BY rank",
                )
                .bind::<Text, _>(query)
                .load(connection)
                .map_err(StoreError::Search)
            })
            .await?;

        self.hits(matches).await
    }

    /// Turns index rows into hits, rolling variants up to their bullets.
    async fn hits(&self, matches: Vec<Match>) -> Result<Vec<SearchHit>, StoreError> {
        let mut hits = Vec::new();
        let mut seen_bullets = Vec::new();

        for found in matches {
            match found.kind.as_str() {
                "entry" => {
                    if let Some(entry) = self.entry(found.r#ref).await? {
                        hits.push(SearchHit::Entry {
                            entry,
                            rank: found.rank,
                        });
                    }
                }
                "variant" => {
                    let Some((variant, bullet_id, entry_id, siblings)) =
                        self.matched_variant(found.r#ref).await?
                    else {
                        continue;
                    };

                    if seen_bullets.contains(&bullet_id) {
                        continue;
                    }

                    if let Some(entry) = self.entry(entry_id).await? {
                        seen_bullets.push(bullet_id);
                        hits.push(SearchHit::Bullet {
                            entry,
                            bullet_id,
                            matched_variant: variant,
                            other_variants: siblings,
                            rank: found.rank,
                        });
                    }
                }
                _ => continue,
            }
        }

        Ok(hits)
    }

    /// The variant, the bullet and entry it belongs to, and how many other wordings there are.
    async fn matched_variant(
        &self,
        id: i64,
    ) -> Result<Option<(Variant, i64, i64, usize)>, StoreError> {
        self.with_connection(move |connection| {
            let variant = variant_table::table
                .find(id)
                .select(Variant::as_select())
                .first::<Variant>(connection)
                .optional()
                .map_err(StoreError::ReadVariant)?;

            let Some(variant) = variant else {
                return Ok(None);
            };

            let entry_id = bullet_table::table
                .find(variant.bullet_id)
                .select(bullet_table::entry_id)
                .first::<i64>(connection)
                .map_err(StoreError::ReadBullet)?;

            let siblings = variant_table::table
                .filter(variant_table::bullet_id.eq(variant.bullet_id))
                .count()
                .get_result::<i64>(connection)
                .map_err(StoreError::ReadVariant)?;

            let bullet_id = variant.bullet_id;

            Ok(Some((
                variant,
                bullet_id,
                entry_id,
                siblings.saturating_sub(1) as usize,
            )))
        })
        .await
    }
}

/// Computes `search_text` for every entry left without one.
pub(crate) fn backfill_search_text(connection: &mut SqliteConnection) -> Result<(), StoreError> {
    let rows: Vec<(i64, String, String)> = entry_table::table
        .filter(entry_table::search_text.eq(""))
        .select((
            entry_table::id,
            entry_table::entry_type,
            entry_table::fields,
        ))
        .load(connection)
        .map_err(StoreError::ReadEntry)?;

    for (id, entry_type, fields) in rows {
        let entry_type = entry_type.parse()?;
        let value = serde_json::from_str(&fields).map_err(|source| StoreError::DecodeEntry {
            id,
            source: crate::entry::EntryError::Fields { entry_type, source },
        })?;
        let fields = crate::entry::EntryFields::parse(entry_type, value)
            .map_err(|source| StoreError::DecodeEntry { id, source })?;

        diesel::update(entry_table::table.find(id))
            .set(entry_table::search_text.eq(fields.search_text()))
            .execute(connection)
            .map_err(StoreError::WriteEntry)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::entry::EntryFields;
    use crate::section::EntryType;
    use serde_json::json;
    use tempfile::TempDir;

    async fn open_temp_store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("store").join("hoskinator.db"))
            .await
            .expect("opening the store");
        (dir, store)
    }

    async fn experience(store: &Store) -> Entry {
        let fields = EntryFields::parse(
            EntryType::Experience,
            json!({
                "company": "Acme",
                "position": "Platform Engineer",
                "start_date": "2021-06",
                "summary": "Ran the Kubernetes migration.",
            }),
        )
        .unwrap();

        store.create_entry(&fields).await.unwrap()
    }

    fn bullet_hit(hit: &SearchHit) -> (i64, &Variant, usize) {
        match hit {
            SearchHit::Bullet {
                bullet_id,
                matched_variant,
                other_variants,
                ..
            } => (*bullet_id, matched_variant, *other_variants),
            other => panic!("expected a bullet hit, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_entry_is_found_by_a_word_in_its_fields() {
        let (_dir, store) = open_temp_store().await;
        let entry = experience(&store).await;

        let hits = store.search("Kubernetes").await.unwrap();

        assert_eq!(hits.len(), 1);
        assert!(matches!(&hits[0], SearchHit::Entry { entry: found, .. } if found.id == entry.id));
    }

    #[tokio::test]
    async fn a_variant_wording_is_found_and_answers_with_its_bullet() {
        let (_dir, store) = open_temp_store().await;
        let entry = experience(&store).await;
        let bullet = store
            .create_bullet(entry.id, "Cut p99 latency in half.", None)
            .await
            .unwrap();

        let hits = store.search("latency").await.unwrap();

        assert_eq!(hits.len(), 1);
        let (bullet_id, variant, others) = bullet_hit(&hits[0]);
        assert_eq!(bullet_id, bullet.id);
        assert_eq!(variant.text, "Cut p99 latency in half.");
        assert_eq!(others, 0);
    }

    #[tokio::test]
    async fn every_wording_is_searched_not_only_the_default() {
        let (_dir, store) = open_temp_store().await;
        let entry = experience(&store).await;
        let bullet = store
            .create_bullet(entry.id, "Cut p99 latency in half.", None)
            .await
            .unwrap();
        let added = store
            .add_variant(
                bullet.id,
                "Halved tail latency for the LLVM toolchain.",
                None,
            )
            .await
            .unwrap();

        let hits = store.search("LLVM").await.unwrap();

        let (_, variant, _) = bullet_hit(&hits[0]);
        assert_eq!(variant.id, added.id);
        assert!(!variant.is_default, "a non-default wording still matches");
    }

    #[tokio::test]
    async fn wordings_of_one_bullet_roll_up_to_one_hit() {
        let (_dir, store) = open_temp_store().await;
        let entry = experience(&store).await;
        let bullet = store
            .create_bullet(entry.id, "Cut latency in half.", None)
            .await
            .unwrap();
        store
            .add_variant(bullet.id, "Halved latency across the fleet.", None)
            .await
            .unwrap();
        store
            .add_variant(bullet.id, "Drove latency down by 50%.", None)
            .await
            .unwrap();

        let hits = store.search("latency").await.unwrap();

        assert_eq!(hits.len(), 1, "three wordings, one accomplishment");
        let (_, _, others) = bullet_hit(&hits[0]);
        assert_eq!(others, 2);
    }

    #[tokio::test]
    async fn separate_bullets_stay_separate_hits() {
        let (_dir, store) = open_temp_store().await;
        let entry = experience(&store).await;
        store
            .create_bullet(entry.id, "Cut latency in half.", None)
            .await
            .unwrap();
        store
            .create_bullet(entry.id, "Halved latency in the ingest path.", None)
            .await
            .unwrap();

        let hits = store.search("latency").await.unwrap();

        assert_eq!(hits.len(), 2);
    }

    #[tokio::test]
    async fn hits_come_back_best_first() {
        let (_dir, store) = open_temp_store().await;
        let entry = experience(&store).await;
        store
            .create_bullet(entry.id, "Latency, latency, latency.", None)
            .await
            .unwrap();
        store
            .create_bullet(entry.id, "Wrote a note about latency once.", None)
            .await
            .unwrap();

        let hits = store.search("latency").await.unwrap();

        assert!(
            hits[0].rank() <= hits[1].rank(),
            "ranks out of order: {:?}",
            hits.iter().map(SearchHit::rank).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn rewording_a_variant_changes_what_matches() {
        let (_dir, store) = open_temp_store().await;
        let entry = experience(&store).await;
        let bullet = store
            .create_bullet(entry.id, "Cut latency in half.", None)
            .await
            .unwrap();
        let variant = bullet.variants[0].id;

        store
            .update_variant(variant, Some("Rewrote the scheduler."), None)
            .await
            .unwrap();

        assert!(store.search("latency").await.unwrap().is_empty());
        assert_eq!(store.search("scheduler").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn deleting_a_variant_removes_it_from_results() {
        let (_dir, store) = open_temp_store().await;
        let entry = experience(&store).await;
        let bullet = store
            .create_bullet(entry.id, "Cut latency in half.", None)
            .await
            .unwrap();
        let added = store
            .add_variant(bullet.id, "Rewrote the scheduler.", None)
            .await
            .unwrap();

        store.delete_variant(added.id).await.unwrap();

        assert!(store.search("scheduler").await.unwrap().is_empty());
        assert_eq!(store.search("latency").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn deleting_an_entry_removes_its_bullets_from_results() {
        let (_dir, store) = open_temp_store().await;
        let entry = experience(&store).await;
        store
            .create_bullet(entry.id, "Cut latency in half.", None)
            .await
            .unwrap();

        store.delete_entry(entry.id).await.unwrap();

        assert!(store.search("latency").await.unwrap().is_empty());
        assert!(store.search("Kubernetes").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn updating_an_entry_changes_what_matches() {
        let (_dir, store) = open_temp_store().await;
        let entry = experience(&store).await;

        store
            .update_entry(
                entry.id,
                json!({ "company": "Globex", "position": "Compiler Engineer" }),
            )
            .await
            .unwrap();

        assert!(store.search("Kubernetes").await.unwrap().is_empty());
        assert_eq!(store.search("Globex").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn field_names_and_dates_are_not_searchable() {
        let (_dir, store) = open_temp_store().await;
        experience(&store).await;

        assert!(store.search("summary").await.unwrap().is_empty());
        assert!(store.search("company").await.unwrap().is_empty());
        assert!(store.search("2021").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_empty_query_matches_nothing() {
        let (_dir, store) = open_temp_store().await;
        experience(&store).await;

        assert!(store.search("   ").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_word_in_no_record_matches_nothing() {
        let (_dir, store) = open_temp_store().await;
        experience(&store).await;

        assert!(store.search("underwater").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn search_survives_reopening_the_store() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");
        let store = Store::open(&path).await.unwrap();
        let entry = experience(&store).await;
        store
            .create_bullet(entry.id, "Cut latency in half.", None)
            .await
            .unwrap();
        drop(store);

        let reopened = Store::open(&path).await.unwrap();

        assert_eq!(reopened.search("latency").await.unwrap().len(), 1);
        assert_eq!(reopened.search("Kubernetes").await.unwrap().len(), 1);
    }
}
