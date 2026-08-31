//! Syncing the Master Store to Turso, so the bank is available on more than one machine.
//!
//! Push and pull each move the *whole* store as one snapshot — every Section, Entry, Bullet,
//! and Variant, replacing whatever the other side already holds. There is no field-level merge:
//! the side that syncs last wins, the same rule `docs/decisions/google-sync.md` already applies
//! to Applications. Sync is never automatic; it runs only when `bank.push` or `bank.pull` is
//! called.

use serde::{Deserialize, Serialize};

use crate::entry::Entry;
use crate::section::Section;
use crate::store::{Store, StoreError};

/// The table holding the one row a snapshot lives in.
const SNAPSHOT_TABLE: &str = "hoskinator_bank_snapshot";

/// Pushing or pulling the bank against Turso failed.
#[derive(Debug, thiserror::Error)]
pub enum BankSyncError {
    #[error("no Turso database is configured")]
    Unconfigured,

    #[error("could not reach the Turso database")]
    Connect(#[source] libsql::Error),

    #[error("a query against the Turso database failed")]
    Query(#[source] libsql::Error),

    #[error("could not read the local store")]
    Store(#[from] StoreError),

    #[error("could not encode the snapshot")]
    Encode(#[source] serde_json::Error),

    #[error("the remote snapshot is not valid JSON")]
    Decode(#[source] serde_json::Error),
}

/// The whole Master Store, as one transferable unit.
#[derive(Debug, Serialize, Deserialize)]
struct BankSnapshot {
    sections: Vec<Section>,
    /// Every entry, each carrying its own bullets.
    entries: Vec<EntrySnapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EntrySnapshot {
    entry: Entry,
    bullets: Vec<crate::bullet::Bullet>,
}

/// Reads every Section, Entry, and Bullet the store holds into one snapshot.
async fn build_snapshot(store: &Store) -> Result<BankSnapshot, BankSyncError> {
    let sections = store.sections().await?;
    let entries = store.entries(None).await?;

    let mut with_bullets = Vec::with_capacity(entries.len());
    for entry in entries {
        let bullets = store.bullets(entry.id).await?;
        with_bullets.push(EntrySnapshot { entry, bullets });
    }

    Ok(BankSnapshot {
        sections,
        entries: with_bullets,
    })
}

/// Replaces every Section, Entry, Bullet, and Variant the store holds with `snapshot`'s.
async fn apply_snapshot(store: &Store, snapshot: &BankSnapshot) -> Result<(), BankSyncError> {
    for entry in store.entries(None).await? {
        store.delete_entry(entry.id).await?;
    }
    for section in store.sections().await? {
        store.delete_section(&section.name).await?;
    }

    for section in &snapshot.sections {
        store
            .create_section(&section.name, section.entry_type)
            .await?;
    }

    for EntrySnapshot { entry, bullets } in &snapshot.entries {
        let created = store.create_entry(&entry.fields).await?;
        if let Some(braindump) = &entry.braindump {
            store.set_braindump(created.id, Some(braindump)).await?;
        }

        let mut ordered = bullets.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|bullet| bullet.position);

        for bullet in ordered {
            let Some(default) = bullet.variants.iter().find(|variant| variant.is_default) else {
                continue;
            };
            let created_bullet = store
                .create_bullet(created.id, &default.text, default.note.as_deref())
                .await?;
            for variant in &bullet.variants {
                if variant.is_default {
                    continue;
                }
                store
                    .add_variant(created_bullet.id, &variant.text, variant.note.as_deref())
                    .await?;
            }
        }
    }

    Ok(())
}

/// Opens a connection to the configured Turso database and ensures the snapshot table exists.
async fn connect(url: &str, token: &str) -> Result<libsql::Connection, BankSyncError> {
    let database = libsql::Builder::new_remote(url.to_string(), token.to_string())
        .build()
        .await
        .map_err(BankSyncError::Connect)?;
    let connection = database.connect().map_err(BankSyncError::Connect)?;

    connection
        .execute(
            &format!(
                "CREATE TABLE IF NOT EXISTS {SNAPSHOT_TABLE} (\
                    id INTEGER PRIMARY KEY CHECK (id = 1), \
                    data TEXT NOT NULL\
                )"
            ),
            (),
        )
        .await
        .map_err(BankSyncError::Query)?;

    Ok(connection)
}

/// Uploads the whole local store to Turso, replacing whatever is already there.
pub async fn push(store: &Store, url: &str, token: &str) -> Result<(), BankSyncError> {
    let snapshot = build_snapshot(store).await?;
    let data = serde_json::to_string(&snapshot).map_err(BankSyncError::Encode)?;

    let connection = connect(url, token).await?;
    connection
        .execute(
            &format!(
                "INSERT INTO {SNAPSHOT_TABLE} (id, data) VALUES (1, ?1) \
                 ON CONFLICT (id) DO UPDATE SET data = excluded.data"
            ),
            [data],
        )
        .await
        .map_err(BankSyncError::Query)?;

    Ok(())
}

/// Downloads Turso's snapshot and replaces the local store with it. `false` if Turso has never
/// been pushed to.
pub async fn pull(store: &Store, url: &str, token: &str) -> Result<bool, BankSyncError> {
    let connection = connect(url, token).await?;
    let mut rows = connection
        .query(
            &format!("SELECT data FROM {SNAPSHOT_TABLE} WHERE id = 1"),
            (),
        )
        .await
        .map_err(BankSyncError::Query)?;

    let Some(row) = rows.next().await.map_err(BankSyncError::Query)? else {
        return Ok(false);
    };
    let data: String = row.get(0).map_err(BankSyncError::Query)?;
    let snapshot: BankSnapshot = serde_json::from_str(&data).map_err(BankSyncError::Decode)?;

    apply_snapshot(store, &snapshot).await?;
    Ok(true)
}

/// Writes or clears `turso_url`/`turso_auth_token` in the config file, keeping anything else
/// already set.
pub fn remember_credentials(
    config_path: &std::path::Path,
    url: Option<&str>,
    token: Option<&str>,
) -> Result<(), crate::config::ConfigError> {
    crate::config::remember_key(config_path, "turso_url", url).map_err(|source| {
        crate::config::ConfigError::Write {
            path: config_path.to_path_buf(),
            source,
        }
    })?;
    crate::config::remember_key(config_path, "turso_auth_token", token).map_err(|source| {
        crate::config::ConfigError::Write {
            path: config_path.to_path_buf(),
            source,
        }
    })
}

// `connect`/`push`/`pull` are not exercised here — see docs/decisions/store.md for why. Every
// other piece — building a snapshot, applying one back, and the JSON both travel as — is tested
// directly, against real `Store`s, with no `libsql` involved.
#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    async fn open_temp_store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("store.db")).await.unwrap();
        (dir, store)
    }

    /// Round-trips a store's snapshot through the exact JSON `push`/`pull` exchange, without
    /// `libsql` in the loop.
    async fn sync_via_json(from: &Store, to: &Store) -> bool {
        let snapshot = build_snapshot(from).await.unwrap();
        let data = serde_json::to_string(&snapshot).unwrap();

        let snapshot: BankSnapshot = serde_json::from_str(&data).unwrap();
        apply_snapshot(to, &snapshot).await.unwrap();
        true
    }

    #[tokio::test]
    async fn a_snapshot_of_an_empty_store_round_trips() {
        let (_dir, store) = open_temp_store().await;

        assert!(sync_via_json(&store, &store).await);

        assert_eq!(store.sections().await.unwrap(), vec![]);
        assert_eq!(store.entries(None).await.unwrap(), vec![]);
    }

    #[tokio::test]
    async fn sections_entries_bullets_and_variants_survive_a_round_trip() {
        use crate::entry::EntryFields;
        use crate::section::EntryType;

        let (_dir, store) = open_temp_store().await;

        store
            .create_section("Experience", EntryType::Experience)
            .await
            .unwrap();
        let entry = store
            .create_entry(&EntryFields::Experience(crate::entry::ExperienceFields {
                company: "Acme".into(),
                position: "Engineer".into(),
                location: None,
                date: None,
                start_date: None,
                end_date: None,
                summary: None,
            }))
            .await
            .unwrap();
        let bullet = store
            .create_bullet(entry.id, "Shipped the thing", None)
            .await
            .unwrap();
        store
            .add_variant(bullet.id, "Shipped the thing, punchier", None)
            .await
            .unwrap();

        // A second machine: a fresh, empty local store pulling for the first time.
        let (_dir2, store2) = open_temp_store().await;
        assert!(sync_via_json(&store, &store2).await);

        let sections = store2.sections().await.unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "Experience");

        let entries = store2.entries(None).await.unwrap();
        assert_eq!(entries.len(), 1);
        let EntryFields::Experience(fields) = &entries[0].fields else {
            panic!("expected an experience entry");
        };
        assert_eq!(fields.company, "Acme");

        let bullets = store2.bullets(entries[0].id).await.unwrap();
        assert_eq!(bullets.len(), 1);
        assert_eq!(bullets[0].variants.len(), 2);
        assert!(
            bullets[0]
                .variants
                .iter()
                .any(|v| v.is_default && v.text == "Shipped the thing")
        );
        assert!(
            bullets[0]
                .variants
                .iter()
                .any(|v| !v.is_default && v.text == "Shipped the thing, punchier")
        );
    }

    #[tokio::test]
    async fn pulling_replaces_whatever_the_local_store_already_held() {
        use crate::entry::EntryFields;
        use crate::section::EntryType;

        let (_dir, remote_side) = open_temp_store().await;
        remote_side
            .create_section("Skills", EntryType::OneLine)
            .await
            .unwrap();

        let (_dir2, store) = open_temp_store().await;
        // Local, unsynced content that a pull should discard.
        store
            .create_section("Projects", EntryType::Normal)
            .await
            .unwrap();
        store
            .create_entry(&EntryFields::Normal(crate::entry::NormalFields {
                name: "Local-only project".into(),
                location: None,
                date: None,
                start_date: None,
                end_date: None,
                summary: None,
            }))
            .await
            .unwrap();

        assert!(sync_via_json(&remote_side, &store).await);

        let sections = store.sections().await.unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "Skills");
        assert_eq!(store.entries(None).await.unwrap(), vec![]);
    }

    #[tokio::test]
    async fn braindump_survives_a_round_trip() {
        use crate::entry::{EntryFields, NormalFields};

        let (_dir, store) = open_temp_store().await;
        store
            .create_section("Projects", crate::section::EntryType::Normal)
            .await
            .unwrap();
        let entry = store
            .create_entry(&EntryFields::Normal(NormalFields {
                name: "Side project".into(),
                location: None,
                date: None,
                start_date: None,
                end_date: None,
                summary: None,
            }))
            .await
            .unwrap();
        store
            .set_braindump(entry.id, Some("started as a weekend hack"))
            .await
            .unwrap();

        let (_dir2, store2) = open_temp_store().await;
        assert!(sync_via_json(&store, &store2).await);

        let entries = store2.entries(None).await.unwrap();
        assert_eq!(
            entries[0].braindump.as_deref(),
            Some("started as a weekend hack")
        );
    }
}
