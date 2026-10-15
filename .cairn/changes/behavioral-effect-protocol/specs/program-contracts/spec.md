# Bounded behavioral effect protocol

## ADDED Requirements

### Requirement: EPROTO-SCOPE-01
r[EPROTO-SCOPE-01]

An optional `Effect-Protocol-Draft` profile MAY state behavioral claims beyond an effect set. Its first selection MUST be limited to `RegionProtocol-v1`:
- the existing synchronous `noble-test:bounded-region/bounded@1.0.0` world and its `noble component read-region` host route;
- one host-registered owner injected before guest start;
- finitely many borrowed `region.read` calls;
- at most one release.

It MUST keep five concerns separate:
- effect permission (approved imports and effect bound);
- behavioral proof (order, results and cleanup obligations);
- invocation applicability (initial protocol state and host premises);
- ownership (linear `own`/adapter-local borrow);
- cleanup authority (guest obligation on normal paths, host-only retirement on abnormal exit).

Host authorization is a separate host-only fact; it MUST NOT be inferred from effect permission or from any protocol claim. The profile MUST NOT claim liveness, exactly-once remote effects, concurrency, suspension, or that the pure MC1/MC2 fragment covers resources.

### Requirement: EPROTO-TRACE-01
r[EPROTO-TRACE-01]

`RegionProtocol-v1` traces MUST distinguish guest requests, host responses and externally committed effects (protected region read, guest release, destructor release, host retirement). The normative transition table, result schema (`ok` bytes of the requested length, `err("bounds-reject")`, `err("invalid-handle")`), resource identity tuple and host assumptions are those of the change design. Any response outside the schema MUST be classified `outside-model`, and no protocol claim MAY be reported for that execution.

Claims MUST be labeled separately:
- normal-return postconditions for both the success and the ordinary-error result;
- finite-prefix safety (single release, no protected effect after release or abort, every committed read preceded by an admitted in-bounds request on the authentic owner, synchronous request/response pairing).

Cleanup after a trap, quota exhaustion or cancellation MUST be stated as a host obligation that preserves the observable prefix. Total correctness is not claimed.

### Requirement: EPROTO-COMPOSE-01
r[EPROTO-COMPOSE-01]

Composition of protocol claims MUST require identical intermediate protocol states for every shared owner (identity tuple and owner state). It MUST concatenate traces in program order. It MUST NOT use a union of effect sets, assumed commutation or a generic frame rule; preservation of unrelated state requires an explicit locality hypothesis. Out-of-order use, omitted release, double release, wrong resource identity and unmodeled responses MUST be rejected by the checker, refused by the host, or reported as monitor violations, as specified in the change design.
