# Hoskinator

A git-native resume management system. Its core job is to manage all your resumes: a global pool of everything you've done (the Master Store) feeds tailored, per-application resumes, each a rendercv YAML on its own git branch. A light, optional AI layer scores your material against a job description, flags gaps, and drafts tailored wordings — but the base system is fully useful with the AI turned off.

## Language

**Master Store**:
The global, specialization-independent pool of every fact and accomplishment statement a user has accumulated — the "everything-CV." Authoritative for raw facts (orgs, dates, projects) and reusable raw material. _Not_ the source of truth for tailored wording.
_Avoid_: source of truth (it is the source of truth only for facts, not wording), database.

**Home**:
The single directory holding Hoskinator's own data — the Master Store database. Resolved from `HOSKINATOR_HOME`, then the `home` key in the config file, then the platform data directory (`~/.local/share/hoskinator` on Linux); never from the current working directory, because Hoskinator is not a per-directory tool the way `git` is. The rendercv repo is _not_ inside Home: it is the user's own artifact, located separately so they can keep it wherever they like and push it to their own remote.
_Avoid_: workspace (ambiguous with the Cargo sense — the repo is itself a Cargo workspace), data dir, root.

**Entry**:
A single record in the Master Store — a job, project, degree, publication, award, skill line, etc. Each Entry has a **type** mirroring rendercv's entry types (experience, education, normal, publication, one-line, bullet, numbered, reversed-numbered, text), which determines its fields and whether it carries bullets. Also carries section eligibility and (where applicable) bullets. (Tagging is a deferred post-v1 concept.)

**Braindump**:
Free-write notes on an Entry — unstructured detail about a job or project, kept in the Master Store
as a plain text column, never in `fields` and never in a `resume.yaml`. The raw material AI bullet
suggestions draw from.
_Avoid_: note (that name is taken by `Variant.note`).

**Section**:
A managed record in the Master Store — `{ name, entry_type }` — corresponding directly to a rendercv section (a list of entries of one type). Names are user-defined (e.g. "Experience", "Selected Projects", "Open Source"); there is no fixed section enum. An Entry is eligible for the sections whose `entry_type` matches its type, and on any one resume appears under exactly one section. Curated records, not ad-hoc strings, so the section vocabulary cannot drift.
_Avoid_: category.

**Bullet**:
An accomplishment identity within an Entry — *the thing you did*, not a fixed string. It owns a set of Variants (wordings) with one marked default. Belongs to an Entry whose type carries highlights. There is no `base_text`; the base wording is simply the default Variant.

**Resume**:
A tailored artifact for one specialization or application, stored as a rendercv YAML on a git branch. The primary database of the system: authoritative for which bullets are selected and exactly how they are worded in that context.

**Variant**:
One wording of a Bullet, held in the Master Store. A Bullet owns one or more Variants with one marked default; further Variants accrue when you save a good wording so it is "on tap" for future applications. A Variant may carry a free-text `note` ("punchy, LLVM-forward") but never a branch/application reference — the store knows nothing of branches.
_Avoid_: version (reserved for git history — see below).

**Profile**:
The singleton record held in the Master Store mirroring rendercv's `cv:` header: `name`, `headline`, `location`, `photo`, `email`, `phone`, `website`, `social_networks`, and `custom_connections` — everything under `cv:` except `sections`. The engine injects it into the header of every resume YAML it writes, so the header block is engine-owned boilerplate, not hand-authored tailoring. Every field is optional, because rendercv requires none of them; a user may omit even their name. Presentation of the header (which connections appear, and in what order) is _not_ Profile — that lives in `design:`, which the user owns (ADR-0006).
_Avoid_: contact info (it carries more than contacts), header (that is the rendered output, not the record).

**Job Description (JD)**:
The pasted text of a role posting. Stored as a standalone record in the Master Store (searchable, reusable across applications), not bound to any branch. The input the tool scores and generates bullets against.

**Version history**:
Git itself. Per-branch wording differences are emergent from git, not a stored data structure; a "version" of a bullet is simply its text on a given branch. There is no separate versions array or `promoted` flag. Reusing a wording across branches is plain manual git (cherry-pick/merge); the tool models no branch hierarchy and offers no promotion feature.
_Avoid_: version array, version records, promotion.
