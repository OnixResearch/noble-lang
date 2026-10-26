## Context

Grounded at worktree HEAD `ef4044a30` (issue #44 cites `2c96d4c1`; the cited
facts still hold):

- `crates/noble-ngraph/src/ir.rs` declares `IR_VERSION = "ngraph-ir/0"`,
  `SEMANTIC_REVISION = "ngraph-sem/0"`, `FACT_RULE_REVISION = "ngraph-facts/0"`,
  and an `Op` set whose binary32 arithmetic is `F32Add`, `F32Sub`, `F32Mul`,
  the ordered affine fold `F32Affine` (documented
  `v₀ = bias; vᵢ = rn(vᵢ₋₁ + rn(cᵢ × xᵢ))` with terms in stored order), plus
  `F32Relu`, `F32Reglu`, `F32Step` and `F32Select`. Graph facts and differential
  oracles exist; no roundoff statement exists.
- `proofs/foundation/NobleFoundation/Binary32.lean` (issue #38) is a
  kernel-reducible model with raw 32-bit encodings, an exact scaled-integer
  interpretation of finite encodings, round-to-nearest ties-to-even with
  **gradual underflow**, observation classes where every NaN is one `obsEq`
  class and signed zeros stay distinct, and the three selected arithmetic
  operations. It is a model and says nothing about the Rust evaluator,
  `wasm-opt` or a host FPU.
- `.cairn/changes/binary32-lean-reference/tasks.md` still leaves open the
  ordered affine fold's per-operation rounding/extraction layers and the
  `sem.rs` correspondence decision. The rounding bridge for this design therefore
  starts from the model, not from a completed correspondence claim.

## Decisions

### Decision: certificate over a fragment, with the real graph defined separately

**Choice:** the certificate covers a finite acyclic add/multiply fragment over
explicit finite inputs with no overflow, division, branch discontinuity or
training derivative claim. Every constant and input bit pattern is interpreted
exactly, and the intended real graph preserves the stored machine operation
order. `F32Affine` is excluded from the first fragment even though it is
add/multiply-like, because its documented bias and per-term rounding order need
their own bridge statement.

**Rationale:** a roundoff certificate over an unspecified operation set is a
claim about the wrong graph. Excluding the affine fold and the piecewise
operations keeps the first theorem small enough to be checked and keeps their
rounding layers in #38, where they are already named as open.

### Decision: the input relation is explicit, never implicit

**Choice:** real inputs are either the exact dyadic values of the supplied F32
inputs, or the certificate includes a separately bounded input-rounding error.
Initial representation error MUST NOT be silently omitted.

**Rationale:** an interval over already-rounded inputs bounds only the
*intermediate* rounding. Reporting it as a bound from the reals would overstate
what was proved, which is exactly the reuse FloVer forbids.

### Decision: checker acceptance implies the bound; the analyzer is untrusted

**Choice:** an untrusted analyzer proposes rational input ranges, per-node
real/machine range enclosures and absolute-error bounds. A small total Lean
checker accepts or refuses the certificate, and the proved statement is
"accepted certificate ⇒ per-output bound for every input satisfying the checked
premises". The analyzer's search is never the proof.

**Rationale:** this is the FloVer architecture with Lean as authority. It also
keeps search quality (how tight a bound is found) separate from soundness (the
bound that was found is true).

### Decision: subnormals are handled or refused, never flushed

**Choice:** either a proved absolute-error rule covers subnormals/gradual
underflow, or the stronger no-subnormal premise is mechanically established for
the domain and unsupported domains are refused. A real-range interval alone is
not evidence that every rounded intermediate satisfies that premise.

**Rationale:** the published binary64 bridge carries an avoids-subnormals
premise; Noble's reference has gradual underflow. Silent flush-to-zero would
weaken the execution contract this certificate is supposed to describe.

### Decision: applicability is a consumer obligation

**Choice:** a certificate binds the exact graph/parameter snapshot, ordered
interfaces/outputs, constants, input domain, input-error model, RNE/overflow
policy and semantic/model/checker revisions. A consumer MUST establish
applicability for the actual inputs before advertising a certified bound, and
any snapshot edit invalidates the old certificate.

**Rationale:** a bound is only meaningful for the artifact and domain it names;
reusing it after a snapshot change would certify bytes that were never checked.

### Decision: budgets distinguish `unknown`, `unsupported` and `exhausted`

**Choice:** certificate parsing, rational sizes, range operations and checking
work are bounded, and `unknown`, `unsupported` and `exhausted` are distinct from
a disproved bound.

**Rationale:** a search that timed out, an unsupported domain and a refuted
bound are three different facts; collapsing them would let an unproved case be
read as a success.

## Risks / Trade-offs

- The first fragment excludes the affine fold and piecewise operations, so it
  cannot yet certify the graphs most training code builds. That is deliberate:
  the affine fold needs #38's per-operation rounding statement first.
- Rational interval arithmetic can be expensive on cancellation-heavy graphs;
  the budgets and the analyzer comparison exist to detect that rather than to
  promise a bound is cheap.
- A certified bound is not a claim about the Rust/Wasm execution chain, nor
  about derivative correctness; #42 and #43 keep their separate obligations.
- The no-subnormal premise may exclude much of the useful input domain. If the
  proved absolute-error rule cannot be established, the honest outcome is a
  narrower domain, not a weaker premise.

## Fixtures and plan

Phase 2 (future, unchecked here) would add:

- one nontrivial bound on an actual encoded graph, including a cancellation case
  and a near-overflow case;
- negative controls: underestimated error, underestimated range, missing input
  error, changed operation order, FMA contraction, wrong snapshot/domain/output,
  stale model revision, unproved underflow premise and an oversized certificate;
- an independent expected-value lane that does not reuse the adapter's
  conversion or the analyzer's arithmetic as the oracle;
- a bound-quality and checker-cost comparison with an untrusted analyzer on a
  pinned bounded corpus, with numerical sampling recorded as secondary evidence
  only.

## Proof obligations

Named here, not discharged by this package:

- a total Lean checker whose acceptance entails the per-output absolute-error
  bound under the checked premises;
- a machine-to-real bridge statement relating the checker's abstract rounding
  and range model to #38's `Binary32` reference for the covered fragment,
  including whichever subnormal policy is selected;
- a range-operations statement, so closure of the proposed enclosures is checked
  rather than trusted;
- a binding statement for snapshot/domain/revision identity.

Numerical sampling, differential oracles and analyzer search are evidence about
implementation or search quality, never the proof.

## Acceptance mapping

| Issue criterion | Where |
|---|---|
| Theorem, machine-to-real bridge, admissible fragment and applicability checks, reusing the #15/#38 ledger | NGRAPH-ERR-01, NGRAPH-ERR-02, NGRAPH-ERR-04 |
| Small Lean-checked prototype on an actual graph including cancellation and near-overflow | Phase 2 task |
| Negative controls for underestimated error/ranges, missing input error, changed order/FMA, wrong snapshot/domain/output, stale model, unproved underflow premise, oversized certificates | NGRAPH-ERR-03, NGRAPH-ERR-05, fixtures |
| Bound quality and checker cost versus an untrusted analyzer | Phase 2 task, fixtures |
| Go/no-go before broadening operations or integrating certification | Phase 2 task |

## Non-claims

- No proved numerical guarantee yet: this package designs the certificate and
  names its obligations.
- No hidden native/SMT trust, no `native_decide`, no automatic precision change,
  fast-math or reassociation.
- No derivative-correctness, schedule-correspondence or Rust/Wasm-chain claim.
- No required certification for ordinary graphs, and no promotion of
  `NG-PO-A..H`.
- The design is gated on #38: statements that depend on the rounding bridge stay
  open until it is established.
