# Store decisions

Decisions shaping `hoskinator-core`'s store layer, newest last. Repo-wide decisions live in
`CLAUDE.md`; architectural ones in `docs/adr/`.

## Store client: the `libsql` crate (Slice 1, #2)

**Why:** the only client that is actually libSQL, matching the PRD and #2 as written, and it keeps
the deferred Turso sync a config change rather than a port.

**Accepted costs:** youngest of the candidates, thinner docs, and no compile-time SQL verification
(`sqlx` was the alternative that offered it) — SQL errors surface at runtime, so store code needs
real test coverage to compensate.

## The store API is async, behind one `Arc<Store>` (Slice 1, #3)

`hoskinator-core`'s public store API is `async`, and the crate exposes a `Store` owning a single
libSQL `Connection`, shared as `Arc<Store>`. WAL mode is set at initialisation.

**Why async:** `libsql` is async and axum is async, so the daemon composes without an adapter. A sync
wrapper would have to `block_on` internally, which panics when called from a tokio worker thread —
a runtime failure rather than a compile error. Local SQLite I/O is blocking underneath, so async buys
nothing today; it pays off for Turso remote later, and for holding many AI calls in flight.

**Why one handle, not a pool:** ADR-0003 says the engine owns the single connection, and `Connection`
is already `Clone + Send + Sync`. It also keeps pooling *reversible* — `Store`'s internals can grow a
pool with no call-site changes, whereas passing a connection per call would put it in every
signature. For bulk work the bottleneck is LLM API latency, not the database, by roughly three orders
of magnitude.

## Migrations: numbered SQL files + `PRAGMA user_version` (Slice 1, #3)

Embedded `.sql` files applied in order, with `user_version` recording what has run.

**Why:** near-forced — no migration crate supports libSQL. `refinery`'s driver features are
`rusqlite`, `postgres`, `mysql`, `tokio-postgres`, `mysql_async`, and `tiberius` (checked against its
`Cargo.toml`); `sqlx::migrate` requires sqlx itself. Idempotent `CREATE TABLE IF NOT EXISTS` was rejected because later slices add FTS5 tables
that must be created *and backfilled* on databases that already hold data.

## Profile mirrors the whole `cv:` header (Slice 1, #3)

Every `cv:` field except `sections`, with `email`, `phone`, `website`, `social_networks`, and
`custom_connections` stored as JSON columns.

**Why the full mirror:** the user must be able to express any header rendercv supports, including
omitting ubiquitous fields — `Cv.required` is `none`, so *every* field is optional. A single-valued
subset could not represent multiple emails or `custom_connections`.

**Why JSON, not child tables:** Profile is a singleton always read and written whole, so normalising
buys no query power, and JSON preserves array order — which carries through to the rendered header.

**Two constraints this imposes:** every field must be nullable, and the YAML writer must *omit*
`None` rather than emit `null` or `""` — an empty string renders as a blank connection.
