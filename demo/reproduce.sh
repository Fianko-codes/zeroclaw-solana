#!/usr/bin/env bash
# Reproduce every offline claim in the submission with one command.
#
# Runs the exact CI command set — the pinned toolchain, --locked, both Clippy
# targets, and the wasm release build — over all three plugins and the shared
# core, then prints a summary table. No network, no wallet, no wasm runtime, and
# no funds are required: the one committed real-network artifact is a verbatim
# mainnet response that is replayed offline.
#
# Usage:  demo/reproduce.sh [--fast]
#           --fast   skip the two wasm builds (they dominate the runtime)
#
# On a small machine, set the caps this repository's evidence was produced with:
#   CARGO_BUILD_JOBS=2 CARGO_PROFILE_TEST_DEBUG=0 CARGO_TARGET_DIR=~/.cache/zc
set -uo pipefail

TOOLCHAIN="${TOOLCHAIN:-1.96.1}"
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
PLUGINS="$ROOT/zeroclaw-plugins/plugins"
FAST=0
[[ "${1:-}" == "--fast" ]] && FAST=1

declare -a ROWS=()
STATUS=0

step() { # step <label> <dir> <command...>
  local label=$1 dir=$2
  shift 2
  printf '\n\033[1m▸ %s\033[0m\n' "$label"
  if (cd "$dir" && "$@"); then
    ROWS+=("PASS  $label")
  else
    ROWS+=("FAIL  $label")
    STATUS=1
  fi
}

if ! command -v cargo >/dev/null; then
  echo "cargo is required" >&2
  exit 2
fi
if ! cargo "+$TOOLCHAIN" --version >/dev/null 2>&1; then
  echo "toolchain $TOOLCHAIN is not installed: rustup toolchain install $TOOLCHAIN" >&2
  echo "(set TOOLCHAIN=stable to run on the default toolchain instead)" >&2
  exit 2
fi

printf 'toolchain: %s\n' "$(cargo "+$TOOLCHAIN" --version)"
printf 'core:      nanosol %s\n' "$(git -C "$ROOT" rev-parse --short HEAD)"
printf 'plugins:   %s\n' "$(git -C "$ROOT/zeroclaw-plugins" rev-parse --short HEAD)"

# The shared core: golden vectors against the official Solana crates.
step "nanosol · tests (byte oracles vs official Solana crates)" "$ROOT/nanosol" \
  cargo "+$TOOLCHAIN" test --quiet
step "nanosol · clippy" "$ROOT/nanosol" \
  cargo "+$TOOLCHAIN" clippy --all-targets --quiet -- -D warnings

for plugin in solana-pay-request spl-transfer-build solana-pay-confirm; do
  dir="$PLUGINS/$plugin"
  [[ -d "$dir" ]] || { echo "missing $dir" >&2; exit 2; }
  step "$plugin · fmt" "$dir" cargo "+$TOOLCHAIN" fmt --all -- --check
  step "$plugin · tests --locked" "$dir" cargo "+$TOOLCHAIN" test --locked --quiet
  step "$plugin · clippy (host)" "$dir" \
    cargo "+$TOOLCHAIN" clippy --locked --all-targets --quiet -- -D warnings
  step "$plugin · clippy (wasm32-wasip2)" "$dir" \
    cargo "+$TOOLCHAIN" clippy --locked --target wasm32-wasip2 --quiet -- -D warnings
  if [[ $FAST -eq 0 ]]; then
    step "$plugin · release build (wasm32-wasip2)" "$dir" \
      cargo "+$TOOLCHAIN" build --locked --target wasm32-wasip2 --release --quiet
  fi
done

printf '\n\033[1mSummary\033[0m\n'
printf '%s\n' "${ROWS[@]}"
if [[ $STATUS -eq 0 ]]; then
  printf '\n\033[32mAll checks passed.\033[0m See each plugin'"'"'s RESULTS.md for the\n'
  printf 'live-network evidence that cannot be reproduced offline.\n'
else
  printf '\n\033[31mSomething failed above.\033[0m\n'
fi
exit "$STATUS"
