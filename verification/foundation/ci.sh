#!/usr/bin/env bash
# CI recipe for the issue #15 foundation (proofs/foundation/README.md, Phase 4).
#
# Usage: verification/foundation/ci.sh NEW_OUTPUT_DIR [--skip-slice]
#
# Preconditions:
#   - a clean committed checkout;
#   - the policy-selected Lean and Node in the Nix store;
#   - proofs/foundation/.lake populated by
#     `lake exe cache get`, which downloads Mathlib .oleans. This is the one
#     networked step, and it is why the recipe is not a Nix sandbox check;
#   - for the slice: a user systemd manager (XDG_RUNTIME_DIR) and the
#     bubblewrap/prlimit/systemd-run tools named in slice.mjs.
# Each stage runs under `env -i`. A stage passes only with exit status 0, and
# the recipe exits nonzero if any stage fails. `summary.txt` records every
# stage's status. The stale-receipt control must FAIL (exit 1) for the run to
# pass.
set -u
out=${1:?usage: ci.sh NEW_OUTPUT_DIR [--skip-slice]}
skip_slice=${2:-}
root=$(cd "$(dirname "$0")/../.." && pwd)
node=$(sed -n 's/.*"output": "\(\/nix\/store\/[a-z0-9]*-nodejs-[^"]*\)".*/\1/p' "$root/policy/tool-selection.json" | head -n 1)/bin/node
[ -x "$node" ] || { echo "ci: selected node not found: $node" >&2; exit 2; }
mkdir "$out" || exit 2
cd "$root" || exit 2
status=0
stage() {
  name=$1; shift
  "$@" > "$out/$name.log" 2>&1
  rc=$?
  echo "$name=$rc" >> "$out/summary.txt"
  return $rc
}
stage check env -i "$node" verification/foundation/check.mjs --output "$out/check" || status=1
if [ "$skip_slice" != "--skip-slice" ]; then
  stage slice env -i XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-}" "$node" verification/foundation/slice.mjs \
    --foundation "$out/check/foundation-check.json" --output "$out/slice" || status=1
else
  echo "slice=skipped" >> "$out/summary.txt"
fi
stage differential env -i "$node" verification/foundation/differential.mjs --seed 15 --count 80 \
  --foundation "$out/check/foundation-check.json" --output "$out/differential" || status=1
stage verify env -i "$node" verification/foundation/differential.mjs \
  --verify "$out/differential/differential.json" || status=1
# Keep the EMI controller's V8 compilation and heap bounds explicit. The
# receipt records these flags separately from the guest runtime limits.
emi_node=(env -i "$node" --no-liftoff --no-wasm-lazy-compilation --no-wasm-tier-up \
  --stack-size=1024 --max-old-space-size=256)
stage emi "${emi_node[@]}" verification/foundation/differential.mjs --emi --seed 53 --count 16 \
  --foundation "$out/check/foundation-check.json" --output "$out/emi" || status=1
stage emi-verify "${emi_node[@]}" verification/foundation/differential.mjs --emi \
  --verify "$out/emi/emi.json" || status=1
stage emi-replay "${emi_node[@]}" verification/foundation/differential.mjs --emi \
  --replay "$out/emi/minimal-control.json" \
  --foundation "$out/check/foundation-check.json" --output "$out/emi-replay" || status=1
stage emi-replay-verify "${emi_node[@]}" verification/foundation/differential.mjs --emi \
  --verify "$out/emi-replay/emi.json" || status=1
# The one-call linking pilot shares the bounded V8 controller configuration;
# its receipt is identity/finite-execution evidence, not a lowering proof.
stage linking "${emi_node[@]}" verification/linking/run.mjs \
  --foundation "$out/check/foundation-check.json" --output "$out/linking" || status=1
stage linking-verify "${emi_node[@]}" verification/linking/run.mjs \
  --verify "$out/linking/linking.json" || status=1
# Stale-evidence control: the retained in-repo receipt names an older tree and
# must be refused.
stale=$(ls -d verification/foundation/receipts/*/ | head -n 1)
if stage stale-receipt-refused env -i "$node" verification/foundation/differential.mjs \
  --verify "${stale}differential-summary.json"; then
  echo "ci: stale receipt was accepted" >&2; status=1
fi
echo "overall=$status" >> "$out/summary.txt"
cat "$out/summary.txt"
exit $status
