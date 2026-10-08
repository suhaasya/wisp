# Size and memory budgets

PRD guardrails for shipped artifacts and runtime footprint. **Automatic CI gating** is
[LUM-004](../README.md); this document is the source of truth for targets, caps, and baselines.

## Product budgets

| ID | Metric | Target | Hard cap | Notes |
| --- | --- | --- | --- | --- |
| B1 | Empty shell binary (stripped, `release`) | track trend | **5 MB** (M0 gate) | Current milestone: placeholder app without GPUI/drivers |
| B2 | Production binary (stripped, `dist`) | **18 MB** | **25 MB** | Full feature set at 1.0 |
| B3 | Compressed download (gzip of B2 artifact) | **9 MB** | **12 MB** | Distribution proxy used by `cargo xtask size` |
| B4 | Idle resident memory | **200 MB** | **350 MB** | Measured after window open, no active query |
| B5 | Peak resident memory (large result grid) | **512 MB** | **768 MB** | 100k-row grid scroll benchmark (later milestone) |

Memory baselines (B4–B5) are **not applicable** to the empty shell; record them when LUM-005+ land.

## Release settings (LUM-002)

Workspace `[profile.release]`: `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`,
`opt-level = "s"`.

Per-crate overrides: `wisp-ui` and `wisp-core` (including `render` / `grid` modules) use
`opt-level = 3`.

Shipping builds should use **`dist`** (`inherits = "release"`, `debug = false`, `incremental = false`).

## Baseline: empty shell (`release`, 2026-10-08)

Record with:

```bash
cargo install cargo-bloat --version 0.11.1 --locked   # Rust 1.85 toolchain
cargo xtask size
cargo xtask size --target <triple>   # when cross-toolchain is available
```

| Target triple | Stripped bytes | Stripped (MiB) | Gzip bytes | Gzip (MiB) | Measured on |
| --- | ---: | ---: | ---: | ---: | --- |
| `aarch64-apple-darwin` | 302 320 | 0.288 | 136 134 | 0.130 | macOS 15, Rust 1.85 |
| `x86_64-apple-darwin` | 283 520 | 0.270 | 144 665 | 0.138 | macOS 15 cross, Rust 1.85 |
| `x86_64-unknown-linux-gnu` | — | — | — | — | Run `cargo xtask size --target x86_64-unknown-linux-gnu` on Linux |
| `aarch64-unknown-linux-gnu` | — | — | — | — | Run `cargo xtask size --target aarch64-unknown-linux-gnu` on Linux |
| `x86_64-pc-windows-msvc` | — | — | — | — | Run `cargo xtask size --target x86_64-pc-windows-msvc` on Windows |

Update this table whenever the shell grows materially or when measuring on the remaining hosts
(LUM-003 CI should append Linux/Windows rows).

### Top crates (host `aarch64-apple-darwin`, `cargo bloat --crates -n 20`)

The empty shell is mostly `std`; Wisp workspace crates are below bloat’s resolution threshold.
Re-run after GPUI and database stacks land to populate crate-level rows.

## Policy

- Prefer **`default-features = false`** on dependencies; enable features explicitly per crate.
- Do **not** pass `-C target-cpu=…` for `release` / `dist` artifacts (local benchmarks only).
