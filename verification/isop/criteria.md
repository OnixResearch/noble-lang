# ISOP prototype criteria (frozen 2026-11-13)

This document freezes the bounded prototype and the adoption rule before any
performance measurement. It separates the Lean model, Rust implementation,
independent finite checker, and unmeasured source-integration claim.

## Admitted Boolean region

The experimental input is a checked, topologically ordered DAG with only
Boolean constants, named Boolean atoms, NOT, AND, and OR. Node references MUST
point strictly backward; atom slots MUST be in range. One fixed variable order
is used: split the highest numbered remaining atom first, recursively down to
atom zero. The graph is pure and total: no branch effects, calls, exceptions,
host operations, floating-point operations, or source-language control flow.

The issue #32 source whitelist is not such a region. Its first-projection
program is exactly two I64 operations chosen from add, subtract, and multiply
(`proofs/foundation/NobleFoundation/FirstProjection.lean:153-167`); its residual
is one I64 graph (`:400-411`). The graph IR has I64 equality but no Boolean
AND/OR/NOT nodes (`crates/noble-ngraph/src/ir.rs:279-346`). The foundation
profile also explicitly excludes branching from its established fragment
(`proofs/foundation/README.md`, “Selected profile and whitelist”). Thus the
source whitelist census is 0 Boolean-composition regions by construction; it
is not a scan of runtime traces or a measurement of #32 output.

## Totality, interval and observation contracts

The machine input bounds are at most 6 atoms, 64 truth-table rows, 512 DAG
nodes, 1,024 memo states and 256 output cubes. Caller limits may lower but
MUST NOT raise these fixed caps. Every allocation is preceded by the relevant
size/cap check. DAG validation and truth-table construction are finite. ISOP
recursion decreases the number of remaining variables; its memo contains at
most the declared state cap. Any cap exhaustion returns fallback without an
optimized cover; malformed DAGs and invalid intervals are errors, never
fallback success.

For each row `r`, a proposed cover `P` is admissible exactly when
`L(r) ⇒ P(r) ⇒ U(r)`. The production-independent checker enumerates the finite
rows, evaluates lower/upper DAGs directly, and checks each cube's literals;
it MUST NOT call the ISOP recurrence or consume its truth-table memo. Exact
mode (`L = U`) therefore requires equality on every row. This relation is the
only semantic observation in scope. The Lean model proves the interval
property; Rust tests show finite agreement between the core and this independent
checker. Neither implies source, compiler, runtime, or Wasm correspondence.

## Frozen synthetic comparison

The benchmark corpus is fixed in the Rust test producer and consists of five
checked DAGs: (1) `x0 ∧ x1 ∨ x0 ∧ ¬x1` with 2 atoms; (2) the same absorption
identity over a 6-atom interface with four irrelevant atoms; (3) a 4-atom
parity chain; (4) a 3-atom shared-factor region
`(x0 ∨ x1) ∧ x2 ∨ (x0 ∨ x1) ∧ ¬x2`; and (5) a 3-atom region with explicit
true/false constants and a repeated atom. All rows and outcomes are checked
before timing.

Three arms use identical checked inputs and cycle through all valuation rows:

1. **No pass:** evaluate the original DAG.
2. **Folding:** apply only Boolean constant, identity, and repeated-child
   folding, then evaluate the folded DAG. This is a synthetic control, not an
   existing source-language Boolean optimizer.
3. **ISOP:** build the exact interval cover, pass the independent exhaustive
   checker, then evaluate the cover.

Record preprocessing and per-evaluation latency separately, plus node/cube
counts. Use the pinned release build, 30 calibrated samples per arm with the
existing ≥20 μs batching helper, and three independent invocations on the same
committed source and host. Report all three runs; do not drop outliers or
replace medians with a pooled value. This synthetic comparison characterizes
the prototype only; it is not an adoption measurement.

## Frozen adoption rule and current decision boundary

Source integration is GO only if all conditions hold: (a) a separately
approved #32 language extension admits at least one real Boolean-composition
region while preserving K-CONTROL-01 branch laziness; (b) every admitted
region is within this whitelist and every rewrite passes the independent
interval checker with zero disagreements and no cap fallback; and (c) over
three source-bound runs, end-to-end cost including analysis, independent
checking, and use is at least 10% lower at the observed dynamic invocation
count than both no pass and the folding baseline. Missing any condition is
NO-GO/deferred; synthetic results cannot satisfy (a) or (c).

The present whitelist census is zero, and there is no admitted Bool/branch
extension. Therefore the current source-integration decision is
NO-GO/deferred: no admissible workload. Reopen only after a reviewed #32
language extension preserves branch laziness and produces a real residual
corpus containing Boolean-composition regions. Freeze SHA: recorded by the
criteria commit in the slice history before benchmark execution.
