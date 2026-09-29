# hoskinator
Resume management system using Git for version control. This is what I actively use (deployed on gcloud) to make my resumes for the 2027 SWE intern application season!

## Development

Prerequisites: a stable Rust toolchain (see `rust-toolchain.toml`), Node 22, and
[`just`](https://just.systems).

- `just dev` — runs the daemon (`:8737`) and the Vite dev server (`:5173`, proxying `/rpc` and
  `/preview.pdf` to the daemon) together. Ctrl-C stops both.
- `just build` — builds the web bundle, then the workspace.
- `just ci` — runs the same checks CI runs: web build, `cargo fmt --check`, `cargo clippy`,
  `cargo test`.

Run `just` with no recipe to list all of them.
