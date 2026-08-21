---
name: pr-comments
description: Fetch unresolved review comments on a PR and work through them. Use when the maintainer says they have reviewed a PR, left comments, or asks you to address review feedback.
allowed-tools:
  - "Bash(gh *)"
  - "Bash(gt *)"
  - "Bash(git *)"
  - "Bash(cargo *)"
  - Read
  - Edit
  - Write
  - Grep
  - Glob
---

# Addressing PR Review Comments

Pull review feedback from GitHub and work through it. A human maintainer reviews every line (see
`CLAUDE.md`), so review comments are the primary channel for course corrections — treat them as
authoritative.

## Why not `gh pr view --comments`

It shows only the PR conversation timeline. It **misses line-anchored review comments entirely** and
has no notion of resolved vs unresolved, so you get stale feedback mixed with current. Resolution
state exists only in GraphQL. Use the query below.

## Step 1 — Resolve which PR

If given a number, use it. Otherwise infer from the current branch:

```bash
gh pr view --json number,title,headRefName -q '{number, title, headRefName}'
```

## Step 2 — Fetch the feedback

```bash
gh api graphql -F owner=clmazzac -F repo=hoskinator -F number=<PR> -f query='
query($owner:String!, $repo:String!, $number:Int!) {
  repository(owner:$owner, name:$repo) {
    pullRequest(number:$number) {
      reviewThreads(first:100) {
        nodes {
          id
          isResolved
          isOutdated
          path
          line
          comments(first:50) {
            nodes { author { login } body createdAt diffHunk }
          }
        }
      }
      reviews(first:50) { nodes { author { login } state body submittedAt } }
      comments(first:50) { nodes { author { login } body createdAt } }
    }
  }
}'
```

Three distinct sources, all of which matter:

- **`reviewThreads`** — line-anchored comments. Where most substantive feedback lives.
- **`reviews`** — the review body plus `state` (`APPROVED`, `CHANGES_REQUESTED`, `COMMENTED`).
- **`comments`** — PR-level conversation.

## Step 3 — Filter the noise

- **Skip `isResolved: true`** threads. Already handled; re-raising them wastes review time.
- **Skip Graphite's stack comment.** Graphite posts an auto-generated stack map on every PR *using
  the maintainer's own account*, so filtering by author will not catch it. Match on its body
  instead: it contains `This stack of pull requests is managed by` or the trailing
  `<!-- Current dependencies on/for this PR: -->` marker.
- **Note `isOutdated: true`** threads but do not silently skip them — the comment is anchored to a
  line that has since changed, so the concern may or may not still apply. Read it and decide.

## Step 4 — Restate before changing anything

List each item as `path:line — the ask`. If any comment is ambiguous, or if you think it is wrong,
**say so before editing**. A review comment is an instruction, but a mistaken instruction acted on
silently is worse than a question. Do not batch-apply blindly.

## Step 5 — Fix on the branch that owns the line

**This is the step that goes wrong in a stack.** A comment on PR #17 must be fixed on *that PR's*
branch, not on whatever branch is currently checked out. Fixing it at the top of the stack puts the
change in the wrong PR and leaves the reviewed one untouched.

```bash
gh pr view <PR> --json headRefName -q .headRefName   # which branch owns this PR
gt checkout <that-branch>
# ...make the edits...
git add <files>
gt modify                      # amend the existing commit
gt restack                     # rebase everything above it
```

Prefer `gt modify` over a new commit, so each PR stays one clean commit. Use `gt modify --into
<branch>` to send a fix down to a lower branch without checking it out.

After restacking, verify the whole stack still builds — a fix low in the stack can break a branch
above it:

```bash
gt top && cargo build && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

## Step 5b — When the parent PR merged while you were working

Expect this whenever the bottom of a stack gets merged mid-review. `gt submit` will refuse with one
of:

- `PR for the following branch has already been merged but the merged commits are not contained in
  the latest trunk branch main`
- `Branch <x> has been updated remotely outside of Graphite`

Two things happened at once: local `main` is stale, **and** Graphite server-side rebased the
children onto the new trunk, so their remote tips no longer match your local ones. Reconcile
explicitly rather than reaching for `gt get`, which pulls the remote version and can bury the
amendments you just made.

```bash
git checkout main && git pull --ff-only
gt checkout <lowest surviving branch>
gt track -p main                       # reparent, the old parent is merged and gone
gt restack
gt delete <merged branch> -f -q        # safe: its commits are in main now
```

Then verify the build at `gt top` again, since the branches were just rebased onto a different base.

### Before force-pushing, prove the remote holds no unique work

Submitting now requires `--force`, which discards the remote tip. That is only safe if the remote
has nothing of its own — someone may have pushed to your branch. Check, and **show the output**:

```bash
git log --oneline <branch>..origin/<branch>          # commits only on the remote
git log --format='%an %s' main..origin/<branch>      # who authored them
git diff --stat origin/<branch> <branch>             # content delta
```

Expected: the remote has the same single commit by the same author, and the diff is *exactly* the
review fixes you just made. Anything else — an unfamiliar author, an extra commit, a file you did
not touch — means stop and ask.

```bash
gt submit --no-interactive --force
```

**Never force-push without showing that check first.** "Graphite told me to" is not a justification;
the whole point is distinguishing a stale remote from someone else's work.

## Step 6 — Reply, but do not resolve

Reply on each thread saying what changed, so the PR carries the reasoning:

```bash
gh api graphql -f threadId='<thread id>' -f body='<what changed and why>' -f query='
mutation($threadId:ID!, $body:String!) {
  addPullRequestReviewThreadReply(input:{pullRequestReviewThreadId:$threadId, body:$body}) {
    comment { id url }
  }
}'
```

**Leave threads unresolved.** Resolving is the reviewer's signal that they are satisfied; closing
your own threads removes the maintainer's ability to see what still needs a second look.

If you disagreed with a comment and did not make the change, reply saying that explicitly rather
than staying silent.

## Step 7 — Push

```bash
gt submit --no-interactive
```

Then report per PR: what changed, what you pushed back on, and anything still open.
