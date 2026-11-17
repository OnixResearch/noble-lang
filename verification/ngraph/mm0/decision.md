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

## Observed decision: direct upstream run

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

## Qualified backend recheck (compiler-lab 46a988b)

**Still NO-GO for expansion or promotion.** Condition 1 is now met;
condition 5 is not.

The qualified backend is Compiler Lab #220's `validate mm0-certificate`
at compiler-lab commit `46a988bcbc3bf9b3a6296cecbef8eb4d499c454e`. The
`qualified_mm0` record in the tool-selection policy pins the commit, the
Nix-built tools closure and closure-identity digest, the qualified
parser-enabled `mm0-c` digest (`7b00b9c1…`), the negative `NO_PARSER`
control digest, the pins and expression digests, and the digests of the
`peano.mm0` and `peano_hex.mm0` base theory. Compiler-lab is private, so
Noble records these identities, not a fetch location. That checker is a
different build from this lane's direct checker (`9fe789c0…`, profile.json).
For condition 1, Noble consumes compiler-lab's admission report and does
not run its own checker build.

The narrow consumer is `compiler-lab-request.mjs`:

```sh
node verification/ngraph/mm0/compiler-lab-request.mjs \
  <compiler-lab checkout at the selected commit> <compiler-lab binary built from it> \
  <tools root written by compiler-lab's setup-mm0-certificate.py> \
  <new directory outside this repository>
```

The consumer refuses another compiler-lab commit, tools closure, checker,
pins or base theory. It builds the frozen request only from the retained,
digest-pinned inputs in this lane:

- It decodes both graphs and the ngraph-cert/0 step, replays the
  FoldAddConstants step on the original graph, and requires the exact
  rewritten graph.
- It derives the endpoint-equivalence and fold-fact declarations itself
  from the graphs and the certificate. The numerals come from the base
  theory's successor numerals and the evaluated hex definitions of
  `theory.mm0`. It refuses a caller theory that states anything else.
- The approved assumptions are exactly the 49 axioms of the pinned base
  theory. The original line-anchored inventory in `profile.json` listed
  33; it missed the 16 digit-value axioms `h2n0`–`h2nf`, which share a
  line with a theorem.
- It computes the compact frozen digest itself and requires every backend
  report to bind that digest. All 19 reports carry `0989392a…`.
- It compares the backend's reconstructed output terms with its own
  derivation.

The receipt is in `retained/compiler-lab-46a988b/`. It holds
`receipt.json`, the scrubbed `request.json`, and the 19 path-scrubbed
backend reports. They were produced at the clean Noble commit `16d8c7c3`
from a clean compiler-lab checkout. All runs reported `producer_invoked:
false`.

| Run | Changed input | Outcome | First refusing stage | Checker stage |
| --- | --- | --- | --- | --- |
| positive | none | accepted (exit 0), 23/23 controls held | — | accepted |
| replay of positive | retained bundle | accepted; retained and fresh verdicts agree | — | accepted |
| malformed-proof | proof header zeroed | refused | logical-check | mmb-header |
| truncated-proof | proof cut in half | refused | logical-check | mmb-proof |
| unrelated-endpoint | small after.graph uses another valid node | refused | claim-reconstruction | accepted (raw) |
| reordered-outputs | small after.graph output order | refused | claim-reconstruction | accepted (raw) |
| changed-shared-use | small before.graph shared use | refused | claim-reconstruction | accepted (raw) |
| changed-consumed-bits | small before.graph constant 5 → 6 | refused | claim-reconstruction | accepted (raw) |
| changed-certificate-bits | small certificate 8 → 9 | refused | claim-reconstruction | accepted (raw) |
| proof-of-another-claim | `different` specification and proof | refused (obligations also refused) | assumption-policy | accepted (raw) |
| unapproved-axiom | `forged` arbitrary-proposition axiom | refused | assumption-policy | accepted (raw) |
| sorry-proof | `sorry` specification and proof | refused | assumption-policy | sorry |
| extra-axiom | specification plus `oracle_any` | refused | assumption-policy | specification-match |
| changed-axiom-formula | `peano1` formula | refused | assumption-policy | proof-or-specification |
| changed-arithmetic-definition | specification `uadd` uses `*` | refused | definitions | proof-or-specification |
| changed-model-theory | theory and specification `umul` modulus | refused | definitions | specification-match |
| stale-subject | small subject identity, same meaning | refused | subject-identity | accepted (raw) |
| exhausted-wall-limit | caller `--timeout-ms 1` | unavailable; first Nix command `timeout` | checker-identity | not run |
| exhausted-storage-limit | caller `--max-storage-mib 1` | unavailable; first Nix command `storage_limit` | checker-identity | not run |

Every admission also reruns the backend's 23 required controls. On the
positive submission these include the 16 MiB address-space run on the
presented proof (`resource-exhausted`, exit 139) and a specification
stream that never ends (`timeout` at 1,001 ms). When caller limits stop
closure authentication, the backend reports `unavailable`, not its
documented `timeout` or `storage-limit` classes. The receipt therefore
also keeps the command status that names the cause. No resource run is
acceptance.

Costs, from this receipt. The qualified fresh check of the retained proof
took 11 ms and 1,836 KiB peak RSS. A whole admission, including Nix
closure authentication and the 23 controls, took 3.4–5.8 s of wall
time. The artifacts are the same 34,499-byte specification and
1,242,704-byte proof. Generation costs are unchanged (`production.json`).
The backend's reconstruction duplicates the claim derivation, alongside
this consumer, `replay.mjs`, the Rust checker and the Lean model: five
encodings of the same fold semantics, none proved equal to another.

The backend's trust section lists these remaining correspondence
assumptions:

- the standard-model reading of numerals and the `h2n` digit axioms;
- cons injectivity for output tuples;
- the Euclidean reading of `div` and `mod`;
- the ngraph-sem/0 reading of `i64.add`, `i64.mul` and input slots;
- the trusted `mm0-c`/GCC/glibc/kernel/hardware stack.

It reports these items as unresolved:

- a checked MM0-to-Lean representation argument;
- the unproved graph-to-term projection and subject decoder;
- correspondence with the Rust checker, lowering, emitted Wasm and runtime.

Conditions after the recheck:

1. **Met.** The qualified, pinned, parser-enabled consumer independently
   reconstructs the claim, and Noble independently derives the same claim.
2. **Met for MM0 and Rust.** Both fixtures pass `mm0::retained_subjects_equal_source_generated_admitted_pairs`
   (graph admission, Rust certificate replay and byte regeneration) and
   the qualified MM0 check. The model-level Lean theorems are unchanged and
   covered by the batch-13 foundation receipt
   (`verification/foundation/receipts/b9ab2ceff/`). Lean is not run per
   fixture.
3. **Met.** See the refusal table and the 23 controls of each report.
4. **Met.** The backend replay and `replay.mjs` consume retained bytes
   without the proposer.
5. **Not met.** There is still no checked representation argument
   between the MM0 Peano model and Noble's `BitVec 64` Lean denotation.

The decision therefore stays **NO-GO for expansion and promotion**. The
frozen rule is unchanged. Reopen only with a checked MM0-to-Lean
representation argument for this profile, followed by a fresh clean run
of this consumer. A changed model, checker, backend commit or closure
requires a new frozen profile and a new receipt.

## Issue #127 evidence map

| Acceptance item | Evidence |
| --- | --- |
| Real fold and signed-wrapping boundary pass the existing checks and the selected verifier | The Rust test above; `replay.mjs` with the direct checker (`retained/replay.json`); the qualified `positive` and `replay` reports in `retained/compiler-lab-46a988b/` |
| Unrelated endpoint, output order or sharing, consumed bits, or a proof of another claim is refused | unrelated-endpoint, reordered-outputs, changed-shared-use, changed-consumed-bits, changed-certificate-bits and proof-of-another-claim rows, and the backend's matching controls |
| Changed arithmetic or model, unapproved axiom, malformed or truncated proof, stale subject and exhausted limits cannot accept, with the actual stage | the corresponding rows, their stages and checker stages, and the positive report's resource controls |
| Fresh replay checks the exact retained fixture without the proposer | the backend `replay` row (retained and fresh verdicts agree; `producer_invoked: false`) and `retained/replay.json` |
| Costs, sizes, duplicated semantics, remaining assumptions and a go/no-go | the cost and assumption paragraphs above, `retained/production.json`, and this NO-GO |

| Bounded implementation step | Evidence |
| --- | --- |
| 1. Real Rust-produced, admitted fixtures | `subjects/`, regenerated and admitted by the Rust test |
| 2. Frozen specification and model policy | `theory.mm0`, `retained/spec.mm0`, `profile.json`, and this file's frozen rule |
| 3. Untrusted producer and narrow consumer adapter | `produce.mjs` and `compiler-lab-request.mjs` |
| 4. Existing checks kept | Rust test, Lean receipts and the independent-replay pilot status are unchanged |
| 5. MM0-to-Lean relationship stated | the duplicated-semantics paragraphs and condition 5 above |
| 6. Complete retained bytes, identities, costs and the backend's refusal classes | `retained/`, `retained/compiler-lab-46a988b/receipt.json`, and the backend reports |
