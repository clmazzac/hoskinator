# Web UI decisions

Decisions shaping the web UI in `web/`, newest last. Repo-wide decisions live in `CLAUDE.md`;
architectural ones in `docs/adr/`.

## React + Vite + TypeScript (Slice 1, #2)

**Why:** chosen for Slice 9's two-panel assembly screen, not Slice 1's form — it has the deepest
ecosystem for drag-and-drop, virtualized lists, and an embeddable YAML/code editor. Bundle size is
irrelevant for a localhost-served single-user tool.

**Prerequisite (resolved):** Node had to be installed **inside WSL**, because `npm` resolved to the
Windows install and broke Vite builds run from WSL. `node` and `npm` now resolve to linuxbrew ahead
of `/mnt/c/Program Files/nodejs`.

## Product design is done by hand, not generated (Slice 1, #2)

Layout, visual design, interaction, and the wording a user reads are the maintainer's, produced
manually. An agent does not choose them, and does not arrive at them as a side effect of building
something else.

**Why:** AI is not yet good at product design, and design decisions smuggled into an
infrastructure PR are the hardest kind to notice — they arrive as working code with taste already
baked in, and reviewing them means reverse-engineering intent from CSS.

**How to apply:** when a task needs a user-facing surface before its design exists, build the
smallest thing that proves the machinery and looks unfinished on purpose. Raw data beats an invented
form. If a real interface is unavoidable, stop and ask rather than picking.

## The Slice 1 web shell is a validation harness, not the product UI (Slice 1, #2)

It shows the Profile as raw JSON in a `textarea` with Reload and Save. No styling, no per-field
inputs, no vocabulary of its own.

**What it exists to prove:** that a compile-time-embedded SPA is co-hosted by axum on the same port
and speaks the same JSON-RPC contract as the CLI. That is the Slice 1 acceptance criterion; the
interface is not.

**Why raw JSON:** a field-by-field form cannot avoid inventing semantics. A first attempt collapsed
a one-element `email` list into a scalar on save, silently overriding the `OneOrMany` decision in
`docs/decisions/store.md` — a data change disguised as a form control. Editing the JSON directly
invents nothing.

**The real UI is Slice 9 (#10)**, where the two-panel assembly screen gets a design conversation of
its own. React was chosen for that screen, not for this form.
