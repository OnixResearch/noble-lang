# Revision-3 checked in-place plan decision (#110)

**Decision: retain the checked safe-Rust plan path as an opt-in correctness experiment; NO-GO for performance-based adoption.** It is not the default evaluator. This decision does not amend #37's frozen packed/net threshold of 1.15 or its NO-GO result.

## Frozen rule and matched evidence

[The criteria](criteria.md) were committed as `f5e4710b` before timing. The clean-source [current comparison receipt](comparison-2026-11-15-unit-clean.json) names source commit `09d2fd7cec0f8e6332dc22c50f18df46a2158213` and test executable SHA-256 `2afcda6b5e6f719aa16c392e3cd29ca130fe57fe9e1856164a524f43bef7bd33`.

Earlier receipts remain immutable: the [first comparison](comparison-2026-11-14.json), source `69b5ef3`, includes a 1.1402 chain regression; the [queue-guard comparison](comparison-2026-11-14-final.json), source `2e2bb92`, preceded restoration of the unchanged revision-3 net lowering identity; the [stable-lowering comparison](comparison-2026-11-14-stable-lowering.json), source `f4ef54ff`, preceded the checked operation-counter and fixture changes; the [prior checked-plan comparison](comparison-2026-11-14-checked-plan.json) predates the extended foundation proof; the [byte-grammar comparison](comparison-2026-11-14-byte-arena.json) preceded four finite rule-image fixtures; the [four-rule-image comparison](comparison-2026-11-15-four-rule-images.json) preceded the decoded physical-executor proofs; the [physical-image comparison](comparison-2026-11-15-physical-images.json) preceded the generic output-identity theorem; the [physical-free-ports comparison](comparison-2026-11-15-physical-free-ports.json), source `edfb39a5`, preceded the Rust-emitted Lean plan corpus; and the [corpus-bound comparison](comparison-2026-11-15-corpus-bound.json), source `32113b7c`, preceded the parser-only counterexample. Their timings are not substituted for the current-source measurements.

The [parser-bound comparison](comparison-2026-11-15-parser-bound.json), source `178520ff`, preceded the grammar-valid missing-write Rust control and retained chain/fanout regressions. The [checker-control comparison](comparison-2026-11-15-checker-control.json), source `4d59ae5`, preceded the three byte-offset unit-suffix lint corrections. Each family uses the same checked graph, lowered net, input row and completion rule in both arms, with one untimed warm-up and 31 alternately ordered pairs. Timing spans independent candidate validation, checked-plan compilation, reservation and execution, complete readback, and reducer-local cleanup. Source admission and net lowering are intentionally excluded from both arms. Full ordered raw samples, all resource counters, source/executable/checker/plan/representation identities, and the plan/replay digests are retained in each receipt.

| Family | Median reuse / allocate (ns) | Ratio (reuse ÷ allocate) | Interactions in each arm | Reuse slots allocated / reused / freed | Allocate slots allocated / reused / freed | Arena high-water reuse / allocate |
|---|---:|---:|---:|---:|---:|---:|
| chain | 116738 / 117520 | 0.9933 | 6 | 0 / 6 / 6 | 6 / 0 / 12 | 7 / 13 |
| fanout | 127267 / 131957 | 0.9645 | 6 | 0 / 8 / 4 | 8 / 0 / 12 | 6 / 14 |
| erasure | 28814 / 28814 | 1.0000 | 1 | 0 / 0 / 2 | 0 / 0 / 2 | 3 / 3 |
| asymmetric subtraction | 48451 / 48621 | 0.9965 | 2 | 0 / 2 / 2 | 2 / 0 / 4 | 3 / 5 |
| wrapping subtraction | 48611 / 48831 | 0.9955 | 2 | 0 / 2 / 2 | 2 / 0 / 4 | 3 / 5 |

All paired full observations agree with the independent reference evaluator; abstract interactions agree across storage policies. The reuse arm demonstrably avoids six, eight, two and two slot allocations respectively in the non-erasure families; erasure needs no replacement. The current source's median ratios lie between 0.9645 and 1.0000. This is not a demonstrated backend speedup: the preceding parser-bound comparison retained chain/fanout regressions, the physical-image receipt shows a 1.0161 chain ratio, and the oldest receipt retains a 1.1402 chain regression. Timings are descriptive and subject to host contention; no statistical claim or adoption threshold is inferred from these five small fixtures. Both arms record committed live nodes (chain/erasure/subtraction: 1; fanout: 2), transient live slots (reuse/allocate: chain 7/8, fanout 6/7, erasure 3/3, subtraction 3/4), queue high-water (fanout 2, all others 1), scratch reservation high-water (256 plan-buffer bytes), plan-byte totals and internal microsteps separately from abstract interactions in each receipt. Scratch here is the bounded encoded-plan buffer, not a measurement of all Rust stack frames.

The reuse/allocate plan streams differ under the same rule revision; both the exact-byte plan digest and the replay digest change. The Rust replay digest binds the graph/input-bound net key, rule, plan, representation, checker and executor revisions and the emitted plan stream. The receipt's separate evidence key additionally binds the clean source commit, test executable SHA-256, pinned inets upstream commit `98bbb01070664e4462a35efa5d62de6f27143572`, verified vendored theorem bytes (SHA-256 `c7a35dad49822583a9ee209b47d8ba5617d5abcb5a7f9f96df05e8502f362f54`) and current foundation plan-model source (SHA-256 `692862f23af90024d249e2814ede99babbd9bd590805deb5740dbc4eb9110d84`). A rule name alone cannot authenticate a prior result. These formal-reference hashes are provenance, not a Rust-to-Lean refinement proof.

## Previous correctness scope and reopen conditions (before semantic admission)

**Model proof:** the kernel-checked foundation model preserves the selected revision-3 right-hand side for both orientations of OP2, OP1, DUP and ERA. The compiled local image commits as one inets `HNet` firing, with observational pre-commit stutters, frame preservation outside declared agents/wires, and one abstract `CStep` under the stated matching/redex premises. The independent version-one Lean writer/parser roundtrip covers any bounded/admissible field image, including I64 payloads and both endpoint kinds. Five fixed Lean byte images decode: OP2 output-only, OP2 with one newly active external peer, wrapping OP1, DUP and ERA. A model physical executor decodes each image, and kernel proofs establish exact post-state and equality to the corresponding `Plan.commit.toNet` for all five; the nonempty OP2 case also proves the directed-pair worklist. For every successful decoded image, a separate theorem proves that physical execution preserves the ordered abstract free-port identities. The fixed model images are not exported Rust bytes or a theorem for every checker-admitted image. Generation-tagged slots, stable output labels, queue orientation and journaled pre-commit/rollback are modeled separately. A universal Rust-to-model refinement is **not** proved.

**Rust implementation:** the independent decoder/checker and safe arena/worklist executor cover the exact emitted bytes, reuse/fresh/free mapping, payloads, boundary writes, ownership and reservations. Unit controls include forged bytes, stale generations, alias rejection, duplicate publication, interruption and failed reservation with rollback; the all-family test checks complete observations, interactions and resource counters. These are finite tests, not a universal correspondence proof.

**Finite Rust-byte correspondence:** the generated NetI64PlanCorpus retains actual `rewrite::compile` output independently accepted by `rewrite::check` for OP2, OP1, DUP and ERA in both orientations and both storage modes on two input sets (32 streams, including maximum/zero numeric payloads). All 32 decoded field/result-symbol agreements are kernel-checked. A separate Lean physical lowering derives all plan fields and byte encodings from the rule and arena: kernel theorems prove full literal-byte equality for **all 32** Rust streams against independently reconstructed physical layouts, not only the four preexisting images. Five lowered model images pass the full Lean semantic checker at exact reservations; four aligned Rust forward-reuse streams inherit exact modeled physical execution. A normal Rust test re-emits and checks corpus freshness, with a test-net digest binding both the original model and new lowering source. These are **finite** producer-byte comparisons, not a universal Rust checker/executor correspondence; the other 28 streams have no complete physical execution proof.

**Model-emitter completeness decision: NO-GO for a universal claim at this cut.** The executable `PhysicalState.lowerPlan` derives OP2/OP1/DUP/ERA image fields from a selected physical pair, both orientations and both reuse/append policies; it refuses stale identities, missing principal reciprocity, unsupported boundary aliases, malformed peers, finite-generation exhaustion and unencodable images. `PhysicalState.lowerPlan_decodes` proves byte roundtrip for *every* image it returns, `PhysicalState.five_lowered_images_admitted` proves five checker-positive emitted images, and `PhysicalState.lowerPlan_refines_if_admitted` derives the whole-net step for any emitted image that passes the independent semantic checker. The kernel theorem `PhysicalState.overflow_pair_refuses_both_modes` exhibits a live pair matching the abstract identities but at exhausted generation, so matching alone cannot be the legality premise. More fundamentally, existing `PhysicalState.WellFormed` asserts only canonical queue ordering and exact free-slot classification; it says nothing about global distinct owners, all reciprocal port edges, output-target uniqueness, or finite headroom. The missing theorem must first define a **noncircular** legal physical layout (not "the checker accepted") and prove by generic per-rule physical-execution and projection lemmas that the emitted bytes satisfy *every* `BytePlan.SemanticFields` conjunct: ordered boundary, exact writes, output targets, outside frame, ownership, live-agent directed queue permutation and bounds. Five concrete proofs and a 32-case differential do not close that universal obligation. Reopen this model-emitter NO-GO only with those independent representation premises, a kernel `lowerPlan`-completeness theorem for every legal matched layout and a clean full foundation axiom audit; do not infer it from byte parser roundtrip or extra fixtures. This NO-GO does not retract the **DONE checker-admitted-image alternative**, whose universal theorem quantifies over every independently admitted version-one byte stream.

**Boundary admission:** two distinct auxiliary ports may target different ports of the same external agent (the queued-neighbour fixture exercises this). An auxiliary loop back into either consumed agent or two captured ports naming the identical endpoint is rejected as `plan-boundary-alias`; no claim is made for those inets shapes. The Rust candidate is rebuilt and compared up to agent renaming against a checked DAG-origin graph before reduction, so the tested profile does not admit those loop/duplicate-endpoint cases. The Lean inets model's broader loop cases do not extend Rust admission.

**Output identity contract:** the physical model carries a stable abstract free-port label separately from each output's mutable arena target. Its projection derives reciprocal output-to-agent wires from the target. Kernel theorem `PhysicalState.executeCheckedImage_preserves_free` establishes that every successful decoded image preserves the ordered label list through result installation, all boundary/output writes, retirement and pair publication, for any existing worklist. It does not establish that the mutable output targets correspond to the selected rule. Identifying an abstract free port with the currently referenced agent port instead would make OP2/OP1/DUP refinement false; the remaining emitted-image proof must preserve this distinction.

**Queue-order contract:** Rust appends one canonical generation-tagged physical pair per reciprocal principal connection after the existing pending worklist. Vendored inets records **both directed orientations** of that pair in `principalPairs fired.wires`; its abstract `Plan.commit` prepends these entries before pending work. The physical projection therefore expands each current Rust pair into two abstract entries and drops generation-stale pairs. For every successfully decoded physical image, `PhysicalState.executeCheckedImage_work_append` proves the exact old-queue-plus-publications physical order; `PhysicalState.executeCheckedImage_project_work_split` proves the projected old-work-plus-new-orientations order **provided** every old queued endpoint retains its physical generation and logical agent identity. The checker-admitted-image theorem that derives this premise for every representable layout is still open. Given a checked publication inventory, Lean proves tail append/head prepend have equal directed-pair multisets (`List.Perm`) and physical queue publication cannot change `toNet`; the nonempty OP2 image proves one actual two-orientation case. **Whole-`HNet` equality of ordered worklists is false** when both pending and published parts are nonempty. This is not a proof that every Rust scheduling policy selects the same next pair; inets normal-form/equal-count results still require their own invariant and termination premises.

**Open correspondence obligation:** prove that **Rust's** compiler emits the modeled `ngraph-net-plan/1` bytes for **every** admitted rule instance (the four aligned finite cases above do, but not universally), and that its independent decoder/checker admits only images whose slot/port/worklist mutations (saved boundary, generations, marks, reservations, rollback and overflow refusal) project to the model's one abstract rewrite. The remaining general Lean sublemma is: for every checker-admitted OP2/OP1/DUP/ERA image and representable live layout, `PhysicalState.executeCheckedImage` preserves the untouched frame, produces `Plan.commit`'s agent/wire and output-target projection, and gives a directed-pair worklist permutation (including reuse/fresh slots, external peers, reverse orientations, existing work and alias refusals). The free-port **identity** and physical queue-append components are proved for every successful model image; the projected old-work split additionally assumes stable old-pair slot generations/identities. Five fixed model images discharge whole-net equality, four of which now have actual Rust-byte identity, but not the universally quantified sublemma. A separate unproved Rust correspondence must bind the actual compiler, checker and executor transitions to this physical model. The obligation statement is **DONE**; universal implementation refinement is **OPEN**; hardware/concurrency are outside the issue.

**Why the universal premise matters:** the kernel checks a parser-only OP2
counterexample: `BytePlan.op2_omitted_external_decodes` accepts a versioned
image with one omitted output write;
`PhysicalState.op2_omitted_external_execution_exact` still executes it; and
`PhysicalState.op2_omitted_external_miswires` witnesses a different second
output wire from the correct OP2 image. A matching Rust byte-control removes
the external write while retaining valid grammar, and independently checks
that `rewrite::check` returns `PlanMismatch`; this image is **not**
checker-admitted. A theorem using only successful decoding/execution as
its premise would be false; the general refinement must derive exact-write
completeness from an independently specified semantic checker.

## Prior acceptance status by evidence class (superseded where noted below)

| Issue #110 acceptance item | Kernel model | Safe-Rust / finite evidence | Rust-to-model correspondence |
|---|---|---|---|
| Actual OP2/OP1, DUP, ERA plans and consumed-slot reuse | **Proved** for every abstract rule orientation and payload | **Passed:** emitted bytes, freshness, and zero-allocation reuse controls | **Proved finite:** exact producer bytes equal independently modeled byte images for four forward-reuse cases; **open universal** |
| Emitted-plan semantics, frame and ordered boundary refinement | **Partial:** HNet image, admissible byte writer/parser roundtrip, all-successful-image free-port identity and physical queue append, conditional projected old/new queue split, five decoded Lean physical execution fixtures (four rule shapes plus nonempty OP2 publication), exact projected-net equalities, directed-pair queue case and outside-frame theorems; universal admitted-image agent/wire/target/queue theorem still open | **Passed:** independent decoder/checker, generated byte freshness and boundary-alias refusal controls | **Proved finite:** four Rust-emitted reuse images have exact modeled physical execution; **open** arbitrary checker-admitted image and Rust executor correspondence |
| Explicit safe-Rust execution correspondence obligation | **DONE:** stated exact missing four-rule projection lemma and Rust-step correspondence above; abstract HNet commit proved | **Passed** on finite checked graph fixtures, not a universal proof | **DONE:** obligation stated; universal microstep/commit proof **OPEN** |
| Forged-map, stale/free, write, operand, alias, enqueue and reservation refusals | No model claim about forged bytes | **Passed:** adversarial byte and state controls | **Open** beyond tested cases |
| Zero-free-slot reuse, every interruption boundary, resource-limit boundaries, rollback | No physical-resource theorem | **Passed:** finite reservation/interruption/limit controls | **Open** beyond tested cases |
| Same-net allocating/reusing comparisons on five frozen families | No workload-wide theorem | **Passed:** complete observations and abstract interaction counts match in 31 pairs per family | **Open** beyond tested cases |
| Alloc/reuse/free, live/transient/high-water/scratch/time report | Not a proof obligation | **Measured:** all counters and raw timings retained in the current receipt | Not claimed |
| Fresh replay identity rejects changed plans under the same rule | Not a semantic theorem | **Passed:** source/executable/formal pins and distinct plan/replay digests | Not claimed |

Reopen the performance decision only with a separately frozen comparison on realistic workloads that shows a material end-to-end gain over the existing packed backend **including** lowering, validation and readback, and with the Rust↔model correspondence obligation discharged before claiming proved implementation refinement. The plan path remains optional regardless of these finite microbenchmarks.

## Semantic-checker generalization and decision

**GO for the universal *Lean semantic-checker* refinement alternative; NO-GO
for a universal Rust implementation-refinement claim.** The new
NetI64PlanChecked.lean checker consumes the actual version-one byte *format*,
decodes it, reserves arena/scratch/queue before simulating the model physical
executor, and admits the result only if every independent semantic comparison
passes. The kernel theorem `BytePlan.semanticCheck_refines` quantifies over
arbitrary decoded bytes, any physical arena/output/worklist state, every
selected revision-3 OP2/OP1/DUP/ERA rule and both orientations, subject to
checker admission and explicit matching/redex premises. It proves an entire
projected agent/wire/free-port net equality to the abstract compiled plan's
commit, one abstract `CStep`, ordered stable free-port labels and permutation
of directed **live-agent** pending pairs. The checker also checks the exact
ordered write and reciprocal-write inventory computed from result ports,
ordered captured boundary, result symbols, selected opcode, physical
outside-frame slots, each output's current target, distinct live logical
owners, exact in-bounds free-slot inventory, canonical queued slot order and
resource reservations. `BytePlan.semanticCheck_frame`,
`BytePlan.semanticCheck_ownership` and `BytePlan.semanticCheck_journal`
separately expose these checked facts, precommit stutter and exact rollback.
`BytePlan.op2_omitted_external_semantically_refused` rejects the known
grammar-valid missing-write counterexample for *all* resource limits.

The inets `principalPairs` candidate list includes a port-zero connection to
an external free output, even though no second agent can fire there. This is
why the old full worklist-permutation premise is false for the output-only
OP2/OP1/DUP fixtures. The new checker filters abstract candidates to
live-agent directed pending pairs before comparing them to the physical
generation-checked queue; it does not erase real active agent-agent pairs or
claim exact scheduling order.

The model checker runs a shadow physical execution and compares the complete
projected post-state with the independently compiled abstract image. Its
kernel soundness theorem is **not** a theorem that a bare list of plan fields
by itself entails refinement, nor that every legal image from the Rust
compiler passes these checks. Five additional kernel theorems prove checker
admission at exact reservations for the output-only OP2, published-peer OP2,
wrapping OP1, DUP and ERA physical fixtures; the omitted-write image is
kernel-refused. These fixed positive witnesses prove admission is
nonvacuous, not that every legal model or Rust emitted image passes. The
accepted-image theorem is not based on their finite enumeration. The Rust
checker does not currently run
this model semantic check: the existing four exact Rust-byte/model matches
and 16 emitted-field cases remain finite evidence only. NO-GO for calling
the Rust implementation universally proved until a source-bound theorem
establishes, for every admitted rule instance, that its emitted bytes and
independent checker map into this predicate and that its executor's
reservation, microsteps, commit, generation handling and rollback correspond
to the physical model. Reopen that NO-GO only with this proof plus clean
foundation/implementation gates; do not infer it from additional finite
fixtures, a matching rule name or a successful parser roundtrip.

### Six numbered refinement properties

| # | Status and exact scope |
|---|---|
| 1. Selected pair | **DONE for checker-admitted model images:** the executable physical step requires generation-current logical pair identities; the kernel theorem separately requires the real rule and matched-redex premises. Rust universality remains NO-GO as above. |
| 2. Untouched frame and ordered boundary | **DONE for checker-admitted model images:** exact ordered prestate boundary and each unchanged old physical slot are checked; complete post-map comparisons and target checks enforce the output boundary, while stable free labels are proved for every successful physical execution. Unsupported Rust loop/identical-endpoint aliases continue to refuse. |
| 3. Declared payload and wiring only | **DONE for checker-admitted model images:** result symbols agree with the selected rule; both ordered wire lists equal the entire result-port inventory; simulated final agents, wires and output targets must match the independent abstract image. |
| 4. Stutter then one rewrite | **DONE in the model:** resource admission returns an unchanged journal, staged internal changes are invisible, rollback restores the original physical snapshot, and the checker-admitted commit produces exactly one `CStep`. |
| 5. Ownership and queued inventory | **DONE for checker-admitted model images:** distinct live identities, in-bounds exact free-slot membership and canonical physical queued pairs are checked; projected generation-valid pairs are a permutation of the abstract live-agent directed pending candidates. Stale old physical entries remain skippable, not reidentified. |
| 6. Reserved completion | **DONE for checker-admitted successful model executions:** arena, scratch and queue reservations are checked *before* execution, and admission requires the physical execution to return a completed state. This does not prove termination of every Rust run or reserve capacity on all host machines. |

### Eight acceptance items and owner follow-ups

| Item | Disposition |
|---|---|
| 1. Arithmetic-stage, fanout, erasure reuse | **DONE:** existing safe-Rust plans and five-family paired receipt demonstrate consumed-slot reuse and avoided allocations; no default switch. |
| 2. Emitted-format semantics and frame/boundary refinement | **DONE via the checker-admitted-image alternative:** the general kernel theorem covers *every* image admitted by the executable Lean semantic checker, not all grammar-valid bytes or all Rust-emitted bytes. Five separately lowered model images are kernel-admitted and 32 Rust images have full model-byte agreement; the exact-write negative control is kernel-proved. **DECIDED NO-GO** for the stronger claim that *all* legal model-emitted images are admitted until the noncircular physical representation and generic field-completeness proof described above pass a clean axiom audit. |
| 3. Safe-Rust correspondence obligation | **DONE as an explicit obligation; DECIDED NO-GO for universal implementation proof claims:** finite emitted-byte matches and Rust tests are not a proof of compiler/checker/executor correspondence. Reopen criteria are above. |
| 4. Forged/stale/duplicate/missing controls | **DONE as finite Rust refusals:** existing negative tests; model missing-write counterexample is additionally refused for every resource limit. No assertion of universal Rust refusal theorem. |
| 5. No-free-slot, interruptions, resource edges, rollback | **DONE as finite Rust executions:** existing controls; model reservation/journal/rollback theorems remain separate. |
| 6. Matched five-family observations | **DONE:** prior clean-source 31-pair receipts compare complete results and abstract interactions. |
| 7. Counters, peaks, timing and regressions | **DONE:** retained receipts retain the older 1.1402 chain regression; opt-in path remains NO-GO for performance adoption. Reopen only under the separately frozen full-path criterion above. |
| 8. Fresh replay and same-rule changed plan | **DONE:** exact bytes and net/rule/representation/checker/executable bindings retained; the two storage policies yield distinct plan and replay digests. |

The owner's dated follow-ups are disposed separately rather than promoting
the older local-model results into evidence about the wired backend:

| Owner follow-up | Disposition and reopen condition |
|---|---|
| 2026-11-11 first checked slice, named-index refusal/conditional index-zero step | **DONE in its original m3 local model:** named-index legality and conditional abstract agreement have kernel theorems; **DECIDED NO-GO** as evidence of wired output delivery. Reopen only after a wired-model representation/refinement proof, not a local size invariant. |
| 2026-11-11 size/reducible-count schedule independence | **DONE in its original m3 local model; DECIDED NO-GO** for a revision-3 backend resource or result claim because those firings discard values. Reopen when the selected wired rules and queue/refinement are proved against that schedule criterion. |
| 2026-11-11 plan legality | **DONE locally:** named-node refusal theorem checks legality without trusting a schedule. A Rust arena and separate independent checker now exist, but **DECIDED NO-GO** for equating this unwired index model with the Rust representation absent the same wired proof. |
| 2026-11-11 value-reporting step and correction of its commit hash | **DONE as a correction/local theorem** (the corrected reference is 7c8be12f); **DECIDED NO-GO** for a delivered-output claim since the value is not fed to another agent. Reopen only on a proved output-delivery relation. |
| 2026-11-11 whole-plan value log | **DONE as a local observation theorem; DECIDED NO-GO** for a wired plan result or replay theorem. Reopen only if the logged values are shown to drive the wired continuation in a representation proof. |
| 2026-11-11 certificate bounds and mistaken absence of a Rust arena | **DONE for the m3 local-model certificate and corrected repository state:** the earlier no-backend assertion predates the current safe-Rust ngraph arena. **DECIDED NO-GO** for lifting the local potential bound to actual slot peaks without an explicit state/budget correspondence; the #109 integration is optional. Reopen when a real consumer needs such a bound and its relation is proved. |
| 2026-11-12 errata and 2026-11-13 follow-up | **DONE:** the unwired m3 model is excluded above, and the current theorem uses the selected wired revision-3 inets plan. Reopen any m3-as-backend claim only after proving value delivery and wiring. |
| 2026-11-15 batches 7/8 open model projection and Rust correspondence | **DONE for every Lean semantic-checker-admitted image:** the general kernel theorem includes entire projected net, output targets and live-agent pending-pair permutation; five independent lowered images prove admission is nonvacuous. **DECIDED NO-GO** for automatic universal model-emitter admission or Rust emission/checker/executor correspondence, with separate proof criteria and reopen conditions stated above. |

The earlier unwired m3 local reducer theorems are **DECIDED not evidence** for
this wired revision-3 backend: they discard computed values and cannot prove
delivery. Reconsider their use only after a separately proved wiring/value
refinement. #37/#124's selected rule and boundary remain authoritative;
#109's physical peak integration is **DECIDED optional**, reopened only if a
downstream consumer requires a capacity bound. Hardware, host effects,
concurrency and a default evaluator switch remain outside this issue.
