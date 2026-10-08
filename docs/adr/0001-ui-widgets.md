# ADR 0001: UI widget library (`gpui-component`)

## Status

Accepted — **do not** add `gpui-component` for the M0 shell (LUM-005).

## Context

[LUM-005](https://github.com/suhaasya/wisp) requires a native GPUI frame (title bar, router,
status bar) aligned with `design/lumen-design-mockup.html`. We must choose between:

- **Hand-rolled GPUI views** in `wisp-ui`, or
- **[gpui-component](https://github.com/longbridge/gpui-component)** (Longbridge), which bundles
  themed controls, icons, and a `Root` wrapper.

## Decision

Use **Zed GPUI directly** inside `wisp-ui` for the application shell. Defer `gpui-component`
until we implement dense widgets (data grid, connection forms, modals) in later milestones.

## Rationale

- **Budget / cold start:** Fewer crates and no global `gpui_component::init` + asset bundle for
  an empty shell.
- **Design control:** Mockup-specific title bar, environment strip, and status slots are simple
  `div` layouts; no need for a component library yet.
- **Upgrade path:** GPUI bumps stay confined to `wisp-ui`; adding `gpui-component` later is
  optional and can be revisited in LUM-010+ when real content lands.

## Consequences

- We implement primitive layout helpers in `wisp-ui` (`theme`, shell modules).
- When adding tables and forms, re-evaluate `gpui-component` vs custom grid code (LUM-010).

## Review trigger

Re-open this ADR when implementing:

- Virtualized result grid
- Multi-step connection wizard with validated inputs
- Shared dialog / toast patterns across ≥3 views
