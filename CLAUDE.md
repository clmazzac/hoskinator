# CLAUDE.md

## Agent skills

### Issue tracker

Issues live in GitHub Issues for `clmazzac/hoskinator`, managed via the `gh` CLI. See `docs/agents/issue-tracker.md`.
(The repo was transferred from `arniber21/hoskinator`; issue numbers carried over.)

### Triage labels

Default vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context (one `CONTEXT.md` + `docs/adr/` at the repo root). See `docs/agents/domain.md`.

## Workflow

### Stacked PRs via Graphite

Build work as **stacked PRs using Graphite (`gt`)**, not one large branch. Each PR should be
small enough to review on its own and build on the one below it. See `.claude/skills/graphite/SKILL.md`.

### The maintainer reviews every line

Cam reads every line of generated code before it lands, to keep AI slop out of the codebase.

**How to apply:** optimise for reviewability over throughput. Keep PRs small and single-purpose,
prefer several small PRs to one large one, and stop for review at the points where an API surface
gets defined rather than building the whole stack first — a correction to a lower PR means rewriting
code that was already reviewed, so it costs a re-review, not just a rebase. Explain non-obvious
choices in the commit message or a comment so review does not require reverse-engineering intent.

### Doc comments state the rule, not the reasoning

Keep doc comments terse: say what the item does or what the rule is, and stop. Rationale — why the
rule is that way, what it guards against, what the alternative was — belongs in the commit message,
the PR description, an ADR, or the decision log below, not inline.

**Why:** reasoning inline reads as padding, and it goes stale in the place least likely to be
reread. One line that is true beats three that explain themselves.

### Implementation decisions are the maintainer's call

Do not pick libraries, frameworks, schema shapes, or API surfaces unilaterally. Propose options with
trade-offs and get sign-off before writing code. Record what gets decided in the log below.

## Decision log

Design decisions made during implementation, newest last. Architectural decisions with lasting
consequences belong in `docs/adr/` instead; this log is for the smaller calls that shape the code.

### Transport: axum owns HTTP, jsonrpsee dispatches (Slice 1, #2)

`axum` owns the socket, static-file serving, and the no-op auth middleware. `jsonrpsee`'s
`RpcModule` is used **purely as a dispatcher** — an axum handler passes the request body to
`RpcModule::raw_json_request` and returns the JSON response.

**Why:** ADR-0003's premise is a stable JSON-RPC contract spoken by arbitrary frontends, so
protocol correctness (2.0 spec edge cases, batching, reserved error codes) is the wrong thing to
hand-roll. `#[rpc(server, client)]` also generates the client, which the CLI needs — hand-rolling
means writing both ends for every method across Slices 2–14. Using `raw_json_request` rather than
jsonrpsee's own server keeps axum in charge of the port, so co-hosting the embedded SPA stays trivial.

**Accepted costs:** jsonrpsee is pre-1.0 (0.26) and churns across versions; requests round-trip
through strings rather than typed values.

### Store client: the `libsql` crate (Slice 1, #2)

**Why:** the only client that is actually libSQL, matching the PRD and #2 as written, and it keeps
the deferred Turso sync a config change rather than a port.

**Accepted costs:** youngest of the candidates, thinner docs, and no compile-time SQL verification
(`sqlx` was the alternative that offered it) — SQL errors surface at runtime, so store code needs
real test coverage to compensate.

### Web UI: React + Vite + TypeScript (Slice 1, #2)

**Why:** chosen for Slice 9's two-panel assembly screen, not Slice 1's form — it has the deepest
ecosystem for drag-and-drop, virtualized lists, and an embeddable YAML/code editor. Bundle size is
irrelevant for a localhost-served single-user tool.

**Prerequisite:** Node must be installed **inside WSL**. `npm` currently resolves to the Windows
install (`/mnt/c/Program Files/nodejs`), which breaks Vite builds run from WSL.

### Layout: Cargo workspace at the repo root (Slice 1, #2)

`crates/core` (package `hoskinator-core`) and `crates/hoskinator` (the binary), with the web UI in
`web/`. The pre-existing `backend/` scaffold is removed — that name contradicts ADR-0005, where the
one binary is daemon, CLI, and web host at once.

**Note:** the package is `hoskinator-core`, not `core`, because a crate literally named `core`
shadows Rust's built-in `core` and causes confusing resolution errors.

### Toolchain: the whole repo tracks `stable` (Slice 1, #2)

`rust-toolchain.toml` sets `channel = "stable"` with the `rustfmt` and `clippy` components, and CI
uses the same channel rather than naming a version of its own.

**Why:** an exact pin (`1.96.0`) would have had to be repeated in the CI workflow, because
`dtolnay/rust-toolchain` does not read `rust-toolchain.toml` — its `toolchain` input just defaults
to `stable`. Two copies of a version string drift, and when they drift CI silently tests a different
compiler than the one used locally. One channel, named once, cannot.

**Accepted cost:** a new stable release can introduce `clippy` lints that fail CI on a PR that did
not change. The fix is to address the lint or bump the pin deliberately.

### "Home", not "workspace" (Slice 1, #2)

The directory holding Hoskinator's data is **Home** (`HOSKINATOR_HOME`), not "workspace" as issue #2
words it. Defined in `CONTEXT.md`.

**Why:** this repo *is* a Cargo workspace, so a type called `Workspace` would be ambiguous on every
read. `Home` follows `CARGO_HOME` / `RUSTUP_HOME`, so the environment variable reads naturally.

Resolution order is `HOSKINATOR_HOME` → the `home` key in the config file → the platform data
directory, and never the current working directory. A per-command `--home` flag was deliberately
skipped: nothing consumes it until the CLI exists, and it layers on cleanly then.

### The store lives in Home; the resume repo does not (Slice 1, #2)

Home holds only the store, at `store/hoskinator.db` — its own directory, because SQLite writes
`-wal` and `-shm` files beside the database and backups may join them later. The rendercv git
repository is located separately, by configuration.

**Why:** the store is tool-managed and opaque, so burying it in a platform directory is right. The
repo is the opposite — it is the user's own artifact, which they will want to `cd` into, run `git`
in, and push to their own remote. ADR-0001's "database beside the repo" is satisfied by the two
being independently located, and the repo stays vanilla either way.

**Accepted cost:** two paths to resolve rather than one, and first run must either ask where the
repo goes or create a default.

### Platform paths via `directories`; errors via `thiserror`; config in TOML (Slice 1, #2)

- **`directories`** gives native paths per platform (XDG on Linux, `Application Support` on macOS,
  `%APPDATA%` on Windows) rather than forcing XDG everywhere, since ADR-0003 makes multi-device a
  v1 goal. Cost: no single documented path.
- **`thiserror`** typed enums per module, not `anyhow`. `core` is a library, and PR 4 must map its
  errors onto JSON-RPC codes by matching variants rather than parsing message strings.
- **TOML** for the config file, with `deny_unknown_fields` so a typo is an error rather than a
  setting that silently does nothing.
