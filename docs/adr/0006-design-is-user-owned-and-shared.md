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
