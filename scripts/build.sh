#!/usr/bin/env bash
# Wrapper around `cargo tauri build` that signs the updater bundle.
#
# Tauri only signs the .tar.gz updater bundle when both:
#   TAURI_SIGNING_PRIVATE_KEY            — the base64 content of the .key file
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD   — the rsign passphrase (if encrypted)
# are present. We load the key from ~/.tauri/zapterm.key and the passphrase from
# the macOS Keychain so neither ends up in the repo or in shell history.
#
# First-time setup (once per machine):
#     security add-generic-password \
#         -s zapterm-signing-key -a "$USER" -w
#     # …enter the passphrase you set when you ran `tauri signer generate`
#
# Usage:
#     ./scripts/build.sh                # full signed build
#     ./scripts/build.sh --no-updater   # skip the updater bundle entirely
#                                       # (use for local smoke tests)

set -euo pipefail

KEY_FILE="${TAURI_KEY_FILE:-$HOME/.tauri/zapterm.key}"
KEYCHAIN_SERVICE="${TAURI_KEYCHAIN_SERVICE:-zapterm-signing-key}"

# ── --no-updater short-circuit ──────────────────────────────────────────────
if [[ "${1:-}" == "--no-updater" ]]; then
  echo "→ building without updater artifact (no signing required)"
  shift
  exec cargo tauri build \
    --config '{"bundle":{"createUpdaterArtifacts":false},"plugins":{"updater":{"active":false}}}' \
    "$@"
fi

# ── Key file ────────────────────────────────────────────────────────────────
if [[ ! -f "$KEY_FILE" ]]; then
  cat >&2 <<EOF
✗ Signing key not found at $KEY_FILE

  Either:
    • Restore the original key (you may have one in GitHub Secrets), or
    • Generate a fresh keypair:
        cargo tauri signer generate -w "$KEY_FILE"
      Then copy the matching public key into crates/app/tauri.conf.json
      under plugins.updater.pubkey AND update the GitHub Actions secret.

  Or build without the updater artifact:
        ./scripts/build.sh --no-updater
EOF
  exit 1
fi

# Strip any trailing newline / CR — Tauri is picky.
TAURI_SIGNING_PRIVATE_KEY="$(tr -d '\r\n' < "$KEY_FILE")"
export TAURI_SIGNING_PRIVATE_KEY

# ── Passphrase ─────────────────────────────────────────────────────────────
# The repo's signing key is currently empty-passphrase (regenerated with
# `cargo tauri signer generate --password ""`). Tauri still requires the env
# var to be SET — even if empty — to take the "no password" branch.
#
# If you ever rotate the key to one that needs a real passphrase, store it
# in the Keychain once:
#     security add-generic-password -s zapterm-signing-key -a "$USER" -w 'PASS'
# and the lookup below picks it up automatically.
if PASS="$(security find-generic-password -s "$KEYCHAIN_SERVICE" -a "$USER" -w 2>/dev/null)" && [[ -n "$PASS" ]]; then
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$PASS"
  unset PASS
elif [[ -n "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]]; then
  : # caller exported it — respect that
else
  # No stored passphrase and none in the env — assume empty-passphrase key.
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
fi

echo "→ signing key loaded; running cargo tauri build"
exec cargo tauri build "$@"
