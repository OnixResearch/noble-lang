#!/usr/bin/env bash
# Issue #81 refinement-link lane: measure both subjects' closures, build the manifests, check drift.
#
# Usage: verification/refinement-links/run.sh [OUTPUT_DIR]
#
# Requires each subject's `.lake` to be populated (`lake build <root module>`), which is the same
# precondition `verification/foundation/ci.sh` already documents for its Lean stages. Every stage
# runs under `env -i` with only the policy-selected Lean and Node on PATH, so an ambient toolchain
# cannot decide what "reviewed" means.
#
# Exit status: 0 only when every closure built and every drift check reported no finding.
set -u
out=${1:-}
root=$(cd "$(dirname "$0")/../.." && pwd)
[ -n "$out" ] || out=$(mktemp -d)
mkdir -p "$out" || exit 2
node=$(/run/current-system/sw/bin/sed -n 's/.*"output": "\(\/nix\/store\/[a-z0-9]*-nodejs-[^"]*\)".*/\1/p' "$root/policy/tool-selection.json" | /run/current-system/sw/bin/head -n 1)/bin/node
lean=$(/run/current-system/sw/bin/sed -n 's/.*"output": "\(\/nix\/store\/[a-z0-9]*-noble-lean-release-[^"]*\)".*/\1/p' "$root/policy/tool-selection.json" | /run/current-system/sw/bin/head -n 1)/bin
[ -x "$node" ] || { echo "run: selected node not found: $node" >&2; exit 2; }
[ -x "$lean/lean" ] || { echo "run: selected lean not found: $lean" >&2; exit 2; }
lane="$root/verification/refinement-links"
status=0

closure() {
  local name=$1 project=$2 modules=$3 prefix=$4
  ( cd "$root/proofs/$project" && env -i PATH="$lean:/run/current-system/sw/bin:/usr/bin:/bin" \
      HOME="$HOME" NOBLE_CLOSURE_MODULES="$modules" NOBLE_CLOSURE_PREFIX="$prefix" \
      LEAN_PATH="$lane" "$lean/lake" env lean "$lane/closure.lean" ) \
    > "$out/closure-$name.json" 2> "$out/closure-$name.err"
  local rc=$?
  echo "closure-$name=$rc" >> "$out/summary.txt"
  return $rc
}

manifest() {
  local name=$1 generated=$2 closure=$3
  env -i PATH="/run/current-system/sw/bin:/usr/bin:/bin" "$node" "$lane/links.mjs" \
    --generated "$generated" --closure "$out/$closure" \
    --rust-root "$root" --out "$out/links-$name.json" > "$out/links-$name.log" 2>&1
  local rc=$?
  echo "links-$name=$rc" >> "$out/summary.txt"
  return $rc
}

drift() {
  local name=$1
  env -i PATH="/run/current-system/sw/bin:/usr/bin:/bin" NOBLE_REFINEMENT_REPO="$root" \
    "$node" "$lane/drift.mjs" --manifest "$out/links-$name.json" \
    --reviewed "$lane/reviewed.json" --report "$out/drift-$name.json" > "$out/drift-$name.log" 2>&1
  local rc=$?
  echo "drift-$name=$rc" >> "$out/summary.txt"
  return $rc
}

closure mc1 mc1 "NobleContractImpl.Projection NobleContractImpl.Statement" noble_contracts || status=1
manifest mc1 proofs/mc1/NobleContractImpl/Funs.lean closure-mc1.json || status=1
drift mc1 || status=1

# M3's lane does not compile at this revision (`NobleM2/Embed.lean` builds a kernel value missing six
# fields the current `NobleKernel` requires), so its closure cannot be read. The manifest is still
# measured there, and every link is `closure-unavailable`: reporting it as `unlinked` would claim
# nobody mentions those functions, and skipping the subject would hide the break. The stage fails.
if closure m3 m3 "NobleM2.Refinement" noble_kernel; then
  manifest m3 proofs/m3/NobleKernel/Funs.lean closure-m3.json || status=1
  drift m3 || status=1
else
  status=1
  manifest m3 proofs/m3/NobleKernel/Funs.lean /nonexistent-closure.json || status=1
  echo "closure-m3=unavailable" >> "$out/summary.txt"
fi

echo "run: output $out, status $status"
exit "$status"
