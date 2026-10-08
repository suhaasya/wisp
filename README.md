# Wisp

Wisp is a database client (PostgreSQL, MySQL, and more). This repository is a Rust Cargo
workspace split into small crates for compile time and feature gating.

## Workspace layout

| Crate | Role |
| --- | --- |
| `wisp` (`app/`) | Application binary |
| `wisp-ui` | GPUI widgets and views |
| `wisp-core` | Sessions, pager, query runner |
| `wisp-drivers` | Database drivers |
| `wisp-transport` | TLS, SSH, and connection transport |
| `wisp-store` | Config, secrets, history |

Dependency flow: `app → ui → core → drivers → transport`, with `store` used from `core` and
`app` only.

## Getting started

Requires the pinned toolchain in `rust-toolchain.toml` (Rust 1.99 + rustfmt + clippy; GPUI/Zed).

```bash
cargo build --workspace
cargo test --workspace
cargo run -p wisp
```

Component gallery (debug / PR visual review, not shipped in release builds):

```bash
cargo run -p wisp --features ui-gallery
```

Then use **Gallery** in the title bar.

See [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/ci.md](docs/ci.md) for CI and local checks.

Release/dist profiles and size budgets: [docs/budgets.md](docs/budgets.md). Measure with
`cargo xtask size` (requires [cargo-bloat](https://github.com/RazrFalcon/cargo-bloat)).

## Status

- **LUM-001** — workspace skeleton
- **LUM-002** — release/dist profiles and size baseline
- **LUM-003** — CI pipeline (lint)
- **LUM-004** — budget gates in CI
- **LUM-005** — GPUI application shell
- **LUM-006** — theme system and design tokens
- **LUM-007** — internal UI component kit (current)

## License

Licensed under MIT OR Apache-2.0. See [LICENSE](LICENSE).
