# Store decisions

Decisions shaping `hoskinator-core`'s store layer, newest last. Repo-wide decisions live in
`CLAUDE.md`; architectural ones in `docs/adr/`.

## The store API is async, behind one `Arc<Store>` (Slice 1, #3)

`hoskinator-core`'s public store API is `async`, and the crate exposes a `Store` owning a single
`SqliteConnection`, shared as `Arc<Store>`. WAL mode is set at initialisation.

**Why async:** axum is async, so the daemon composes without an adapter. A sync wrapper would have to
`block_on` internally, which panics when called from a tokio worker thread — a runtime failure rather
than a compile error. Local SQLite I/O is blocking underneath, and Diesel's SQLite backend is
synchronous, so the connection sits behind an `Arc<Mutex<_>>` and every query runs on
`spawn_blocking`. Async buys nothing today; it pays off for a remote database later, and for holding
many AI calls in flight.

**Why one handle, not a pool:** ADR-0003 says the engine owns the single connection. It also keeps
pooling *reversible* — `Store`'s internals can grow a pool with no call-site changes, whereas passing
a connection per call would put it in every signature. For bulk work the bottleneck is LLM API
latency, not the database, by roughly three orders of magnitude.

## Migrations: numbered SQL files + `PRAGMA user_version` (Slice 1, #3)

Embedded `.sql` files applied in order, with `user_version` recording what has run.

**Why not `diesel_migrations`:** it records applied migrations in its own table, so adopting it would
tell every existing database that nothing had run.

**Why not idempotent `CREATE TABLE IF NOT EXISTS`:** later slices add FTS5 tables that must be
created *and backfilled* on databases that already hold data.

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

## The Master Store holds decomposed records, not YAML blobs (Slice 2, #3)

A Section is a managed `{ name, entry_type }` record, and Entries and Bullets are rows. The
alternative considered was storing each section as a free-text field holding raw rendercv YAML.

**Why not blobs:** a blob has no addressable units, and four slices need them — FTS5 search (#6)
matching a bullet rather than a section, the assembly loop (#10) picking units into a resume, and
the AI slices (#12–#14) scoring and drafting per bullet. Bullet/Variant is the product: an
accomplishment identity owning several wordings. That requires the accomplishment to be a record.

Resumes are already text on git branches. A store of text blobs is another resume, which makes the
tool a file copier; the store earns its existence by decomposing.

**What blobs are genuinely better at:** first-time entry. That is recovered by making raw YAML an
*input path* — paste a section, parse it into Entries and Bullets — rather than the storage format.

## `entry_type` is a Rust enum of the nine rendercv arms (Slice 2, #3)

`EntryType`, stored as plain `TEXT`. rendercv 2.8's `ListOfEntries` has nine arms: text (a bare
string), one-line, normal, experience, education, publication, bullet, numbered, and
reversed-numbered.

**Why an enum rather than a validated string:** the arms are *homogeneous* arrays, so a section
mixing entry shapes is rejected outright — checked against the vendored schema, `[experience,
publication]` and `[experience, "text"]` both fail. `entry_type` is the invariant that keeps every
emitted section homogeneous. A permissive string would also buy no forward compatibility: each type
is a different field set, so an unknown one has nothing behind it to render.

**The failure mode it prevents:** entry types are detected from fields, not tagged — `entry_type`
appears nowhere on the wire — and `additionalProperties` is `true`, so an object carrying the
required fields of two types validates and rendercv picks the renderer by its own arm ordering. That
is wrong output with no error.

**Accepted cost:** the usual drift — a tenth arm is rejected until the enum is bumped. Cheaper here
than for `SocialNetworkName`, since a new arm needs its field set modelled regardless.

**No `CHECK` constraint on the column:** it would be a second copy of the variant list, in a file
that only changes by migration. The enum is the one authority.

## Section ordering is not a Section field (Slice 2, #3)

Order is a property of one rendered resume, not of the store — two resumes built from the same
store legitimately order their sections differently. It belongs to assembly (#10).

Section identity is likewise unforced: Entries are eligible for sections by `entry_type` match and
never point at a Section, so a rename orphans nothing.

## Note: verbatim passthrough sections (Slice 2, #3)

Sections that are never tailored and hold no accomplishments — a one-line Skills list, a short
Summary — gain nothing from Bullet/Variant and are overhead as decomposed records. A section type
that stores its rendercv fragment verbatim would serve them.

Deliberately not built: nothing needs it before assembly (#10), and it is a user-facing surface, so
its design is the maintainer's.

## Every query is built by Diesel (#43)

Diesel builds each query from `crates/core/src/store/schema.rs`, which declares every table and
column. See ADR-0007. What that buys:

1. **Rows decode by name.** `Profile` and `Section` derive `Selectable` and are read with
   `as_select()`, which builds the `SELECT` list from the struct's field names. Reordering the
   columns in `schema.rs` changes nothing.
2. **Column names are checked, not retyped.** A column that `schema.rs` does not declare is a
   compile error, and a column `schema.rs` declares that the migrations never created fails
   `every_declared_column_exists_after_migrating`.
3. **Types are checked at build time.** `check_for_backend(Sqlite)` on each row struct rejects a
   Rust field whose type does not match the SQL type declared for its column.

**The one exception is FTS5 search.** `job_description_fts` is a virtual table, which `table!` cannot
describe and Diesel's DSL has no `MATCH` operator for, so `job_descriptions(Some(query))` is written
out as `sql_query` and bound by hand. It decodes by column name through `QueryableByName`, so the
sharpest coupling is still gone; the table and column names in that one string are not checked.

**What is still not checked:** a column added by a migration but never declared in `schema.rs`. The
drift test selects what Diesel knows about, so it sees a missing column and not a surplus one. Such
a column is invisible to the store rather than wrong, which is why it is left.

**One trap worth naming:** `AsChangeset` skips the primary key. `section.name` *is* the primary key,
so `update_section` sets its columns one by one rather than from a changeset struct — otherwise a
rename would silently do nothing.

## Entries hold rendercv's fields as JSON, keyed by a type column (Slice 3, #4)

An entry is one row: an `entry_type` discriminator, a `fields` JSON column holding exactly what
rendercv reads for that type, an integer `id`, and copies of `date`, `start_date`, and `end_date`
promoted into columns of their own. `entry_type` is indexed; the date columns are not.

**Why JSON over columns:** the nine rendercv entry types share almost no fields. One table with a
column per field would be mostly null in every row, and nine tables would need nine of everything
above them. Nothing queries into an entry's fields — an entry is read whole, rendered whole, and
replaced whole — so the columns would buy nothing and cost a migration every time rendercv adds a
field.

**Why the dates are promoted anyway:** ordering a section reverse-chronologically is the one read
that must not open every row's JSON. rendercv accepts either `date` alone or `start_date` with
`end_date`, so all three are copied out. They are copies: `fields` stays the authority, and
`EntryRow` does not even select them.

**Why one index:** listing by type is the only filter that exists. Assembly (#10) is what will order
by date, and it can add the index it needs when it can measure it.

**Fields are read against the type, never guessed.** `EntryFields` serialises untagged, so the stored
JSON is rendercv's own shape rather than a Hoskinator wrapper. Nothing deserialises it untagged:
`EntryFields::parse` takes the type and reads exactly that shape, and `deny_unknown_fields` rejects a
key rendercv does not have rather than dropping it. A field rendercv gained after this was written is
a rejected write, not a silent loss.

**An entry's type does not change.** `update_entry` replaces every field and reads the new fields
against the type the entry already has. Retyping an entry would discard the fields that made it that
type, so it is a delete and a create.

## Section eligibility stays derived (Slice 3, #4)

Entries do not point at a section. `entries_for_section` reads the section's `entry_type` and lists
the entries of that type, so retyping a section changes what it is eligible for and orphans nothing.

Issue #4 asks instead that an entry "can be assigned only to sections whose `entry_type` matches",
which reads as a stored assignment. That contradicts the Slice 2 decision above, and assembly (#10)
is where a resume records which entries it actually placed.

## Bullets are rows, not strings in the Entry blob (Slice 4, #5)

An accomplishment is a `bullet` row owning one or more `variant` rows. `highlights` is gone from
`NormalFields`, `ExperienceFields`, and `EducationFields`, and migration 0005 moves what those
columns held into bullets and variants before stripping the key out of `entry.fields`.

**Why out of the blob:** a Variant is an alternative wording *of one accomplishment*, so something
has to be that accomplishment. A string in a JSON array cannot be pointed at. Neither can a score
(#12) or a resume recording which wording it placed (#10). Slice 3 kept bullets in the blob because
nothing needed to address them; Slice 4 is the thing that needs to.

**The Bullet holds no text.** It is identity and order: `id`, `entry_id`, `position`. Every wording
lives in a Variant, so promoting a different wording is not a rewrite of the accomplishment.

**Exactly one default, half enforced by SQLite.** `variant.is_default` carries the flag, and a
partial unique index on `(bullet_id) WHERE is_default = 1` makes a second default per bullet
impossible rather than merely wrong. The other half — at least one — is the store's: creating a
Bullet requires the wording of its first Variant, and deleting the last Variant of a Bullet is
rejected. Deleting the default promotes the lowest remaining variant id, which is arbitrary but
deterministic, and therefore testable.

**Why not `bullet.default_variant_id`:** the first Variant must exist before the Bullet can point at
it, so the column could not be `NOT NULL`, and "no default" would be representable again.

## Foreign keys are enforced (Slice 4, #5)

`PRAGMA foreign_keys = ON` runs at connection setup beside WAL, and child rows declare
`REFERENCES ... ON DELETE CASCADE`. Deleting an Entry removes its Bullets, and deleting a Bullet
removes its Variants, in one statement.

**Why the pragma is needed at all:** SQLite parses `REFERENCES` and then ignores it unless foreign
keys are switched on, per connection, every time. A declared constraint nothing enforces is worse
than none, because it reads as a guarantee.

**The cost:** enforcement is store-wide and applies to every table added later. A migration that
rewrites a parent table now has to think about its children.

## Search is one FTS5 index over Entries and Variants (Slice 5, #6)

`search_fts` is a single FTS5 table with `kind`, `ref`, and `body`, kept current by triggers on
`entry` and `variant`. One `MATCH` answers with one ranked list.

**Why one table:** FTS5's `rank` is a score within an index. Two indexes give two scales, so ranking
an Entry hit against a Variant hit would mean inventing a comparison. One index is the only shape
where the ordering means anything.

**Every Variant is indexed, not just the default.** Finding that a wording you are not currently
using matches the posting is the point of keeping several.

**Job Descriptions stay out.** `job_description_fts` is untouched and `jd.list(query)` still searches
postings. A posting says what an employer asked for; the Master Store says what the user did. One
ranked list mixing them would make each hit ambiguous about whose claim it is.

## An Entry's searchable text is computed in Rust (Slice 5, #6)

`entry.search_text` holds the words an Entry is findable by, written by the store on every create and
update. The trigger indexes that column and knows nothing about rendercv.

**Why not index `fields` directly:** the blob contains its own keys, so searching `summary` or
`date` would match nearly every Entry.

**Why not `json_extract` in the trigger:** it would put rendercv's field taxonomy in SQL, beside the
copy the Rust structs already hold, and every rendercv field would need a migration to become
searchable.

**What goes in:** every string the fields hold, at any depth, except `date`, `start_date`, and
`end_date`. Walking the serialised fields rather than matching on each type means a field added to
`EntryFields` is searchable without touching this code. Dates are excluded because they are
structured, already promoted to their own columns, and would make `2021` match every entry of that
year.

**Backfill:** opening the store recomputes `search_text` for any Entry whose column is empty, which
is what migration 0006 leaves behind for rows that already existed. SQL cannot compute it, so the
migration cannot.

## A search hit is a Bullet, not a Variant (Slice 5, #6)

Variant matches roll up to the Bullet that owns them, keeping the best-ranked one and naming which
wording matched. Entry matches are their own kind of hit.

**Why:** one role can hold several Bullets, each with several Variants written for different
applications, and a term common to that role matches most of them. Flat Variant hits would answer a
question about one accomplishment with a page of rewordings of it. During assembly the decision is
which accomplishment to place; which wording to use is the next decision, not the same one.

**Each hit carries its Entry** so a caller can group by role without another read. It carries the
Entry's type and fields rather than a formatted label: how a hit reads to a user is a design
decision.

## The store syncs to Turso as a whole snapshot, not a live replica

ADR-0001 names "SQLite/Turso" as the store's home and says it never lives inside the resume
repository. This is that: `bank.push`/`bank.pull` move the *whole* store — every Section, Entry,
Bullet, and Variant — to and from one row in a Turso database, so the bank is available on more
than one machine. Sync is manual only; nothing runs on a timer.

**Why a snapshot, not `libsql`'s embedded replica** (Turso's actual designed-for-this feature, a
local file kept transparently in sync in the background): `libsql`'s `core` feature links its own
vendored copy of SQLite, and Diesel's `libsqlite3-sys` links a separate one. Confirmed by hand: the
two cannot both initialise in the same process — the second one panics
(`libsql was configured with an incorrect threading configuration`). `hoskinator-core`'s Diesel
connection is not going anywhere (a full rewrite onto `libsql`'s own async query API was the other
way to avoid this, and touches every query in every store module), so `libsql` is built with
`default-features = false, features = ["remote"]` — this drops the vendored SQLite entirely and
talks to Turso over its HTTP client only. A snapshot is what that mode can do without also hand-
rolling row-level replication.

**Whole-snapshot replace, no merge.** A pull deletes every local Section and Entry (cascading their
Bullets and Variants) and recreates them from what Turso holds. The side that syncs last wins,
whole-store — the same rule `docs/decisions/google-sync.md` already applies to Applications, just
without the field-level reconciliation Sheets sync does. Ids are not preserved across a sync; they
are local autoincrements, reassigned on every pull.

**Accepted cost: `push`/`pull`'s network calls are not covered by `cargo test`.** The same
in-process conflict that rules out the embedded replica also rules out using `libsql`'s local mode
as a stand-in for "the remote" in this crate's own tests. Everything else — building a snapshot,
applying one back, and the JSON both travel as — is tested directly against real `Store`s.
Confirmed by hand against a real Turso database instead: push, a local-only change, then pull,
which discarded the local change and restored exactly what was pushed, bullets and variants
included. `remote` alone is not enough to reach a `libsql://` URL — it needs the `tls` feature too
(`hyper-rustls`), or the connection panics with `the tls feature is disabled`. `libsql` is also
pre-1.0 (`0.10.0-pre.4`); the repo already depends on release candidates elsewhere (`yamlpatch`,
`yamlpath`).
