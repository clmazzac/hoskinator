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
