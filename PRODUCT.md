# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

One user: the maintainer, applying to many roles. Every session starts from a specific job posting
and ends with a resume tailored to it.

The tool is not built for anyone else. It assumes fluency with git, YAML, and rendercv, and it
assumes the user already knows the vocabulary below. No onboarding path, no guided first run, and no
second audience.

## Product Purpose

Hoskinator manages every resume the user owns. A global pool of their history — the Master Store —
feeds tailored per-application resumes, each a rendercv `resume.yaml` on its own git branch.

It exists because tailoring by hand scatters near-duplicate files and forgets why a bullet was
worded a given way. Success is that the user never rewrites the same accomplishment from scratch,
and that their strongest phrasing of it is on tap.

## Positioning

The resume repo is a vanilla rendercv git repo. It carries no bullet IDs, no sidecars, and no
tool-specific files, so anyone can clone, branch, diff, and render it without knowing Hoskinator
exists (ADR-0001, ADR-0002).

The two layers share no stored cross-references. Data moves one way: store → tool or human →
resume. The one write back into the store is an explicit "save this wording as a Variant."

The AI layer is severable. The base product manages resumes with no API key and no LLM code
(ADR-0005).

## Operating Context

- The engine runs as a single binary that is daemon, CLI, and web host at once (ADR-0005). It binds
  to loopback.
- Home holds the Master Store and is resolved from `HOSKINATOR_HOME`, then the config file, then the
  platform data directory. Never from the working directory.
- The rendercv repo lives outside Home. It is the user's own artifact, kept wherever they like and
  pushed to their own remote.
- One git branch is one specialisation — a role archetype or a single application.
- `rendercv` renders the YAML. The assembly loop ends in a commit.
- A second device reaches the daemon over the user's own tunnel.

## Capabilities and Constraints

**Confirmed:**

- rendercv is the canonical and only resume representation (ADR-0002). The store mirrors its nine
  entry types.
- Sections are curated `{name, entry_type}` records. An Entry is eligible for the sections whose
  type matches its own, and appears under exactly one section per resume.
- A Bullet is an accomplishment, not a string. It owns Variants, one marked default.
- Full-text search covers Entries and Variants. Job Descriptions are standalone searchable records
  bound to no branch.
- The engine mediates git via libgit2 (ADR-0004) and serves one JSON-RPC contract over HTTP
  (ADR-0003). Frontends are HTTP clients.
- Generated YAML is validated against rendercv's embedded schema before it touches disk.
- `resume.read` returns raw YAML text and `resume.write` takes the full document, because the file
  is hand-authored as well as tool-written.
- The engine writes only what the user explicitly places. No reconciliation, no de-duplication, no
  merge.
- The domain vocabulary in `CONTEXT.md` surfaces on screen as written: Master Store, Entry, Section,
  Bullet, Variant, Profile.
- Desktop and laptop only. Laptop width is the narrowest target. No phone or tablet layout.
- An auth seam exists and is a no-op.

**Undecided:**

- The placement gesture. Whether the user places material into a resume as a one-way action, or
  toggles it per item, is open. A toggle has to know what is already on the resume, and ADR-0001
  leaves no key to match on.
- Which panel holds which layer, and what each panel is.

## Brand Commitments

The name is Hoskinator.

`CONTEXT.md` binds terminology in both directions. Each term carries an `_Avoid_` list, and those
alternatives are barred from UI copy as much as from code: the Master Store is not a "database", Home
is not a "workspace", a Variant is not a "version", a Section is not a "category".

## Evidence on Hand

- `CONTEXT.md` — the domain glossary, authoritative for vocabulary.
- Issue #1 — the PRD, with 39 user stories.
- `docs/adr/0001`–`0007` — architecture decisions with lasting consequences.
- `docs/decisions/` — per-component decision logs, including `web.md` for the existing UI.

No user research, no usage data, no testimonials, and no second user exist. Future work must not
invent them. The store ships empty; there is no seeded material to design against.

## Product Principles

1. **The repo stays vanilla.** Nothing tool-specific lands in the user's resume repo, ever.
2. **One way out, one gesture back.** Material flows store → resume. It returns only when the user
   explicitly saves a wording as a Variant.
3. **The base product owes nothing to the AI.** Every core workflow works with the AI absent and no
   key configured.
4. **The vocabulary is the interface.** `CONTEXT.md` terms are UI copy, not internal shorthand.
5. **Built for one expert.** Density and speed beat discoverability. There is no novice to protect.
