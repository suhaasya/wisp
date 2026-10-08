# Size and memory budgets

PRD guardrails for shipped artifacts and runtime footprint. **Automatic CI gating** runs in
[`.github/workflows/budgets.yml`](../.github/workflows/budgets.yml) (LUM-004).

## Deal-breaker table (CI enforced)

| Metric | Target (warn) | Hard cap (fail) | CI scenario |
| --- | --- | --- | --- |
| Stripped binary (`dist`) | 18 MB | **25 MB** | size gate |
| Gzip download proxy | 9 MB | **12 MB** | size gate |
| Idle RSS | 45 MB | **60 MB** | `idle` (implemented) |
| + per open connection | — | **+6 MB** each | `connections_10` (skipped) |
| 1M-row scroll RSS | — | **220 MB** | `scroll_1m_rows` (skipped) |

Scenarios that are not implemented yet report **`skipped`** in JSON and PR comments — never
**passed**.

## Commands

```bash
cargo xtask budgets gate --build --output budgets-result.json
cargo xtask budgets comment --current budgets-result.json --baseline benchmarks/latest.json
cargo xtask budgets record --input budgets-result.json --csv benchmarks/history.csv
```

Local size-only check (no 30 s RAM settle):

```bash
cargo xtask size --profile dist
```

## Release settings (LUM-002)

Workspace `[profile.release]`: `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`,
`opt-level = "s"`.

Per-crate overrides: `wisp-ui` and `wisp-core` (including `render` / `grid` modules) use
`opt-level = 3`.

Shipping builds should use **`dist`** (`inherits = "release"`, `debug = false`, `incremental = false`).

## RAM harness

The gate launches `wisp --bench-scenario idle`, samples RSS for **30 s**, then terminates the
process. Platform readers:

- **Linux:** `/proc/<pid>/status` (`VmRSS`)
- **macOS:** `ps` RSS (child process)
- **Windows:** working-set size via `GetProcessMemoryInfo`

Connection scenarios will use [`docker-compose.bench.yml`](../docker-compose.bench.yml) once
drivers and sessions exist.

## Prove the size gate fails

Build with the stress feature and run the gate (expect failure &gt; 25 MB cap):

```bash
cargo xtask budgets gate --build --features bench-stress
```

## Trend history

On pushes to `main`, CI updates the **`gh-pages`** branch:

- `benchmarks/latest.json` — last report
- `benchmarks/history.csv` — one row per commit for plotting

## Policy

- Prefer **`default-features = false`** on dependencies; enable features explicitly per crate.
- Do **not** pass `-C target-cpu=…` for `release` / `dist` artifacts (local benchmarks only).
