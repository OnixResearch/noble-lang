## Why

Checked graph facts, differential oracles and bit-exact execution tests do not
state how far a selected binary32 result may be from a separately defined
real-valued computation. FloVer separates an untrusted range/error analyzer
from a formally proved certificate checker; Noble needs that separation with
Lean as the proof authority, and its published IEEE bridge is a binary64 result
with an avoids-subnormals premise, so it cannot simply be reused as a guarantee
for Noble's binary32 gradual-underflow semantics.

r[NGRAPH-ERR-01] r[NGRAPH-ERR-02] r[NGRAPH-ERR-03] r[NGRAPH-ERR-04]
r[NGRAPH-ERR-05]

## What Changes

- **ADDED** `NGRAPH-ERR-01`: the finite acyclic add/multiply fragment, exact
  input and constant bit-pattern interpretation, the separately defined
  intended real graph that preserves stored operation order, and the input
  relation, which either treats real inputs as the exact dyadic values of the
  supplied F32 inputs or includes a separately bounded input-rounding error.
- **ADDED** `NGRAPH-ERR-02`: the certificate shape and a total Lean checker whose
  acceptance implies the per-output bound for every input satisfying the checked
  premises, with the machine-to-real bridge to #38's binary32 reference proved
  rather than assumed.
- **ADDED** `NGRAPH-ERR-03`: subnormal/gradual-underflow policy. Either a proved
  absolute-error rule covers subnormals or the stronger no-subnormal premise is
  mechanically established and unsupported domains are refused; a real-range
  interval alone is not evidence for the premise, and flushing to zero or
  weakening the execution contract is forbidden.
- **ADDED** `NGRAPH-ERR-04`: certificate binding to graph/parameter snapshot,
  ordered interfaces/outputs, constants, input domain, input-error model,
  RNE/overflow policy and semantic/model/checker revisions, plus the consumer's
  applicability check before a bound is advertised, and invalidation on any
  snapshot edit.
- **ADDED** `NGRAPH-ERR-05`: bounded certificate parsing, rational sizes, range
  operations and checking work, with `unknown`, `unsupported` and `exhausted`
  distinct from a disproved bound.

This is a design change. It adds no checker to the Rust build, requires no
certificate for ordinary graphs, and does not touch `NumericGraph-Design`,
`ngraph-ir/0` admission or any execution path.

## Impact

- Rust source subjects: none. The checker is named as future, separately
  admitted work; `noble-ngraph` keeps its current operation set and admission.
- Affected specs: `.cairn/specs/backend-experiments/spec.md` (delta only).
- Dependencies: the design is gated on #38's binary32 model and rounding bridge.
  Where that bridge is not established, the affected statements stay open rather
  than being assumed.
- Adoption: none. No automatic precision change, fast-math, or required
  certification for ordinary graphs.
