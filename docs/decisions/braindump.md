# Braindump and bullet-suggestion decisions

Decisions shaping the Braindump field and AI bullet suggestions, newest last. Repo-wide decisions
live in `CLAUDE.md`; architectural ones in `docs/adr/`.

## Braindump lives on Entry, in the Master Store

A nullable `braindump` column on the `entry` table (migration 10), not a separate table and not a
field inside `fields`.

**Why:** an Entry already is "a job, project, degree, publication, award, skill line, etc."
(`CONTEXT.md`) — the exact unit a braindump is written against. `fields` is rendercv input and
`deny_unknown_fields`-checked per entry type; braindump is neither, so it stays a sibling column
rather than a field. A 1:1 relationship is a plain nullable column, not a join.

Braindump is Master-Store-only, the same as Bullets and Variants: it is never written to
`resume.yaml` and carries no branch or application reference.

## Blank text clears it

`set_braindump` trims the given text and stores `NULL` for anything that trims to empty, mirroring
`Variant.note`'s clear-by-empty-string convention.

## Each suggested bullet quotes what it comes from

`suggest_bullets` (`crates/ai`) returns `DraftBullet { text, why }`. `why` names the phrase in the
braindump the bullet is grounded in, not a generic rationale — the prompt tells the model to invent
no number, tool, or outcome the notes do not already state, and `why` is what lets a reader check
that against the source.

Model choice, transport, and the "AI unconfigured" fallback all reuse `ai.assess`'s existing
`Transport`/`Config` machinery (`HOSKINATOR_AI_SUGGEST_MODEL` alongside
`HOSKINATOR_AI_ASSESS_MODEL`). No new AI infrastructure — one more per-task prompt over the same
seam.

## A missing braindump is rejected, not sent empty

`ai.suggest_bullets` requires a non-blank braindump and answers `BRAINDUMP_EMPTY` otherwise, rather
than calling Claude with nothing to work from and getting back invention instead of drafts.

## The suggestion UI has no copy explaining itself

"Suggest bullets" is a plain button under the braindump field, hidden until that field holds text.
No onboarding text, no "AI-powered" label. A drafted bullet shows its own grounding quote instead
of a disclaimer; accepting one is a single "Add" click, and it becomes a normal Bullet — nothing
about it is marked as AI-originated afterward, the same way a Variant carries no note about who
typed it.

## The Anthropic key can be set from the UI, not only `ANTHROPIC_API_KEY`

A settings dialog (the key icon in `MenuBar`) writes `anthropic_api_key` into Hoskinator's own
`config.toml`, alongside `resume_repo` and `applications_sheet`. `hoskinator_ai::Config::resolve`
checks it first and falls back to the environment variable, so a key set from the UI takes effect
immediately without an env var or a restart.

**Anthropic only, not OpenRouter:** ADR-0005 and `docs/decisions/tailoring.md` already settled this
for a tiny model; this dialog does not reopen it.

**Plaintext on disk, not a keychain:** `config.toml` already holds equivalent trust-level settings
un-encrypted, and Hoskinator is a local, single-user tool. The one protection added is that
`remember_key` sets the file to mode 0600 (owner-only) on every write, key or not.

**Never round-tripped back to the client:** the RPC surface is `ai.set_api_key(key) -> bool` and
`ai.status() -> bool` — never a getter for the key itself. The dialog always starts with an empty
field and shows only whether a key is configured, so a typed key can't leak back out over a
future request, a log line, or a screen share.

**`remember_key` is now shared, not triplicated:** `sheets::remember` and
`workspace::remember_repository` already rewrote one line of `config.toml` in place, keeping every
other line; adding a third copy for `anthropic_api_key` was the point to extract the shared
`config::remember_key`, which the other two now call.
