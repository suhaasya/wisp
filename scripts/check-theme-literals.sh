#!/usr/bin/env bash
# Fail if colour literals appear outside ui/src/theme (LUM-006).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if rg 'gpui::rgb\(|0x[0-9A-Fa-f]{6}' ui/src \
  --glob '!ui/src/theme/**' \
  --glob '!ui/src/theme.rs'; then
  echo "error: hard-coded colour literals must live in ui/src/theme/ only"
  exit 1
fi

echo "theme literal check: ok"
