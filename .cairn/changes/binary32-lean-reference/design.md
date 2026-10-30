## Context

Numeric-Graph-v1's binary32 arithmetic is implemented in
`crates/noble-ngraph/src/sem.rs` (add/subtract/multiply, an ordered affine fold,
graph observations) with signed zeros distinct, all NaNs one observation class,
retained subnormals, and no reassociation or implicit FMA. The independent
APFloat `ieee::Single` oracle in `crates/noble-ngraph-experiment` is finite
differential evidence and its guard-fact checking is shared, so it cannot be
advertised as kernel evidence.

At the Noble pin, Lean 4.31.0's `Float32` arithmetic and bit conversion in
`Init/Data/Float32.lean` are opaque, so they cannot be a kernel-reducible
reference, and a newer Lean logical FP model would be an unaudited pin change.
The foundation project already audits inventories, axioms and clean sources, so
the smallest honest step is one explicit model inside that project.

## Decisions

### Decision: select a small explicit model instead of an opaque or newer Lean one

**Choice:** `NobleFoundation.Binary32` defines raw encodings, classification,
an exact scaled-integer interpretation (`mantissa * 2^exponent`), and
round-to-nearest/ties-to-even with gradual underflow. Every definition is total
and structural, so the kernel reduces it. Lean 4.31.0 and Mathlib4 `fabf563a`
stay pinned; no dependency is added.

**Rationale:** an opaque `Float32` cannot produce kernel-checked expectations at
the pin, and adopting a newer Lean model means auditing its NaN representation
and operation coverage against Noble first. The explicit model has a small
trusted base (Lean kernel, integer arithmetic, Nat division and the fuel-bounded
`log2`), a named theorem inventory and no custom arithmetic axioms.

### Decision: an external tool proposes expectations, the kernel checks them

**Choice:** `verification/binary32/propose.py` proposes expected bits with its
own integer rounding search; `retained_expectations_checked` kernel-checks all
26 retained expectations against the model.

**Rationale:** proposals are cheap and independent, but a proposal is not
evidence. Making the kernel check every retained expectation keeps the trust in
Lean and keeps the external tool honest about being only a proposer.

### Decision: separate proved model from trusted correspondence

**Choice:** the nine theorems are model statements. Correspondence with
`sem.rs`, `wasm-opt`, V8 or any host FPU is **not** claimed, and the APFloat lane
stays secondary.

**Rationale:** the graph's obligations are machine arithmetic on the selected
target, and a model-level proof plus independently proposed expectations is the
strongest honest claim available without a refinement proof of the Rust
evaluator. Claiming correspondence would be a subjective trusted assumption that
is not reviewed here.

## Semantics

- Encoding: sign, 8-bit biased exponent (bias 127), 23-bit fraction.
- Classes: infinity, NaN (any other exponent-255 encoding), negative zero,
  positive zero, subnormal, normal.
- Exact value: `mantissa * 2^exponent` with `mantissa = 2^23 + fraction`,
  `exponent = parity - 150` for normals and `(fraction, -149)` for subnormals.
- Rounding: one rounding per operation, nearest with ties to even, `2^-149`
  spacing under gradual underflow, infinity at or above `2^128`.
- Signed zeros: distinct observations; multiplication uses the sign
  exclusive-or, addition keeps a negative zero only for two negative zeros or a
  negative exact sum, and `x - x` is `+0`.
- NaNs: every NaN encoding is one observation class (`obsEq`), and arithmetic
  produces the canonical `0x7FC00000`.
- Infinities: same-sign addition keeps the sign, opposite signs give NaN, and
  zero times infinity is NaN.
- Fold and contraction: reassociation and implicit contraction are observable,
  which is why the fold order and single rounding per operation are normative.

## Risks / Trade-offs

- The model is not proved equivalent to `sem.rs`; a correspondence claim needs a
  reviewed refinement or a bounded differential replay that retains the model
  as the authority. Untested correspondence is open, not assumed.
- Rounding correctness is checked by kernel-reduced boundary examples rather
  than a general theorem. The mutation lane shows that six declared wrong models
  are detected, which is detection evidence, not a correctness proof.
- The fuel-bounded `log2` and Nat division are trusted kernel-reducible code,
  not proved against a specification.
- Extending the slice to the ordered affine fold, extraction, training,
  snapshots and codecs is future work, and no claim is made for them.

## Executed phase 2

- Test-local variants (`NobleFoundation/Binary32Mutations.lean`): away-from-zero
  ties, positive-zero collapse, flush-to-zero, zero-sign-insensitive
  observation, commutative subtraction and implicit contraction. Each is caught
  by a named retained expectation; `all_declared_mutations_are_caught` closes the
  declared set.
- Packed-reference replay: `crates/noble-ngraph-experiment/tests/b2_differential.rs`
  replays all 26 retained expectations through `noble_ngraph::sem::add/sub/mul`,
  matching NaN expectations by observation equality and every other class by
  exact encoding. The first run found one genuine difference: the Rust
  reference returns `0xFFC00000` where the Lean model produces its canonical
  `0x7FC00000` for `inf + -inf`. Both are one NaN observation, which is why the
  lane compares observations rather than payload bytes, and why the model's
  canonical-NaN statement is a model claim and not a reference claim.
- Compiled-legs replay: each of the 26 expectations also builds a two-input F32
  graph, and `run_legs` must agree across L2 APFloat, L3 reference and L4
  compiled while each of those observations matches the retained encoding. This
  lane exercises the checker, the graph reference and the compiled path rather
  than the bare semantics.
- Receipts: `verification/foundation/receipts/aa074e1/index.json` (212
  theorems, sixteen binary32, clean source) and `verification/binary32/replay.json`
  (clean commit `834e14592`, `rustc 1.96.0-nightly`, `Linux 6.18.54 x86_64`, target
  `x86_64-unknown-linux-gnu`, with `--verify` digest checks; first
  recorded at `9845a9e9d`).
- Shrinking: a bounded deterministic shrinker walks a documented simplification
  list over an operand pair, accepting only strictly simpler pairs under the
  documented order, so the walk is monotone and terminates inside a documented
  attempt budget. Its result is a LOCAL minimum under that order and never a
  global minimum: a pair that needs a simplification outside the list is
  reported unchanged rather than invented, and the lane withholds any
  reachability claim beyond what the list reaches. `verification/binary32/shrinker.json`
  records the flush-to-zero witness (`0x00800000 * 0x3f000000` keeps the
  subnormal product against flush-to-zero's zero), the clean commit, the
  toolchain and the host. The shrinker helps a future mismatch become a
  minimized replayable record; it is not kernel evidence, and driving it from a
  retained-expectation discrepancy stays open.

## Acceptance mapping

| #38 acceptance item | This change |
|---|---|
| Versioned F32 specification, explicit relations, named inventory, audited assumptions | selected model, scaled interpretation, `obsEq`, nine inventoried theorems, axiom audit inside the allow-list |
| Kernel-checked examples for ties, cancellation, both zero signs, subnormals, boundaries, overflow, infinities, NaNs | `retained_expectations_checked`, `halfway_ties_round_to_even`, `signed_zero_rules`, `encoding_boundaries` |
| Ordered affine examples detect reassociation and contraction; round-to-real cannot discharge binary32 | `reassociation_is_observable`, `implicit_contraction_is_observable`, `exact_sum_exceeds_the_rounded_result` |
| Source-bound receipts identify Lean/Mathlib/model, generator, Rust, target, tested implementations | `verification/foundation/receipts/aa074e1/index.json` identifies Lean, Mathlib, the model, the proposer and the six test-local variants; `verification/binary32/replay.json` identifies the clean Rust commit, `rustc 1.96.0-nightly`, the host and the lane/proposal digests for the packed-reference replay. The compiled graph legs, `wasm-opt`, V8 and the loader stay open |
| Separate proved model / tested implementation / trusted correspondence / open | README claim table and non-claims list |
| Update only the established slice | only the model-level slice; extraction, codec, optimizer, engine and loader stay open under #10/#15 |
