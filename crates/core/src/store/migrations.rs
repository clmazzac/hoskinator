//! Schema migrations.
//!
//! Numbered SQL files applied in order, with `PRAGMA user_version` recording which have run.

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sql_types::BigInt;

use super::StoreError;

/// Every migration, in the order they must be applied.
///
/// A migration is never edited once it has shipped; correcting the schema means adding another.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../../migrations/0001_profile.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../../migrations/0002_section.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("../../migrations/0003_job_descriptions.sql"),
    },
    Migration {
        version: 4,
        sql: include_str!("../../migrations/0004_entry.sql"),
    },
];

struct Migration {
    version: i64,
    sql: &'static str,
}

/// The version a fully migrated database reports.
#[allow(dead_code)]
pub(super) const LATEST_VERSION: i64 = MIGRATIONS.len() as i64;

/// One row of `PRAGMA user_version`.
#[derive(QueryableByName)]
struct UserVersion {
    #[diesel(sql_type = BigInt)]
    user_version: i64,
}

/// Applies every migration the database has not yet seen.
pub(super) fn apply(connection: &mut SqliteConnection) -> Result<(), StoreError> {
    let current = schema_version(connection)?;

    for migration in MIGRATIONS.iter().filter(|m| m.version > current) {
        // A migration and the version that records it must land together, or a later open would
        // replay a migration that has already run.
        connection
            .batch_execute(&format!(
                "BEGIN;\n{}\nPRAGMA user_version = {};\nCOMMIT;",
                migration.sql, migration.version
            ))
            .map_err(|source| StoreError::Migrate {
                version: migration.version,
                source,
            })?;
    }

    Ok(())
}

/// The version recorded in the database, or 0 for one that has never been migrated.
pub(super) fn schema_version(connection: &mut SqliteConnection) -> Result<i64, StoreError> {
    diesel::sql_query("PRAGMA user_version")
        .get_result::<UserVersion>(connection)
        .map(|row| row.user_version)
        .map_err(StoreError::SchemaVersion)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_versions_start_at_one_and_do_not_skip() {
        for (index, migration) in MIGRATIONS.iter().enumerate() {
            assert_eq!(
                migration.version,
                index as i64 + 1,
                "migration {index} is numbered {}",
                migration.version
            );
        }
    }

    #[test]
    fn every_migration_carries_sql() {
        for migration in MIGRATIONS {
            assert!(
                !migration.sql.trim().is_empty(),
                "migration {} is empty",
                migration.version
            );
        }
    }
}
