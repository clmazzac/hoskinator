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

## `email`, `phone`, and `website` preserve scalar-vs-list (Slice 1, #3)

rendercv types these as `T | list[T] | None`. `OneOrMany<T>` round-trips whichever form the user
wrote rather than normalising to a list.

**Why:** normalising is not display-neutral. Rendering the same CV with `email: ada@example.com`
against `email: [ada@example.com]` gives byte-identical Typst, PDF, and PNG — but the Markdown and
HTML come out as `- Email: [['ada@example.com']](mailto:['ada@example.com'])`. rendercv's
`markdown/Header.j2.md` interpolates `{{cv.email}}` as a scalar and never loops over it, unlike
`social_networks` directly below it, so any list stringifies as a Python repr. HTML is generated
from the Markdown, so one template breaks both.

**Accepted cost:** every consumer handles two arms. Normalising would have broken Markdown and HTML
for *every* user, since everyone has at least one email.

**Not our bug to fix:** multi-value `email` is mangled the same way whatever we store — it is
upstream, and it only affects Markdown and HTML.

## `social_networks.network` is a Rust enum (Slice 1, #3)

The seventeen names rendercv accepts, as `SocialNetworkName`, not a `String`.

**Why:** it makes an invalid network unrepresentable and gives Slice 9's web UI its dropdown for
free.

**Accepted cost:** the drift ADR-0003 warns about — when rendercv adds a network, Hoskinator rejects
YAML that is actually valid until the enum is bumped. A test pins all seventeen wire strings, and
`Google Scholar`, `IMDB`, and `ORCID` need `serde(rename)` to match.
