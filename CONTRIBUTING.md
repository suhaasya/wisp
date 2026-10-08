# Contributing to Wisp

Thanks for helping build Wisp. This repo is a Cargo workspace; read the crate-level doc
comments in each `lib.rs` before adding dependencies so the layer graph stays one-way.

## Crate layers

```text
app → ui → core → drivers → transport
         ↘ store ↗        (store also used by app)
```

- **transport** — TLS, SSH, raw sockets.
- **drivers** — PostgreSQL, MySQL, etc.
- **store** — config, secrets, history (no other Wisp crates).
- **core** — sessions, pager, runner.
- **ui** — GPUI widgets (LUM-005).
- **app** — binary entry point.

Do not add reverse or skip-layer dependencies without an architecture review.

## Toolchain

`rust-toolchain.toml` pins the Rust version. Install components with:

```bash
rustup show active-toolchain
```

## Local checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo build --workspace
cargo test --workspace
```

Optional (install [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)):

```bash
cargo deny check
```

## CI-style smoke (warnings denied)

Use the same flags CI will use (LUM-003):

```bash
./scripts/ci-smoke.sh
```

That sets `RUSTFLAGS="-D warnings"` for build, clippy, and test.

## Windows / Linux / macOS

Run `./scripts/ci-smoke.sh` (or the individual `cargo` commands above) on each OS before
opening a PR that touches build or platform code.
