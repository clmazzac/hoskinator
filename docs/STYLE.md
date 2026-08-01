# Style guide

Rules every change is reviewed against before it is submitted, enforced by `/style-review` and the
`gt submit` hook in `.claude/settings.json`.

Each rule below traces to specific review feedback. Nothing here is invented: a rule earns its place
by having been asked for at least once. See "Growing this guide" at the bottom.

---

## 1. Doc comments state the rule, not the reasoning

Say what the item does or what the rule is, then stop. Rationale — why it is that way, what it
guards against, what the alternative was — goes in the commit message, the PR description, an ADR,
or a decision log (`CLAUDE.md` for repo-wide, `docs/decisions/` per component).

```rust
// No
/// Unknown keys are rejected rather than ignored: in a file the user hand-edits, a typo that is
/// silently dropped looks exactly like a setting that does not work.

// Yes
/// Unknown keys are rejected rather than ignored.
```

Reasoning inline reads as padding and goes stale in the place least likely to be reread.

_Source: review of #20 — three doc comments trimmed, each dropping a "because" clause._

## 2. Do not document what does not exist

Describe the thing in front of the reader. Do not explain a module, crate, or feature that has not
been written yet, and do not describe a sibling's relationship to this code.

```rust
// No — in hoskinator-core, which has no `ai` crate to point at yet
//! The future `ai` crate depends on `hoskinator-core`, never the reverse, and routes every read
//! and every write through this crate's public API, so there is a single validated mutation path.

// Yes
//! Contains no LLM code and reads no API key — this crate is fully functional standalone
//! (ADR-0005).
```

Architecture that spans components belongs in an ADR, which is where someone looks for it and where
it stays correct.

_Source: review of #17 — "this is referencing a package that does not even live here."_

## 3. Do not declare what nothing verifies

Leave out metadata, configuration, and annotations that no tool checks and no consumer reads. An
unverified declaration is a claim that silently becomes false.

```toml
# No — nothing tests it, CI builds on stable which always satisfies it, and no external
# consumer reads it
rust-version = "1.96"
```

Applies beyond manifests: a comment asserting an invariant nothing enforces has the same problem.
If a claim matters, make something check it — a test, a lint, a type.

_Source: review of #17 — "Why does this file have rust-version pinned? Is that really necessary?"_

## 4. Do not restate what the name already says

A comment that repeats the identifier above it earns nothing. This bites hardest in test bodies,
where a descriptive test name has already stated what is being checked.

```rust
// No
#[test]
fn the_platform_default_is_an_absolute_hoskinator_directory() {
    // Guards the `ProjectDirs::from` arguments: adding a qualifier or organization would
    // silently relocate every user's data.
    let Some(dir) = platform_data_dir() else { return };

// Yes
#[test]
fn the_platform_default_is_an_absolute_hoskinator_directory() {
    let Some(dir) = platform_data_dir() else { return };
```

Unlike rule 1, this one applies to every comment, inline ones included. A comment earns its place by
saying something the code does not already say. If the name fails to convey the intent, fix the name
rather than annotating it.

_Source: review of #20 — comment deleted from a test whose name already stated the check._

---

## Growing this guide

When review feedback reveals a preference not written here, add it as a numbered rule with a
no/yes example and a `_Source:_` line naming the PR. Do not add rules speculatively — a guide full
of invented rules is the failure mode this exists to prevent.

When a rule turns out to be wrong, delete it. A style guide nobody agrees with gets ignored
entirely, which is worse than having no rule.
