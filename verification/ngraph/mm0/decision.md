# Issue #127: frozen MM0 pilot decision rule

This decision rule is committed **before** any Noble/MM0 fixture measurement. It governs only the opt-in, one-step FoldAddConstants / pure-I64 profile; current Rust replay, Lean checks, independent replay's pilot status and ordinary execution are unchanged.

## Continuation rule

Expand the experimental profile **only if all** conditions hold on one clean, source-identified snapshot:

1. Compiler-lab #220 provides a qualified frozen-specification consumer using a pinned, parser-enabled MM0 checker. The consumer independently reconstructs the expected claim from Noble's admitted original graph, rewritten graph and ngraph-cert/0 derivation; a producer-supplied hash alone is insufficient.
2. A real admitted fold and a signed-wrapping-boundary fold each pass graph admission, Rust certificate replay, applicable Lean checks and MM0 checking of a nonempty, separately frozen theorem inventory. The MM0 conclusion expresses equivalence of **all ordered outputs for every input**, not only equality of one scalar constant. Exact 64-bit modular arithmetic, node order, sharing and slots have an explicit interpretation. No desired fold claim is declared as an axiom.
3. Retained controls refuse changed endpoints (including another valid graph), output order, sharing, operand bits, a valid proof of another claim, changed definitions/axioms, malformed/truncated proof, stale subject and resource exhaustion. The receipt records each actual failure stage; unapproved axioms are never accepted merely because the raw verifier checked them.
4. Fresh, bounded replay consumes the **retained** full specification/proof/subject bytes without the original proposer, verifies the selected binary/profile/source identity and every required control, and distinguishes nonaccepting classes. Costs for proof generation and checking are measured separately (wall time, peak RSS, byte sizes), alongside existing Rust/Lean costs and duplicate semantics.
5. Any theorem-transfer claim to Noble's Lean denotation is supported by a checked representation argument. Without one, report only an MM0-model theorem, Rust/Lean evidence separately, and the semantic/encoding assumption.

If **any** condition is missing or unsupported, the decision is **NO-GO for expansion and promotion**. Direct use of upstream mm0-c without the qualified compiler-lab adapter may provide useful local pilot evidence, but it does not satisfy condition 1. Cost alone cannot override a failed safety condition. Reopen on a qualified compiler-lab backend plus a clean, retained run meeting all controls; a materially altered model or checker requires a new frozen profile and replay rather than changing this receipt.

## Observed decision

**NO-GO for expansion or promotion.** The frozen source
`26f5e25855ba1db01b51c6686934b7b86fb272ba` produced the complete
retained bytes in `retained/spec.mm0`, `retained/proof.mmb`,
`retained/controls/` and `subjects/`. The source-bound generation/checking
measurements are in `retained/production.json`; the separate **proposer-free**
consumer's two acceptances and 17 refusals, including their actual failure
stages and checker statuses, are in `retained/replay.json`. The consumer is
replayable as:

```sh
node verification/ngraph/mm0/replay.mjs \
  <built parser-enabled MM0 checker selected by profile.json> \
  verification/ngraph/mm0/retained <new directory outside this repository>
```

The selected *experimental* verifier is upstream mm0-c at
`0d414c0bfdaaeb7fea571895127abc1fa5a3d956` (source archive, compiler
wrapper, flags, producer lock and both executable digests pinned in the
tool-selection policy, `profile.json` and production receipt). It is built
with the **parser enabled**, checks the entire flattened MM0 specification,
and rejects a proof containing the `Sorry` opcode (raw exit 3). An otherwise
valid proof with a deliberately different theorem and an arbitrary-proposition
axiom both pass raw checking against *their own* specifications (exit 0) but
fail this adapter's independently established specification digest. The
NO_PARSER build verifies the original proof despite a changed arithmetic
definition (raw exit 0), yet the adapter refuses its binary identity.
Spec/model changes fail at `spec-digest`; malformed/truncated proofs fail
inside the parser-enabled checker (exit 255); the 16 MiB address-space run
terminates with exit 139 and the 1 ms wall limit with `SIGKILL`/`ETIMEDOUT`.
Neither resource result is acceptance.

The two retained, Rust-emitted, independently admitted graph/certificate
pairs are 238/236/42 bytes (3 + 5) and 256/273/78 bytes
(`i64::MAX + 1 → i64::MIN`) respectively. Both bind the same one-slot
topology, operands in order, constants also used as ordered outputs, the
shared add/mul use and all **three** outputs; the final `endpoint_small` and
`endpoint_boundary` MM0 theorems quantify the symbolic input. The selected
whole flattened specification is 34,499 bytes and lists 33 exact, approved
upstream Peano/logic axioms; the proof is 1,242,704 bytes. No desired fold
equivalence is an axiom. The proof producer took **344.8 ms** wall
(**102,016 KiB** peak RSS), and the selected parser-enabled checker took
**23.9 ms** wall (**1,588 KiB** peak RSS) under 1 GiB address-space and
20-second CPU caps. The independently replayed small/boundary checks took
8–10 ms each on this run. The focused warm Rust graph-admission and
certificate-replay test took 0.12 seconds and 37,892 KiB peak RSS **for the
whole Cargo command**, so these differently scoped process measurements
do not establish a matched-budget performance advantage. The unchanged
whole-tree Lean m3/foundation build and axiom/inventory audit passed all
four stages (`m3-build`, `m3-gate`, `foundation-build`,
`foundation-check`, each exit 0); the external timing of that **whole**
script was 301.55 seconds and 4,133,296 KiB maximum RSS, including
global build-slot waiting and thousands of unrelated declarations. It
is **not** a per-fixture comparator. The existing model-level
`accepted_fold_step_denotes` and `accepted_endpoint_equivalence`
theorems are preserved, not replaced or derived from MM0.

The MM0 model encodes an I64 value as a Peano natural modulo $2^{64}$,
reads the high bit as a negative Rust I64 solely in the independent consumer,
and uses ordered nested natural constructors as an output tuple. Noble's Lean
certificate model uses `BitVec 64` and its own graph/derivation denotation.
These are **duplicated semantics**, not a shared checked representation.
The consumer's graph-to-MM0-term projection, parser, proof selection, Rust
implementation-to-Lean correspondence, compiler lowering and runtime
correspondence remain unproved. Existing Rust certificate checks and Lean
model theorems are separate evidence; this direct MM0 run proves only its
bounded MM0-model endpoint theorem under the pinned axioms and encoding.
Neither the checked representation bridge nor Compiler Lab #220's qualified
backend is available here, so conditions 1 and 5 of the frozen continuation
rule fail regardless of the positive local run. The experiment adds no
production dependency, changes no ordinary execution, and does not promote
the separate independent-Lean replay pilot.
