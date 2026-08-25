# Resume editor

The design of the screen where one resume gets assembled — Slice 9 (#10).

A human decided the layout below. This file records that decision and what it asks of the engine.
Design decisions live here; `docs/decisions/web.md` records how the code got built, and `PRODUCT.md`
holds product truth.

Nothing here is built. The two branches that exist are described at the bottom, and neither matches
this layout.

## The screen

Three columns. The left panel is split into two even halves; the right panel is the whole right-hand
side.

```
┌──────────────┬──────────────┬────────────────────────┐
│ MASTER STORE │ THIS RESUME  │    RENDERED RESUME     │
│              │              │                        │
│ Bullets you  │ ▾ Experience │   ┌────────────────┐   │
│ select to    │   • Built…   │   │  Name          │   │
│ use in this  │   • Cut…     │   │  ──────────    │   │
│ resume       │ ▸ Projects   │   │  EXPERIENCE    │   │
│              │ ▾ Education  │   │  Acme · SWE    │   │
│              │   • BS CS    │   │  • Built…      │   │
│              │              │   └────────────────┘   │
├──────────────┴──────────────┼────────────────────────┤
│    left panel (split)       │      right panel       │
└─────────────────────────────┴────────────────────────┘
```

**Left half — the Master Store.** The material you own, and the place you select from.

**Right half — this resume.** One collapsible category per part of the resume, each holding the
Bullets and elements in use for that part.

**Right panel — the rendered resume.** The real rendered document, not a structural approximation of
one.

## Settled

- The user is completing a task, not being persuaded or entertained. Density and speed win.
- Desktop and laptop only. Laptop width is the narrowest target.
- The `CONTEXT.md` vocabulary is UI copy, as written, including its `_Avoid_` lists.

## Design system

shadcn/ui on the Base UI primitives, over Tailwind v4.

`resume-builder-ui/tailwind-shadcn-init` (`01e7c53`) established it and holds the whole of it: the
Nova preset, Geist as the one typeface, an achromatic token set in `web/src/index.css` where every
colour is `oklch(L 0 0)` apart from `--destructive`, and eight vendored components — button, card,
checkbox, input, label, scroll-area, separator, and textarea.

That branch has no pull request yet, so the choice is incumbent rather than reviewed. Anything built
for this screen extends those tokens and components rather than introducing a second system beside
them.

## What the design asks of the engine

1. **The render is a subprocess, not a preview.** The right panel means running `rendercv render`
   against the repo, not drawing an approximation in React. It needs the repo and a working
   `rendercv`, so it also needs a state for each being absent. This is Slice 10 (#11) work pulled
   into Slice 9.

2. **The middle column is a structured editor.** `resume.read` and `resume.write` are raw text by
   decision (`docs/decisions/resume.md`), so a collapsible outline of sections and their elements
   needs a structured read alongside the raw one, and one patch operation per gesture. That decision
   log already accepts each new write shape as a new patch operation; this is the largest instance.

3. **Three columns at laptop width** is the constraint that decides the rest. Mock it at the
   narrowest real width before designing anything inside a column.

## Open decisions

Nobody has ruled on these. A builder must not invent an answer.

1. **Whether the store column shows what is already in use.** ADR-0001 keeps no cross-references
   between store and repo, so the only available match is exact text, and that match breaks the
   moment a wording is edited on the branch. A "used" marker that silently goes stale is worse than
   no marker.

2. **What drives the render.** An explicit button, automatic on change, or automatic as an option
   the user turns on. Automatic needs a debounce and a cancel, because every keystroke would
   otherwise start a process that outlives its own input.

3. **Whether this screen owns the commit.** The assembly loop ends in a commit. Whether the commit
   control and a dirty marker sit in this screen's chrome, or on a separate git page, is open.

4. **What the middle column's categories are called.** Two things are called "section": the store's
   curated `{name, entry_type}` vocabulary, shared by every resume, and the parts of one resume in
   one order. The middle column is where they collide, so they need different names on screen.

## Where the code stands

Two branches, stacked on `main`, neither with a pull request:

- `resume-builder-ui/tailwind-shadcn-init` — Tailwind v4, shadcn/ui on Base UI primitives, and eight
  vendored components. An achromatic token set in `web/src/index.css`. No app logic.
- `resume-builder-ui/two-panel-builder` — a two-column builder in
  `web/src/components/ResumeBuilder.tsx`, with the old JSON-RPC harness behind a disclosure.

The second branch reads as evidence, not as a starting point. It disagrees with this design on
layout, and with two decisions that predate it:

- **It regenerates the whole document on write.** `buildResumeYaml` stringifies `cv.sections` from
  scratch, so anything hand-authored in `resume.yaml` is lost on write. `docs/decisions/resume.md`
  chose format-preserving patches to prevent exactly that, and ADR-0002 treats the file as
  hand-authored too.
- **Rewording a Bullet writes to the Master Store.** `updateVariant` fires on blur, so tailoring a
  wording for one application rewrites the canonical wording for every resume. ADR-0001 and user
  story #29 in issue #1 both allow one write back into the store, and only as an explicit
  save-as-Variant.
