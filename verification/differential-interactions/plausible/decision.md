# Plausible P1 pilot — frozen decision criteria

This prospective protocol selects four cells from the already-frozen P1 source-to-Wasm matrix, rather than adding a new semantic profile. `matrix-v1.json` freezes the subset, seed, 16 cases per arm and optimization mode, generator size, shrink cap, controls and three-way decision before any new measurements. A positive verdict on one finite campaign is not an implementation theorem, a general Plausible adoption, or source-to-Wasm correspondence for all programs. The current foundation generator and its independently checked BigInt/Lean expectations remain authoritative. The generated program source is executed verbatim; an AST tag alone does not witness reachability.

A typed Lean test driver may import Plausible and generate/shrink model cases, but no foundation theorem source may invoke its test-passing tactic. The proof gate must reject that tactic's `sorryAx` dependency. The selected interactions include literal I64 wrapping, ordered subtraction, quotation/composition order and preserved Bool/I64 prefix order. P2, snapshots and per-run fuel remain unsupported. A tested minimized witness must preserve admission, its selected cell and originally attributed failure, not merely remain interesting; a bounded local minimum is not a global minimum.

## Measured decision: adopt this optional stratified P1 arm

At clean source commit `fb63cb968`, the pinned Lean driver generated 16 cases
from master seed 125 (four attempts per cell). The existing foundation
comparison executed **16 current and 16 Plausible cases in each of opt-off
and opt-on**. Of the four *preselected* cells, Plausible reached 4/4 and the
current seeded generator reached 0/4 (it reached one different P1 matrix
cell). Both arms had zero attributed baseline failures. Plausible had 32
accepted generated execution rows, zero generated refusals/exhaustions, and
one separately counted fixed invalid-premise refusal. The per-case independent
BigInt reference and Lean-kernel-checked expectations were reused, not inferred
from the candidate executable.

All seven existing differential oracle controls failed for their intended
reason. Four test-side simulations of the existing P1 mutant-bank effects
were detected on admitted, execution-witnessed cases; **the actual
implementation-side mutant binaries were not rebuilt in this follow-up**.
Their earlier, source-bound qualification remains in the original
`verification/differential-interactions/receipts/756ccc9/campaign.json`.
One output-order fault was shrunk from 27 to 15 source bytes; 48/48 attempts
were spent and the termination class is *attempt-budget-exhausted*, not
globally minimal. Profile changes, removal of the target interaction or
prefix premise, earlier refusal and a different arithmetic failure were
each rejected. A fresh process re-executed both retained source byte strings
without invoking the generator. A scratch `by plausible` proof reported
successful randomized testing but depended on `sorryAx`; the foundation
gate's actual axiom predicate rejected it with exit 1. None of that test
output is proof authority.

The frozen adoption threshold is met (four additional selected-cell reaches
versus the required two) with all safety controls satisfied: **adopt the
optional, cell-indexed Plausible P1 test arm for these four selected
interactions**. Retain the current foundation generator as the default for
other fragments and profiles. This is a selected-subset comparison, not a
claim that Plausible intrinsically outperforms the current generator under
arbitrary unstratified seeds, nor that the implementation is correct for
arbitrary programs. P2, runtime fuel overrides, parameter snapshots and
universal correspondence remain unsupported.

The immutable receipt index at
`verification/differential-interactions/plausible/receipts/fb63cb9/index.json`
binds the original case bytes, model/source/tool identities, independent
comparison, emitted expected-proof source, both shrink witnesses, fresh
replay and proof-gate negative control. Reopen only with another frozen
matrix/seed/budget or new failure evidence; do not relabel this historical
receipt for changed candidate bytes.
