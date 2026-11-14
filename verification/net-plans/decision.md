# Revision-3 checked in-place plan decision (#110)

**Decision: retain the checked safe-Rust plan path as an opt-in correctness experiment; NO-GO for performance-based adoption.** It is not the default evaluator. This decision does not amend #37's frozen packed/net threshold of 1.15 or its NO-GO result.

## Frozen rule and matched evidence

[The criteria](criteria.md) were committed as `f5e4710b` before timing. The clean-source [final comparison receipt](comparison-2026-11-14-final.json) names source commit `2e2bb92a6b878d87dfc78f4599e8563026fb5607` and test executable SHA-256 `b818ebeed5f8f40fcc85f7025fa13ff73ae90d5dae7fc942f6bad83add680c5a`. An earlier [first comparison](comparison-2026-11-14.json), source `69b5ef3`, remains retained, including its 1.1402 chain regression; the later source tightened stale-queue admission, and the earlier timings are not substituted for the final-source measurements. Each family uses the same checked graph, lowered net, input row and completion rule in both arms, with one untimed warm-up and 31 alternately ordered pairs. Timing spans independent candidate validation, checked-plan compilation, reservation and execution, complete readback, and reducer-local cleanup. Source admission and net lowering are intentionally excluded from both arms. Full ordered raw samples, all resource counters, source/executable/checker/plan/representation identities, and the plan/replay digests are retained in each receipt.

| Family | Median reuse / allocate (ns) | Ratio (reuse ÷ allocate) | Interactions in each arm | Reuse slots allocated / reused / freed | Allocate slots allocated / reused / freed | Arena high-water reuse / allocate |
|---|---:|---:|---:|---:|---:|---:|
| chain | 196768 / 196207 | 1.0029 | 6 | 0 / 6 / 6 | 6 / 0 / 12 | 7 / 13 |
| fanout | 205995 / 205654 | 1.0017 | 6 | 0 / 8 / 4 | 8 / 0 / 12 | 6 / 14 |
| erasure | 51236 / 49983 | 1.0251 | 1 | 0 / 0 / 2 | 0 / 0 / 2 | 3 / 3 |
| asymmetric subtraction | 101229 / 101019 | 1.0021 | 2 | 0 / 2 / 2 | 2 / 0 / 4 | 3 / 5 |
| wrapping subtraction | 84087 / 84849 | 0.9910 | 2 | 0 / 2 / 2 | 2 / 0 / 4 | 3 / 5 |

All paired full observations agree with the independent reference evaluator; abstract interactions agree across storage policies. The reuse arm demonstrably avoids six, eight, two and two slot allocations respectively in the non-erasure families; erasure needs no replacement. The final source's worst median reuse regression is the 1.0251 erasure ratio, not a demonstrated speedup; the older receipt retains a 1.1402 chain ratio. Timings are descriptive and subject to host contention; no statistical claim or adoption threshold is inferred from these five small fixtures. Both arms record committed live nodes (chain/erasure/subtraction: 1; fanout: 2), transient live slots (reuse/allocate: chain 7/8, fanout 6/7, erasure 3/3, subtraction 3/4), queue high-water (fanout 2, all others 1), scratch reservation high-water (256 plan-buffer bytes), plan-byte totals and internal microsteps separately from abstract interactions in each receipt. Scratch here is the bounded encoded-plan buffer, not a measurement of all Rust stack frames.

The reuse/allocate plan streams differ under the same rule revision; both the exact-byte plan digest and the replay digest change. The Rust replay digest binds the graph/input-bound net key, rule, plan, representation, checker and executor revisions and the emitted plan stream. The receipt's separate evidence key additionally binds the clean source commit and test executable SHA-256. A rule name alone cannot authenticate a prior result.

## Correctness scope and reopen conditions

**Model proof:** the kernel-checked foundation model preserves the selected revision-3 right-hand side for both orientations of OP2, OP1, DUP and ERA. The compiled local image commits as one inets `HNet` firing, with observational pre-commit stutters and one abstract `CStep` under the stated matching/redex premises. This is an abstract hash-map/worklist plan model, not a theorem about physical Rust slot reuse or the emitted byte sequence.

**Rust implementation:** the independent decoder/checker and safe arena/worklist executor cover the exact emitted bytes, reuse/fresh/free mapping, payloads, boundary writes, ownership and reservations. Unit controls include forged bytes, stale generations, alias rejection, duplicate publication, interruption and failed reservation with rollback; the all-family test checks complete observations, interactions and resource counters. These are finite tests, not a universal correspondence proof.

**Open correspondence obligation:** prove that decoding every admitted `ngraph-net-plan/1` byte stream yields the kernel's rule image, and that each actual Rust arena/worklist microstep (including saved boundary, generation changes, queue marks, reservations, rollback and finite counter overflow) represents the same committed abstract net as the Lean model until its unique commit, then represents exactly that model's one abstract rewrite. The current Lean `HNet` hash-map is not the generation-tagged Rust vector arena. Neither the model theorem nor the finite tests discharges this Rust↔Lean obligation; hardware/concurrency are outside the issue.

Reopen the performance decision only with a separately frozen comparison on realistic workloads that shows a material end-to-end gain over the existing packed backend **including** lowering, validation and readback, and with the Rust↔model correspondence obligation discharged before claiming proved implementation refinement. The plan path remains optional regardless of these finite microbenchmarks.
