## Context

Grounded at worktree HEAD `1b581105` (issue #50 cites `2c96d4c1`; the cited
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
events. The target alphabet is the emitted ABI step sequence the bounded-region
host actually performs — the Canonical-ABI import crossing, the host decision
steps, the protected copy, the return, the abnormal-exit class, and the
post-return cleanup or retirement. The relation is defined first; the properties
are *derived* from it.

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
boundary and the cleanup outcome as separate observations. Where the current host
collapses them into `adapter-trap`, the pilot names the exact observation needed
to separate them, and an absent observation stays **unknown** rather than being
filled in with a successful protocol outcome.

**Rationale:** the host already distinguishes `Err(error)` from a rejected
response carrying a reason string, but its single outcome label discards that
difference. A theorem stated over a coarse label would be a theorem about the
label, not the execution.

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

## Risks / Trade-offs

- The pilot's observation surface is small: only a read-then-release execution
  has been exercised, so most of the fixture set (trap, exhaustion, quota-prefix,
  cleanup failure, destructor release) needs instrumentation that does not exist
  yet. That is the pilot's cost, not a hidden assumption.
- Distinguishing quota from trap may require a host-side observation change;
  until then those classes stay unknown, so some corollaries stay conditional.
- Unsupported cancellation remains unsupported: there is no real control to
  cancel, so no cancellation lane is modelled.
- The relation can be made vacuous by declaring an unbounded erasure class; the
  per-class justification requirement is the guard, and it is a proof obligation
  rather than a syntactic check.

## Proof obligations

Named here, not discharged by this package:

- the relation itself (a Lean definition with named source/target alphabets);
- per-class erasure justifications;
- prefix compatibility and the RegionProtocol corollaries (permitted ordered
  prefixes, no forged identity, no unexplained read commit, at-most-once
  release/retirement under the declared host, identity, cost and cleanup
  premises);
- the cost/accounting relation that keeps the committed prefix across a fuel
  boundary;
- the correspondence route, kept separate from the model theorems.

## Fixtures (future, unchecked here)

- normal success (`region.read`, then `regions.release`, normal return);
- ordinary error (rejected response with a reason);
- host refusal (identity or bounds);
- trap;
- quota prefix (fuel exhaustion after a committed prefix);
- unsupported cancellation, explicitly not modelled.

Controls: swapped requests, substituted responses, substituted resource
identity, an erased denial, a missing protected commit, host cleanup disguised
as a guest release, and cleanup-selection vs cleanup-invocation vs successful
retirement distinguished with cleanup-failure evidence retained.

## Non-claims

- No completed compiler proof, no source-to-artifact correctness claim and no
  claim about a real Wasmtime instance beyond what the correspondence route
  establishes.
- No liveness, rollback, cancellation-support or cleanup-after-host-death claim.
- No instrumentation is added by this package, and the accepted `#9` design is
  not reopened.
