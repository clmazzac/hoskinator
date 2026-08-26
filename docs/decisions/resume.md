# Resume decisions

Decisions shaping how `hoskinator-core` reads and writes the per-branch `resume.yaml`. Repo-wide
decisions live in `CLAUDE.md`; architectural ones in `docs/adr/`.

## Writes go through `yamlpath` + `yamlpatch`, not a general editor (Slice 8, #9)

`resume.write` applies targeted, format-preserving patches — Profile fields into the `cv:` header,
and later an Entry or Bullet into a section — rather than parsing the whole document into a tree and
re-serialising it.

**Why not a general CST editor** (the YAML analogue of `toml_edit`, e.g. `yaml-edit`): the writes
this engine makes are a small, known set of targeted merges, not arbitrary tree edits a user might
make by hand. A patch-shaped API matches that shape directly, and carries less unverified-dependency
risk than a young, general-purpose editor.

**Accepted cost:** each new kind of write Slice 9 needs is a new patch operation, not free with the
tool. That is expected to stay cheap, because the write surface is still small.

## `resume.write` takes the full text, not just the Profile (Slice 8, #9)

`resume.write(text)` takes the whole `resume.yaml` document — hand-edited or not — and re-injects
the Profile into its `cv:` header on every write, rather than a Profile-only call that merges into
whatever is already on disk.

**Why:** `resume.read` returns raw text so a human can hand-edit it and write it back (see below); a
write path that only accepted Profile fields would leave no way to save that edit. `CONTEXT.md`
already frames the Profile header as engine-owned boilerplate on every resume YAML the engine
writes, so re-injecting it on every write is what keeps the header from drifting out of sync with
the store.

**Accepted cost:** the injection is a `yamlpatch::Op::MergeInto`, which only adds and replaces keys
— clearing a Profile field that was previously set does not remove it from an existing resume.yaml.
Worth a follow-up once Slice 9's write shapes are known.

**A side effect:** because the caller now supplies the full text, `resume.yaml` no longer has to
exist first — a blank write seeds a bare `cv:` key and bootstraps the file.

## `resume.read` returns raw YAML text (Slice 8, #9)

Not a parsed JSON projection of the `cv:` structure.

**Why:** the file is hand-authored as well as tool-written (ADR-0002), and format-preserving writes
only make sense against the same text a human would edit and read back. A JSON projection would ask
the frontend to reconstruct formatting it never saw.

## The rendercv JSON Schema is embedded in `hoskinator-core`, not test-only (Slice 8, #9)

The vendored schema moves from `crates/core/tests/fixtures/` into `crates/core/src/`, `include_str!`
'd at runtime so `resume.write` can validate before it touches disk. `jsonschema` moves from a
dev-dependency to a real one.

**Drift detection is unchanged:** the existing `#[ignore]` test that checks the vendored schema
against `rendercv --version` still owns catching a version bump. Re-vendoring is a manual step.

## `yamlpatch` is pinned to a pre-release (Slice 9, #10)

`yamlpatch` and `yamlpath` move from `1.29.0` to `1.30.0-rc1`.

**The bug:** on 1.29, `resume.write` failed on its own output. Merging a sequence-valued Profile
field over a key that already held one emitted text that no longer parsed, so the first write to any
resume poisoned it and every later write returned `ResumeError::Patch`. `read` then `write` was not
a round trip. `social_networks` is the field that triggers it in practice, because it is the only
Profile field that is always a sequence of mappings.

**Why a pre-release rather than a workaround:** removing the key before the merge also fixes it and
needs no dependency change, but it moves the field to the end of the `cv:` header and re-emits it in
flow style. That defeats the reason this file chose `yamlpath` and `yamlpatch` in the first place.
A fix that is not format-preserving contradicts the decision it implements. The upstream fix leaves
the field where the user put it, in the style the user wrote it.

**Accepted cost:** a pre-release can change its API or be yanked. `1.29.0` is the newest stable and
does not carry the fix, so the alternatives were this or a workaround that damages the file. Move to
`1.30.0` when it ships.

**Guarded by** `writing_twice_keeps_a_sequence_valued_profile_field_intact`, which writes, reads
back, and writes again — the sequence the bug needed.
