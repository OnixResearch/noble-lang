# Region trace relation pilot (WIT-TRACE-01)

Model-theorem check for `.cairn/changes/region-trace-relation`. It builds
`proofs/m5/RegionTrace.lean` and generated `proofs/m5/RegionTraceRuntime.lean` in an isolated project
with the pinned Lean toolchain, then checks
that each inventoried theorem is declared in the sources, that it elaborates to a compiled statement
the receipt records, and that its transitive axioms stay inside the row's allow-list. Statement text
is recorded, not compared with an expected statement, so a weaker statement is not detected here;
what the lane enforces is the declaration set and the axiom set.

## Command

```sh
env -i HOME="$HOME" USER="$USER" LOGNAME="$USER" PATH=/run/current-system/sw/bin:/usr/bin:/bin \
  "<absolute path to the Node executable pinned in policy/tool-selection.json>" \
  verification/region-trace/check.mjs \
  --output "<new directory outside the repository>"
```

The harness refuses a caller environment carrying `LEAN*`, `LAKE*`, `BASH*`, `GIT_*`, `NIX_*` or
`ELAN*` variables, refuses an output directory inside the source tree, and refuses a dirty worktree
unless `--allow-dirty` is passed explicitly (a dirty receipt records it, it does not hide it).

## What is proved

The source alphabet is `RegionProtocol-v1`'s `Req`/`Resp`/`Commit`; the target alphabet is a
handwritten model of the host's ABI-step shape (`importCrossing`, `hostDecision`, `protectedCopy`,
`returned`, `normalReturn`, `abnormalExit`, `cleanup`, and the two named erasable bookkeeping steps).

A request carries the guest's signed (`s64`) arguments as `Int`, in the source alphabet, the target
alphabet and the observed trace alike. Until #90 all three carried `Nat`, so the refused negative
requests the host really receives could not be written down. Meanwhile the adapter printed them as
their unsigned 64-bit reinterpretation. A protected read commit keeps a `Nat` region, because the
host commits only a region it admitted. A release request carries no arguments: the adapter records
`null`, and the model writes `0 0`. The host report does not expose a separate admission-decision
event. The lowering therefore labels each request's identity witness accepted; the relation uses
that model bit only to authorize a commit, while refused responses without commits remain visible.
This is an observation premise, not a measured decision.

The relation is the projection of a **well-formed** target trace onto the source alphabet, where
well-formedness itself is a condition of the relation:

* every commit identity is authorized by an earlier accepted `hostDecision`; registration adds
  authority only for host retirement (a rep number or handle class can never stand in for exact identity);
* every protected read commit is paired with the preceding read crossing, its accepted decision
  identity and copy length; the exact embedding of `Nat` into `Int` means a negative crossing explains
  no commit;
* a guest-release commit is paired with its own release request and accepted decision identity; a
  host retirement cannot supply that guest request or commit;
* a trace commits at most one release or retirement (`guestRelease`, `destructorRelease` or
  `hostRetire`), so a guest release followed by a host retirement of the same region is refused.
  Until 2026-10-29 the condition bounded each kind separately, so that pair was well formed even
  though the definition's own docstring said "at most one release or retirement per trace";
  `related_rejects_release_then_retirement` is the control for the corrected condition.

| Theorem | Claim |
|---|---|
| `erase_preserves_projection`, `erase_preserves_{decisions,commit_identities,read_crossings,release_members,read_commits}` | erasing the two named bookkeeping classes changes none of the projected trace's scan inputs |
| `erase_preserves_{commit_authority,read_trace_events,release_trace_events}` | erasure preserves ordered authorization and request/decision/copy/commit/response scans |
| `commit_authority_mono`, `authorisedFrom_of_identitiesDecided` | registration only adds retirement authority; it cannot revoke a prior decision |
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
| `related_rejects_{read_as_guest_release_commit,read_commit_before_its_decision,crossed_read_commit_identities,crossed_guest_release_identity}`, `related_accepts_guest_release_commit` | read/release commits stay bound to the exact preceding operation and decision, not merely any decided identity |
| `related_accepts_well_formed_success` | the refusals are not vacuous: a well-formed success relates |
| `retainFrom_prefix`, `retainFrom_cost_bounded` | a trace retained before fuel exhausts is a prefix of the full trace and costs no more than the budget |
| `filterMap_prefix`, `exhaustion_keeps_committed_prefix`, `exhaustion_keeps_commit_identities` | the retained trace's projected commits and identities are prefixes of the full trace's: exhaustion may cut a trace but cannot invent or drop a commit |
| `exhaustion_cannot_relate_an_unaffordable_commit`, `exhaustion_relates_when_affordable` | the exhaustion rule bites, and the same trace relates when the budget affords it |
| `retained_projection_is_permitted_prefix` | WIT-TRACE-03 corollary: a retained projection is a permitted ordered prefix of the *source* trace |
| `source_commits_are_decided`, `source_release_is_at_most_once` | WIT-TRACE-03 corollaries: exact identity and at-most-once release are inherited by the source alphabet |
| `fixture_{ordinary_error,host_bounds_refusal,trap_after_protected_copy,cleanup_failure_after_protected_copy,quota_prefix_stops_before_the_copy,quota_prefix_keeps_the_affordable_commit}` | WIT-TRACE-05 fixtures at model level: the design's observable executions relate as designed, and the quota boundary is measured rather than assumed |
| `observed_read_and_release_lowers_to_a_related_trace`, `observed_abnormal_exit_lowers_to_a_related_trace`, `lowered_projection_starts_with_the_request` | WIT-TRACE-05: the ordered trace the host actually records (a read, its protected commit, the response, a consuming release; or an abnormal exit's own retirement) lowers onto the target alphabet and relates, in the host's order |
| `observed_traces_are_paired`, `swapped_response_is_unpaired` | WIT-TRACE-02/03: a response answers its preceding request of the same operation, tolerating only that request's protected commits in between |
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
| `authorisedFrom_of_identitiesDecided`, `relatedFrom_of_related` | #50: the registration premise only widens the relation; every trace `related` relates under any registration |
| `related_projects_only_paired_traces`, `paired_prefix_may_end_with_an_unanswered_request` | WIT-TRACE-02: every related response answers its preceding request of the same operation; an unmatched final request remains valid at a cut prefix |
| `related_accepts_paired_response` | WIT-TRACE-02 positive control: the relation accepts a response paired to its preceding request |
| `registration_authorises_an_early_retirement` | #50: a host retirement before any guest request relates under the registration the host reports, and is refused without it |
| `relatedFrom_rejects_a_forged_retirement`, `registration_does_not_authorise_a_read_commit`, `registration_does_not_authorise_a_guest_release` | #50 controls: a registration authorises a host retirement of the registered identity and nothing else; a forged retirement, a read commit decided for another identity and a bare guest release stay refused |
| `observed_exit_lowers_to_its_abnormal_exit_class` | #50: the reported exit class is an observed event and lowers to the target's own `abnormalExit` step, for `trap`, `quotaExhaustion`, `cancelled`, `adapterInvalid` and `outsideModel` |
| `exit_classes_stay_distinct_after_lowering` | #50 control: a trap, a quota exhaustion, a cancellation, an adapter fault and an outside-model result stay distinct target steps after lowering |
| `exit_class_is_not_in_the_projection`, `exit_without_events_relates_vacuously` | #50: the projection erases the exit class, so two observations that differ only there project to the same source trace; the class is a target-side observation and no source trace can carry a cancellation |
| `retirement_refused_is_a_cleanup_failure` | #50: a refused retirement lowers to the cleanup-failure exit class and commits nothing |
| `a_refused_retirement_is_not_a_retirement`, `refused_retirement_keeps_the_committed_prefix` | #50 control: a source trace that claims a refused retirement did retire the owner is refused, while the trace that claims nothing relates and the commit the run already made survives |

## Runtime fixtures (WIT-TRACE-05)

`proofs/m5/RegionTraceRuntime.lean` is generated by `runtime-fixtures.mjs` from the effect-protocol
lane's retained receipt `verification/effect-protocol/execution-9b1fe313.json`. Do not edit it by
hand: the check regenerates it, with `region-trace-runtime-inventory.json`, and requires both byte
for byte. Regenerate with `/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node verification/region-trace/runtime-fixtures.mjs`.

Each fixture is one executed case's recorded `noble-bounded-region/v2` trace, written as
`ObservedEvent`s in the host's order; the separate report `exit_class` is placed before the first
post-exit `host.retire` request, or at the end if no host retirement was attempted. Each statement's
source trace is the generator's own projection. A JavaScript mirror of `wellFormed`
decides each verdict, and the kernel re-checks it by `decide`, so the two must agree. Results:
- **18 traces relate** (`runtime_*_relates`):
  - success and ordinary errors, including `-1` and `i64::MAX` offsets;
  - the scope-exhaustion trace, which now reaches the host's refusal of the 65th read;
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

The report's separate `exit_class` field is represented by `ObservedEvent.exit`; an absent field
becomes `.exit .unknown`, never a successful outcome. The generator
places it immediately before the first post-exit `host.retire` request, or at the end when no host
retirement was attempted. Thus a trap, quota exhaustion and cancellation that leave the same host
events lower to *distinct* target steps while still projecting to the same source trace
(`exit_class_is_not_in_the_projection`). This is generated from the report field, not an event in
the host's `events` array. A conformant return (`normal`, `rejected`) adds no exit step. The v2 trace bound reaches the scope ceiling's own trace, so
EPROTO-CASE-03's complete trace — 64 admitted reads, the refused 65th read and the guest's release
— is the pilot's runtime host-refusal trace. A refused `host.retire`, which no executed case
produces, is accepted by the generator and lowers to the cleanup-failure exit class with no commit.
The fixtures are finite tests of recorded traces. They bind the observations, not the component or
engine that produced them.

`verification/region-trace/receipt.json` is the retained receipt of one run, on the clean commit
`109ac318` with Lean 4.35.0: **116** theorems, the 74 model theorems and 42 runtime fixtures. Of
the fixtures, 18 traces relate, each with its control, and 2 relate under their registration, each
with its premise-free refusal and its forged-retirement control. The reported axioms are `propext`,
`Classical.choice` and `Quot.sound`, all inside the standard allow-list. Most proofs report
`Classical.choice` under Lean 4.35.0. The receipt at `1bf39a52`, under Lean 4.31.0, reported only
`propext` and `Quot.sound`. The previous receipt, at `6b4301b1`, had 106 theorems and the two early
retirements as model gaps.

## Revisions the verdict depends on (WIT-TRACE-06)

The receipt's `bindings` block binds five revisions, and each binding states what this replay
actually exercised:

| Binding | Contents | Label |
|---|---|---|
| `model` | pinned Lean version and executable hash, plus the four source/inventory hashes | binding `tested`; theorem declarations `proved` by the kernel and audited in this run |
| `observations` | the effect-protocol receipt, its SHA-256, and the source commit and tree it was recorded from | `tested`: the fixtures are regenerated from it and must match byte for byte; correspondence `open` |
| `component` | every `component.wasm` SHA-256 the receipt retains (the compiled subject and each hostile component) | `tested`: the retained byte strings are re-hashed and must equal their records; `correspondence: open` |
| `helper` | the CLI's `cli_sha256`, the wasm-tools version and the vendor NAR hash | `tested` against the pinned tool selection; the CLI itself is not re-hashed here; `correspondence: open` |
| `engine` | the Wasmtime version, cross-checked against the `Cargo.lock` entry and the pinned tool selection | `tested`, refusing on any disagreement; `correspondence: open` |

`correspondence: open` on the component/helper/engine and observation bindings is deliberate: they
name the revisions the observations came from, and no emitted-step correspondence is proved. The
receipt labels model theorems `proved`, runtime fixtures and cleanup host-method evidence `tested`,
the identity/pairing/fuel premises `assumed`, and emitted-step correspondence, guest-reachable
cleanup refusal and liveness `open`. Cancellation is a tested host control represented target-side;
no general cancellation API is claimed.

### Correspondence route (named, not proved)

* The Canonical ABI's lift/lower of a guest request is `importCrossing` plus the host's
  `hostDecision`; its lowered result is `returned`.
* `Host::read`, `Host::release` and `Host::abnormal_exit_cleanup` are `hostDecision`,
  `protectedCopy` and `cleanup`.
* A Wasmtime trap, `OutOfFuel` included, and the host's own cancellation are `abnormalExit` with
  the class the report names; a result outside the schema is `abnormalExit outsideModel`; a refused
  retirement is `cleanupFailure`, committing nothing.
* PO-17/PO-18 stay the home of emitted-component evidence, and #10 owns any real lowering
  correspondence. This package does not claim one.

### A stale binding fails the replay

The check resolves the bindings before it writes the receipt, and it runs eight refusal controls:
stale component digest and bytes, stale wasm-tools versions in the observation receipt and pinned
tool selection, a modified observation receipt (which includes its CLI-hash provenance), stale
Wasmtime versions in the observation receipt and pinned tool selection, and a stale `Cargo.lock`
Wasmtime version. The CLI hash is retained and protected by the fixed observation-receipt digest;
the executable itself is not re-hashed in this lane. Each control must refuse, and the receipt
records all eight as `expected: refused, observed: refused`.

The checker also passes a synthetic refused `host.retire` observation through the generator and
requires a cleanup-refusal event with no retirement commit. This pins the generator path without
pretending that any executed guest trace contains a cleanup refusal.

## Cleanup selection, invocation and retirement (acceptance item 4)

The relation's target alphabet commits at most one release or retirement and never converts a
retirement into a guest release. The finer distinction item 4 asks for — that cleanup be *selected*,
*invoked* and then *successfully retired* are three different observations, with failure evidence
retained — is proved over the host's cleanup path in `proofs/m5/HostLocality.lean`:
`cleanup_failure_is_selected_invoked_and_retained`, `cleanup_without_owner_is_skipped` and
`cleanup_success_is_not_selected_twice`, over the `CleanupSelection` / `CleanupInvocation` /
`CleanupOutcome` step relation. The retained control is
`cleanup-release-refusal-retains-owner` in `verification/host-locality/evidence-1bf39a52.json`,
which shows the retained owner still live after a refused release.

Both are **tested at the host method, not reachable by a guest**: reaching a refused retirement needs
a corrupted host context, which the WIT interface cannot produce. The model therefore gained a
refused-retirement observation (`ObservedEvent.retirementRefused`) that the generator accepts and
that lowers to the cleanup-failure class with no commit. The checker pins that generator branch
with a synthetic control, but no executed case records one. The
decision *not* to add a production fault-injection seam for it is recorded in the change's
`design.md`; a test-only seam in the shipped adapter would be a larger risk than the gap.

## Lentil is not pinned

#50's follow-on comment (infinite-trace projection, weak fairness) is recorded as a decision, not a
dependency: Lentil main needs Lean 4.34.1 while this pilot is on 4.35.0, and the comment itself
defers the progress gate to #46. The finite-trace embedding it sketches is only meaningful once a
progressing transition system and its observation mapping exist, so nothing is pinned here and no
temporal property is claimed.

## Non-claims

The target alphabet is a **handwritten machine**, not an emitted step sequence: there is no
Canonical-ABI, Wasmtime, Rust host or lowering correspondence here, no liveness claim and no
cancellation *API* claim. Pairing is stated over projected source traces, not proved for the emitted
component's own steps, and the ordered trace's lowering maps an *observation* onto the target
alphabet rather than corresponding to the emitted steps. The component, helper and engine bindings
name revisions; they are not a correspondence. A refused retirement is modelled and generator-ready
but not observed from a guest-reachable path. Any emitted-step correspondence remains open under
#10; WIT-TRACE-05's cleanup observation is the host-method evidence above.

The four fuel, omitted-read, trap and empty-input controls follow the adoption review of
`Langlib/Common/Compilation.lean` (#50). They use it as a checklist only: no Langlib code is reused
or pinned, and its completed-trace agreement does not replace the every-prefix relation here.
