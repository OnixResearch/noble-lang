## Context

Grounded at the worktree baseline `fed77835` (issue #50 cites `2c96d4c1`; the cited
facts still hold):

- `.cairn/changes/behavioral-effect-protocol/design.md` is an accepted design.
  It fixes `RegionProtocol-v1` over exactly the
  `noble-test:bounded-region/bounded@1.0.0` world, the host route
  `noble component read-region COMPONENT HOST_BUFFER_HEX OFFSET LENGTH`, one
  host-injected `own<region>`, finitely many borrowed reads, at most one
  release, synchronous calls only, and it separates three event classes —
  `Req(op,args)`, `Resp(op,r)`, `Commit(k)` with
  `k ∈ {region-read(start,len), guest-release, destructor-release, host-retire}` —
  and five authorities (effect permission, behavioral proof, invocation
  applicability, ownership, cleanup authority). Its ideal protocol and abstract
  exhaustion events are **not** a correspondence theorem for emitted ABI steps.
- `crates/noble-cli/src/component/bounded_region.rs` reports abnormal exits
  through one class: `Err(error) => ("adapter-trap".into(), None, Some(...))`,
  which also absorbs malformed-payload `bail!`s such as `invalid success
  payload`, `invalid byte result`, `invalid read-region variant` and
  `invalid read-region result`. Cleanup runs afterwards and can itself fail:
  `store.data_mut().release(1)?` returns early with `?`, so no report is emitted
  at all in that case. The report carries the outcome string, the returned
  payload and an error string, and a nonzero terminal `live_owners`/`native_pins`
  is the only trace of a surviving owner (design F7).
- Only `region.read swap regions.release` has executed evidence, and it has no
  ordered event trace: the report carries counters, not a sequence (design F6).

## Decisions

### Decision: relate traces over the protocol's own event classes, not over a new calculus

**Choice:** the source alphabet is `RegionProtocol-v1`'s `Req`/`Resp`/`Commit`
events. The target alphabet is a handwritten sequence of abstract ABI/host steps
for the bounded-region operation — the Canonical-ABI import crossing, host
decision, protected copy, return, abnormal-exit class, and post-return cleanup or
retirement. It is not claimed to be the emitted step sequence of a particular
component or engine. The relation is defined first; the properties are *derived*
from it.

**Rationale:** the TOPLAS discipline is explicit that a relation between
non-identical traces does not preserve arbitrary properties, so anything not
derived here is not a claim. Reusing #9's event classes keeps the accepted
design authoritative instead of inventing a second protocol.

### Decision: erasure only for named internal ABI steps, with a proved justification

**Choice:** stuttering and erasure are permitted only for identified internal
ABI bookkeeping steps (for example handle-table indexing or store-data
bookkeeping) and each erased class carries its own justification theorem.
Observable ordering, response-to-request pairing, exact resource identity and
every protected commit are preserved by the relation itself. A denied request
is never erased, and host retirement is never converted into a guest release.

**Rationale:** those two erasures are exactly the ones that would make a broken
execution look correct: erasing a denial removes the evidence that a request did
not happen, and rewriting retirement as a guest release moves a host obligation
onto the guest.

### Decision: preserve the trap/quota/cleanup distinction instead of one trap class

**Choice:** the target model carries the abnormal-exit class, the quota/fuel
boundary and the cleanup outcome as separate observations. The opt-in v2 host
report records `normal`, `rejected`, `trap`, `quota-exhausted`, `cancelled`,
`adapter-invalid` and `outside-model`, plus cleanup state/reason and fuel left.
The runtime fixtures lower the reported abnormal-exit class before post-exit
retirement; an absent class stays **unknown** rather than being filled in with a
successful protocol outcome. A refused cleanup is retained at the host method but
is not guest-reachable.

**Rationale:** the host already distinguishes `Err(error)` from a rejected
response carrying a reason string. The new v2 observation keeps those classes
distinct without changing the v1 schema or trace prefix. A theorem stated over a
coarse label would be a theorem about the label, not the execution.

### Decision: cost/fuel is an explicit relation, never a free trap clause

**Choice:** target ABI steps can exhaust fuel before a source step completes, so
the relation carries an explicit cost/accounting relation or a justified
abstraction. The committed prefix and the precise termination/cleanup status are
preserved across that boundary, and an unrestricted "the target may trap
anywhere" clause is refused.

**Rationale:** a free trap clause excuses arbitrary wrong code by construction.
Preserving the prefix is what makes the claim usable: a trap may cut the trace,
but it may not invent or drop commits.

### Decision: normal return is a premise, not a conclusion

**Choice:** the normal-return postcondition is conditional on normal return.
Prefix safety does not imply eventual return, and it does not imply successful
cleanup.

**Rationale:** #9's protocol permits ordered prefixes, and an execution may trap
after the prefix; conflating the two would turn prefix safety into a liveness
claim, which no part of this workstream establishes.

### Decision: model theorem stays a model theorem

**Choice:** the correspondence route from the target model to the actual
lowering/runtime steps is named with pinned source, and Wasmtime/Canonical-ABI
assumptions stay visible until established separately.

**Rationale:** two handwritten machines give a model theorem. Claiming more
would need the emitted-step correspondence, which is #10's territory.

### Decision: no production fault-injection seam for cleanup failure

**Choice:** the pilot does not add a seam to the production host that would let it
fail a retirement on demand. The refused-retirement observation is modelled
(`ObservedEvent.retirementRefused`, which lowers to the cleanup-failure exit class
and commits nothing) and the generator accepts a refused `host.retire` when a run
records one. The checker exercises that generator branch with a synthetic control;
no executed case produces the observation.

**Rationale:** the only way to reach a refused retirement today is a corrupted
host context, which a guest cannot produce. The retained evidence for that path is
the test-only host fixture `cleanup-release-refusal-retains-owner` in
`verification/host-locality/evidence-1bf39a52.json`, and the model proof is
`cleanup_failure_is_selected_invoked_and_retained` in `proofs/m5/HostLocality.lean`.
Adding a production seam to manufacture the observation would put a fault-injection
affordance in the shipped adapter, which is a larger risk than the gap it closes.

**Reopen if:** a real API path (for example a retirement refused because the owner
was already retired by another path) becomes guest-reachable, or the owner
authorises an explicitly test-only, feature-gated seam.

### Decision: no Lentil pin

**Choice:** the Lentil projection from #50's follow-on comment is not pinned or
imported. The issues it raises are recorded instead: the finite traces embed as a
terminal-padded `Nat → state` only after the relation exists, and any progress
claim belongs to a separate temporal theorem under named fairness assumptions.

**Rationale:** Lentil's moving main requires Lean 4.34.1 while this pilot is on
4.35.0, and the comment itself defers the progress gate to #46. Pinning an
incompatible dependency would either force a toolchain move this package does not
need or leave an unbuildable dependency in the tree.

**Reopen if:** #46 defines a progressing transition system and its runtime
observation mapping, and a compatible pinned closure is available.

## Evidence and remaining boundaries

The model and finite fixtures are implemented and kernel-checked. The event
relation enforces response pairing (while permitting an unmatched final request
at a truncated prefix), exact identity, read-commit explanation and at-most-once
release/retirement; the two erased bookkeeping classes have per-scan preservation
proofs. Runtime fixtures cover normal success, ordinary refusal, the complete
scope-exhaustion refusal, traps, quota boundaries, cancellations, omitted release,
adapter-invalid results and outside-model results. Controls cover swapped
requests/responses, substituted values and identities, missing or unexplained
commits, erased denials, release/retirement confusion, and an empty-input-only
lowering.

The remaining boundary is deliberate: the source/target relation is a theorem
about handwritten alphabets. The region-trace receipt binds the effect-protocol
observations and their source, component, helper and engine revisions, along with
the model revision it checks; the correspondence route is named, but no
emitted-component/Canonical-ABI/Wasmtime step correspondence is proved. Cleanup
selection, invocation and failed-retirement retention are proved/tested at the
host method, not guest-reachable; no production fault-injection seam is added. The
`--cancel-after-reads` lane is a real host test control, not a general asynchronous
cancellation API. No liveness, eventual return, rollback, or host-death cleanup
claim is made. Per-class erasure justification remains a model proof obligation,
not a syntactic check.

## Non-claims

- No source-to-artifact correctness or emitted-step correspondence claim. The
  runtime receipt is finite execution evidence and does not establish the named
  Canonical-ABI/Wasmtime route.
- No liveness, rollback, general cancellation API or cleanup-after-host-death
  claim.
- The opt-in host report is instrumentation for the existing bounded-region
  route; the accepted `#9` protocol design is not reopened.
