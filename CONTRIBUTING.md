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

CI runs the same steps via **xtask** (see [docs/ci.md](docs/ci.md)):

```bash
cargo xtask ci smoke
```

Individual commands:

```bash
cargo xtask ci fmt
cargo xtask ci clippy
cargo xtask ci test
cargo xtask ci deny    # cargo install cargo-deny --version 0.20.2 --locked
cargo xtask ci audit   # cargo install cargo-audit --version 0.22.2 --locked
```

Release size baseline ([docs/budgets.md](docs/budgets.md)):

```bash
cargo install cargo-bloat --version 0.11.1 --locked
# cargo-llvm-lines needs Rust ≥ 1.89; optional until toolchain bump:
# cargo install cargo-llvm-lines
cargo xtask size
cargo xtask size --profile dist
```

When adding third-party crates, prefer `default-features = false` and enable only what Wisp needs.
Do not set `-C target-cpu=…` for shipped release/dist builds (local benchmarks only).

## CI

GitHub Actions builds all five shipping targets on every PR. `./scripts/ci-smoke.sh` wraps
`cargo xtask ci smoke`. Repo admins enable branch protection with `./scripts/configure-branch-protection.sh`.
