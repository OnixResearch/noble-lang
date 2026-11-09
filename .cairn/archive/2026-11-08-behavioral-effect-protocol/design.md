## Context

This is a design-only change for issue #9. Nothing below is implemented, executed or proved by this change.

**Specification baseline re-verified at `44329dc`** (no drift from the issue's `0ea10d3` in `.cairn/specs/`, `crates/noble-cli/src/component/`, `crates/noble-wasm/wit/` or `crates/noble-kernel/src/resources/`).

The program-contracts requirements are:

| Requirement | Location | Content |
|---|---|---|
| VC-LOGIC-01 | `.cairn/specs/program-contracts/spec.md:107` | Ordinary `Result` errors are normal returns. |
| VC-LOGIC-03 | `:171` | Separates partial-correctness, prefix-safety and total-correctness. |
| VC-LOGIC-04 | `:181` | Quota and allocation abstractions must be stated. |
| VC-LOGIC-05 | `:191` | Host-state and trace contracts must name modeled operations and response assumptions. |
| VC-COMPOSE-02 | `:240` | Ordering and ownership must be preserved; a set union of effects is not commutation, and there is no generic frame rule. |
| VC-USE-01 | `:1024` | Host-state predicates need a protocol that prevents stale observations. |
| VC-BOOT-01 | `:673` | Defers host trace/resource protocols. |

The delivered MC1/MC2 fragment is resource-free (`§8.1` unsupported corners: "resources, host/effectful instructions and nonempty effects; invariants, effect/trace or ownership clauses"). The language and resource requirements are:

- **Language.** K-RES-01 (`.cairn/specs/language/spec.md:1404`): one ownership obligation per live handle, moved, consumed or explicitly released on every normal path. K-RES-02 (`:1414`): no `dup`, generic `drop`, `quote` or serialization of resources. K-RES-03 (`:1431`): every normal result alternative accounts for ownership. K-RES-04 (`:1458`): the host retires invocation-owned handles on abort, idempotently, without guest continuation. K-STACK-01 covers `dip` and abnormal outcomes.
- **Resource adapters.** RA-STATE-01 (`.cairn/specs/resource-adapters/spec.md:153`): validate before protected work. RA-CLEAN-01 (`:232`), RA-CLEAN-02 (`:273`) and RA-CLEAN-03 (`:306`): abnormal cleanup without continuation, cancellation revokes access immediately, and retirement is idempotent.
- **Protocols and outcomes (existing; reused, not duplicated).**
  - DX-PROTOCOL-01/02/03 (`.cairn/specs/developer-experience/spec.md:559`, `:576`, `:595`): states, legal operations, failure outcomes, cleanup and runtime-handle correspondence; no usable prior state after a failed transition; explicit coverage of every state/event pair.
  - H-OUTCOME-01 (`language/spec.md:2937`): distinguish normal return, trap, budget exhaustion, cancellation, host deadline and internal failure.
  - K-ERROR-03 (`:721`): quotas and traps are not successful results.
  - K-EVAL-04 (`:665`): after an abnormal `p`, `q` does not start, and effects are not rolled back.
  - S-MEM-02 (`safety/spec.md:245`): use-after-retirement, double release and forged handles have specified failures.

  This design instantiates those obligations for one adapter.

### Adapter audit (actual behavior, not a proposed API)

| World (`crates/noble-wasm/wit/`) | Shape | Fit for acquire/use/release |
|---|---|---|
| `authorized-fs.wit` | `fs.read(path) -> result<list<u8>,string>`; the host retains the Directory | No guest-held resource, so there is no release step |
| `callback-owner.wit` | `issue() -> own<token>`, `consume(own<token>) -> s64` | Acquire/consume only; no use step and no ordinary error |
| `bootstrap.wit` counters | borrowed `counter.read() -> result<s64,string>`, `transfer(own<counter>)` | Plausible, but `transfer` mixes consumption with a domain result |
| **`bounded-region.wit`** | host-injected `own<region>`; borrowed `region.read(s64,s64) -> result<list<u8>,string>`; `release(own<region>)` | **Selected.** It has all three phases and ordinary errors, and its canonical case is accepted source-bound |

The `noble-test:bounded-region@1.0.0` adapter lives in `crates/noble-cli/src/component/bounded_region.rs`; its actual behavior at `44329dc` is listed below by line.

| Lines | Behavior |
|---|---|
| `:34-48` | `Host::new` accepts up to 4096 host-selected bytes (`MAX_REGION_BYTES`, `:16`). It reserves a fresh nonrecycled table/context identity (`component/identity.rs:8-13`), builds a 1-slot table and registers **one** owner with `Requirement{context, kind 1, READ}`. Store limits are a 4 MiB memory cap (`:44`) with fuel `10_000_000` (`:205`). |
| `:208-212` | The guest receives only `Resource::<Region>::new_own(1)` as the first argument of the `read-region` export. |
| `:56-87`, `:132-149` | **Use** (`[method]region.read`). The linker rejects an owned receiver with a host error, so a borrow is required (`:139`). `read` checks `rep == 1`, owner presence, `Table::validate(claim, required)` and `claim == actual` (otherwise `invalid-handle`). It then checks signed checked `offset+length`, nonnegativity, extent and the 4096 output cap (otherwise `bounds-reject`). Only then does it run `begin` → `native_access` → copy `bytes[start..finish]` → `complete`, restoring the owner. A failed `begin`/`native_access` returns `invalid-handle` with the owner restored. |
| `:89-100`, `:150-157` | **Release** (`release(own<region>)`). It requires an owned value. `owner.take()` fails with `"region already released"` (a host error, so a trap). Otherwise `Table::release` runs and `releases += 1`. |
| `:128-131`, `:165-168` | **Resource destructor.** If the owner is still present, the destructor also calls `release`. |
| `:209-234` | **Return/abort.** Any result variant other than `ok(list<u8>)`/`err(string)` is reported as `adapter-trap`. Any call error, including guest trap and fuel/memory exhaustion, is also `adapter-trap`. Afterwards, if the owner is still present, the host calls `release(1)` (`:232-233`, "Abnormal guest exit must retire its owner"). |
| `:102-117` | **Report** `noble-bounded-region/v1`: `outcome` (`read`, the error string, or `adapter-trap`), `bytes_hex`, `error`, `protected_operations`, `region_reads`, `native_memory_access:false`, `released_owners`, `live_owners`, `native_pins`. |

The guest source is `region.read swap regions.release` (`verification/scase02/corrected-gate.mjs:73`). It threads the borrow, keeps the result and releases on both branches. S-CASE-02 (`specs/conformance/safety-cases.json`) has `execution:"passed"`, `proof:"open"`.

**Audit findings that matter for a protocol contract:**

- **F1.** `released_owners` counts guest `release`, destructor drop and host post-return retirement identically, so an omitted guest release is not observable in the report.
- **F2.** `adapter-trap` conflates guest trap, fuel exhaustion, memory-limit failure, host bail-outs (double release, owned receiver) and an unmodeled result variant.
- **F3.** No cancellation/interruption path exists for this adapter (no epoch deadline or cancel request). Fuel is the only budget.
- **F4.** The WIT error payload is an unrestricted `string`; the host emits only `bounds-reject` and `invalid-handle`, which is a code-audit fact, not a typed guarantee.
- **F5.** A read after release fails at the linker with `"missing region"` (`:140`, a host error, so a trap) before `Host::read` could return `invalid-handle` (unit test `:283`). For a foreign component, the Canonical ABI handle table most likely rejects a moved or dropped handle before either host check [INFERENCE]; `:140` and `:92` are defense in depth. A destructor release can only come from a foreign component's `resource.drop`; Noble-compiled guests emit no resource-drop import.
- **F6.** The report has no ordered event trace, only counters.
- **F7.** In `read`, the owner is taken before `begin` (`:69`). If `Table::complete` returns an error, or returns no owner (`crates/noble-kernel/src/resources/table/access.rs:85-117`), `self.owner` stays `None` (`:79-82`). Post-return cleanup then skips `release` (`:232-233`), so a table entry or pin could survive and appear only as `live_owners`/`native_pins > 0` in the report. Whether the table can reach that path right after a successful `begin` is [INFERENCE] not established. The protocol model therefore treats a nonzero terminal `live_owners`/`native_pins` as a host-obligation violation rather than assuming it is impossible.
- **F8.** The table allows 64 scopes (`:40`). The 65th admitted read fails `begin` and is reported as `invalid-handle` (`:72`), indistinguishable from an identity refusal.
- **F9.** Cleanup calls `Table::release`, never `retire`/`retire_context`, so no retirement reason is recorded. The kernel module contract says abnormal cleanup must use `retire_context` (`crates/noble-kernel/src/resources/mod.rs:6-8`).
- **F10.** `read-region` performs no recipe byte-match of the submitted component (`:185-208`), unlike the fs and callback routes. The guest program is bound only in gate receipts.
- **F11.** Only `region.read swap regions.release` has executed evidence (`verification/scase02/corrected-gate.mjs:91-116`). Traps, exhaustion, multiple reads, scope exhaustion, the destructor path, foreign components and invalid variants are not exercised.

## Decisions

### Decision: One synchronous bounded protocol, `RegionProtocol-v1`, over the existing adapter

**Choice:** The first optional profile `Effect-Protocol-Draft` covers exactly the `noble-test:bounded-region/bounded@1.0.0` world, its existing host route `noble component read-region COMPONENT HOST_BUFFER_HEX OFFSET LENGTH`, and subjects that receive one host-injected `own<region>`. Its scope is:

- one owner per invocation;
- finitely many borrowed reads;
- at most one release;
- synchronous calls only, with no suspension, callbacks, concurrency, liveness or exactly-once remote effects.

"Acquire" is the host-side registration and injection of the owner before guest start. The guest cannot acquire. No new adapter operation is invented.

**Rationale:** It is the only audited world with host-chosen identity, a use step that has ordinary success/error outcomes, and an explicit consuming release. It also already has accepted source-bound evidence and a strict resource-table proof lane (`proofs/m5/M5Resources.lean:201-274`, `transition_refines`, `normal_completion_owner_once`, `busy_owner_not_reusable`, etc.).

### Decision: Trace model with three separated event classes

**Choice:** An execution trace is a finite sequence over three event classes:

- `Req(op, args)`: a guest request crossing the Canonical ABI import, with `op ∈ {read, release, drop}`;
- `Resp(op, r)`: the host response returned to the guest;
- `Commit(k)`: an externally committed effect, with `k ∈ {region-read(start,len), guest-release, destructor-release, host-retire}`.

A `Commit(region-read)` is the host-side protected copy (`region_reads += 1`); it is distinct from the `Resp(read, ok bytes)` that carries the copy back. Host decisions (identity, bounds, authorization) are not guest-visible events. They appear only through the response and the presence or absence of a commit.

**Rationale:** VC-LOGIC-05 needs modeled operations and response assumptions. Keeping requests, responses and commits separate lets prefix safety talk about committed effects even when no response arrives (trap after commit). It also stops "the guest asked" from being read as "the host did".

### Decision: Separate the five authorities

**Choice:** Each of the following is checked by its own mechanism. None implies another.

| Concern | Established by | Not established by |
|---|---|---|
| **Effect permission** ("may call `region.read` and `regions.release`") | Program effect bound and the linker's approved-import set (`bounded_region.rs:120-175` bails on unapproved imports) | Any behavioral claim or ordering |
| **Behavioral proof** ("calls them in this order with these results and cleanup obligations") | A `RegionProtocol-v1` claim over the model below, independently checked | Effect sets, runtime observation of one trace, or host code inspection |
| **Invocation applicability** | Initial state `Live(o)` for the injected owner, the host assumptions A1–A6, and (for the in-bounds branch only) `0 ≤ off`, `0 ≤ len`, `off+len ≤ |R|` without overflow. The contract states both branches, so applicability is the host premise `Live(o)` only | A guard (no ordered-comparison guard exists; see #8) |
| **Ownership** | Linear typing of `own<region>` (K-RES-01/02), adapter-local borrow per read (RA-SCOPE-01), move into `release` | Possession of a handle claim or rep number |
| **Cleanup authority** | Guest: obligation to release on every normal path (K-RES-01). Host: sole authority to retire on abnormal exit and after normal return (K-RES-04, RA-CLEAN-01) | Guest continuation after a trap; the protocol proof |

Host **authorization** (the `READ` right and identity fixed at registration and revalidated per request) is a fifth, host-only fact. It is never inferred from the effect permission (VC-LOGIC-05).

## Protocol state machine

The state is `σ = (ownerState, R, counters)`, where `ownerState ∈ {Live(o), Busy(o), Released(o, by)}` with `by ∈ {guest, destructor, host}`. `R` is the immutable host region (`|R| ≤ 4096`). Initial state: `Live(o)`, created by the host before guest start.

| From | Guest request | Host check (in order) | Response | Commit | To |
|---|---|---|---|---|---|
| `Live(o)` | `read(borrow o, off, len)` | identity ok; `off,len ≥ 0`; `off+len` no overflow; `off+len ≤ |R|`; `len ≤ 4096`; `begin`+`native_access` ok | `ok(R[off..off+len))` | `region-read(off,len)` | `Live(o)` (via transient `Busy(o)`) |
| `Live(o)` | `read(borrow o, off, len)` | identity ok, bounds fail | `err("bounds-reject")` | none | `Live(o)` |
| `Live(o)` | `read(borrow o, …)` after 64 admitted reads (scope limit, `:40`), or a `begin`/`native_access` refusal | begin/authorization fail; owner restored (`:72`) | `err("invalid-handle")` | none | `Live(o)` |
| `Live(o)` | *(host fixture only)* forged, foreign, stale, wrong-kind or wrong-right claim | `validate` / `claim == actual` (`:57-61`) | `err("invalid-handle")` | none | `Live(o)` |
| `Live(o)` | `read(own o, …)` (owned receiver) | ABI shape | none (trap) | none | abort |
| `Live(o)` | `release(own o)` | owner present, table release ok | `()` | `guest-release` | `Released(o, guest)` |
| `Live(o)` | handle drop (destructor) | owner present | none | `destructor-release` | `Released(o, destructor)` |
| `Released(o, _)` | `read(…)` | missing owner | none (trap, F5) | none | abort |
| `Released(o, _)` | `release(…)` | already released | none (trap) | none | abort |
| `Released(o, _)` | handle drop (destructor) | owner absent | none | none (no-op, `:129`) | `Released(o, _)` |
| `Busy(o)` | any guest event | unreachable at a guest boundary (single host call) | — | — | — |
| `Busy(o)` | `complete` fails or yields no owner (F7) | host-internal | `err("invalid-handle")` | none | `Lost(o)` (host-obligation violation) |
| `Live(o)` | guest normal return | host post-return check | n/a | `host-retire` | `Released(o, host)` + `omitted-release` |
| any non-terminal | trap, fuel or memory exhaustion, (future) cancellation | none | none | `host-retire` if `Live` | `Released(o, host)` |

The table covers every state/event pair of this schema, as DX-PROTOCOL-03 requires. Forged-claim rows are not guest-reachable in production, because the claim passed to `read` is always the retained handle (`:140`); they are exercised only as direct `Host::read` fixtures (`:252-285`, `:299-313`). The current report prints exit 0 even for `adapter-trap` (`:239`); a failed setup or cleanup release exits 2 with no report (`:233`, `:240-242`).

`Busy(o)` is not observable at a guest boundary: no host-function path returns to the guest between `begin` and `complete` (`bounded_region.rs:73-82`). A failed or ownerless `complete` (F7) is modeled as the host-side transition `Busy(o) → Lost(o)`. A terminal `Lost(o)` violates the host cleanup obligation (live owner or pin remains), and no protocol claim is reported for that execution.

**Result schema** (refining the WIT `result<list<u8>, string>`):

```text
ReadResult  ::= ok(bytes : list<u8>, |bytes| = len ≤ 4096)
              | err("bounds-reject") | err("invalid-handle")
ExportResult = ReadResult of the subject's chosen read (for the selected subject)
HostReport  ::= noble-bounded-region/v1 { outcome ∈ {read, bounds-reject, invalid-handle, adapter-trap},
                 protected_operations, region_reads, released_owners, live_owners, native_pins, … }
```

Any other error string, a wrong-length `ok`, or an unknown variant is an **unmodeled response**. No `RegionProtocol-v1` claim covers an execution that contains one.

**Explicit host assumptions** (trusted, not proved):

- **A1.** The host selects `R` before instantiation and `R` is immutable for the invocation.
- **A2.** Table/context identities are fresh per invocation and nonrecycled **within one process** (`identity.rs:8-13`). Nothing guarantees uniqueness across processes.
- **A3.** Responses are exactly those in the transition table (F4 is a code audit, not a type).
- **A4.** Wasmtime 40.0.2 Canonical ABI resource-table semantics, borrow scoping and fuel accounting are correct.
- **A5.** Host post-return/abort cleanup always runs (`:232-233`) and the process does not crash between trap and retirement.
- **A6.** The relevant environment state is only `R`, the one table, and the fuel/memory limits. There is no file system, network or clock.

**Resource identity** is the tuple `(TableId, Context, slot, generation, kind 1, rights READ)`. The guest-visible `rep` is not identity.

## Claims

All claims are about `Effect-Protocol-Draft` subjects under A1–A6 and use the VC-LOGIC-03 labels separately.

**Normal-return postconditions (partial correctness, VC-LOGIC-01)** for the selected subject `region.read swap regions.release` started in `Live(o)` with arguments `(off, len)`. Every normal return satisfies exactly one of:

- **Success:** `result = ok(b)`, `b = R[off..off+len)`, the in-bounds condition holds, `trace = [Req read, Commit region-read, Resp ok b, Req release, Commit guest-release]`, final state `Released(o, guest)`, `live_owners = 0`, `native_pins = 0`.
- **Ordinary error:** `result = err(e)` with `e ∈ {bounds-reject, invalid-handle}`, `trace = [Req read, Resp err e, Req release, Commit guest-release]`, **no** `Commit region-read`, final state `Released(o, guest)`. For the injected authentic owner with fewer than 64 previously admitted reads (and no `begin`/`native_access` refusal), `e = bounds-reject` iff the in-bounds condition fails. Otherwise `e = invalid-handle`.

Both alternatives return the owner's custody to the guest after the borrow (K-RES-03) and release it once.

**Prefix safety (every finite prefix, including those ending in trap, exhaustion or cancellation):**

- **P1.** At most one `Commit` among `{guest-release, destructor-release, host-retire}` per owner.
- **P2.** No `Req read` is answered by `Resp ok` after the owner has left `Live`.
- **P3.** Every `Commit region-read(off,len)` is immediately preceded by a `Req read(off,len)` on the authentic owner that satisfies the in-bounds condition, and is followed by at most one `Resp`.
- **P4.** Every `Resp` answers the immediately preceding unanswered `Req` (synchronous, no unsolicited responses).
- **P5.** Every `Resp` belongs to the result schema.
- **P6.** No protected commit follows an abort point.

**Cleanup after abnormal termination** (host obligation, not guest; K-RES-04, RA-CLEAN-01):

- After a trap, fuel or memory exhaustion, or (future) cancellation, the terminal host state has `live_owners = 0`, `native_pins = 0` and exactly one release commit in total. The observable trace is the prefix up to the abort point plus `Commit host-retire`, if the guest had not already released.
- The guest is **not** credited with cleanup. A prefix-safety claim says nothing about whether a normal result exists.
- **Total correctness is not claimed.** Fuel is finite and the ideal model abstracts from it (VC-LOGIC-04).

## Composition

```text
{Live(o)} p {Live(o) ; T₁}     {Live(o)} q {S ; T₂}     same o, same R, same host model
----------------------------------------------------------------------------------- (Seq)
                         {Live(o)} p;q {S ; T₁·T₂}
```

- The intermediate protocol state must be **identical** per owner: same identity tuple and same `ownerState`. Matching stack types are not enough (VC-COMPOSE-01).
- Traces are concatenated in program order, never merged as sets.
- There is no frame rule. A third party's state is preserved only with an explicit locality hypothesis. For `RegionProtocol-v1`, `R` is immutable (A1), so reads frame trivially over `R`, but counters do not commute.
- A program that leaves `Released(o, _)` composes only with continuations that do not mention `o`.

**Worked safe composition:**

- `U := region.read` satisfies `{Live(o)} U {Live(o) ; [Req read, (Commit region-read)?, Resp r]}`, with the result `r` pushed above the region.
- `C := swap regions.release` satisfies `{Live(o)} C {Released(o, guest) ; [Req release, Commit guest-release]}`.
- By (Seq), `U;C` satisfies the success/error postconditions above.
- Two reads `U;(swap U')` (where `U'` reads at caller-chosen arguments) compose only because the intermediate state is `Live(o)` again after the first borrow returns.

**Rejected uses:**

| Attempt | Where it is rejected | Observation |
|---|---|---|
| Out-of-order use: release then read (`regions.release region.read` on a reordered stack) | Noble checker: the region is consumed (K-RES-01/02), so the second operand is missing (type-reject). A foreign component traps (F5). The protocol checker has no transition `Released --read-->` | no `Commit region-read` after release |
| Omitted release: guest returns without releasing (`drop` of a resource is type-rejected, K-RES-02; a foreign component can simply return) | Noble: static. Foreign: host retires post-return | the claim fails (no `Commit guest-release`); the monitor reports `omitted-release`, **after F1 is fixed** |
| Double release | Noble: static (owner consumed). Foreign: the second `release` traps ("region already released") | P1 holds: exactly one release commit |
| Wrong resource identity (forged/foreign/stale/wrong-kind/wrong-right claim; cross-host same slot/generation) | A guest cannot present a foreign claim: the claim is always the retained handle (`:140`), and a foreign or ill-typed handle traps in Wasmtime [INFERENCE]. The host defense is `Table::validate` + `claim == actual` → `err("invalid-handle")` (`bounded_region.rs:56-61`; fixture tests `:252-285`, `:299-313`) | zero `Commit region-read`; owner unchanged |
| Unmodeled response (e.g. `err("timeout")`, `ok` of the wrong length, an unknown variant) | Not producible by the audited host (A3). A monitor or a modified host fixture flags `unmodeled-response` | no protocol claim applies; the result is reported as `outside-model`, not as success or error |

## Test and proof plan

### Unexecuted scenario designs

The IDs are design-local; all are `absent / not-run / open / unassessed` with empty evidence. Every row must retain the observable trace prefix (once F6 is fixed) and the full `noble-bounded-region/v1` report.

| Case | Input | Expected observation |
|---|---|---|
| EPROTO-CASE-01 success | compiled `region.read swap regions.release`; `R=000102`, `(1,2)` and `(3,0)` | `read`, bytes `0102` / empty; trace `[Req read, Commit region-read, Resp ok, Req release, Commit guest-release]`; `released_owners=1` by guest; `live_owners=0`, `native_pins=0` |
| EPROTO-CASE-02 ordinary error | same subject, `(3,1)`, `(2,2)`, `(-1,1)`, `(i64::MAX,1)` | `bounds-reject`; no `Commit region-read`; guest release recorded; `region_reads=0` |
| EPROTO-CASE-03 host refusal | (i) foreign component issuing 65 in-bounds reads (scope exhaustion, F8); (ii) host fixtures for the forged, cross-host and `native_access`-refusal claims, labeled fixture-level because they are not guest-reachable | (i) first 64 `ok`, 65th `invalid-handle` with no commit; (ii) `invalid-handle`; zero reads; owner unchanged until the single release |
| EPROTO-CASE-04 trap after commit | independently constructed hostile component: `read(1,2)` then `unreachable` | prefix `[Req read, Commit region-read, Resp ok]`, then abort; `host-retire` once; classification `trap` (requires F2) |
| EPROTO-CASE-05 trap before use | hostile component traps before any import call | empty prefix + `host-retire`; zero reads |
| EPROTO-CASE-06 quota exhaustion | host fuel set (new explicit test knob) so exhaustion occurs after the first commit and before release | prefix preserved; classification `quota-exhausted`, distinct from `trap`; one `host-retire` |
| EPROTO-CASE-07 cancellation | **requires a new host interruption control** (F3). Host-initiated interrupt between two reads | access revoked immediately; prefix preserved; `cancelled`; one `host-retire`; until the control exists the case remains `unsupported`, not passed |
| EPROTO-CASE-08 omitted release | foreign component returns `ok` without release | outcome `read`; `host-retire` (not `guest-release`); monitor verdict `omitted-release` |
| EPROTO-CASE-09 double release / use-after-release | foreign component: `release; release` and `release; read` | trap at the second call; exactly one release commit; zero reads after release |
| EPROTO-CASE-10 out-of-order Noble source | `regions.release region.read`-style reordered source | check-time `type-reject`; no component produced |
| EPROTO-CASE-11 unmodeled response | test-only host fixture returning `err("timeout")` and a wrong-length `ok` | `outside-model`; no protocol claim reported |

### Proof plan

These are theorem obligations; none is discharged here.

- **T1.** A Lean model `RegionProtocol` covering the transition table, the trace semantics and A1–A6 as explicit hypotheses.
- **T2.** The selected subject satisfies the two normal-return postconditions and P1–P6 under the model.
- **T3.** Composition (Seq) is sound and no set-union rule is derivable.
- **T4.** The `ownership::Table` transitions used here refine the model. This reuses `M5Resources.lean` `transition_refines` and `normal_completion_owner_once` for the table only.
- **T5.** The `bounded_region.rs` host functions refine the model's host transitions. This is open; Aeneas coverage of `wasmtime`/`anyhow`-using CLI code is [INFERENCE] not established and may need the decision logic factored into an extractable crate.
- **T6.** Noble component compilation of the subject preserves the request order (PO-17/PO-18 scope, coordinated in #10).

### Monitor

A runtime monitor replays a recorded ordered trace against the transition table and reports violations such as `omitted-release`, `double-release`, `post-release-use`, `unsolicited-response` and `outside-model`. A finite monitored trace is **evidence about that execution only**, never a universal protocol proof. A proof over the host model does not establish real Wasmtime or host behavior.

## Recommendation

1. **First optional profile:** `Effect-Protocol-Draft` with `RegionProtocol-v1` only. It allows normal-return and prefix-safety claims with explicit assumptions A1–A6, and no total-correctness or liveness claims. Ordinary execution of the adapter is unchanged and remains available without any protocol claim.
2. **Checker and model extensions,** in order:
   - (a) host report extensions fixing F1/F2/F6–F10:
     - separate `guest_releases`, `destructor_releases` and `host_retirements`, with abnormal cleanup through `retire_context` and its reason;
     - classify `trap`, `quota-exhausted`, `outside-model` and `scope-exhausted` (per H-OUTCOME-01);
     - add an ordered `events` array;
     - report `Lost` owners;
     - add a component recipe byte-match before instantiation (coordinated with #10);
   - (b) a contract IR extension adding trace/ownership clauses, which VC-INPUT-01 already requires the IR to distinguish, limited to the `RegionProtocol-v1` vocabulary;
   - (c) the Lean model T1–T3;
   - (d) the trace monitor;
   - (e) a host interruption control for F3 before any cancellation claim.
3. **Remaining correspondence obligations,** none closed by this design:
   - source → component bytes (Noble component compiler; #10);
   - component → Wasmtime execution (A4);
   - host Rust → model (T5);
   - host report authenticity and process-crash cleanup (A5);
   - real-world region selection (A1).

## Coordination

- **#3:** owner, borrow and lifetime semantics, saved `Program`s holding resources, and returning rejected owners belong there. This design only consumes K-RES-01–04 as they stand.
- **#5:** allocation policy for the host copy and the guest list is out of scope.
- **#8:** in-bounds applicability is not guardable (no ordered comparison), so both branches are specified.
- **#10:** T6 and artifact binding of the compiled component.
- **#6:** monitor verdict codes should be surfaced as diagnostics.

## Risks / Trade-offs

- One world and one subject is narrow. Generalizing requires a per-adapter transition table and response assumptions, not inference from WIT signatures.
- Fixing F1/F2 changes the `noble-bounded-region/v1` report schema. Existing S-CASE-02 receipts must stay byte-identical, and a new schema version is needed.
- The WIT `string` error type means A3 stays an assumption until the world adopts an enum, which would be a new world version.
- Cancellation remains unsupported until a real interruption control exists. The plan does not simulate it.

## Acceptance mapping

| Issue #9 acceptance checkbox | Section |
|---|---|
| One chosen existing adapter and bounded protocol with a transition table, result schema and explicit assumptions | "Adapter audit"; "Decision: One synchronous bounded protocol"; "Protocol state machine" (transition table, result schema, A1–A6, resource identity) |
| Precise separation of effect permission, behavioral proof, invocation applicability, ownership and cleanup authority | "Decision: Separate the five authorities" |
| Worked safe composition plus rejected out-of-order use, omitted/double release, wrong resource identity and unmodeled response | "Composition" (worked composition and rejected-uses table) |
| Test/proof plan covering success, ordinary error, host refusal, trap, cancellation and quota exhaustion with preserved observable trace prefixes | "Test and proof plan" (EPROTO-CASE-01…11, T1–T6, monitor) |
| Recommendation for a first optional profile, checker/model extensions and remaining source/runtime/host correspondence obligations | "Recommendation" |
