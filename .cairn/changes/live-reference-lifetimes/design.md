## Context

Issue #1 asks for a design outcome: chosen semantics and conformance scenarios, before any implementation. All citations below were re-read at `44329dc`. **[INFERENCE]** marks anything not observed in source. Each claim carries one of three labels: **Design argument**, **Planned test** or **Proof obligation**. Nothing is executed or proved here.

### Already selected (canonical, unchanged by this change)

`Live-Slot-Design` is a separate opt-in profile. Its selected contract is:

- `K-LSLOT-01`: ordinary `[ n ]` and saved Programs keep their exact dependencies forever. Only a checked generic caller may contain the typed `slot.invoke`, whose identity embeds no slot, target or credential and whose reflection stays inert.
- `K-LSLOT-02`: `slot.invoke` borrows a host-issued, root-scoped `LiveRef<I,O,E>` and has effect `E ∪ {live.dispatch}`. A ref is never `Data`/`Capture`, never aggregated, returned, serialized or forged, and is never authority. Nested refs are supplied by the root host and passed through explicit ordered interfaces.
- `H-LSLOT-01`: candidates are admitted without executing them. Publication is a CAS on global epoch plus slot ID, incarnation and generation (no ABA). Rollback is a new publication. Obsolete maps are retained while pinned, saved exact Programs keep their code, and capacity is checked against post-commit reachability.
- `S-LSLOT-01`: one immutable map and epoch are pinned per root for all nested dispatch. Authority is checked per dispatch and per actual effect. Owners are accounted separately, with no permanent quota leak.
- `VC-LSLOT-01`: evidence applies only to the exact selected target. `DX-LSLOT-01`: inert reflection, exact-version traces, frozen replay refusing any mismatch. `EV-LSLOT-01`: cases stay absent/not-run. `B-LSLOT-01`: unsupported in Core and guarded `Live-Wasm-Draft`.
- LSLOT-01..09 in `specs/conformance/live-reference-cases.json` are registered designs, still absent/not-run/open/unassessed.

### Experimental runtime facts at 44329dc (source reading; README says "acceptance not established")

| Path | Source | Relevance |
|---|---|---|
| Root frame and per-site frames pin the selected target by count and release it on restore | `crates/noble-wasm/src/source/emit/live.rs:18-62, 84-106, 150-167`; `runtime/live-borrows.wat:42-65, 117-147` | in-flight pinning |
| "A saved generic caller is reusable with a newly issued root borrow set and newly pinned registry map" | `live.rs:150-151` | saved generic callers |
| Abnormal root: owners created by the failed root are released, `registry.abortRoot`, session poisoned | `crates/noble-cli/src/core/runtime/slot-engine.mjs:1218-1234`; `slot-registry.mjs:882-889` | abnormal exit |
| Counter exhaustion refusals `generation-exhausted` and `incarnation-exhausted` at `U64_MAX` | `slot-registry.mjs:387-391`; compiler `retention_revision == u64::MAX` refusal at `crates/noble-wasm/src/source/mod.rs:877` | counters |
| Post-commit retirement failure raises `CommittedRetirementError` and poisons | `slot-registry.mjs:71-78, 294-307` | atomic commit point |
| A saved returned graph counts every reachable admitted version once; the host walk follows `a/b/c` edges and Program entry spans only | `slot-engine.mjs:766-791, 1030-1046`; `slot-registry.mjs:895-922` | escaped results, package retention |
| Module sweep releases module roots once version and replay owners are gone; table span cleared only at `code_owners == 0` | `slot-engine.mjs:663-679, 703-733`; `live.rs:187-201` | retirement order (links to `OWN-CASE-06`) |
| Retained-version quota is a host-ledger count (1..16,384) checked on post-commit reachability | `slot-registry.mjs:109-121, 340, 398` | host quota versus guest cells |

None of these behaviors is required by canonical text. That gap is the subject of this change. The README's diagnostic gate output must not be copied into canonical case evidence.

## Decisions

### Decision: Answers to the issue's "Questions to settle"

**Choice (each answer cites the governing requirement; the LREF items are new):**

1. **Transitive snapshot or per-call selection; allowed mixed generations.** One root pins one immutable map for every nested and transitive dispatch (`S-LSLOT-01`). A nested call never selects independently. Allowed mixing:
   - (a) across distinct roots, including held or reentrant host-started roots, each with its own pin (LSLOT-02);
   - (b) within one root, between slot-selected versions from the pinned map and exact saved Programs passed as ordinary data, which run their own captured code (r[LREF-ESCAPE-01]).

   Forbidden: two different versions of the same slot selected within one root, and use of a ref issued to one root inside another root (`K-LSLOT-02`).
2. **Generation checks, atomic publication and rollback.** CAS on global epoch plus slot ID, incarnation and generation, with one persistent-map swap as the sole commit point (`H-LSLOT-01`). Rollback is a freshly admitted and authorized publication. New in this change:
   - All counters are non-wrapping, and exhaustion refuses explicitly (r[LREF-CTR-01]).
   - Old-version retirement after a committed swap cannot un-commit. A retirement failure poisons the session (r[LREF-PKG-01], matching `slot-registry.mjs:294-307`).
3. **Reflection, identity, portability and replay.** The generic caller's identity and reflection are inert and slot-free (`K-LSLOT-01`, `DX-LSLOT-01`). A `LiveRef` is not portable. An exported generic caller is ordinary checked code that needs host-issued refs at its destination. Replay needs a frozen, admitted map and exact bindings (`DX-LSLOT-01`). New in this change: a Program escaping a selected target is an exact, non-live value with its own lineage identity. It is reflected and replayed as ordinary code and does not follow later publications (r[LREF-ESCAPE-01]).
4. **Trace identity and safe retention/reclamation.** Traces record the root epoch, slot incarnation and generation, the exact selected identity and nested selections (`DX-LSLOT-01`, `EV-LSLOT-01`). New in this change: the retention unit is the selected version's complete executable package, and retirement follows a fixed order (r[LREF-PKG-01]). Abnormal root exit releases pins exactly once (r[LREF-EXIT-01]).

State and live-resource migration stay out of scope, as the issue requires.

### Decision: Slot-version retention unit and retirement order

**Choice:** For every admitted version `v`, the retained unit is `Pkg(v)` as defined by `OWN-PKG-01` (#3): code and table entries, exact named dependencies read through module globals or tables (transitively), captures, recipes, static data and metadata. Registry maps, frames and saved-owner tokens are *owners* of `Pkg(v)`. A saved Program that escaped from `v`'s execution is a further owner of every member it reaches. Retirement proceeds in this order:

1. The last root or frame pin of an obsolete map releases it.
2. `v` loses its last map, frame or saved owner.
3. Each package member's own owner count (`OWN-RECLAIM-01` edge kinds) drops to zero. Module roots are released only when no surviving owner reaches any member through code, global or table edges.
4. `code_owners == 0` is confirmed for the code span.
5. The table span is cleared and the retained-version charge reclaimed.

Steps 2–5 happen after the commit point. A failure there poisons rather than reverting the publication. Capacity admission keeps using post-commit reachability, now counting packages rather than single cells. r[LREF-PKG-01]

**Rationale (Design argument):** `H-LSLOT-01`'s "code version" is defined by this rule rather than left to the implementation. The experimental sweep releases module roots once version and replay owners are gone (`slot-engine.mjs:663-670`), while saved-owner attribution follows only `a/b/c` edges and Program entry spans (`:766-791`). Whether a surviving escaped result can lose a global-read dependency is exactly the #3 `OWN-CASE-06` question. LREF-CASE-03 is its slot-replacement form. This is **not** a confirmed defect.

### Decision: Abnormal root exit

**Choice:** These exits all end the root:

- trap or quota exhaustion;
- host abort;
- cancellation or worker loss;
- poisoned transport;
- failure while saving outputs.

On any of them, the host MUST release the root's map pin and every active frame pin exactly once from its own ledger. It must not resume the guest or depend on guest unwinding (`K-RES-04` analog). The root's refs expire. No saved-owner token is issued for that root's outputs, and owners provisionally created inside the root are released. Retirement then follows the order of the previous decision. The host MAY poison the session, as the experimental runtime does. If it continues, post-exit accounting MUST be exact: no permanent quota leak, and no surviving held root or saved Program invalidated. Performed effects are never rolled back. r[LREF-EXIT-01]

**Rationale:** `S-LSLOT-01` only says pins end at root completion. Without an abnormal-exit rule, a trapped root could retain a pin forever and block quota-1 replacement permanently, or release a pin twice and retire a version still pinned elsewhere.

### Decision: Non-wrapping counters

**Choice:** These counters MUST NOT wrap:

- the global registry epoch;
- per-slot generation and incarnation;
- the code-retention revision;
- guest handle epochs.

At exhaustion, the affected publication, bootstrap, recreation or code retirement refuses explicitly with a distinct diagnostic. The registry is left unchanged, existing pins and saved Programs stay valid, and no prior value is reissued. r[LREF-CTR-01]

**Rationale:** A wrapped epoch would let a stale writer's expected epoch match again. That reintroduces the ABA problem `H-LSLOT-01` excludes. The experimental registry already refuses at `U64_MAX` (`slot-registry.mjs:387-391`), but no canonical requirement says it must.

### Decision: Escaped results, saved generic callers and the #3 borrow shape

**Choice:** A Program value returned or saved from a root, produced by a slot-selected target, is an ordinary exact immutable Program:

- Its ProgramValueId/DefinitionId is its own recipe and capture lineage (`P-ID-03`), never the slot's or the generic caller's.
- It reaches no `LiveRef` (`K-LSLOT-02`).
- It owns its package (`LREF-PKG-01`) and never follows a later publication.
- Evidence for it binds that exact lineage (`VC-LSLOT-01`).

A *saved generic caller* (a checked Program containing `slot.invoke`) is ordinary reusable code. Each invocation needs a new root, a fresh pin and fresh host-issued refs, and a ref retained from an earlier root is refused. r[LREF-ESCAPE-01]

**Design argument (relation to #3):** `LiveRef` is the #3 bounded borrow shape (`OWN-BORROW-01`) with the *root invocation* as its scope and the host, not a guest combinator, as the scope creator. `K-LSLOT-02` forbids refs in root outputs, which is the occurs check at the outermost scope. Refs are inputs, never package edges, and maps are roots, never package members. Live indirection therefore adds no package dependency cycles. That preserves the logical-order acyclicity argument of `OWN-RECLAIM-01` (#3 condition C4). If a Program could capture a slot ID, a version could reach itself through its slot, and the argument would fail.

**Relation to #5:** the retained-version quota is a host-ledger logical budget (`slot-registry.mjs:109-121`). Code spans use the compiler's table-span allocator (A8 in `allocation-strategy-contract`). Neither is a guest-cell allocation, and reports keep them separate (`ALLOC-OBS-01`).

### Decision: Conformance scenarios

**Choice (Planned tests):** The issue's design outcome maps onto the registered LSLOT designs, which are not re-registered here:

| Issue outcome | Registered design |
|---|---|
| stable old captures | LSLOT-01, LSLOT-09 |
| successful compatible replacement | LSLOT-01, LSLOT-03 (`same-exact-interface`) |
| rejection of incompatible replacements | LSLOT-03 |
| rejection of unauthorized replacements | LSLOT-07 |
| rejection of stale replacements | LSLOT-04 |
| in-flight version pinning | LSLOT-02 |
| refusal of evidence for the wrong version | LSLOT-05 |

New LREF designs, recorded only here. Each is `implementation: absent`, `execution: not-run`, `proof: open`, `trust: unassessed`, `evidence: []`:

| ID | Input | Expected observation |
|---|---|---|
| LREF-CASE-01 abnormal exit releases pins | quota-1 slot B at epoch 1; root R pins epoch 1 and enters A→B frames; B traps (variants: quota, host abort, worker cancellation, save-time failure); no saved owner of B-v1 exists | R's map pin and both frame pins are released exactly once; no saved owner is issued for R. Continue-variant: freshly authorized B-v2 publishes at epoch 2 and B-v1 retires. Poison-variant: the ledger shows zero pins for R before poisoning. No effect is rolled back. |
| LREF-CASE-02 counter exhaustion | fixture registry at global epoch `2^64−1`; separately slot generation `2^64−1`; separately recreation with incarnation `2^64−1`; separately code-retention revision `2^64−1` | each operation refuses with a distinct exhaustion diagnostic; the registry is unchanged; held roots and saved Programs still run; no counter returns to a prior value |
| LREF-CASE-03 package retention across replacement | slot S version v1 from module M1, whose body returns `[ h ]`, where `def h [ 1 + ]` is read through an M1 global; root R saves the returned Program O; publish v2; R ends; module sweep runs | `20 O run` yields `21`; M1's span is not cleared while O lives; after O is released, the span retires and the charge is reclaimed. A stale-handle trap fails the case. |
| LREF-CASE-04 escaped result never follows publication | v1 returns a Program capturing `1` with `+`, saved as O; publish v2 returning one capturing `2` | O on `20` gives `21` after publication; a new root's fresh result gives `22`; O's identity is its v1 lineage, not the slot; reflection of O shows its own recipe; O contains no `LiveRef` |
| LREF-CASE-05 saved generic caller across roots | saved generic caller G; root R1 with a fresh ref (epoch 1); publish v2; root R2 with a fresh ref; R2 variant retaining R1's ref | R1 selects v1, R2 selects v2; the retained ref is refused before dispatch; G's identity is unchanged |
| LREF-CASE-06 held roots and allowed mixing | R1 held at a protected checkpoint at epoch 1 with an input saved exact Program P built from S-v0 lineage; publish S-v2; R2 completes; R1 resumes and dispatches S again and runs P | R1 selects S-v1 for both dispatches and P runs its v0 code; R2 selects S-v2; R1's trace records epoch 1 throughout; no root selects two versions of S |

## Risks / Trade-offs

- Counting packages instead of cells makes retained-version quotas coarser: one escaped result can hold a whole module. Finer retention would require per-definition code spans.
- The poison-or-continue choice on abnormal exit stays a host policy. Continuing requires exact ledger accounting that is not yet verified.
- LREF-CASE-03 may expose the #3 correspondence gap. Resolving it may extend module lifetime to escaped results.

### Remaining decisions and evidence before implementation claims

1. Execute LSLOT-01..09 and LREF-CASE-01..06 against an implemented profile, with source-bound evidence bound per `EV-LSLOT-01`/`LREF-EV-01`.
2. Resolve `OWN-CASE-06` and choose the package-retention mechanism.
3. Decide the poison-or-continue policy after abnormal exit for any accepted profile.
4. Register LREF-CASE designs in conformance only through a separate reviewed change.

## Acceptance mapping

| Issue #1 item | Section |
|---|---|
| Q: transitive snapshot vs independent nested selection; allowed mixed generations | Decision: Answers to the issue's "Questions to settle" (1); LREF-CASE-06 |
| Q: generation checks against stale/racing updates; atomic publication and rollback rules | Answers (2); Decision: Non-wrapping counters; r[LREF-CTR-01]; LSLOT-04; LREF-CASE-02 |
| Q: reflection, identity, portability and replay for explicit live dispatch | Answers (3); Decision: Escaped results, saved generic callers and the #3 borrow shape; r[LREF-ESCAPE-01]; LREF-CASE-04/05 |
| Q: trace identity/generation; safe retention and reclamation of old versions | Answers (4); Decision: Slot-version retention unit and retirement order; Decision: Abnormal root exit; r[LREF-PKG-01] r[LREF-EXIT-01]; LREF-CASE-01/03 |
| Outcome: stable old captures, compatible replacement, rejection of incompatible/unauthorized/stale replacements, in-flight pinning, wrong-version evidence refusal | Decision: Conformance scenarios (LSLOT mapping table) |
| Relation to #3 package lifetime and in-flight pinning | Decision: Slot-version retention unit and retirement order; Decision: Escaped results… (borrow shape); r[LREF-EV-01] |
