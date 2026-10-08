# Continuous integration (LUM-003)

Pull requests and pushes to `main` run [.github/workflows/ci.yml](../.github/workflows/ci.yml).

## Pipeline

**Active:** lint only. The **build** matrix and **ci-ok** jobs are commented out in
`ci.yml` until we turn on 5-target release CI again.

| Job | Runner | What it does |
| --- | --- | --- |
| **lint** | `ubuntu-24.04` | `cargo xtask ci fmt`, `clippy`, `deny`, `audit` |

### Shipping targets (build matrix — disabled in CI)

| OS runner | Rust target |
| --- | --- |
| `macos-14` | `aarch64-apple-darwin` |
| `macos-14` | `x86_64-apple-darwin` (cross) |
| `ubuntu-24.04` | `x86_64-unknown-linux-gnu` |
| `ubuntu-24.04` | `aarch64-unknown-linux-gnu` (cross + QEMU tests) |
| `windows-latest` | `x86_64-pc-windows-msvc` |

Artifacts: **`wisp-<target>`** — stripped `dist` binary from each matrix leg.

Caching: [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) on registry + `target/`.

## Local parity

```bash
cargo xtask ci smoke          # fmt + clippy + build + test (+ deny/audit if installed)
cargo xtask ci fmt
cargo xtask ci clippy
cargo xtask ci test --target aarch64-apple-darwin
cargo xtask ci build-release --target aarch64-apple-darwin --profile dist
```

## Branch protection (manual, repo admin)

Configure **`main`** on GitHub:

1. **Require status checks** before merge:
   - `lint`
2. **Require pull request reviews** — at least **1** approval.
3. **Require branches to be up to date** (recommended).

Using GitHub CLI (admin):

```bash
./scripts/configure-branch-protection.sh
```

## Proving the gate

Open a test PR that introduces a clippy warning or failing test; merge should stay blocked until
fixed. Example: add `#[allow(dead_code)] fn broken() { let _x = 1; }` in a library crate and
confirm **lint** fails.
