# `design:` is user-owned and shared across resumes

The engine writes only the **`cv:`** block of a resume YAML. The **`design:`** block — theme, margins, fonts, spacing, and the entry layout templates — belongs to the user and lives in a **single shared file at the repo root**, passed to rendercv with `rendercv render --design <path>`.

Reproducing a specific existing resume style takes a substantial design block: matching a common LaTeX template (sb2nov-derived) to near-identical output took roughly forty lines of overrides, verified against a real resume. Inlining that into every branch's `resume.yaml` would mean editing every branch to change one margin. What varies per branch is which bullets are selected and how they are worded (ADR-0001); how the document *looks* is a property of the person, not of the application.

This keeps the repo vanilla in the sense ADR-0001 requires. A shared `design.yaml` is an ordinary rendercv design file, not a tool-specific artifact — anyone who has never heard of Hoskinator can still clone the repo and render any branch.

## Consequences

- **Schema validation covers `cv:` only.** ADR-0003 calls for validating generated YAML against rendercv's emitted JSON Schema. That cannot extend to `design:`: the schema models it as a closed discriminated union over the nine built-in theme names, each a `const`, so a working custom Typst theme fails schema validation while rendering correctly — confirmed by scaffolding one with `rendercv create-theme` and rendering it. The engine validates what it writes and leaves the rest alone.
- The engine never writes, migrates, or lints `design:`. A user on a custom theme is unaffected by engine changes.
- **Per-resume design overrides are out of scope for v1.** One application wanting a different look is rarer than wanting one style change to apply everywhere.
- Rendering and export must pass `--design`. A branch's `resume.yaml` is therefore not a complete render input on its own, and a resume is only reproducible alongside the design file.

## Considered options

- **`design:` inlined in every `resume.yaml`.** Each branch becomes self-contained and renders with a bare `rendercv render`. Rejected: any style change becomes an N-branch edit, and the design block is larger than the tailoring it would sit beside.
- **Design held in the Master Store and injected on write.** Rejected: it contradicts ADR-0001's one-way flow and would make the repo unrenderable without the tool, since the YAML on disk would be missing its design.

## Amendment: the engine writes `design:`, from a fixed set of themes (Slice 9, #10)

**Superseded above:** the shared `design.yaml` at the repo root, and "the engine never writes,
migrates, or lints `design:`."

The engine now writes a `design:` block into each branch's `resume.yaml`, and the user picks its
theme from rendercv's built-in set. There is no separate design file and no `--design` flag.

**Why the original reasoning no longer holds.** It rejected inlining because a design block matching
a LaTeX template took roughly forty lines of overrides, and any style change would then be an
N-branch edit. A built-in theme is one line, not forty. `theme: engineeringresumes` costs nothing to
repeat and nothing to change.

**What this buys.** Schema validation now covers the whole document. The original text records that
`design:` could not be validated, because rendercv's schema models it as a closed union over the
built-in theme names and a custom Typst theme fails validation while rendering correctly. Restricting
the choice to that same closed set turns the obstacle into a guarantee: every document the engine
writes validates whole.

**What it costs.** Custom themes are out. A user who scaffolds one with `rendercv create-theme` can
still hand-edit `design:` and the engine will not touch it, but the picker cannot offer it and the
schema check will reject a write that carries it. Per-branch design overrides remain out of scope;
the picker sets one theme per resume, which is per-branch by construction.

The repo stays vanilla either way (ADR-0001, ADR-0002). A `resume.yaml` carrying a built-in theme
name is an ordinary rendercv file, and unlike the shared-design arrangement it is now a complete
render input on its own.
