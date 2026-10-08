# GPUI dependency (Zed)

Wisp pins GPUI to a single commit in the upstream [Zed](https://github.com/zed-industries/zed)
monorepo. Only **`wisp-ui`** may depend on `gpui` / `gpui_platform`; other crates use
`wisp_ui::prelude`.

## Pinned revision

| Crate | Git rev | Date |
| --- | --- | --- |
| `gpui`, `gpui_platform` | `dc3fb21676457b84d2233ac4c6bec5cebc698ec3` | 2026-04 (Zed `main`) |

Declared in the workspace root `Cargo.toml` under `[workspace.dependencies]`.

**Rust toolchain:** this Zed revision requires **Rust 1.99+** (see `rust-toolchain.toml`).

## Bump procedure

1. Pick a new Zed commit (run Zed locally or read GPUI release notes in Zed’s changelog).
2. Update both `rev = "…"` fields in `Cargo.toml`.
3. `cargo update -p gpui -p gpui_platform`
4. `cargo build -p wisp-ui -p wisp` on **macOS, Linux, and Windows** if possible.
5. Fix breakages only inside `wisp-ui` (single wrapper crate).
6. Record the new rev and rationale in this file.

## Platform deps (Linux CI / dev)

See LUM-003 workflow: Vulkan, `libxkbcommon`, Wayland, Fontconfig, etc.

## Widget library

See [docs/adr/0001-ui-widgets.md](adr/0001-ui-widgets.md) for the `gpui-component` decision.
