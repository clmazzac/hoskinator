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

**Correction — batching is not among the costs jsonrpsee absorbs here (Slice 1, #2).** The rationale
above lists batching as a reason not to hand-roll, but `raw_json_request` does not do it:
it runs `serde_json::from_str::<Request>`, which rejects a batch array. A batch POST answers `400`.

Nor can it be recovered while axum owns the port. `jsonrpsee_server::transport::http::call_with_service`
*is* public and does handle batches, but it needs an `S: RpcServiceT`, and the only implementation
that drives an `RpcModule` — `jsonrpsee_server::middleware::rpc::RpcService` — has a `pub(crate)`
constructor and a `pub(crate)` config enum. Checked against jsonrpsee 0.26.0's sources.

So the choice is: no batching, hand-roll the array in the axum handler, or give up axum owning the
port. v1 takes the first — nothing batches today, and both the CLI and the web UI make single calls.
