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
