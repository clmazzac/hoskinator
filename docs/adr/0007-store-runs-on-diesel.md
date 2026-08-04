# The store runs on Diesel over SQLite

The Master Store reaches SQLite through **Diesel**. Every query is built from a schema declared in Rust, so the compiler checks what a query string leaves to run time: the columns a row is decoded from, their types, and the table and column names themselves.

Diesel drives SQLite through `libsqlite3-sys`, which is bundled and built from source.

## Consequences

- **Diesel's SQLite backend is synchronous.** `diesel-async` has no SQLite support. The store keeps its async public API (ADR-0003, and axum needs it) by holding the connection in an `Arc<Mutex<SqliteConnection>>` and running every query on `tokio::task::spawn_blocking`. Local SQLite I/O is blocking underneath either way, so no call site changes.
- **The schema is declared twice**: once in `migrations/*.sql`, which SQLite reads, and once in `crates/core/src/store/schema.rs`, which Diesel reads. A test selects every declared column from a migrated database, so the two cannot drift apart silently.
- **Migrations stay as numbered SQL files under `PRAGMA user_version`.** `diesel_migrations` records applied migrations in its own table, so adopting it would tell every existing database that nothing had run.
- **Building requires a C compiler.** `libsqlite3-sys` compiles SQLite from source rather than linking the system library, which fixes the version and guarantees `STRICT` table support.
- **Remote SQLite, if it is ever wanted, is a port rather than a setting.** Diesel's backend trait is public, so the path is open, but it is work.
