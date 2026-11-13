# Lean-checked numerical-error certificates (#44): NO-GO on integration, broadening deferred

**Question.** Should the proved roundoff-certificate checker be integrated into graph execution, and
should its add/multiply fragment be broadened inside #44?

**Decision: NO-GO on integration; defer every broadening.**

1. Certification stays **outside** the execution path. No graph is admitted, evaluated, cached or
   refused on the strength of a certificate, and no certificate is required for ordinary graphs.
2. No new operation joins the fragment inside #44. Subtraction, the ordered affine fold, activations
   and everything else outside `add`/`mul` becomes a **new issue** whose preconditions are listed
   under "What would reopen it".
3. The checker, its soundness theorem, its binding layer and the corpus stay as retained model and
   prototype evidence. Nothing is retracted: the NO-GO is about adoption, not about the proofs.

This is the go/no-go task "Bind exact graph/parameter snapshot, domain, model and checker revisions in
a retained receipt and publish a go/no-go before broadening operations" (NGRAPH-ERR-04). It is
published before any broadening, so it governs no measurement that predates it.

## Decision rule

The go/no-go is decided against four conditions, all of which integration would need:

| Condition | State | Evidence |
|---|---|---|
| A proved or reviewed correspondence from the checker's machine model to the code that actually computes (`crates/noble-ngraph/src/sem.rs`, Wasm, host) | **not met** | `check_sound` is stated over #38's own model; `sem.rs` computes with the Rust `f32` operators (`sem.rs:90`, `:96`, `:102`), and the correspondence is a reviewed trusted assumption on one target, not a proof (#38; `../binary32/` and `proofs/foundation/README.md`) |
| Producer-side identity derivation for the certificate context | **not met** | the six context identities are *compared*, not derived (`Binary32Binding.lean`); the digests use the tests' SipHash scheme, deterministic for the pinned toolchain but not cryptographic |
| A measured cost for an executable checker on real artifacts | **not met** | the only cost measured is the Lean kernel checking a generated corpus, not a Rust/Wasm checker running in a pipeline (below) |
| Bounds usable on mixed magnitudes without losing most of their precision | **not met** | certified bounds up to about `5.6·10^135` times Gappa's on graphs that mix the subnormal domain with ordinary or large magnitudes (below) |

Integration needing all four and having none is the whole decision. Three conditions would each be a
separate project; adopting now would advertise a bound the repository cannot yet substantiate for
the code it would constrain.

## Evidence

Both receipts come from clean committed sources and are retained in this lane. Read them; the
numbers below are quoted from them.

### Kernel lane and corpus — `receipts/2026-10-30/lane.json` (`holds: true`)

- 24 add/multiply graphs from SplitMix64 seed `44` over domains `[1, 2]`, `[-3, 5]`, `[-1, 1]`,
  `[2^-130, 2^-120]`, `[2^60, 2^62]` and `[0.1f, 0.3f]`; 2 of 26 candidate graphs were discarded
  because their reach meets the checker's `2^127` overflow test. 202 nodes, 48 theorems
  (24 accepted certificates, 24 refused weakened copies; `corpus.lean` sha256 `9ec794e5…`).
- Checker cost: about **100.4 ms per kernel check** — corpus median 7,197 ms against an import-only
  median 2,378 ms, divided over 48 checks, including elaborating the generated definitions.
- Sampling (secondary evidence only): 404,000 node checks at 2,000 inputs per graph, **0 violations**
  of enclosure, finiteness or error bound. Certified output bound over the worst observed output
  error: min 1.20, quartiles 2.57 / 4.01 / 7.40 over the 23 graphs with a nonzero observed error.
- Budget headroom: the largest certificate rational is 775 bits of a 832-bit budget.

### Bound quality against an untrusted analyzer — `receipts/2026-11-08/gappa.json` (`holds: true`)

Gappa 1.6.1 (flake-locked nixpkgs, untrusted here) on the same seed-44 corpus.

- Gappa bounds all 24 graphs, and every Gappa bound covers the worst observed output error.
- Certified bound over Gappa's bound: min **1.1588**, quartiles **1.3644 / 1.6563 / 2.9091**, max
  **5.582·10^135**, over the 23 graphs whose Gappa bound is nonzero. Graph 11's output is exact
  under Gappa where the certificate allows `7.2·10^16`.
- Cost: Gappa median **15.72 ms** and max 56.11 ms per graph, process start-up included, against
  about 100 ms per kernel check. These measure different work, so the ratio is context, not a
  target.
- The five large ratios (graphs 1, 6, 17, 22, 23) all mix the subnormal domain with ordinary or
  large magnitudes, where the checker propagates interval reach over the whole domain. In four of
  them Gappa's bound matches the worst observed error to six significant digits. Mixed magnitudes
  are therefore the checker's weak point, not a tuning detail.
- FPTaylor and Daisy are not packaged in the pinned nixpkgs and were not compared.

### Model, prototype and binding

- `Binary32Checker.lean`: `check` is total, `check_sound` gives every node a finite machine value, a
  real value inside its enclosure and an error within its bound for every input meeting the declared
  domains; `check_output_bound` states it per output. The overflow test is sufficient, not tight.
- Two real `noble-ngraph` graphs are certified through `Binary32CheckerEncoded.lean`
  (`Encoded.cancellation_certified`, `Encoded.square_near_overflow`) and bound to contexts computed
  from the real artifacts in `Binary32CheckerBound.lean` (`Encoded.cancellation_bound_certified`,
  `Encoded.square_bound_certified`), with refusal controls for a cross-context certificate and a
  stale model text. Reading checked `noble-ngraph` nodes as fragment nodes is the one trusted
  translation.
- Retained foundation receipts: `c0b0bdf93` (encoded graphs), `a41435fd2` (1223 theorems, contexts
  carried in the statements).

## The numoracle research note, mapped to controls that already exist

The issue's numoracle comment proposed four bounded comparisons. None needs new work to be
*addressed*; each is already covered by a kernel-decided control or is explicitly outside the
fragment. This table is the mapping, so a future broadening issue inherits the requirements rather
than rediscovering them.

| numoracle case | Existing control | Scope |
|---|---|---|
| A rounded intermediate used more than once versus separately rounded computations; DAG identity and error provenance kept | `cancellationGraph` reuses node 0 as both operands of the `mul` (`x · x`), and the checker reads one accepted bound per node position, so a shared node's error is propagated once and never re-derived; `checkNode_sound` is per node position over the stored order | the provenance kept is per node *position*; whether two separate re-computations of the same value are correlated is **not** modelled, and an analyzer claiming tightness from correlation cannot be checked here |
| Cancellation with signed constants, small magnitudes and rounding-boundary inputs; complete enclosure versus independently checked expectations | `Encoded.cancellation_certified` / `Encoded.cancellation_sound` (encoded graph, `x ∈ [1, 2]`, cancels towards zero); `cancellation_applies` shows the premises are satisfiable; the lane's two independent expected-value paths (host binary32 and exact dyadics) compare against every certified enclosure on 404,000 sampled node checks | sampling is supporting evidence, never the universal statement; the universal statement is `check_sound` |
| A mutation removing an intermediate rounding boundary, and one underestimating an error or range contribution, with the certificate bound to the exact ordered graph | `fma_contraction_is_not_applicable`, `reordered_execution_is_not_applicable`, `changed_snapshot_is_not_applicable`, `changed_domain_is_not_applicable`, `changed_revision_is_not_applicable`, `stale_model_is_not_applicable`, `mismatch_is_not_an_unknown_outcome` (binding); `implicit_contraction_is_caught` (#38 model mutation); `underestimated_error_is_refused`, `underestimated_range_is_refused`, `missing_input_error_is_refused` (checker) | boundary removal is refused as a *context* difference, not re-analysed; no rule accepts a differently-rounded graph |
| Missing finite-domain or underflow evidence must stay unsupported/inconclusive, never a passed check | `subnormal_needs_eta` (a bound without the absolute term is refused and would be false); `outside_operation_is_unsupported`, `later_operand_is_unsupported`; `oversized_is_exhausted`; the three outcome classes stay distinct | an unsupported or exhausted graph has no bound at all; it is never reported as verified |

Two further points from the numoracle comment are already policy here: the acceptance authority is
the proved total checker and its axiom allow-list (no probabilistic tail bound, calibrated constant or
external success flag is imported as an axiom), and a fixed-input acceptance region is not a
universally quantified certificate. The comment's tt-onix cross-reference is test-idea sharing only;
its BF4/BF16/SFPU arithmetic is not interchangeable with this binary32 RNE model.

## Correction to the NGRAPH-ERR-01 rationale

The delta said the ordered affine fold is excluded from the first fragment "because their rounding
layers remain open under #38". That reason is **stale**. Under #38 the fold's rounding layers are now
proved: `Sequence.fold_error` bounds the ordered affine fold of `n` terms under normal-range premises,
and `SequenceFinite.fold_finite_error` bounds it with subnormal intermediates under the single
premise that no rounding overflows (`proofs/foundation/NobleFoundation/Binary32Sequence{,Finite}.lean`).

The exclusion still stands, for a different reason: the checker has **no rule** for a fold node or a
piecewise operation. `Binary32Checker.checkNode` accepts `input`, `const`, `add` and `mul` only, so a
fold or activation is `unsupported` (`outside_operation_is_unsupported`). Proved rounding error is not
a certificate rule; the certificate still has to enclose each node and carry an error, and the
sequence bounds are a different shape. The change's spec delta records that corrected rationale.

## What would reopen it

Integration:

- a proved or reviewed correspondence between the machine model and the computing code — i.e. #38's
  reopen condition (a float-capable Aeneas, #92) or a separately reviewed refinement of a Wasm/Rust
  evaluator, plus an explicit model of the host arithmetic;
- producer-side identity derivation: the context digest computed from the producer's bytes under a
  scheme that resists substitution, with the comparison's non-cryptographic properties stated;
- a measured executable checker: Rust or Wasm checking the retained encoded artifacts, with its cost
  and refusal classes measured on the same corpus, not only the Lean kernel's elaboration cost;
- a bound on the mixed-magnitude loss, for example a per-node relative/absolute split or a
  domain-partitioning rule, measured against Gappa with a stated factor;
- an owner policy decision that certification may constrain execution at all.

Broadening the fragment (subtraction, ordered affine fold, activations, tensor shapes) is a new issue
with the four preconditions above plus its own rule soundness proof. It is explicitly **not** a
checkbox of #44.

## Non-claims

- No graph is certified, required or refused by these certificates today, and no execution-path
  behaviour changed.
- Gappa's enclosures are untrusted and not kernel-checked; no Gappa-generated proof is used.
- The checker's fragment is Lean model graphs plus two encoded add/multiply graphs. No Rust or Wasm
  evaluator is certified, and no host correspondence is claimed.
- The corpus measures the checker's own rules on 24 graphs from one seed; bound quality on another
  corpus is not claimed.
- Sampling is secondary evidence; the proof is the kernel-checked certificate.
