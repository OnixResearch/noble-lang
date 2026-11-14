## Context

The v3 measurement leaves the issue's executable-optimization claim unresolved: all eight sources admitted and eight identity rows executed, but no non-identity proposal reached execution because the candidate retained dead operand nodes. The old certificate format also binds every rule premise to the original graph, so it cannot safely authorize a dependent fixed-point chain. Existing accepted endpoints, source inputs, frozen quality targets and the v2 cost thresholds remain the baseline.

## Decisions

### D1 — Checker-derived dead-node pruning
**Choice:** After an ordered sequence of rewrite transitions, derive the live closure from the ordered output list using the candidate's verified node table. Retain guard nodes even if they are not output ancestors. Construct a dense old-to-new `NodeId` survivor map by scanning old IDs in order, rewrite all retained operands and outputs through that map, then independently check the resulting graph. Refuse if any output/operand/guard cannot map, any mapped operand is not earlier than its user, or the resulting graph fails the unchanged admission checker. Never accept a proposer-supplied prune map or trust deadness metadata.

**Rationale:** Pruning is an exact graph projection rather than a rewrite rule: every retained node is denotation-preserving, every removed node is unreachable from all outputs and guards, and each ordered output maps to a surviving node with the same denotation. Stable source-order numbering keeps the result deterministic and makes dense-index integrity directly testable. Retaining every guard preserves the existing guard/failure surface, even when a guard is not output-reachable.

### D2 — Explicit chained certificate revision
**Choice:** Keep the existing `ngraph-cert/0` public format and checker unchanged for old callers. Add an explicitly named `ngraph-cert/2` transition API. Each transition carries the exact before/after graph digests, ordered rewrite premises and claims; checker replay derives each intermediate graph, verifies the next transition against that graph, then verifies pruning and the final endpoint. The Rust checker derives rather than trusts each intermediate state. The decimal-token codec is canonical and bounded before allocation.

**Rationale:** Chaining changes the meaning of what a premise authenticates, so overloading the legacy derivation would be ambiguous. Explicit endpoints prevent a certificate from replaying on the wrong intermediate graph; decoder and checker reject stale graph identity, malformed ordering, unknown revisions and incomplete transitions. Neither proposal nor producer metadata is acceptance authority. A model token corpus tests only the model decoder and exact emitted tokens; it does not establish Rust/Lean correspondence.

### D3 — Frozen v4 cost/quality registration
**Choice:** Add `rewrite-cost-v4.txt` before the integrated run, binding the exact v2 quality corpus digest and reusing its frozen 50 us/step ceiling and `no-foldable-sites` target. Use the same eight fixtures, four proposal controls, source build, identity scheme, warmup/sample counts, and refusal accounting as v3. Preserve v2 and v3 plan and receipt bytes. Run from a clean implementation commit and record accepted source/candidate admission separately from executions, non-identity executions, residual target, break-even and cached/uncached cost.

**Rationale:** This isolates the effect of enabling real non-identity execution without moving goalposts or changing the fixture population. A refusal is not an execution; proposal cost does not substitute for certified candidate cost. Report GO only if the frozen threshold and residual target are attained with non-identity candidates actually executed; otherwise retain the measured result as NO-GO without tuning the plan.

## Risks / Trade-offs

- A complete chain can change intermediate node numbering only through the certified rewrite/prune rules; all transitions and the final pruned endpoint must remain bound to exact graph digests.
- Stable pruning is narrower than general graph canonicalization and intentionally retains guard nodes; those retained nodes may keep a candidate over the residual target.
- The finite Rust corpus and Lean model are independent evidence. There is no theorem binding the Rust parser/checker/pruner or its graph evaluator to Lean.
- A correct quality gain can remain slower than source evaluation. Preserve all sample data and report the result rather than weakening the registered thresholds.
- Reopen D1 if a minimized admissible witness disproves output denotation preservation or guard retention. Reopen D2 if any accepted transition can change without matching its bound before/after graph identity. Reopen D3 only for an identified registration error, never because measured performance misses a threshold.
