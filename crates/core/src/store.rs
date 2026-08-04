//! The Master Store.
//!
//! One libSQL connection. Opening runs any migrations the database has not seen.

use std::path::{Path, PathBuf};

use libsql::{Builder, Connection};

mod migrations;

/// The Master Store could not be opened or read.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("could not create the store directory at {path}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not open the store at {path}")]
    Open {
        path: PathBuf,
        #[source]
        source: libsql::Error,
    },

    #[error("could not enable WAL mode on the store at {path}")]
    Wal {
        path: PathBuf,
        #[source]
        source: libsql::Error,
    },

    #[error("could not apply migration {version}")]
    Migrate {
        version: i64,
        #[source]
        source: libsql::Error,
    },

    #[error("could not read the store's schema version")]
    SchemaVersion(#[source] libsql::Error),

    #[error("could not read the Profile")]
    ReadProfile(#[source] libsql::Error),

    #[error("could not write the Profile")]
    WriteProfile(#[source] libsql::Error),

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
    ReadSection(#[source] libsql::Error),

    #[error("could not write a section")]
    WriteSection(#[source] libsql::Error),

    #[error(transparent)]
    Section(#[from] crate::section::SectionError),
}

/// The Master Store: every fact and accomplishment statement the user has accumulated.
pub struct Store {
    connection: Connection,
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

        let open_error = |source| StoreError::Open {
            path: path.to_path_buf(),
            source,
        };

        let database = Builder::new_local(path).build().await.map_err(open_error)?;
        let connection = database.connect().map_err(open_error)?;

        // Before any migration: a writer holding the database in rollback-journal mode would
        // block readers for the length of the migration.
        enable_wal(&connection, path).await?;
        migrations::apply(&connection).await?;

        Ok(Self { connection })
    }

    /// The underlying connection.
    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }
}

/// Switches the database to write-ahead logging.
async fn enable_wal(connection: &Connection, path: &Path) -> Result<(), StoreError> {
    // `journal_mode` reports the mode it settled on, so it is a query rather than an execute.
    connection
        .query("PRAGMA journal_mode = WAL", ())
        .await
        .map(|_| ())
        .map_err(|source| StoreError::Wal {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    /// Opens a store under a fresh temporary directory, which the caller must keep alive.
    async fn open_temp_store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");
        let store = Store::open(&path).await.expect("opening the store");
        (dir, store)
    }

    async fn text(store: &Store, sql: &str) -> String {
        let mut rows = store.connection().query(sql, ()).await.unwrap();
        let row = rows.next().await.unwrap().expect("a row");
        row.get::<String>(0).unwrap()
    }

    async fn number(store: &Store, sql: &str) -> i64 {
        let mut rows = store.connection().query(sql, ()).await.unwrap();
        let row = rows.next().await.unwrap().expect("a row");
        row.get::<i64>(0).unwrap()
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

        assert_eq!(text(&store, "PRAGMA journal_mode").await, "wal");
    }

    #[tokio::test]
    async fn opening_migrates_to_the_latest_version() {
        let (_dir, store) = open_temp_store().await;

        assert_eq!(
            number(&store, "PRAGMA user_version").await,
            migrations::LATEST_VERSION
        );
    }

    #[tokio::test]
    async fn the_profile_table_exists_after_migrating() {
        let (_dir, store) = open_temp_store().await;

        let count = number(
            &store,
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'profile'",
        )
        .await;

        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn reopening_an_existing_store_applies_nothing_further() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");

        Store::open(&path).await.unwrap();
        let reopened = Store::open(&path).await.expect("reopening the store");

        assert_eq!(
            number(&reopened, "PRAGMA user_version").await,
            migrations::LATEST_VERSION
        );
    }

    #[tokio::test]
    async fn reopening_preserves_what_was_written() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store").join("hoskinator.db");

        let store = Store::open(&path).await.unwrap();
        store
            .connection()
            .execute("INSERT INTO profile (id, name) VALUES (1, 'Ada')", ())
            .await
            .unwrap();
        drop(store);

        let reopened = Store::open(&path).await.unwrap();

        assert_eq!(text(&reopened, "SELECT name FROM profile").await, "Ada");
    }

    #[tokio::test]
    async fn the_profile_table_holds_at_most_one_row() {
        let (_dir, store) = open_temp_store().await;

        store
            .connection()
            .execute("INSERT INTO profile (id) VALUES (1)", ())
            .await
            .unwrap();
        let second = store
            .connection()
            .execute("INSERT INTO profile (id) VALUES (2)", ())
            .await;

        assert!(second.is_err(), "a second row should be rejected");
    }
}
