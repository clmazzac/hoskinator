# Transport decisions

Decisions shaping the daemon's HTTP and JSON-RPC surface, newest last. Repo-wide decisions live in
`CLAUDE.md`; architectural ones in `docs/adr/`.

## axum owns HTTP, jsonrpsee dispatches (Slice 1, #2)

`axum` owns the socket, static-file serving, and the no-op auth middleware. `jsonrpsee`'s
`RpcModule` is used **purely as a dispatcher** — an axum handler passes the request body to
`RpcModule::raw_json_request` and returns the JSON response.

**Why:** ADR-0003's premise is a stable JSON-RPC contract spoken by arbitrary frontends, so
protocol correctness (2.0 spec edge cases, batching, reserved error codes) is the wrong thing to
hand-roll. `#[rpc(server, client)]` also generates the client, which the CLI needs — hand-rolling
means writing both ends for every method across Slices 2–14. Using `raw_json_request` rather than
jsonrpsee's own server keeps axum in charge of the port, so co-hosting the embedded SPA stays trivial.

**Accepted costs:** jsonrpsee is pre-1.0 (0.26) and churns across versions; requests round-trip
through strings rather than typed values.
