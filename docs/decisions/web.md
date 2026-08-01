# Web UI decisions

Decisions shaping the web UI in `web/`, newest last. Repo-wide decisions live in `CLAUDE.md`;
architectural ones in `docs/adr/`.

## React + Vite + TypeScript (Slice 1, #2)

**Why:** chosen for Slice 9's two-panel assembly screen, not Slice 1's form — it has the deepest
ecosystem for drag-and-drop, virtualized lists, and an embeddable YAML/code editor. Bundle size is
irrelevant for a localhost-served single-user tool.

**Prerequisite:** Node must be installed **inside WSL**. `npm` currently resolves to the Windows
install (`/mnt/c/Program Files/nodejs`), which breaks Vite builds run from WSL.
