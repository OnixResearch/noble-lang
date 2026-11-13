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

### Decision: the `sem.rs` correspondence stays a reviewed trusted, tested assumption

**Choice:** correspondence between `NobleFoundation.Binary32` and
`crates/noble-ngraph/src/sem.rs` is recorded as a **reviewed trusted, tested
assumption** on `x86_64-unknown-linux-gnu`. It is not proved, and no other target
inherits it. The model theorems stay model claims; the assumption is the extra,
separately labelled bridge a consumer needs before reading a model bound as a
statement about the Rust evaluator.

**Why it cannot be proved now.** `sem.rs` computes `rn(a + b)`, `rn(a - b)` and
`rn(a × b)` with the Rust `f32` operators themselves (`sem.rs:90`, `:96`, `:102`),
so the arithmetic is whatever the pinned rustc/LLVM lowering and the host FPU
implement. A refinement proof would need (a) a float-capable Aeneas translation
of those bodies and (b) a model of the host arithmetic they call into. (a) does
not exist: Aeneas float support is the open draft
[AeneasVerif/aeneas#1073](https://github.com/AeneasVerif/aeneas/pull/1073) (still
a draft) for AeneasVerif/aeneas#828, tracked here as #92. (b) would remain
trusted even after (a) lands, which is why this record states the target
explicitly rather than deferring the whole question.

**Evidence the assumption rests on.** Each lane below is tested-implementation
evidence on one host and target, not proof:

- 26 retained kernel-checked expectations replayed through the packed reference
  (`noble_ngraph::sem::add/sub/mul`) and through the compiled graph legs, with
  NaN comparisons by observation class and every other class by exact encoding:
  `verification/binary32/replay.json` (2 lanes, 26 matched, source commit
  `834e14592`, `rustc 1.96.0-nightly`, `x86_64-unknown-linux-gnu`). The receipt
  predates the Lean 4.35 bump and the later correction of one retained tie
  expectation, so it binds older lane and model bytes and reports staleness
  after its own evidence commit.
- Both replay lanes were re-run at commit `2dd3da3` with the pinned toolchain
  and the current lane and expectation bytes:
  `cargo test -p noble-ngraph-experiment --test b2_differential lean_retained_binary32 -- --nocapture`
  reported `test result: ok. 2 passed; 0 failed; 0 ignored`. That is an
  observation, not a receipt: `replay.json` was not regenerated, because its
  producer drives `nix develop` and writes to `/var/tmp`, both excluded in this
  environment. The follow-up is to re-record it from a clean commit with a
  producer that can run here, so the retained receipt binds current bytes.
- A bounded deterministic shrinker that minimizes one operation discrepancy and
  reports a documented local minimum: `verification/binary32/shrinker.json`
  (flush-to-zero witness `0x00800000 × 0x3f000000`).
- 36 decimal agreements, exact and round-to-nearest, over 18 boundary decimals:
  `proofs/foundation/NobleFoundation/DecimalImportCorpus.lean`, generated by
  `crates/noble-ngraph/tests/soft_float_dir/decimal.rs`, against
  `sem::exact_f32_decimal` and Rust's `f32` parse. This is decimal import, not
  the add/sub/multiply correspondence.
- An independent APFloat `ieee::Single` oracle
  (`crates/noble-ngraph-experiment/src/oracle.rs:21`, revision recorded at
  `:33`) compared against the same operations. Finite differential evidence
  only; the oracle is not kernel evidence.
- The ignored 10^7 bit-pattern soft-float tier (`crates/noble-ngraph/tests/soft_float.rs:163-165`)
  for the reference's own `no_std` helpers on this host. Finite differential
  evidence, not proof.

**Limits of the assumption.** It says the two implementations agree on the
retained expectations, the decimal rows and the sampled inputs above, on this
target. It does not cover an untested input, another target, `wasm-opt`, V8, the
loader, the activations, the guard/index rules, training or snapshots. A stale
receipt, a substituted model file or a changed `sem.rs` body is exactly what the
next replay must catch, so the assumption is not permission to stop replaying.

**Reopen condition (go/no-go).** Revisit — and prefer a proved bridge — when
#92 closes: an Aeneas revision that translates Rust float bodies lands, or a
reviewed float-capable extraction route is selected. At that point the fork
carrying PR #1073 (`OnixResearch/aeneas`, branch `noble/float-basic`) is the
fixed revision to evaluate, together with an audit of its float model against
the model's NaN observation class, signed zeros and round-to-nearest/ties-to-even
policy. Until then no correspondence claim is made in any receipt or spec.

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

- The model is not proved equivalent to `sem.rs`; the reviewed record is the
  trusted, tested assumption above, which covers the retained expectations,
  the decimal rows and the sampled inputs on one target and nothing else. A
  proved correspondence still needs a float-capable extraction (#92) plus an
  audited model of the host arithmetic.
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
| Source-bound receipts identify Lean/Mathlib/model, generator, Rust, target, tested implementations | `verification/foundation/receipts/aa074e1/index.json` identifies Lean, Mathlib, the model, the proposer and the six test-local variants; `verification/binary32/replay.json` identifies the clean Rust commit, `rustc 1.96.0-nightly`, the host and the lane/proposal digests for both the packed-reference and the compiled-graph-leg replays. `wasm-opt`, V8, the loader and every other target stay open |
| Separate proved model / tested implementation / trusted correspondence / open | README claim table and non-claims list |
| Update only the established slice | the model slice plus tested replay and the reviewed trusted correspondence assumption: NGV-F32-01..03 promoted into the canonical verification spec, NGV-F32-CASE-01..03 registered with their real states, and the NG-PO-D slice recorded in `specs/STATUS.json` while NG-PO-D stays open. Extraction, codec, optimizer, engine, loader and every other target stay open under #10/#15 |
