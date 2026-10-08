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

Requires the pinned toolchain in `rust-toolchain.toml` (Rust 1.85 + rustfmt + clippy).

```bash
cargo build --workspace
cargo test --workspace
cargo run -p wisp
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for lint policy, `cargo-deny`, and CI smoke checks.

## Status

- **LUM-001** — workspace skeleton (current)
- **LUM-005** — GPUI hello window
- **LUM-003** — CI pipeline

## License

Licensed under MIT OR Apache-2.0. See [LICENSE](LICENSE).
