# Rust engine with polyglot frontends over a JSON-RPC contract

The core engine is written in **Rust**. This is a deliberate skills/portfolio investment — Rust is overkill for an I/O-bound, single-user tool, and that cost is accepted knowingly. The engine is **headless**: frontends (a user-facing CLI, an MCP server, a TUI, …) may each be written in any language and drive the engine through a single **JSON-RPC contract**.

The v1 transport is **JSON-RPC over HTTP, served by a persistent local daemon** (the engine runs continuously and owns the single SQLite connection). Every frontend is an HTTP client: local ones hit `localhost`, the MCP frontend bridges MCP-stdio ↔ HTTP, a future web UI hits the same endpoint. There is no stdio/subprocess path — one transport covers local and remote.

**Multi-device is a v1 goal**, achieved without replicating data: the daemon binds `localhost`, and a remote frontend reaches it over the **user's own tunnel** (SSH forward / Tailscale). The tool ships **no TLS, no public exposure, and no auth** in v1 — but the daemon includes an **auth seam** (a no-op authenticator middleware) so auth is a later drop-in, not a rearchitect. Because JSON-RPC is transport-agnostic, none of this touches the method contract; data replication via **Turso cloud** and a hardened hosted deployment come later.

## Considered options

- **Python** was the natural fit: it could import rendercv's own Pydantic models, making every generated YAML schema-valid by construction (see ADR-0002). Rejected for the resume-value driver above.
- **FFI / native bindings** (PyO3, napi-rs) rejected — they require a binding per frontend language and fight the "frontends in any language" goal. Subprocess/JSON needs zero binding work.

## Consequences

- rendercv's entry-type schema is **hand-encoded as serde structs** and must be kept in sync with rendercv. Validate generated YAML against rendercv's emitted **JSON Schema** (or a render smoke-test) rather than trusting the structs.
- No official Anthropic Rust SDK; the engine calls the REST API directly (e.g. via `reqwest`).
- The engine is a **long-running daemon**, not a one-shot process — it needs lifecycle management (start/stop, single-writer ownership of the SQLite file).
