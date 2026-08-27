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

## Product design is decided by a human (Slice 1, #2)

Layout, visual design, interaction, and the wording a user reads are decided by a human developer.
An agent may offer options, critique a design, and build one a human has settled. It does not choose
them, and does not arrive at them as a side effect of building something else.

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

## The section harness is one control per method (Slice 2, #3)

Sections as raw JSON, then unstyled Create, Rename, Retype, and Delete controls. Each button issues
exactly one JSON-RPC call and reloads the list.

**Why not an editable array with one Save**, which would match the Profile textarea above it: there
is no bulk-replace method, so Save would have to diff the array into calls, and a row whose name
changed is ambiguous between a rename and a delete plus a create. Those come apart once Entries
exist. That is the failure this file already records for the `OneOrMany` collapse — a data decision
hidden in a form control.

**Still a harness, not the product UI.** No styling and no selectable rows; the name is retyped into
each control, because turning the list into something you click would be interaction design. Slice 9
(#10) owns the real interface.

## Icon-only controls stay icon-only

A control that reads as an icon elsewhere in the header (the theme toggle, and any control like it)
never gains a text label beside the icon. Use `aria-label` and `title` for the accessible name
instead of visible text.

**Why:** this has regressed more than once — a text label ("Day" / "Night") creeping back onto the
theme toggle. Recording it here so it stops recurring.

## Resume outline edits are optimistic; the round trip runs after, not before

Every mutation in `ResumeOutline` (reorder, remove, place, move) used to `await` its RPC call, then
refetch the whole outline before the screen changed at all — for a drag-and-drop-driven surface,
that meant every drop paused, sometimes for multiple sequential round trips (`resumeStep` alone
reads `resume.yaml` twice around the actual mutation). `run()` now applies the resulting
`ResumeSection[]` locally and synchronously, before the mutation is even sent; the real call and its
`resumeStep`/reload still happen, just after, to persist the change and to catch drift. A failure
rolls the optimistic guess back to what was on screen before the drop, rather than leaving a change
that never reached the server.

**Why still call `load()` on success instead of trusting the optimistic guess as final:** the
optimistic transforms are pure reorders/inserts of data the outline already had — they should match
what the server computes, but "should" isn't "always"; the reload is a cheap, invisible-on-the-happy-
path check that they didn't drift.

**Known gap:** two mutations fired in quick succession can have the first one's reload land after
the second's optimistic update, visibly reverting it until the second's own reload arrives a moment
later. Rare enough in a single-user drag-one-thing-at-a-time UI not to be worth a request-generation
guard yet — revisit if it shows up in practice.
