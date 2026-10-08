#!/usr/bin/env bash
# Fail if config dir contains plaintext password material (LUM-009).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

config="${WISP_CONFIG_DIR:-}"
if [[ -z "$config" ]]; then
  if [[ -d "${HOME}/Library/Application Support/wisp" ]]; then
    config="${HOME}/Library/Application Support/wisp"
  elif [[ -d "${XDG_CONFIG_HOME:-$HOME/.config}/wisp" ]]; then
    config="${XDG_CONFIG_HOME:-$HOME/.config}/wisp"
  else
    config="${root}/.wisp/config"
  fi
fi

if [[ ! -d "$config" ]]; then
  echo "secret config check: ok (no config dir at $config)"
  exit 0
fi

if rg -i 'password\s*=' "$config" 2>/dev/null; then
  echo "error: plaintext password assignment found under config dir"
  exit 1
fi

if rg -i 'ssh_passphrase\s*=' "$config" 2>/dev/null; then
  echo "error: plaintext ssh_passphrase found under config dir"
  exit 1
fi

if rg -i '-----BEGIN (RSA |OPENSSH )?PRIVATE KEY-----' "$config" 2>/dev/null; then
  echo "error: private key material found under config dir"
  exit 1
fi

# Vault must live under data, not config.
if [[ -f "$config/secrets.vault" ]]; then
  echo "error: encrypted vault must not live in config dir"
  exit 1
fi

echo "secret config check: ok ($config)"
