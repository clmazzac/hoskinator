# Tailoring decisions

Decisions behind scoring a resume against a job description, newest last. Repo-wide decisions live
in `CLAUDE.md`; architectural ones in `docs/adr/`.

## Deterministic match and AI assessment are separate engines, not one LLM call

`core::tailoring::match_report` scores a resume against a JD by weighted keyword overlap — pure
Rust, no LLM, no API key, no embeddings (out of scope for v1 per the PRD). Style/tone/relevance
judgment is a separate, later `ai`-crate call.

**Why:** HackerRank open-sourced `hiring-agent` in June 2026 — a single LLM call scoring a resume
0-120. Independent testing ran the same resume through it 100 times and got scores from 66 to 99;
subjective categories swung wildly while checklist-style ones held steady. A number framed as
objective (an ATS match) needs to be reproducible, which rules out a single LLM judgment call for
that part. Splitting the two lets each be honest about what it is: the match score is deterministic
and re-runs identically; the assessment is a model's judgment and is allowed to read as one.

This also matches how real ATS platforms work: `sunnypatell/ats-screener`'s research into six
enterprise platforms found most still filter on literal or stemmed keyword matching (Taleo, Lever),
not semantic ML — `srbhr/Resume-Matcher`, the most established open-source JD-matcher, likewise
keeps keyword extraction and its match score separate from its LLM-driven rewrite suggestions.

## The keyword matcher operates on the assembled `resume.yaml`, not the Master Store

`jd.match` reads the current branch's `resume.yaml` text (via `resume::read`) and matches it against
a stored JD's text. It does not read Bullets/Entries from the store directly.

**Why:** the match should answer "would this specific resume, as it will be sent, clear this JD's
keywords" — that's the assembled YAML, not the full store of material the user hasn't placed yet.

## No stemming/NLP dependency; a hand-rolled suffix strip instead

`stem()` strips a handful of common suffixes (`ing`, `ed`, `es`, `s`) rather than using a real
stemmer crate (Porter, Snowball) or an NLP toolkit.

**Why:** the tools this design leans on (Resume-Matcher, ats-screener) reach for `textacy`/spaCy or
TF-IDF because they start from unstructured, OCR'd-from-PDF text. We start from structured YAML we
wrote ourselves, so the parsing problem they solve doesn't exist here — a lightweight suffix strip
is enough to catch "managing" against "managed" without a new dependency.

## Sections/formatting are not scored

ats-screener's five dimensions include Formatting and Sections (parseability, section presence) —
real ATS concerns for a resume a human formatted freely and an ATS then has to parse from a PDF.
Hoskinator's resumes are rendercv YAML validated against rendercv's schema before they're ever
written, so a resume that exists at all already has valid structure and required sections. Scoring
that dimension here would be scoring something that's already guaranteed elsewhere.

## The panels are always visible, not feature-detected away

ADR-0005 says the Web UI hides AI affordances when the addon is absent/unkeyed. The tailoring panel
is a deliberate exception: both the deterministic Match panel and the AI Assessment panel are always
present in the resume editor, independently collapsible by the user. When the `ai` feature isn't
built or isn't keyed, the Assessment panel says so in place rather than disappearing — so the
product visibly gestures at what AI adds, rather than hiding it until someone stumbles onto a key.
The one-way `ai` → `core` dependency and the cargo feature flag (ADR-0005) are unchanged; only the
UI's *hide-when-absent* convention is overridden for this feature.

## `ai.assess`'s transport, config, and error shape

`hoskinator_ai::Transport` is a one-method trait (`complete(model, prompt) -> String`) rather than
modeling Anthropic's Messages API request/response shape at the trait boundary. `AnthropicTransport`
is the real `reqwest`-backed implementation; tests use a stub that returns a canned string and
records the prompt it was given, so `assess`'s orchestration (does the resume and JD both reach the
model, does a well-formed and a fenced reply both parse) is asserted without a network call, per the
PRD's testing decisions.

`Config::from_env` reads `ANTHROPIC_API_KEY` and an optional `HOSKINATOR_AI_ASSESS_MODEL` override,
defaulting to `claude-haiku-4-5-20251001` — cheap and fast fits a per-panel background call better
than a stronger model. `Config::from_env` returns `None` rather than erroring when no key is set;
`ai.assess` turns that into the dedicated `AI_UNCONFIGURED` JSON-RPC code the Assessment panel
matches on to show its "not configured" state, rather than a generic failure.

## The panel lives in a second, outer resizable split

The three-column editor (Slice 9) is one horizontal `ResizablePanelGroup`. The tailoring panel adds
a second, outer `ResizablePanelGroup` in the vertical orientation, wrapping that three-column group
as its top pane and the tailoring panel as its bottom pane — a top/bottom split around a left/right
one, each with its own saved layout size. Collapse follows the render panel's existing pattern
(`collapsible`, `collapsedSize={0}`, an expand button that appears in the freed space).

Within the panel, Match and Assessment are independent `Collapsible` sections rather than one
collapse toggle for the whole panel — "hide the thing I don't need right now" reads as a per-section
action (hide Assessment while unkeyed, keep Match), not an all-or-nothing one.

## Semantic matching lives in `ai.assess`, not `jd.match`, and only Anthropic

`ai.assess` now also takes `jd.match`'s missing keywords and asks the model which ones the resume
actually covers in different words, with the covering line as evidence. `jd.match`'s score itself is
untouched — still pure keyword overlap.

**Why not in `jd.match`:** that score exists to be reproducible; the same resume against the same JD
must always return the same number, which is the property an LLM call can't guarantee. HackerRank's
`hiring-agent` (see above) is the cautionary example — a single LLM call standing in for an objective
score, scoring the same input 66 to 99 across runs. Putting semantic judgment in `ai.assess` instead
keeps it where non-determinism is already expected and accepted, without touching the guarantee the
match score exists for.

**Why Anthropic over a second provider (e.g. OpenRouter):** ADR-0005 commits `ai` to one provider.
The `Transport` trait is provider-agnostic by construction, so nothing technical stops a second
backend, but "a tiny model" doesn't require one — `claude-haiku-4-5-20251001` (already
`DEFAULT_ASSESS_MODEL`) is that tier. Adding a second HTTP client and a second API key for a
capability the existing one already covers isn't worth the deviation from the ADR.

**Why re-check only the missing keywords, not the whole JD:** it's the smallest prompt that answers
the actual question ("is 'missing' really missing?"), reuses `jd.match`'s work instead of
re-deriving requirements from the JD text, and keeps the false-negative case — a keyword the
deterministic pass wrongly flagged as absent — as the one thing this call has to get right.
