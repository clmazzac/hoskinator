# Runs `just --list` when invoked with no recipe.
default:
    @just --list

# Runs the daemon and the Vite dev server together; Ctrl-C stops both.
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    trap 'kill 0' EXIT
    cargo run -p hoskinator --features ai -- serve &
    npm --prefix web run dev &
    wait

# Builds the web bundle first — the daemon's rust-embed step needs it in place already.
build:
    npm --prefix web run build
    cargo build --workspace

# Formats Rust code in place.
fmt:
    cargo fmt --all

# Checks Rust formatting without changing anything (what CI runs).
fmt-check:
    cargo fmt --all --check

# Lints every feature combination CI covers.
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Runs every test CI covers.
test:
    cargo test --all-features

# Runs the full CI sequence locally: web build, format check, lint, test.
ci: build fmt-check clippy test
