//! The Master Store.
//!
//! One SQLite connection behind Diesel. Opening runs any migrations the database has not seen.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use diesel::connection::SimpleConnection;
use diesel::{Connection, SqliteConnection};

mod migrations;
pub(crate) mod schema;

/// The Master Store could not be opened or read.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("could not create the store directory at {path}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("the store path {path} is not valid UTF-8")]
    PathEncoding { path: PathBuf },

    #[error("could not open the store at {path}")]
    Open {
        path: PathBuf,
        #[source]
        source: diesel::ConnectionError,
    },

    #[error("could not enable WAL mode on the store at {path}")]
    Wal {
        path: PathBuf,
        #[source]
        source: diesel::result::Error,
    },

    #[error("could not enforce foreign keys on the store at {path}")]
    ForeignKeys {
        path: PathBuf,
        #[source]
        source: diesel::result::Error,
    },

    #[error("could not apply migration {version}")]
    Migrate {
        version: i64,
        #[source]
        source: diesel::result::Error,
    },

    #[error("could not read the store's schema version")]
    SchemaVersion(#[source] diesel::result::Error),

    #[error("could not read the Profile")]
    ReadProfile(#[source] diesel::result::Error),

    #[error("could not write the Profile")]
    WriteProfile(#[source] diesel::result::Error),

    #[error("the stored Profile column `{column}` is not valid JSON")]
    DecodeProfile {
        column: &'static str,
        #[source]
        source: serde_json::Error,
    },

    #[error("could not encode the Profile column `{column}`")]
    EncodeProfile {
        column: &'static str,
        #[source]
        source: serde_json::Error,
    },

    #[error("could not read a section")]
    ReadSection(#[source] diesel::result::Error),

    #[error("could not write a section")]
    WriteSection(#[source] diesel::result::Error),

    #[error("could not read an entry")]
    ReadEntry(#[source] diesel::result::Error),

    #[error("could not write an entry")]
    WriteEntry(#[source] diesel::result::Error),

    #[error("the stored entry {id} does not hold the fields its type calls for")]
    DecodeEntry {
        id: i64,
        #[source]
        source: crate::entry::EntryError,
    },

    #[error("could not encode an entry's fields")]
    EncodeEntry(#[source] serde_json::Error),

    #[error("could not read a bullet")]
    ReadBullet(#[source] diesel::result::Error),

    #[error("could not write a bullet")]
    WriteBullet(#[source] diesel::result::Error),

    #[error("could not read a variant")]
    ReadVariant(#[source] diesel::result::Error),

    #[error("could not write a variant")]
    WriteVariant(#[source] diesel::result::Error),

    #[error("could not search the store")]
    Search(#[source] diesel::result::Error),

    #[error("could not create a Job Description")]
    CreateJobDescription(#[source] diesel::result::Error),

    #[error("could not read Job Descriptions")]
    ReadJobDescriptions(#[source] diesel::result::Error),

    #[error("could not delete a Job Description")]
    DeleteJobDescription(#[source] diesel::result::Error),

    #[error("could not write an application")]
    WriteApplication(#[source] diesel::result::Error),

    #[error("could not read applications")]
    ReadApplications(#[source] diesel::result::Error),

    #[error(transparent)]
    Section(#[from] crate::section::SectionError),

    #[error(transparent)]
    Entry(#[from] crate::entry::EntryError),

    #[error(transparent)]
    Bullet(#[from] crate::bullet::BulletError),
}

/// The Master Store: every fact and accomplishment statement the user has accumulated.
pub struct Store {
    connection: Arc<Mutex<SqliteConnection>>,
}

impl Store {
    /// Opens the store at `path`, creating and migrating it if needed.
    ///
    /// Creates the parent directory if it is missing.
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| StoreError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let path = path.to_path_buf();
        let connection = blocking(move || establish(&path)).await?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// Runs `work` against the connection on a blocking thread.
    pub(crate) async fn with_connection<F, T>(&self, work: F) -> T
    where
        F: FnOnce(&mut SqliteConnection) -> T + Send + 'static,
        T: Send + 'static,
    {
        let connection = Arc::clone(&self.connection);

        blocking(move || {
            // A poisoned lock still holds a usable connection.
            let mut connection = connection.lock().unwrap_or_else(PoisonError::into_inner);
            work(&mut connection)
        })
        .await
    }
}

/// Opens the database, puts it in WAL mode, and migrates it.
fn establish(path: &Path) -> Result<SqliteConnection, StoreError> {
    let url = path.to_str().ok_or_else(|| StoreError::PathEncoding {
        path: path.to_path_buf(),
    })?;

    let mut connection = SqliteConnection::establish(url).map_err(|source| StoreError::Open {
        path: path.to_path_buf(),
        source,
    })?;

    // Before any migration: a writer holding the database in rollback-journal mode would
    // block readers for the length of the migration.
    enable_wal(&mut connection, path)?;
    enforce_foreign_keys(&mut connection, path)?;
    migrations::apply(&mut connection)?;
    crate::search::backfill_search_text(&mut connection)?;

    Ok(connection)
}

/// Switches the database to write-ahead logging.
fn enable_wal(connection: &mut SqliteConnection, path: &Path) -> Result<(), StoreError> {
    connection
        .batch_execute("PRAGMA journal_mode = WAL")
        .map_err(|source| StoreError::Wal {
            path: path.to_path_buf(),
            source,
        })
}

/// Switches on foreign key enforcement, which SQLite leaves off per connection.
fn enforce_foreign_keys(connection: &mut SqliteConnection, path: &Path) -> Result<(), StoreError> {
    connection
        .batch_execute("PRAGMA foreign_keys = ON")
        .map_err(|source| StoreError::ForeignKeys {
            path: path.to_path_buf(),
            source,
        })
}

/// Runs blocking database work off the async runtime, propagating a panic in it.
async fn blocking<F, T>(work: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    match tokio::task::spawn_blocking(work).await {
        Ok(value) => value,
        Err(error) => std::panic::resume_unwind(error.into_panic()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use diesel::prelude::*;
    use diesel::sql_types::{BigInt, Text};
    use tempfile::TempDir;

    /// Opens a store under a fresh temporary directory, which the caller must keep alive.
    async fn open_temp_store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");
        let store = Store::open(&path).await.expect("opening the store");
        (dir, store)
    }

    #[derive(QueryableByName)]
    struct JournalMode {
        #[diesel(sql_type = Text)]
        journal_mode: String,
    }

    async fn journal_mode(store: &Store) -> String {
        store
            .with_connection(|connection| {
                diesel::sql_query("PRAGMA journal_mode")
                    .get_result::<JournalMode>(connection)
                    .unwrap()
                    .journal_mode
            })
            .await
    }

    #[derive(QueryableByName)]
    struct Found {
        #[diesel(sql_type = BigInt)]
        found: i64,
    }

    /// Counts the `sqlite_master` rows `sql` selects, which must alias the count as `found`.
    async fn objects_named(store: &Store, sql: &'static str) -> i64 {
        store
            .with_connection(move |connection| {
                diesel::sql_query(sql)
                    .get_result::<Found>(connection)
                    .unwrap()
                    .found
            })
            .await
    }

    async fn schema_version(store: &Store) -> i64 {
        store
            .with_connection(|connection| migrations::schema_version(connection).unwrap())
            .await
    }

    #[tokio::test]
    async fn opening_creates_the_database_and_its_directory() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");

        Store::open(&path).await.unwrap();

        assert!(path.exists(), "{path:?} should exist");
    }

    #[tokio::test]
    async fn opening_leaves_the_database_in_wal_mode() {
        let (_dir, store) = open_temp_store().await;

        assert_eq!(journal_mode(&store).await, "wal");
    }

    #[derive(QueryableByName)]
    struct ForeignKeys {
        #[diesel(sql_type = BigInt)]
        foreign_keys: i64,
    }

    #[tokio::test]
    async fn opening_leaves_foreign_keys_enforced() {
        let (_dir, store) = open_temp_store().await;

        let enforced = store
            .with_connection(|connection| {
                diesel::sql_query("PRAGMA foreign_keys")
                    .get_result::<ForeignKeys>(connection)
                    .unwrap()
                    .foreign_keys
            })
            .await;

        assert_eq!(enforced, 1);
    }

    #[tokio::test]
    async fn opening_migrates_to_the_latest_version() {
        let (_dir, store) = open_temp_store().await;

        assert_eq!(schema_version(&store).await, migrations::LATEST_VERSION);
    }

    #[tokio::test]
    async fn every_declared_column_exists_after_migrating() {
        let (_dir, store) = open_temp_store().await;

        store
            .with_connection(|connection| {
                schema::profile::table
                    .select(schema::profile::all_columns)
                    .execute(connection)
                    .expect("selecting every profile column");
                schema::section::table
                    .select(schema::section::all_columns)
                    .execute(connection)
                    .expect("selecting every section column");
                schema::job_description::table
                    .select(schema::job_description::all_columns)
                    .execute(connection)
                    .expect("selecting every job_description column");
                schema::entry::table
                    .select(schema::entry::all_columns)
                    .execute(connection)
                    .expect("selecting every entry column");
                schema::bullet::table
                    .select(schema::bullet::all_columns)
                    .execute(connection)
                    .expect("selecting every bullet column");
                schema::variant::table
                    .select(schema::variant::all_columns)
                    .execute(connection)
                    .expect("selecting every variant column");
            })
            .await;
    }

    #[tokio::test]
    async fn the_job_description_table_and_fts_index_exist_after_migrating() {
        let (_dir, store) = open_temp_store().await;

        let found = objects_named(
            &store,
            "SELECT count(*) AS found FROM sqlite_master \
             WHERE name IN ('job_description', 'job_description_fts')",
        )
        .await;

        assert_eq!(found, 2);
    }

    #[tokio::test]
    async fn entries_are_indexed_by_type_after_migrating() {
        let (_dir, store) = open_temp_store().await;

        let found = objects_named(
            &store,
            "SELECT count(*) AS found FROM sqlite_master \
             WHERE type = 'index' AND name = 'entry_by_type'",
        )
        .await;

        assert_eq!(found, 1);
    }

    #[tokio::test]
    async fn bullets_and_variants_are_indexed_after_migrating() {
        let (_dir, store) = open_temp_store().await;

        let found = objects_named(
            &store,
            "SELECT count(*) AS found FROM sqlite_master WHERE type = 'index' \
             AND name IN ('bullet_by_entry', 'variant_by_bullet', 'variant_one_default')",
        )
        .await;

        assert_eq!(found, 3);
    }

    #[tokio::test]
    async fn opening_a_version_one_store_applies_the_job_description_migration() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hoskinator.db");
        let mut connection = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
        connection
            .batch_execute(&format!(
                "{}\nPRAGMA user_version = 1;",
                include_str!("../migrations/0001_profile.sql")
            ))
            .unwrap();
        drop(connection);

        let store = Store::open(&path).await.unwrap();

        assert_eq!(schema_version(&store).await, migrations::LATEST_VERSION);
        assert_eq!(
            objects_named(
                &store,
                "SELECT count(*) AS found FROM sqlite_master \
                 WHERE name = 'job_description_fts'",
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn opening_a_version_four_store_moves_highlights_into_bullets() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hoskinator.db");
        let mut connection = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
        connection
            .batch_execute(&format!(
                "{}\n{}\n{}\n{}\nPRAGMA user_version = 4;",
                include_str!("../migrations/0001_profile.sql"),
                include_str!("../migrations/0002_section.sql"),
                include_str!("../migrations/0003_job_descriptions.sql"),
                include_str!("../migrations/0004_entry.sql"),
            ))
            .unwrap();
        connection
            .batch_execute(
                r#"INSERT INTO entry (entry_type, fields) VALUES ('experience',
                   '{"company":"Acme","position":"Engineer","highlights":["One.","Two."]}')"#,
            )
            .unwrap();
        drop(connection);

        let store = Store::open(&path).await.unwrap();

        // Reading the entry at all proves `highlights` was stripped: the field no longer exists,
        // and unknown fields are rejected.
        let entries = store.entries(None).await.expect("reading migrated entries");
        let bullets = store.bullets(entries[0].id).await.unwrap();

        let wording: Vec<(&str, bool)> = bullets
            .iter()
            .map(|bullet| {
                let variant = &bullet.variants[0];
                (variant.text.as_str(), variant.is_default)
            })
            .collect();

        assert_eq!(wording, [("One.", true), ("Two.", true)]);
        assert_eq!(
            bullets
                .iter()
                .map(|bullet| bullet.position)
                .collect::<Vec<_>>(),
            [0, 1]
        );
    }

    #[tokio::test]
    async fn opening_a_version_five_store_backfills_what_entries_are_searchable_by() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hoskinator.db");
        let mut connection = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
        connection
            .batch_execute(&format!(
                "{}\n{}\n{}\n{}\n{}\nPRAGMA user_version = 5;",
                include_str!("../migrations/0001_profile.sql"),
                include_str!("../migrations/0002_section.sql"),
                include_str!("../migrations/0003_job_descriptions.sql"),
                include_str!("../migrations/0004_entry.sql"),
                include_str!("../migrations/0005_bullet.sql"),
            ))
            .unwrap();
        connection
            .batch_execute(
                r#"INSERT INTO entry (entry_type, fields) VALUES ('experience',
                   '{"company":"Acme","position":"Platform Engineer"}');
                   INSERT INTO bullet (entry_id, position) VALUES (1, 0);
                   INSERT INTO variant (bullet_id, text, is_default)
                   VALUES (1, 'Cut p99 latency in half.', 1);"#,
            )
            .unwrap();
        drop(connection);

        let store = Store::open(&path).await.unwrap();

        assert_eq!(store.search("Acme").await.unwrap().len(), 1);
        assert_eq!(store.search("latency").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn reopening_an_existing_store_applies_nothing_further() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");

        Store::open(&path).await.unwrap();
        let reopened = Store::open(&path).await.expect("reopening the store");

        assert_eq!(schema_version(&reopened).await, migrations::LATEST_VERSION);
    }

    #[tokio::test]
    async fn reopening_preserves_what_was_written() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");

        let store = Store::open(&path).await.unwrap();
        store
            .with_connection(|connection| {
                diesel::insert_into(schema::profile::table)
                    .values((schema::profile::id.eq(1), schema::profile::name.eq("Ada")))
                    .execute(connection)
                    .unwrap()
            })
            .await;
        drop(store);

        let reopened = Store::open(&path).await.unwrap();
        let name = reopened
            .with_connection(|connection| {
                schema::profile::table
                    .select(schema::profile::name)
                    .first::<Option<String>>(connection)
                    .unwrap()
            })
            .await;

        assert_eq!(name.as_deref(), Some("Ada"));
    }

    #[tokio::test]
    async fn the_profile_table_holds_at_most_one_row() {
        let (_dir, store) = open_temp_store().await;

        let second = store
            .with_connection(|connection| {
                diesel::insert_into(schema::profile::table)
                    .values(schema::profile::id.eq(1))
                    .execute(connection)
                    .unwrap();

                diesel::insert_into(schema::profile::table)
                    .values(schema::profile::id.eq(2))
                    .execute(connection)
            })
            .await;

        assert!(second.is_err(), "a second row should be rejected");
    }
}
