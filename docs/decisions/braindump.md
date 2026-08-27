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
