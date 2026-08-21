# Resume editor

The design of the screen where one resume gets assembled. Cam decided everything below by hand
(`CLAUDE.md`); this file records those decisions and what each one asks of the engine. Design
decisions live here, not in `docs/decisions/web.md`, which records how the code got built.

Nothing here is built yet. The web UI on `main` is still the JSON-RPC harness.

## Design system: IBM Carbon

The Figma library is **(v11) Carbon Design System** from the Figma Community, which carries all four
Carbon themes — White and Gray 10 for light, Gray 90 and Gray 100 for dark. `@carbon/react` matches
that library component for component, so a component placed in Figma exists in code under the same
name.

Carbon's spacing is a fixed token scale, not free-form. A layout drawn off that scale does not
reproduce in the React components.

## The screen

**The first version is two panels**: the editor on the left, the store on the right. That is the
shape Slice 9 (#10) already specifies. The screen opens on a resume that has already been chosen
elsewhere.

Everything below marked deferred is decided but not scheduled. It is recorded so the decision does
not have to be made twice.

### The editor

Two views over one document:

- **Raw YAML** — the branch's `resume.yaml` as text.
- **Visual editor** — a dropdown per section, bullets inside it, drag to reorder, fields editable in
  place. Deferred: it needs a structured read and a patch operation per gesture, which the raw view
  does not.

### The store

Browse and full-text search of the Master Store. The user places a Bullet, Variant, or Entry into
the resume; the engine writes only what was placed.

Placing is an explicit one-way gesture, never a per-bullet toggle inside the editor. A toggle has to
know which store Bullets are already on the resume. ADR-0001 keeps no stored cross-references
between the store and the repo, so the only available match is exact text, and that match breaks the
moment the user rewords a bullet on the branch — which is the whole point of per-branch tailoring.
The same bullet would then appear twice: once as the edited line, once as an unplaced toggle.
Placing the same Bullet twice is allowed, and visible when it happens.

**Deferred: the bank.** A third column holding store material to pull from, which would move the
store out of the right panel and free that panel for the render. How it behaves — a rail, a drawer,
whether it filters to the Entry with focus — is unsettled. Two panels first.

### The render — deferred

When the render lands, it shows the real rendered document, with real spacing, not a structural
approximation. A Render button, plus auto-render on change as an option the user turns on.

It has no panel in the two-panel version, because the right panel holds the store.

## Where everything else lives

**Git is another page.** Branch, checkout, history, and diff sit outside the editor. The editor
still needs a commit control and a dirty marker, because the assembly loop ends in a commit.

**Two things are called "section".** Store Sections are the curated `{name, entry_type}` vocabulary,
shared by every resume, and belong with the store. A resume's sections are what one branch's YAML
holds and in what order, and belong in the editor. Give them different names in the UI.

**Profile is edited once, globally.** It is engine-owned boilerplate injected into every resume
header, so it probably does not belong in the editor at all.

**A Job Description is a lens, not an attribute.** A picker in the editor chrome, defaulting to
none. `CONTEXT.md` binds a Job Description to no branch, so nothing about the choice is stored in
the repo. If the editor should remember the last choice per branch, that is UI state in Home, and
worth writing down as a deliberate departure rather than letting it arrive as a feature.

## The first version has no AI

Version one ships with no AI affordances at all. Scoring and everything downstream of it arrive
after.

The JD picker set to none and an unkeyed AI produce the same screen, so that empty state gets
designed once and covers both.

## What the design asks of the engine

1. **The visual editor is its own slice.** `resume.read` and `resume.write` are raw text on purpose
   (`docs/decisions/resume.md`), so a drag-and-reorder editor needs a structured read alongside the
   raw one, and one patch operation per gesture. A gesture that re-serialises the document destroys
   the user's comments and formatting. The resume decisions already accept that each new kind of
   write is a new patch operation; this is the largest instance of that.
2. **The render is a subprocess, not a preview.** Rendering means running `rendercv render --design
   <path>` against the shared design file at the repo root (ADR-0006), so the render needs the repo,
   not just the document. Auto-render needs a debounce and a cancel, because every keystroke would
   otherwise start a process that outlives its own input.
3. **Column count at laptop width** is the layout constraint that decides the rest. Two panels fit.
   A third one, when the bank arrives, is where the layout breaks, so mock it at the narrowest
   target width before designing anything inside the panels.

## Open questions

- Where model selection lives — a settings screen, or the config file only. Deferred with the AI
  work.
- What the bank is, and what it does to the two-panel layout.

Settled since: the real render replaces the approximate preview, so Slice 10 (#11) keeps the PDF and
DOCX export and loses the preview.
