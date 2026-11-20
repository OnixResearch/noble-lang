## Phase 1: Design selection (no runtime acceptance)

- [x] [serial] Select the separate typed generic dispatch, ephemeral input/authority boundary, root snapshot, admission/CAS, evidence and reflection contracts as active native requirement deltas. r[K-LSLOT-01] r[K-LSLOT-02] r[H-LSLOT-01] r[S-LSLOT-01] r[VC-LSLOT-01] r[DX-LSLOT-01]
- [x] [serial] Preserve ordinary captures and the existing `Live-Wasm-Draft` no-slot exclusion; design nine explicitly unexecuted LSLOT scenarios and state/status boundaries. r[B-LSLOT-01] r[EV-LSLOT-01]
- [x] [serial] Run proposal/design/tasks gates, native validation and document tests without generated-view sync, evidence promotion or report/ledger refresh. r[EV-LSLOT-01]

## Phase 2: Future implementation and independent acceptance (open)

> OWNER DECISION 2026-11-20: "LSLOT promotion: promote LSLOT-01/02/03/04/06/07 as their receipts
> land; only LSLOT-05 and LSLOT-08 wait for template identity." Promotion is still conditional on
> an executed, source-bound receipt for exactly that case.

> OWNER DECISION 2026-11-20: verification trees that enumerate change documents as live inputs are
> historical-only; archiving a change does not regenerate them. `verification/live-slot-current-source/gate.mjs`
> (which asserts all nine cases absent) and `verification/live-update-corpus` (which asserts they stay
> unpromoted) are therefore **not** edited or re-run; their receipts stay as records of the
> pre-promotion source state.

- [ ] [serial] Implement independently checked `Live-Slot-Design` admission, root-issued typed ref inputs/pass-through, root pinning and nested dispatch with current host authorization. r[K-LSLOT-02] r[S-LSLOT-01] r[B-LSLOT-01]
  Progress 2026-11-20: executed, not yet complete. `verification/lslot-cases/gate.mjs` builds the
  selected CLI from a committed clean tree and projects every machine-expressible canonical
  `input`/`expected`/variant field onto the existing lanes. It promoted **LSLOT-01**, **LSLOT-02**
  and **LSLOT-09** (one evidence record each, `proof: open`) from
  `verification/lslot-cases/acceptance.json` (sha256
  `7e9f2f457f031763fe75986f129411711c9cd36c83756ab86dcb805eaecee751`, binary sha256
  `96fe860b1116313df37582af57d650d9feb3733c8a998bfff4b34d4613dbdce4`, source revision
  `sha256:14f1fe00ab86a9c39dd9c1683de683ecf4c50e44c9c136d6b2a17cabd0037c66`).
  - **Admission, pinning and nested dispatch (LSLOT-02).** Both independent epoch-1 registry boots
    are primitive atomic genesis. The A→B→B root performs both B dispatches against B-v1 while
    B-v2 publishes as epoch 2 (result 22), the A→B→C root keeps A-v1/B-v1/C-v1 with host-issued
    refs passed through ordered callee interfaces (result 21), every dispatch rechecks current
    policy for its slot and root epoch, and the next roots select B-v2/C-v2 (result 22). All 21
    machine-expressible fields match; four prose interleaving/body entries are recorded as
    `declared`.
  - **Generic caller and immutable capture (LSLOT-01).** The saved `[ n ]` owner keeps its
    pre-publication target (21, empty dispatch trace) while the separately installed generic
    `slot.invoke` caller with a host-issued `LiveRef<I64,I64,pure>` selects the published v2 (22 at
    epoch 2); its reflected recipe is unchanged across publication and embeds no slot. 16/16
    fields match.
  - **Legacy exclusion (LSLOT-09, r[B-LSLOT-01]).** The same checked caller bytes are refused at
    resolve by Core-Bootstrap `compile` and by an ordinary Core session (exit 4) and at admission
    by `noble live repl` (exit 2, generation 0), each with zero guest requests and zero protected
    operations, while ordinary `[ n ]` capture and guarded `:reload` keep the old capture (21).
  - **Still unexecuted, so not ticked.** LSLOT-03's `effect-ceiling-widened`,
    `unknown-host-contract` and `generic-caller-underdeclares-dispatch` variants cannot be
    constructed in the pinned live-slot profile (no `fs.write`/`unknown.effect` selection, no
    caller-claimed-effect override), and `same-layout-different-resource-schema` rejects with input
    **and** output differing; the gate records all four as unobserved. LSLOT-06's `serialize` and
    `forge` variants have no selected source word, and `aggregate-capture`, `export-or-return` and
    `retain` did not reach their canonical attempt; the case-level `guest_requests: 0` also cannot
    be reconciled with the accepted `valid-borrow` variant, which performs one guest request.
- [ ] [serial] Implement exact immutable target admission and bounded global-epoch/slot-incarnation/generation CAS publication, rollback-as-publication and last-pin retirement/reclamation across maps, frames and saved Programs. r[H-LSLOT-01] r[S-LSLOT-01]
  Progress 2026-11-20: executed, not yet complete. The CAS lane runs and is projected, but
  **LSLOT-04** is refused on `successful_rollback_requires_fresh_admission_authorization_and_evidence`:
  re-admission and the rollback grant are observed (`policy-denied` without it, epoch 20 /
  generation 5 with it), but no publication or rollback request carries evidence, and a
  proof-required slot refuses every publication (`slot-engine.mjs` `publish` has no evidence
  operand), so that conjunct cannot be observed. **LSLOT-03** (exact immutable target admission)
  and **LSLOT-08** (last-pin retirement/reclamation) are likewise unpromoted. Other observed
  fields on LSLOT-04 do match: every committed update increments the epoch by one, same-slot and
  unrelated stale writers `stale-reject`, a deleted-and-recreated slot uses incarnation 8 without
  ABA, and rollback publishes a new epoch/generation without replaying prior roots.
- [ ] [serial] Execute every LSLOT case and adversarial variants including cross-slot pinning, retry after last owner release and hostile frozen-replay bindings against actual runtime; record separately bound trace, evidence, proof and trust results without inheriting LIVE acceptance. r[EV-LSLOT-01] r[VC-LSLOT-01] r[DX-LSLOT-01]
  Progress 2026-11-20: seven of nine cases are projected by the gate; LSLOT-05 and LSLOT-08 are not
  executed because their positive outcomes wait on runtime template identity (P-ID-03/P-ID-04), and
  LSLOT-03/04/06/07 remain refused on the fields named above. No case inherits LIVE acceptance, and
  no promoted case claims more than `proof: open` / `trust: explicit`. This task stays open.
