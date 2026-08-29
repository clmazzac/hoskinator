# CLAUDE.md

## Agent skills

### Issue tracker

Issues live in GitHub Issues for `clmazzac/hoskinator`, managed via the `gh` CLI. See `docs/agents/issue-tracker.md`.
(The repo was transferred from `arniber21/hoskinator`; issue numbers carried over.)

### Triage labels

Default vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context (one `CONTEXT.md` + `docs/adr/` + `docs/decisions/` at the repo root). See
`docs/agents/domain.md`.

## Workflow

### Stacked PRs via Graphite

Build work as **stacked PRs using Graphite (`gt`)**, not one large branch. Each PR should be
small enough to review on its own and build on the one below it. See `.claude/skills/graphite/SKILL.md`.

### The maintainer reviews every line

A human maintainer reads every line of generated code before it lands, to keep AI slop out of the
codebase.

**How to apply:** optimise for reviewability over throughput. Keep PRs small and single-purpose,
prefer several small PRs to one large one, and stop for review at the points where an API surface
gets defined rather than building the whole stack first — a correction to a lower PR means rewriting
code that was already reviewed, so it costs a re-review, not just a rebase. Explain non-obvious
choices in the commit message or a comment so review does not require reverse-engineering intent.

### Doc comments state the rule, not the reasoning

Keep doc comments terse: say what the item does or what the rule is, and stop. Rationale — why the
rule is that way, what it guards against, what the alternative was — belongs in the commit message,
the PR description, an ADR, or a decision log, not inline.

**Why:** reasoning inline reads as padding, and it goes stale in the place least likely to be
reread. One line that is true beats three that explain themselves.

### Prose goes through the `orwell-writing` skill

Invoke `orwell-writing` before writing commit messages, PR descriptions, and the markdown under
`docs/`, `CLAUDE.md`, and `CONTEXT.md`.

**Not** doc comments, and not code. Doc comments are governed by "Doc comments state the rule, not
the reasoning" above and by rule 1 of `docs/STYLE.md`, which is stricter and more specific than the
skill; running both over one line produces churn, not clarity.

**`docs/STYLE.md` wins wherever the two disagree.** In particular, the skill's ASD-STE100 baseline
asks for American spelling, and this repo writes British — `normalising` in
`docs/decisions/store.md`, `serialises` in `crates/core/src/profile.rs`, `optimise` above. Do not
"correct" them.

**Why here rather than a hook:** a hook that restates this would be a second copy of the rule, in a
file nobody reads, that drifts from this one.

### Implementation decisions are the maintainer's call

Do not pick libraries, frameworks, schema shapes, or API surfaces unilaterally. Propose options with
trade-offs and get sign-off before writing code. Record what gets decided in `docs/decisions/`, or
in the log below if it applies repo-wide.

### Product design is decided by a human

Layout, visual design, interaction, and user-facing wording are decided by a human developer. Claude
may consult — offer options with trade-offs, critique a mockup, point at references — and may build
what a human has decided. Claude does not make the call. The rule above asks for sign-off; this one
also fixes who can give it.

**Why:** AI is not yet good at product design, and design smuggled into an infrastructure PR is the
hardest kind to catch — it arrives as working code with taste already baked in.

**How to apply:** when a user-facing surface is needed before its design exists, put the decision to
a human, or build the smallest thing that proves the machinery and looks unfinished on purpose — raw
data over an invented form. Never let a UI decision ride along in a PR that is nominally about
something else. See `docs/decisions/web.md`.

## Decision log

Decisions that apply across the whole repo, newest last. Decisions scoped to one component live in
`docs/decisions/`; architectural decisions with lasting consequences belong in `docs/adr/`.

| Component | File |
|---|---|
| Store (`hoskinator-core`) | `docs/decisions/store.md` |
| AI layer (`hoskinator-ai`) | `docs/decisions/ai.md` |
| Home and config | `docs/decisions/home-and-config.md` |
| Transport (HTTP + JSON-RPC) | `docs/decisions/transport.md` |
| Web UI (`web/`) | `docs/decisions/web.md` |
| Resume YAML (`resume.yaml`) | `docs/decisions/resume.md` |
| Rendering (rendercv) | `docs/decisions/render.md` |
| Workspace (branches, GitHub, applications) | `docs/decisions/workspace.md` |
| Tailoring (resume-vs-JD scoring) | `docs/decisions/tailoring.md` |
| Braindump and bullet suggestions | `docs/decisions/braindump.md` |
| Google Sheets sync | `docs/decisions/google-sync.md` |

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

### Errors via `thiserror`, not `anyhow` (Slice 1, #2)

Typed enums per module. `core` is a library, and PR 4 must map its errors onto JSON-RPC codes by
matching variants rather than parsing message strings.
