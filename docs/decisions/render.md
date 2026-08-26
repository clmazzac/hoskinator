# Render decisions

Decisions shaping how `hoskinator-core` renders a resume through rendercv, newest last. Repo-wide
decisions live in `CLAUDE.md`; architectural ones in `docs/adr/`.

## The caller names the output directory and file (Slice 10, #11)

`render.run(directory, file_name)` writes one PDF where it is told. A relative `directory` resolves
against the resume repository, the way rendercv resolves its own output paths. `.pdf` is appended
unless the name already ends in it.

**Why no default location:** where a rendered resume lands is a product decision, and nobody has
made it yet. Home holds only the store (`docs/decisions/home-and-config.md`), and writing into the
user's repository would dirty `repository.status` on every render. A default is additive once
someone picks one.

## Rendering passes no `--design` (Slice 10, #11)

The command is `rendercv render <repo>/resume.yaml`. The `design:` block inside that file is the
whole design input, so a branch renders from its own YAML alone (ADR-0006, as amended). The original
ADR required a `--design` flag; the amendment removed the file it pointed at.

## Feature detection is a method, not an error (Slice 10, #11)

`render.available` answers whether rendercv can be run. A frontend asks once and hides the button,
rather than rendering first and reading a failure to find out (ADR-0005). `render.run` still answers
`RENDER_PROGRAM_MISSING` if the tool goes missing between the two calls.

**Accepted cost:** the probe runs `rendercv --version`, which starts a Python interpreter. It takes a
few hundred milliseconds and nothing caches the answer.

## Failures carry both output streams (Slice 10, #11)

A failed render reports whatever rendercv printed, on stdout and stderr both.

**Why not stderr alone:** the failure that matters most — a `resume.yaml` rendercv rejects — prints
its table of validation errors on stdout and says nothing on stderr. Checked against rendercv 2.8.

## The Typst intermediate goes to a scratch directory (Slice 10, #11)

rendercv cannot produce a PDF without also writing a Typst file: `--dont-generate-typst` disables the
PDF as well. Each render therefore points `--output-folder` at a temporary directory and `--pdf-path`
at the file the caller asked for, so nothing but the PDF outlives the call.

**Consequence:** `tempfile` moves from a dev-dependency of `hoskinator-core` to an ordinary one. It
was already in the build graph, so this adds no crate.

## DOCX goes through rendercv's Markdown, not its HTML (Slice 10, #11)

`render.docx` asks rendercv for Markdown (`--markdown-path`, with `-notyp -nohtml -nopng` so nothing
else is generated) and pipes it into `pandoc -o <path>`. rendercv's own HTML output was the other
candidate; it renders through `github-markdown-css` and looks nothing like the themed PDF, so
converting through it would add nothing over Markdown.

**Availability needs both tools.** `render.available_docx` answers `true` only when both rendercv and
pandoc are on PATH. A frontend that asks once and hides the button never needs to know which of the
two is missing; `render.docx` still answers a specific `RENDER_PANDOC_MISSING` or
`RENDER_PROGRAM_MISSING` if either vanishes between the two calls.

**Skipping Typst is a speed-up, not just a scope cut.** Asking rendercv for Markdown alone measured
under 10ms locally, against over a second for a PDF — DOCX export never pays for typesetting it then
throws away.

## The live preview renders for real, on a cooldown, not a separate approximation (Slice 10, #11)

The Web UI's preview is `render.preview` itself — a real `rendercv` shell-out — now triggered
automatically after every resume edit (`resume.place`, `.remove`, `.move`, …), not only on a theme
change or a manual click. A trailing throttle holds it to one render per 2-second window: an edit
inside a cooldown extends it rather than queuing another render behind one already running.

**This replaces the "no external dependency" acceptance criterion rather than satisfying it.**
Scoping a client-side approximation of rendercv's classic theme — built from its design tokens and
layout templates, not its (visually unrelated) HTML output — found it tractable. The real thing won
once it proved fast enough. Measured end to end (`render.preview` over the RPC, then again through a
live click in the browser): a cold first render takes about 3.3s (starting rendercv's Python
interpreter); every render after that lands at 1.2–1.7s regardless of resume length, since the cost
is the interpreter start, not the content.

**Why 2 seconds:** comfortably above the measured steady-state latency, so a render has time to
finish before the next edit's cooldown would fire an overlapping request at the same output path.

**Accepted cost:** the preview lags roughly a cooldown-and-a-render behind the last keystroke, rather
than updating live as you type. Given the tools available, a preview through rendercv can't be both
real and instantaneous — only one of those was worth keeping.
