# Region trace relation pilot (WIT-TRACE-01)

Model-theorem check for `.cairn/changes/region-trace-relation`. It builds
`proofs/m5/RegionTrace.lean` in an isolated project with the pinned Lean toolchain, then checks
every inventoried theorem's compiled statement and transitive axioms, so a finite instance, an extra
premise or a weaker statement cannot satisfy the inventory by mentioning a dependency.

## Command

```sh
env -i HOME="$HOME" USER="$USER" LOGNAME="$USER" PATH=/run/current-system/sw/bin:/usr/bin:/bin \
  "$(selected node)" verification/region-trace/check.mjs --output /tmp/region-trace-check
```

The harness refuses a caller environment carrying `LEAN*`, `LAKE*`, `BASH*`, `GIT_*`, `NIX_*` or
`ELAN*` variables, refuses an output directory inside the source tree, and refuses a dirty worktree
unless `--allow-dirty` is passed explicitly (a dirty receipt records it, it does not hide it).

## What is proved

The source alphabet is `RegionProtocol-v1`'s `Req`/`Resp`/`Commit`; the target alphabet is the
emitted ABI step sequence this host performs (`importCrossing`, `hostDecision`, `protectedCopy`,
`returned`, `normalReturn`, `abnormalExit`, `cleanup`, and the two named erasable bookkeeping steps).

A request carries the guest's signed (`s64`) arguments as `Int`, in the source alphabet, the target
alphabet and the observed trace alike. Until #90 all three carried `Nat`, so the refused negative
requests the host really receives could not be written down. Meanwhile the adapter printed them as
their unsigned 64-bit reinterpretation. A protected read commit keeps a `Nat` region, because the
host commits only a region it admitted. A release request carries no arguments: the adapter records
`null`, and the model writes `0 0`. The lowering's decision step writes an acceptance bit of `true`
that the host does not observe, and no definition reads that bit.

The relation is the projection of a **well-formed** target trace onto the source alphabet, where
well-formedness itself is a condition of the relation:

* every commit identity was decided by a `hostDecision` first (a rep number or handle class can never
  stand in for the exact identity);
* every protected read commit is preceded by the crossing that asked for exactly that region, compared
  through the exact embedding of `Nat` into `Int`, so a negative crossing explains no commit;
* a trace commits at most one release or retirement (`guestRelease`, `destructorRelease` or
  `hostRetire`), so a guest release followed by a host retirement of the same region is refused.
  Until 2026-10-29 the condition bounded each kind separately, so that pair was well formed even
  though the definition's own docstring said "at most one release or retirement per trace";
  `related_rejects_release_then_retirement` is the control for the corrected condition.

| Theorem | Claim |
|---|---|
| `erase_preserves_projection`, `erase_preserves_{decisions,commit_identities,read_crossings,release_members,read_commits}` | erasing the two named bookkeeping classes changes none of the projected trace's scan inputs |
| `erase_preserves_well_formedness` | erasure cannot repair a forged target trace |
| `related_erase_invariant` | the relation is erasure-invariant |
| `denial_projects_to_request_only` | a refused request stays visible and gains no response |
| `normal_success_projects_request_and_commit` | a normal success projects its request and its protected commit |
| `identity_is_decision_identity` | the projection keeps the exact identity it was emitted with |
| `retirement_is_not_a_guest_release` | host retirement projects to its own commit kind |
| `abnormal_exit_class_is_not_in_the_projection`, `abnormal_exit_classes_collapse_in_projection` | the trap/quota/cleanup distinction is a target-side observation |
| `append_projection` | prefix compatibility: projections compose over concatenation |
| `take_projection` | a cut trace relates only when its retained prefix is itself well formed |
| `related_rejects_{substituted_identity,unexplained_read_commit,two_guest_releases,release_then_retirement,missing_protected_commit,host_cleanup_as_guest_release,denial_erased_into_success}` | the forged target traces are refused, each by the condition that names it |
| `related_accepts_well_formed_success` | the refusals are not vacuous: a well-formed success relates |
| `retainFrom_prefix`, `retainFrom_cost_bounded` | a trace retained before fuel exhausts is a prefix of the full trace and costs no more than the budget |
| `filterMap_prefix`, `exhaustion_keeps_committed_prefix`, `exhaustion_keeps_commit_identities` | the retained trace's projected commits and identities are prefixes of the full trace's: exhaustion may cut a trace but cannot invent or drop a commit |
| `exhaustion_cannot_relate_an_unaffordable_commit`, `exhaustion_relates_when_affordable` | the exhaustion rule bites, and the same trace relates when the budget affords it |
| `retained_projection_is_permitted_prefix` | WIT-TRACE-03 corollary: a retained projection is a permitted ordered prefix of the *source* trace |
| `source_commits_are_decided`, `source_release_is_at_most_once` | WIT-TRACE-03 corollaries: exact identity and at-most-once release are inherited by the source alphabet |
| `fixture_{ordinary_error,host_bounds_refusal,trap_after_protected_copy,cleanup_failure_after_protected_copy,quota_prefix_stops_before_the_copy,quota_prefix_keeps_the_affordable_commit}` | WIT-TRACE-05 fixtures at model level: the design's observable executions relate as designed, and the quota boundary is measured rather than assumed |
| `observed_read_and_release_lowers_to_a_related_trace`, `observed_abnormal_exit_lowers_to_a_related_trace`, `lowered_projection_starts_with_the_request` | WIT-TRACE-05: the ordered trace the host actually records (a read, its protected commit, the response, a consuming release; or an abnormal exit's own retirement) lowers onto the target alphabet and relates, in the host's order |
| `observed_traces_are_paired`, `swapped_response_is_unpaired` | WIT-TRACE-03: every response follows a request of its own operation, tolerating only that request's own protected commits, and a response answering another operation is unpaired |
| `related_rejects_swapped_response`, `related_rejects_substituted_resource_identity`, `related_rejects_observed_retirement_as_guest_release` | the relation refuses a target trace whose response answers the wrong operation, whose commit carries an identity the host never authorised, or whose host retirement is presented as a guest release |
| `related_implies_read_commits_explained` | the corollary the pilot was missing: every read commit a related trace projects is explained by the import crossing that asked for that exact region |
| `lower_keeps_read_arguments` | #90: for every observed trace, the lowering's read crossings are exactly the observed read requests' signed arguments, in order |
| `observed_negative_request_projects_with_its_sign` | #90 fixture: a refused read with offset `-1` lowers and projects with `-1` |
| `related_rejects_unsigned_reinterpretation`, `related_accepts_the_signed_refused_request` | #90: a source trace that reports the refused `-1` as `2^64 - 1` does not relate to the observed trace, and the source trace that keeps the sign does |
| `related_rejects_commit_of_reinterpreted_region` | #90: a protected read commit of the reinterpreted region is not explained by the signed crossing that asked for `-1` |
| `retainFrom_complete`, `retainFrom_fuel_stable`, `related_stable_under_more_fuel` | fuel stability (Langlib's `halted_stable`): a budget that affords the whole trace cuts nothing, so a run completed under one budget, and any relation it satisfies, is unchanged under every larger budget |
| `magic_fuel_relates_only_at_one_budget`, `magic_fuel_is_not_fuel_stable` | control: a semantics that is right at the completing budget but returns nothing one unit later relates there and not after, and `FuelStable` refuses it |
| `related_rejects_omitted_unused_read`, `related_source_reads_are_target_reads` | control: a trace that omits a protected read whose value is unused ends in the same answer, yet is refused; in general, a related source trace claims exactly the target's read crossings, in order |
| `abnormal_exit_keeps_commits`, `related_rejects_trap_that_drops_a_commit` | an abnormal exit projects to nothing, so a source trace that drops a commit made before a trap is refused, and the one that keeps it relates |
| `lower_correct_on_every_input`, `empty_only_lowering_matches_on_the_empty_input`, `empty_only_lowering_is_not_correct_on_every_input` | the lowering relates every read-and-release input (identity, region, returned value); a lowering that is right only on the empty input agrees there, but the universal statement refuses it |
| `related_rejects_swapped_requests`, `related_rejects_substituted_response_value` | a source trace that claims the release before the read, or a different read value, does not relate to the observed trace |
| `authorisedFrom_of_identitiesDecided`, `relatedFrom_of_related` | #50: the registration premise only widens the relation; every trace `related` relates is related under any registration |
| `registration_authorises_an_early_retirement` | #50: a host retirement before any guest request relates under the registration the host reports, and is refused without it |
| `relatedFrom_rejects_a_forged_retirement`, `registration_does_not_authorise_a_read_commit`, `registration_does_not_authorise_a_guest_release` | #50 controls: a registration authorises a host retirement of the registered identity and nothing else; a forged retirement, a read commit decided for another identity and a bare guest release stay refused |

## Runtime fixtures (WIT-TRACE-05)

`proofs/m5/RegionTraceRuntime.lean` is generated by `runtime-fixtures.mjs` from the effect-protocol
lane's retained receipt `verification/effect-protocol/execution-9b1fe313.json`. Do not edit it by
hand: the check regenerates it, with `region-trace-runtime-inventory.json`, and requires both byte
for byte. Regenerate with `node verification/region-trace/runtime-fixtures.mjs`.

Each fixture is one executed case's recorded `noble-bounded-region/v2` trace, written as
`ObservedEvent`s in the host's order, and each statement's source trace is the generator's own
projection. A JavaScript mirror of `wellFormed` decides each verdict, and the kernel re-checks it by
`decide`, so the two must agree. Results:
- **18 traces relate** (`runtime_*_relates`):
  - success and ordinary errors, including `-1` and `i64::MAX` offsets;
  - the scope-exhaustion prefix the host truncated at 64 events;
  - traps and quota exhaustion after the commit;
  - all four cancellations;
  - the omitted release;
  - the runtime-refused second calls;
  - the outside-model results.

  Each is paired with a control (`*_refuses_an_unexplained_commit`) that appends a read commit no
  crossing explains, which the relation refuses.
- **2 traces relate under their registration** (`*_relates_under_its_registration`): EPROTO-CASE-05's
  trap before use and EPROTO-CASE-06's quota exhaustion before the read. The host retires the owner
  it registered before guest start, and the guest made no request, so no decision step authorises the
  retired identity and `related` still refuses the trace (`*_is_refused_without_a_registration_decision`).
  The v2 report now names the registered owner (`registration.identity`, #50), and `relatedFrom`
  takes it as a premise that authorises a host retirement and nothing else. The premise comes from
  the report, never from the retirement it authorises, and the generator invents no event. A
  retirement of an identity the host did not register stays refused (`*_refuses_a_forged_retirement`).

The exit class is outside `ObservedEvent`. A trap, a quota exhaustion and a cancellation that leave
the same events therefore lower to the same trace, and the model has no cancellation event even
though the host control now exists. The fixtures are finite tests of recorded traces. They bind the
observations, not the component or engine that produced them.

`verification/region-trace/receipt.json` is the retained receipt of one run, on the clean commit
`6b4301b1` with Lean 4.35.0: **106** theorems, the 68 model theorems and 38 runtime fixtures (18
related, each with its control, and 2 model gaps). The reported axioms are `propext`,
`Classical.choice` and `Quot.sound`, all inside the standard allow-list. Most proofs report
`Classical.choice` under Lean 4.35.0. The receipt at `1bf39a52`, under Lean 4.31.0, reported only
`propext` and `Quot.sound`.

## Revisions the verdict depends on (WIT-TRACE-06)

The receipt carries a `bindings` block: the model revision (pinned Lean version and executable hash,
plus the source and inventory hashes) is bound, and `component`, `helper` and `engine` are recorded as
**`null` with the reason stated** — no component, lowering helper or engine instance is a subject of
these theorems, so binding one would imply a correspondence this package does not establish. An
`observations` block binds the effect-protocol receipt the runtime fixtures are generated from. A
`claims` block labels the verdict: 106 theorems, 0 implementation theorems, the runtime fixture
counts, correspondence none, liveness none, and cancellation unmodelled.

## Non-claims

The target alphabet is a **handwritten machine**, not an emitted step sequence: there is no
Canonical-ABI, Wasmtime, Rust host or lowering correspondence here, no liveness claim and no
cancellation model. Pairing is stated over projected source traces, not proved for the emitted
component's own steps, and the ordered trace's lowering maps an *observation* onto the target
alphabet rather than corresponding to the emitted steps. WIT-TRACE-02 through WIT-TRACE-06 remain
open.

The four fuel, omitted-read, trap and empty-input controls follow the adoption review of
`Langlib/Common/Compilation.lean` (#50). They use it as a checklist only: no Langlib code is reused
or pinned, and its completed-trace agreement does not replace the every-prefix relation here.
