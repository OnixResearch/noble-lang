# Table-level replay oracle criteria (revision 2, frozen)

Issue #134, owner follow-up of 2026-11-17 ("A table-level oracle for the replay harness"). This file
is committed **before** any measurement it governs. It adds the table oracle on top of
[criteria.md](criteria.md) revision 1, which stays frozen and unchanged: revision 1 still decides
what one observation is compared with and which differences are allowed. This revision changes no
revision 1 rule and weakens no comparison — every case still runs the revision 1 comparison in the
same pass that emits its rows, and a revision 1 divergence still fails the test.

A receipt produced under this revision names this file's SHA-256 and the clean source commit it ran
on. Do not revise these rules after seeing data: a changed check needs a new, separately versioned
file, and this revision and any result it produced stay retained.

## 1. Rows

Every run of every case of every family emits one row per observation:

| Field | Content |
|---|---|
| `input_digest` | digest of the case's semantic input identity and the observed query, independent of the admitted order, of renumbering and of the schedule |
| `schedule_id` | the admitted order's label plus the row's position in that order (a repeated query is a distinct run); F7 names the pair-selection schedule and resource variant; the thread families name `serial` or `round-<r>/worker-<w>` |
| `normalized_output_digest` | digest of the revision 1 section 2 fields (§1 table below), or none when §2 exempts the row |
| `presentation_digest` | digest of the revision 1 section 3 fields the criteria allow to vary (§1 table below) |

The digests are the only place two rows are compared, so the split must follow revision 1 exactly:
a field that section 2 requires to agree goes into the normalized digest, and a field that section 3
permits to differ goes into the presentation digest.

`digest(text)` is 128 bits, printed as 32 lowercase hex digits: two FNV-1a 64-bit streams over
`noble.schedule.v1` followed by `text`, with the FNV-1a offset basis and the prime 0x100000001b3 for
the first stream and the offset basis 0x84222325cbf29ce4 for the second. The digest is a fixpoint of
the table, not a cryptographic commitment: a collision could merge two rows, which can hide a
conflict but can never invent one, and the retained control pair of §4 is re-executed exactly rather
than trusted from its digests.

### Input identity and digest split by family

| Family | `input_digest` identity | Normalized output (section 2) | Presentation (section 3) |
|---|---|---|---|
| F1 kernel-dependencies | `kernel-dependencies/<graph>/work-<n>`: the abstract dependency graph and the work budget; the two dependency-list orders of one graph share one identity | verdict class: accepted, refused-cyclic, exhausted under a budget at least the edge count, unexpected outcome | the named cycle member (A2); an exhaustion under a budget below the edge count (A3) |
| F2 kernel-threads | `kernel-threads/<environment>/<program>` | the complete `Debug` rendering of the outcome | none |
| F3 contracts-queries | `contracts-queries/<case>/<query bytes>` | the complete rendered preparation or refusal | none; the module-pool case: the remaining request-work balance (A5) |
| F4 contracts-commits | `contracts-commits/<case>/query-<i>` and `/definition-<name>` | the rendered query result; a definition's sorted dependents | a definition's session-local identity (A1) |
| F5 contracts-modules | `contracts-modules/<case>` | the rendered module result; with several defects, only the refusal's stage and kind | with several defects, the reported diagnostic (A2) |
| F6 graph-orders | `graph-orders/<shape>` | the refusal code; every checked row observation, exactly where the row can violate one defect only; a multi-violation row's failing-guard class; an evaluator disagreement | the refusal site where revision 1 permits A2; which guard a multi-violation row reports |
| F7 net-schedules | `net-schedules/<shape>/<input row>/<resource variant>` | the uncapped rows' section 2 tuple (observation, interaction count, final wires, peak live agents); the fuel-capped rows' exhausted observation, which revision 1 section 3 compares exactly; the worklist-capped rows' section 2 tuple when they complete | the queue high-water mark (A4); an exhausted run's remaining fields (A3); a worklist-capped row that exhausts the reservation is exempt (§2) |
| F8 cache-sequences | `cache-sequences/<limits>/request-<i>` | the request's semantic fields, artifacts or refusal; a memoization-invariant violation | the cache status (A4) |
| F9 threads | `threads/<sub-family>/<query>` | the complete `Debug` rendering | none |

A pointer- or allocation-order-dependent rendering is never used as a digest input: every projection
is text the owning family module already produced for the revision 1 comparison.

## 2. Schedule independence

For one family, group the rows by `input_digest`. In each group:

1. **Compared rows** are the rows with a normalized output. Require exactly one distinct normalized
   output digest among them. Two or more is a **normalized conflict**: a failure of the functional
   dependence `=(input; output)` on the sampled table.
2. **Exempt rows** carry no normalized output. The only exemption is a revision 1 section 3 A3
   exhaustion in its stated condition — an F1 run whose budget is below the edge count, and an F7
   worklist-capped run that exhausts the reservation. An exempt row is counted by class; a group that
   mixes compared and exempt rows records one allowed exhaustion difference and is not a conflict,
   because revision 1 permits exhaustion to replace a verdict.
3. **Presentation check.** Among the compared rows of a group, more than one distinct presentation
   digest is recorded as one allowed presentation difference, with the group counted. It is never a
   defect. This is the same functional-dependence check applied to presentation, and its failures are
   the expected allowed differences of revision 1 section 3.
4. A group with no compared rows is counted and checked only for presentation.

Each family prints one `NOBLE_SCHEDULE_ORACLE` line with its row count, group count, normalized
conflict count (zero for a pass), allowed presentation difference groups and exempt rows by class.

## 3. The retained pair

On a normalized conflict the oracle retains **exactly two rows** from the group: the same
`input_digest`, two different `schedule_id`s, two different normalized output digests. The pair needs
no further shrinking to be reviewable (owner note of 2026-11-17); the oracle does not minimize it and
does not widen it to a family report. It prints the two rows, each with the replay token
`<case>@<order>` of the run that produced it, and the test fails.

Every retained row is re-executed to confirm it reproduces its recorded normalized digest, and at
least one row of the pair must replay a divergence through the existing
`NOBLE_SCHEDULE_REPLAY=<case>@<order>` mechanism. The thread families (F2, F9) have no order token —
their schedules are wall-clock interleavings, not orders — so a pair there is printed without a
token; revision 1's exact-equality assertion remains their primary check.

## 4. The seeded order-dependent control

The oracle carries one planted control, `oracle-control/position-leak`, whose observation is the
position of the first occurrence of the query in the schedule: an analysis that leaks scheduler state
into its result. Its serial and reversed orders agree on the observed query, so an unseeded harness
would miss it, and the seeded orders of revision 1 section 5 expose it. The oracle **must** catch it
and retain exactly two rows, from two different schedules, at least one of them seeded. The control
never runs through the passing family check: it is driven on its own, its pair is printed on one
`NOBLE_SCHEDULE_ORACLE_CONTROL` line, both rows are re-executed, and at least one row replays a
divergence. The nine family lines must still report zero normalized conflicts, so a control that
silently passes fails the test just as a genuine conflict does.

## 5. Scope and relation to the net-level results

A pass is evidence about the sampled table only. It is the finite relation
`input_digest -> normalized_output_digest` over the rows the harness ran, not a proof of schedule
independence, not a statement about orders outside the admitted set, and not a Rust-to-Lean
refinement.

Two proved model results at the net level already exist: issue #124's selected revision-3 row in
`proofs/foundation/README.md` (`i64_normal_form_unique`, `i64_equal_interaction_count`,
`parallel_sub_schedules_agree`) and, in the `inets` development, `MRun.confluent`, which proves the
same memory and the same claim count across schedules and thread counts. Both are about reducing
interaction nets. They neither cover compiler analyses nor are re-proved here, so they do not
discharge this issue and this table does not stand in for them.

## 6. Non-claims

- Finite implementation evidence over the sampled cases and orders; no confluence theorem.
- The digest is a 128-bit table fixpoint, not a cryptographic identity claim.
- Allowed presentation differences are counted, not compared; a count is not a correctness result.
- No performance result, and no parallel execution is proposed (see [decision.md](decision.md)).
