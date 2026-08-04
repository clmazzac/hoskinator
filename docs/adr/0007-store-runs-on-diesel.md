# The store runs on Diesel over SQLite

The Master Store reaches SQLite through **Diesel**, not through the `libsql` crate. This reverses the store-client choice recorded in `docs/decisions/store.md`, which picked `libsql` in Slice 1.

Every query in `hoskinator-core` was a hand-written SQL string that nothing checked until it ran. Four things it rested on — decoding by column index, table and column names retyped from the migrations, type agreement, and placeholder arity — were all invisible to the compiler. Test coverage was the only compensation, and it cannot catch a `SELECT` list that grows a column and shifts every index after it. #43 weighed the alternatives; Diesel builds the queries from a declared schema instead, so the compiler checks all four.

Diesel drives SQLite through `libsqlite3-sys`, which is bundled and built from source. There is no Diesel backend for libSQL that we are willing to depend on: `diesel-libsql` exists, but it is a 0.1 crate with one maintainer and a few months of history, and it would sit under every query in the store.

## Consequences

- **The deferred Turso sync becomes a port, not a config change.** That was the reason to choose `libsql`, and it is what this decision spends. Turso speaks the SQLite wire protocol and Diesel's backend trait is public, so the path is not closed; it is no longer free.
- **Diesel's SQLite backend is synchronous.** `diesel-async` has no SQLite support. The store keeps its async public API (ADR-0003, and axum needs it) by holding the connection in an `Arc<Mutex<SqliteConnection>>` and running every query on `tokio::task::spawn_blocking`. Local SQLite I/O was blocking underneath the old async API too, so no call site changes.
- **The schema is declared twice**: once in `migrations/*.sql`, which SQLite reads, and once in `crates/core/src/store/schema.rs`, which Diesel reads. A test selects every declared column from a migrated database, so the two cannot drift apart silently.
- **Migrations stay as numbered SQL files under `PRAGMA user_version`.** `diesel_migrations` would work now that the libSQL constraint is gone, but it tracks applied migrations in its own table, and switching would leave existing databases claiming they had run nothing.
- **Building requires a C compiler.** `libsqlite3-sys` compiles SQLite from source rather than linking the system library, which fixes the version and guarantees `STRICT` table support.
