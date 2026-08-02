# AI layer decisions

Decisions shaping `hoskinator-ai`, newest last. Repo-wide decisions live in `CLAUDE.md`;
architectural ones in `docs/adr/`.

## The crate exists before its contents (Slice 1, #2)

`crates/ai` is empty apart from a test that uses a `hoskinator-core` type. No transport, no scoring,
no gap analysis, no drafting.

**Why:** it makes an ADR-0005 claim that was only prose into something cargo enforces — the
dependency runs one way, `ai` → `core`, and never the reverse. It also means Slice 11 adds the
transport to a crate that already exists, rather than standing up the crate and inventing its API in
the same PR.

**Accepted cost:** a crate that ships nothing.

## The transport seam waits for its first consumer (Slice 1, #2)

Slice 1 (#2) lists a stubbed-transport seam among its four. It is deliberately **not** built.

**Why:** a `Transport` trait written now is an API invented before anything needs it. A first
attempt had one method returning `impl Future`, which looked reasonable and was still a guess —
whether Slice 11 needs streaming, token accounting, structured output, retries, or a model parameter
would each change the shape, and a trait already in `main` is one people preserve rather than
redesign.

**What this costs:** Slice 1 closes with three of its four seams. That is the deliberate choice.

**Where it belongs:** Slice 11 (#12), whose title already names it — *AI module + score (one-way
dep, feature flag, stubbed transport)*.
