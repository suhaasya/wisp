#!/usr/bin/env bash
# Local CI parity (see docs/ci.md).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cargo xtask ci smoke
