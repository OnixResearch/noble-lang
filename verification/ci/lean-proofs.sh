#!/usr/bin/env bash
# Host-only Lean proof gate: build proofs/m3 and proofs/foundation with the
# policy-selected Lean, then run their existing audits.
#
# This is deliberately not a `nix flake check`. The Lake package sources are not
# Nix inputs: `nix/offline-inputs.nix` records store paths that no longer exist
# and no fetcher reproduces a `.lake/packages` Git checkout. A sandboxed Lean
# build is therefore impossible here, and this script is the supported lane.
#
# Usage:
#   verification/ci/lean-proofs.sh [--allow-dirty] [NEW_OUTPUT_DIR]
#
# Preconditions:
#   - a committed checkout. `--allow-dirty` skips that precondition, but then
#     the foundation receipt describes a working tree and is not evidence;
#   - the policy-selected Lean and Node are present in the Nix store;
#   - proofs/m3/.lake and proofs/foundation/.lake are populated, including
#     Mathlib .oleans from `lake exe cache get` (the one networked step).
#
# Stages run in order and the script fails at the first nonzero exit:
#   m3-build          lake build in proofs/m3
#   m3-gate           verification/m3-proof-gate.sh (strict or handoff tier)
#   foundation-build  lake build in proofs/foundation
#   foundation-check  verification/foundation/check.mjs (axiom and statement audit)
#
# summary.txt records every stage's exit status.
set -uo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd) || exit 2
selection="$root/policy/tool-selection.json"
[ -f "$selection" ] || { echo "lean-proofs: missing $selection" >&2; exit 2; }
lean_bin=$(sed -n 's/.*"output": "\(\/nix\/store\/[a-z0-9]*-noble-lean-release-[^"]*\)".*/\1/p' "$selection" | head -n 1)/bin
node=$(sed -n 's/.*"output": "\(\/nix\/store\/[a-z0-9]*-nodejs-[^"]*\)".*/\1/p' "$selection" | head -n 1)/bin/node
[ -x "$lean_bin/lake" ] || { echo "lean-proofs: selected Lean not found: $lean_bin" >&2; exit 2; }
[ -x "$node" ] || { echo "lean-proofs: selected Node not found: $node" >&2; exit 2; }

dirty_args=()
if [ "${1:-}" = "--allow-dirty" ]; then
  dirty_args=(--allow-dirty)
  shift
fi
if [ "$#" -ge 1 ]; then
  out="$1"
else
  out=$(mktemp -d "${TMPDIR:-/tmp}/noble-lean-proofs-XXXXXX") || exit 2
fi
mkdir -p "$out" || exit 2
cd "$root" || exit 2

status=0
stage() {
  name=$1
  shift
  "$@" > "$out/$name.log" 2>&1
  rc=$?
  echo "$name=$rc" >> "$out/summary.txt"
  echo "[lean-proofs] $name rc=$rc"
  if [ "$rc" -ne 0 ]; then
    status=1
    echo "[lean-proofs] $name failed; last lines:" >&2
    tail -20 "$out/$name.log" >&2
  fi
  return "$rc"
}

# The pinned release must be first on PATH: Lake resolves `lean` from it.
lean_path="$lean_bin:/run/current-system/sw/bin:/usr/bin:/bin"
build() { ( cd "$1" && PATH="$lean_path" exec lake build ); }
gate() { PATH="$lean_path" exec verification/m3-proof-gate.sh; }
foundation_check() {
  env -i "$node" verification/foundation/check.mjs "${dirty_args[@]}" --output "$out/foundation"
}

stage m3-build build "$root/proofs/m3" || true
if [ "$status" -eq 0 ]; then
  stage m3-gate gate || true
fi
if [ "$status" -eq 0 ]; then
  stage foundation-build build "$root/proofs/foundation" || true
fi
if [ "$status" -eq 0 ]; then
  stage foundation-check foundation_check || true
fi

echo "overall=$status" >> "$out/summary.txt"
cat "$out/summary.txt"
exit "$status"
