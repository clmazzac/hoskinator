# Workspace decisions

Decisions shaping how resumes are organised across branches, how the repository is set up, and how
applications are tracked. Repo-wide decisions live in `CLAUDE.md`; architectural ones in
`docs/adr/`.

## Branch names carry the hierarchy, under two prefixes (Slice 9, #10)

`archetype/<slug>` is a master resume for a kind of role. `apply/<slug>/<target>` is a resume
tailored from that archetype for one application. `main` is the trunk both descend from.

**Why two prefixes rather than one nest.** A git ref is a file. `archetype/systems` and
`archetype/systems/acme` cannot both exist, because the first is a file where the second needs a
directory. Nesting a child under its parent's name is not a naming preference that can be argued
either way — it is a repository that refuses to write. `lineage.rs` holds the convention and a test
pins it.

**What this buys.** A branch's parent is read from its own name, so the screen draws the tree
without storing anything beside the repository. The repo stays vanilla (ADR-0001, ADR-0002): the
names mean nothing to git, and a clone without Hoskinator still checks out every branch.

**Accepted cost:** one archetype cannot descend from another. A two-level tree covers the workflow
this replaces — an archetype per kind of role, a resume per application — and a third level would
need the prefixes again.

## Wording moves by merge, in both directions (Slice 9, #10)

`repository.merge` fast-forwards where it can, and refuses a merge that would leave conflicts
without writing anything. A half-merged `resume.yaml` is worse than no merge.

A wording worth keeping for a whole kind of role merges up into the archetype. An archetype's
changes merge down into the resumes drawn from it. A resume that never merges up keeps its wording
to itself, which is the ordinary case: most tailoring is for one application and belongs nowhere
else.

**Why merge rather than a copy operation of our own:** the repository is already the record of what
each resume holds. A second mechanism beside it would have to be kept honest against git, and git
would win.

## GitHub is reached through the `gh` CLI (Slice 9, #10)

`workspace.create_github` and `workspace.connect` shell out to `gh`. There is no OAuth flow, no
client secret, and no token in this process or on disk beside the store.

**Why:** the user is already signed in to `gh`, and creating a private repository is one documented
command. An OAuth flow of our own would mean registering an application, holding a secret in a
binary anyone can read, and storing a token — for a tool that binds to loopback and serves one
person.

**Accepted cost:** `gh` must be installed and signed in. `workspace.status` reports both, so the
screen offers what is actually available and names the command that fixes what is not.

**Correction:** a later slice added a second path — a stored personal access token, used only for
push credentials — alongside this one. It duplicated the account the screen showed, and its
"create a repository" action pushed the whole current worktree into the new, empty repository,
because it repointed `origin` there instead of cloning fresh. Removed: `gh` already configures its
own git credential helper on `gh auth login`, so a plain push authenticates without it.

## Applications are tracked in the store, and imported by pasting (Slice 9, #10)

An application holds company, position, status, date, listing, notes, and the branch its resume came
from. That branch is the only link between a posting and a version of the resume, because the
repository holds no reference back (ADR-0001).

**Why no Google Sheets API.** The workflow this replaces is a sheet, and the sheet is already
copied and pasted. Sheets copies as tab-separated rows and exports as CSV, so an importer that reads
both covers it with no OAuth, no API key, and no agreement about column order — headings are matched
by name. Export writes the same shape back, so the sheet stays usable beside the tool.

**Status is the one chromatic thing in the app.** Four statuses read at a glance are categorical
data, and a grey ramp cannot separate four categories the way hue can. Everything else stays
achromatic.

**Scoped to the repository, not to git.** Every application carries the `owner/name` of the
GitHub repository it was tracked against (`workspace::repository_slug`, read off the `origin`
remote), and every read and write is filtered by it. A repository holds no store of its own
(ADR-0001), so the alternative — one global list — meant starting a second repository showed the
first one's applications, and an application never really belongs to a branch (a company might see
several resumes before one is sent). The GitHub identity is the scope key rather than the local
checkout's path, because the path can change — a fresh clone, a different machine — while the
repository on GitHub does not. Applications are not written into the repository itself; the CSV
export is a manual, one-shot action, not something `Save & push` carries along automatically.
