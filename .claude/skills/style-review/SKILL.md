---
name: style-review
description: Review the current branch's diff line by line against docs/STYLE.md, apply fixes, and record the review so gt submit is unblocked. Use before submitting any PR, or when the submit hook reports that no style review has been recorded.
allowed-tools:
  - "Bash(git *)"
  - "Bash(gt *)"
  - "Bash(cargo *)"
  - Read
  - Edit
  - Write
  - Grep
  - Glob
---

# Style Review

Required before every `gt submit`. A `PreToolUse` hook blocks submitting until this has run against
the exact commit being pushed.

## Step 1 — Read the guide

```bash
cat docs/STYLE.md
```

Read it every time. Rules get added and removed; a remembered version is a stale one.

## Step 2 — Get the diff

Our convention is one commit per PR, so this is normally:

```bash
git show HEAD
```

If the branch has several commits, diff against its parent instead — `gt ls` shows which branch that
is. Review only what *this* PR adds; code from lower PRs in the stack was reviewed on its own.

## Step 3 — Walk it line by line

For each rule in `docs/STYLE.md`, check every added or modified line. Read the whole hunk, not the
`+` lines alone — a doc comment can violate rule 1 because of the line above it.

Check the non-code files too — a comment in TOML, YAML, or a shell script can break a rule as easily
as Rust can.

Rule 1 governs **doc** comments: `///`, `//!`, and file-header blocks — anything describing an
interface to its reader. An inline comment inside a function body, explaining a non-obvious guard to
whoever edits that line next, is not a doc comment and is not held to it.

## Step 4 — Apply fixes, and keep a list

Fix each violation directly and record `path:line — rule N — what changed`. The list goes in the
final report; the PR the maintainer reviews should already be clean.

**When a rule seems wrong, do not silently comply.** If following it would make the code worse, or
the rule does not fit the case, leave the code alone and raise it in the report. A style guide that
gets applied where it does not belong is worse than one that gets questioned.

If the change is code rather than comments, re-verify:

```bash
cargo build && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features
```

## Step 5 — Amend, then record

Fixes have to land in the commit being reviewed:

```bash
git add <files>
gt modify --no-edit
gt restack            # only if branches sit above this one
```

**Record the marker last, after the final amend.** It stores the reviewed commit SHA, and amending
changes that SHA — recording it first leaves it stale and the hook blocks again:

```bash
git rev-parse HEAD > "$(git rev-parse --show-toplevel)/.git/style-review-marker"
```

**Run that as its own command.** The hook denies the entire Bash invocation, so combining it with the
submit — `git rev-parse HEAD > ... && gt submit` — is blocked before either half runs, and the marker
never gets written.

The marker lives inside `.git/`, so it is never committed and never shared between clones.

## Step 6 — Report, then submit

State what was fixed, grouped by rule, and anything you pushed back on. Then `gt submit`.

## Growing the guide

When the maintainer's review reveals a preference not in `docs/STYLE.md`, add it as a numbered rule
with a no/yes example and a `_Source:_` line naming the PR — the format is documented at the bottom
of the guide. Only add rules that trace to real feedback; inventing them is the failure this whole
mechanism exists to prevent.
