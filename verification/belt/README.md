# Bounded checked belt backend (issue #113)

`belt-backend-v0` selects `ngraph-belt/0`, a target-neutral, opt-in encoding for
checked total I64/Bool graphs. The Rust implementation lives in
`crates/noble-ngraph/src/belt.rs`; `NobleFoundation.Belt` proves the symbolic
checker-to-reference relation, and `BeltCorpus.lean` is generated from a real
lowered capacity-2 fixture. These are three separate claims: a Lean model
theorem, finite source-level correspondence, and cost observations.

## Reproduce from a committed slice tree

Use the pinned slice tools, not host Cargo/Rust. Set `ROOT` to this worktree and
keep Cargo output under its private target:

```sh
ROOT=/path/to/noble-worktree
TARGET="$HOME/.cache/noble-iss/target/belt-113"
NOBLE_ROOT="$ROOT" NOBLE_TARGET="$TARGET" \
  "$HOME/.cache/noble-iss/cargo.sh" test -p noble-ngraph -p noble-ngraph-experiment \
  --all-targets --all-features --offline --locked
NOBLE_ROOT="$ROOT" NOBLE_TARGET="$TARGET" \
  "$HOME/.cache/noble-iss/cargo.sh" test -p noble-ngraph-experiment \
  --test extract_b2 --offline --locked -- --ignored l2_node_bigint_oracle_agrees_with_l3
NOBLE_ROOT="$ROOT" NOBLE_TARGET="$TARGET" \
  "$HOME/.cache/noble-iss/cargo.sh" test -p noble-ngraph --release \
  --test schedule_correspondence --offline --locked -- --ignored --nocapture \
  belt_measure::belt_cost_report
NOBLE_ROOT="$ROOT" NOBLE_TARGET="$TARGET" \
  "$HOME/.cache/noble-iss/cargo.sh" fmt --all -- --check
NOBLE_ROOT="$ROOT" NOBLE_TARGET="$TARGET" \
  "$HOME/.cache/noble-iss/cargo.sh" clippy --workspace --all-targets \
  --all-features --offline --locked -- -D warnings
```

The extracted-source B2 harness is the ordinary-path comparison: build the CLI
from this committed tree, then run
`crates/noble-ngraph-experiment/src/bin/ngraph-phase-b-extract.rs` in release
mode with `--cli`, the pinned Node 24 `--node`, `--source-rev` set to this
commit, and `--out` outside the repository. Its L5 belt leg independently checks
and executes the extracted graph and compares each result with L3, packed L4,
L2 BigInt, and ordinary Wasm/V8 L1 (`--opt off` and `--opt on`) where supported.
The default campaign uses 64 random programs × 10,000 vectors; L1 uses its
separately declared sample. Capture the receipt before copying it into this
directory, and only from a clean committed tree.

```sh
ROOT=/path/to/noble-worktree
TARGET="$HOME/.cache/noble-iss/target/belt-113"
NODE=/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node
RUSTC=/nix/store/1yvh3d6y3fj3xk2dgwczrp1dj5svd92c-rust-default-1.96.0-nightly-2026-03-21/bin/rustc
NOBLE_ROOT="$ROOT" NOBLE_TARGET="$TARGET" \
  "$HOME/.cache/noble-iss/cargo.sh" build --release -p noble-cli --bin noble --offline --locked
NOBLE_ROOT="$ROOT" NOBLE_TARGET="$TARGET" \
  "$HOME/.cache/noble-iss/cargo.sh" run --release -p noble-ngraph-experiment \
  --bin ngraph-phase-b-extract -- \
  --cli "$TARGET/release/noble" --node "$NODE" \
  --out "$HOME/.cache/noble-iss/scratch/belt-113/phase-b-extract.json" \
  --source-rev "$(git -C "$ROOT" rev-parse HEAD)" --toolchain "$("$RUSTC" -V)"
```

The hand-built graph fixture covers wrapping I64 results, asymmetric
subtraction, equality, fanout, repeated/multiple outputs, and preservation at
capacities 2–4. It has no Wasm source run. The extracted-source B2 lane is the
Wasm/BigInt comparison; no claim is inferred from the hand-built controls.

The ignored `belt_cost_report` reports admission, cold compiled construction,
belt lowering, independent checking, first and warm execution, consumer output
copy, instruction and artifact bytes, save/load traffic, logical belt payload,
checker auxiliary payload, and warm `Compiled` retained-code plus workspace
payload. The direct scalar baseline is retained. A consumer readback is an
extra copy because normal execution already materializes its output vector;
logical payload counts are not allocator/RSS measurements. Timing is descriptive
and does not select a backend or establish target-hardware performance.

## Evidence boundary

The Lean `validate_sound` theorem proves that an accepted symbolic program's
ordered output identities equal the graph-reference outputs. `BeltCorpus.lean`
contains kernel-decided validation/refusal and exact-vector examples for a real
lowered stream; the freshness test ties it to the Rust lowerer. Neither proves
the Rust checker or interpreter. Rust correspondence is finite test evidence.
The extracted-source B2 lane adds independent BigInt and ordinary Wasm/V8
comparisons. Measurements are descriptive only.

The committed `receipt.json` binds the exact source commit and tree to the
separate model, Rust correspondence, B2 and cost commands/results. It does not
claim an implementation theorem, Mill ISA compatibility, or hardware speedup.
