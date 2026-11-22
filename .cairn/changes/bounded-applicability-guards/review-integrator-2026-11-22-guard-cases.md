# Integrator review: GUARD-CASE-01..12 registration and gate (2026-11-22)

This is an **integrator review** under owner decision 28 (reviewer agents unavailable), not a
fresh-context review. The owner keeps a veto before any push.

## Scope

- `specs/conformance/guard-cases.json` (revision `0.1.0-draft.5`): the twelve GUARD-CASE
  registrations and the 2026-11-22 GUARD-CASE-11 amendment.
- `verification/guard-cases/gate.mjs` and its fixtures (`verification/guard-cases/fixtures/`).
- The implementation the cases exercise: the v2 `NonEmptyList` invocation at v1 parity
  (`operations/invocation.rs`), the GUARD-CASE-02 core control (`controls/guard.rs`), the
  abnormal-guard-exit report and session termination, and the test-only guard fault selection
  (`runtime/live-test-barrier.mjs`).
- Reviewed against the change design (`design.md`, "Boundary and hostile test plan") and owner
  decisions 32, 33, 33a and 34.

## Registration fidelity, case by case

| Case | Design row | Registration | Verdict |
|---|---|---|---|
| 01 | v1 `LtI64Max` at MAX, MAX-1, MIN; `NeI64Min` at MIN, MIN+1, MAX; v1 bytes unchanged | same arguments and outcomes; v1 wrapper bytes and no set version | faithful |
| 02 | eq 5 at 5, 6, -5; an `EqI64Literal 6` binding substituted refuses before execution | same; the substitution runs as a core control against `Core::correspond`/`bind` (decision 34), refusing `correspondence-mismatch`, the code the core returns, where the design guessed `MismatchedContext`/`StaleContext` | faithful; the refusal name is the implemented one |
| 03 | `head-of-nonempty` with `[]`, `[MIN]`, `[7, 8]`, nested `[[]]`; tail preserved | same; `[]` spelled `nil dup 0 swap cons drop` (a bare `nil` leaves the element type open and is refused at injection typing); tail clause kept and implemented (decision 32) | faithful |
| 04-06 | undefined selectors, conjunctions, near-miss shapes refuse as unsupported | same contracts; refusal stage observed at invocation | faithful |
| 07 | changed predicate unguarded; changed policy stale | `ensures true` on both contracts (the design's `head` postcondition is unprovable under `requires true`) | executable form of the design |
| 08 | changed subject and changed family capture refuse before any guard | same; the family variant presents a builder with a changed captured constant | faithful |
| 09 | erased evidence runs identically and claims nothing; static proof erased blocks release | same; empty argument spelled as in 03 | faithful |
| 10 | no composite inherits a guard | every guarded composition refuses at the implication (the composer composes only two `IncrementBy` premises); the derived increment composite carries no guard | executable form; the static-witness rule waits for #7 |
| 11 | quota and trap distinct from rejection, body not started, frame restored | amended (decision 33a): the poisoned engine ends the session instead of restoring the frame; the cap-1 quota expires before the guard's first execution step | amended; see below |
| 12 | wrong-typed slot refuses at injection typing | same | faithful |

## GUARD-CASE-11 amendment (decision 33a)

- The engine poisons itself on every abnormal guest exit (`this.poisoned = status !== 0`) and its
  worker exits, so the design's "frame restored" cannot be observed without a recovery path
  that would need the exact-release evidence decision 6 requires. Ending the session is the
  fail-closed reading; the case registers it as `frame_restored: false` with
  `session_state: terminated-after-runtime-failure` and a refused later operation.
- The meter charges guard and body work together and does not mark where the body starts. A
  sweep (caps 1, 2, 3, 4, 6, 8, 12, 16, 24, 32) showed cap 1 exhausting in
  reflection/type-observation work with zero execution steps, and every larger cap exhausting
  in execution work, where the body's start is not observable. Cap 1 is the only cap at which
  "body not started" is observed rather than argued; the design's "inside dup/list.case"
  location is recorded as not placed.
- The trap is raised at the top of the real guest frame (zero execution and reflection steps),
  through the engine's own classification and poisoning path.
- The hook exists only in the `live-test-barrier` build, is one-shot, selects exactly one
  execution by the SHA-256 of its checked source and grants no authority. The shipped binary
  neither defines nor forwards it.

## Implementation review

- `Driver::execute` records the terminal outcome before parsing the report; `dispatch` refuses
  every operation afterwards. Both v1 and v2 guard paths skip restoration and copy release on a
  poisoned engine and report the engine's own fields; neither claims whether the body started.
- No production behavior changes for normal or rejecting invocations; the v1 frozen fields are
  untouched. Before this change an abnormal guard exit surfaced as an opaque engine I/O failure
  on the dead worker and left the session model emptied by `park`.

## Method

- Octet (flake `octet` check through the leviathan probe) at `d2504afd`: status clean, 0 findings.
- `cargo clippy -p noble-cli --all-targets --features live-test-barrier -- -D warnings`: clean.
- `cargo test -p noble-cli --bin noble companions`: passed (the Lean-backed controls are run by
  the gate).
- Development gate run on `d2504afd` (5,885 s, 27 sessions, the production and
  `live-test-barrier` binaries built from the clean checkout): GUARD-CASE-01, 02, 04-08, 10, 11
  and 12 observed as registered. GUARD-CASE-03 and -09 were reported unmet by two gate reading
  errors, not behavior differences: a list output is `{type: List, value: [items]}` (the gate
  read `items`), and a plain submission's stack cell carries its value as the JSON text of
  `detail` (the gate read top-level fields). The transcripts show the registered results
  (`[[]]`'s head is the empty list; the erased wrapper returns `inl 1` and `inr 7`). Fixed in
  `eccbf015`; the final receipt comes from the merged batch tree.
- Before that run, the smoke and sweep runs found GUARD-CASE-03/09's bare `nil` refused at
  injection typing, which the registration now spells as an empty `List I64`.
- Final runs on the batch-4 tree (retained in `verification/guard-cases/runs/`):
  - `20261122-b4` at `f5c4ccec` (4,071 s): every case observed as registered except
    GUARD-CASE-03's tail clause, a third gate reading error: `shape()` read a stack cell's
    `value`, while a cell carries its value as the JSON text of `detail`, so the `I64 41` tail
    read as `null` before and after. The transcript keeps the tail at the bottom of all 21 stacks,
    and all 906 observed cells' `detail` texts parse as JSON. Fixed in `db0fe933`.
  - `20261122-b4r` at `db0fe933` (4,646 s): verdict `all-passed`, GUARD-CASE-01..12 promotable;
    receipt sha256 `da95ab5a...`, source revision `sha256:cc9dda44...`; both binaries
    (`587b3b71...`, fault build `d2ec6346...`) hashed unchanged before and after; all 37 raw
    outputs re-verified against the receipt's digests after retention.

## Verdict

ACCEPT. The registration is faithful to the design rows, the amended and executable forms are
recorded with their reasons (decisions 32, 33, 33a and 34), and the final run observed every
registered expectation. GUARD-CASE-01..12 may be promoted to implemented/passed with proof open
and explicit trust, citing `runs/20261122-b4r`. Not claimed: C1 for `templates_of`, C6 and PO-21;
faults the shipped binary could meet (allocation failure, a trap mid-guard) are not injected.
