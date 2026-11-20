## Phase 1: Design selection (no runtime acceptance)

- [x] [serial] Select the separate typed generic dispatch, ephemeral input/authority boundary, root snapshot, admission/CAS, evidence and reflection contracts as active native requirement deltas. r[K-LSLOT-01] r[K-LSLOT-02] r[H-LSLOT-01] r[S-LSLOT-01] r[VC-LSLOT-01] r[DX-LSLOT-01]
- [x] [serial] Preserve ordinary captures and the existing `Live-Wasm-Draft` no-slot exclusion; design nine explicitly unexecuted LSLOT scenarios and state/status boundaries. r[B-LSLOT-01] r[EV-LSLOT-01]
- [x] [serial] Run proposal/design/tasks gates, native validation and document tests without generated-view sync, evidence promotion or report/ledger refresh. r[EV-LSLOT-01]

## Phase 2: Future implementation and independent acceptance (open)

> OWNER DECISION 2026-11-20 (decision 4, verbatim): "Verification trees that enumerate change
> documents as live inputs (`verification/live-update-corpus`, the intrinsic-proofs gates, the
> safety replay trees) are historical-only: they replay at the source state their receipts record.
> Archiving a change does not regenerate them."

> OWNER DECISION 2026-11-20 (decision 20, verbatim): "Decision 4 extends to frozen verification
> scripts that assert earlier case states: `verification/live-slot-current-source/gate.mjs` and
> `verification/numeric-graph-v1/postpromotion.mjs` are historical-only (replay only at their
> recorded source state)." That tree is marked historical at
> `verification/live-slot-current-source/HISTORICAL.md`; it is not edited or re-run here.

> OWNER DECISION 2026-11-20 (decision 1): a fresh-context reviewer that did not implement the work
> performs any required independent review; its record is retained in the repository. The review of
> the first promotion round is `verification/lslot-cases/review-2026-11-20.md` (round 1, verdict
> REVISE); the corrected gate, receipt and promotions await round 2. Slice agents do not review
> their own work.

> OWNER DECISION 2026-11-20 (decision 8): "LSLOT promotion: promote LSLOT-01/02/03/04/06/07 as
> their receipts land; only LSLOT-05 and LSLOT-08 wait for template identity." Promotion is
> conditional on an executed, source-bound receipt for exactly that case, and
> `specs/conformance/live-reference-cases.json` now records which cases LSLOT-04, LSLOT-07 and
> LSLOT-05 additionally wait for: evidence-carrying publication (the template-identity capability).

> OWNER DECISION 2026-11-20 (decision 19, verbatim): "LSLOT-03 and LSLOT-06 case designs are
> amended: drop or replace each impossible variant with the reviewer's recorded reason (LSLOT-03:
> unselectable `fs.write`; the input-only variant no checked candidate can have; the impossible
> under-declare variant. LSLOT-06: case-level `guest_requests: 0` contradicting its accepted
> valid-borrow variant; serialize/forge variants with no operations by design), record them as
> owner-approved amendments, and run both through the reviewed gate." The amendments, with each
> dropped variant and its reviewer reason, are recorded in the `amendments` block of
> `specs/conformance/live-reference-cases.json` dated 2026-11-20.

- [ ] [serial] Implement independently checked `Live-Slot-Design` admission, root-issued typed ref inputs/pass-through, root pinning and nested dispatch with current host authorization. r[K-LSLOT-02] r[S-LSLOT-01] r[B-LSLOT-01]
  Progress 2026-11-20 (round 2): executed, not yet complete. `verification/lslot-cases/gate.mjs`
  builds the selected CLI from a committed clean tree, requires one projection entry for every
  canonical leaf path and every variant, and refuses a case for each machine-expressible field
  whose canonical attempt did not run. Receipt: `verification/lslot-cases/acceptance.json`
  (sha256 `7925b655b1d624c2e13a64cc6743e47c9ff257ad3b43bc83879a1a1fd313f2a0`, source revision `sha256:5edd74de455377ef0253fb39f7749b78e39088942776d2974cfad50a5ba9a81a`, binary sha256
  `96fe860b1116313df37582af57d650d9feb3733c8a998bfff4b34d4613dbdce4`); raw lane evidence and command output are retained under
  `verification/lslot-cases/raw/`.
  - **LSLOT-02 (promoted):** both independent epoch-1 registry boots; the A-to-B-to-B root
    performs two B dispatches that both select B-v1 while B-v2 publishes as epoch 2 (result 22);
    the A-to-B-to-C root keeps A-v1/B-v1/C-v1 with host-issued typed refs passed through ordered
    callee interfaces (result 21); every dispatch rechecks current policy for its slot and root
    epoch; the next roots select B-v2/C-v2.
  - **LSLOT-01 (promoted):** the saved `[ n ]` owner keeps the pre-publication target (21, empty
    dispatch trace) while the separately installed generic `slot.invoke` caller with a host-issued
    `LiveRef<I64,I64,pure>` selects the published v2 (22 at epoch 2), after the publication in
    call order; its reflected recipe is unchanged and embeds no slot.
  - **LSLOT-09 (promoted, r[B-LSLOT-01]):** the same checked caller bytes are refused at resolve by
    Core-Bootstrap `compile` and an ordinary Core session (exit 4) and at admission by
    `noble live repl` (exit 2, generation 0), each with zero guest requests, while ordinary `[ n ]`
    capture and guarded `:reload` keep the old capture (21). The LIVE-01..10 contract fields are
    compared with the change's base commit `8075d3c0`; later state/evidence transitions are
    recorded separately.
  - **LSLOT-03 and LSLOT-06** were case-design defects, not runtime gaps; decision 19 amended both
    (see the conformance `amendments` block) and the gate now runs the remaining variants.
- [ ] [serial] Implement exact immutable target admission and bounded global-epoch/slot-incarnation/generation CAS publication, rollback-as-publication and last-pin retirement/reclamation across maps, frames and saved Programs. r[H-LSLOT-01] r[S-LSLOT-01]
  Progress 2026-11-20 (round 2): executed, not yet complete. **LSLOT-04** still refuses
  `successful_rollback_requires_fresh_admission_authorization_and_evidence`: re-admission and the
  rollback grant are observed, but no publication or rollback request carries evidence, and a
  proof-required slot refuses every publication. **LSLOT-07**'s `publication-without-grant` and the
  `or evidence` half refuse for the same reason. Both wait for evidence-carrying publication, the
  capability LSLOT-05 waits for under template identity; decision 8's premise therefore does not
  hold for 04 and 07. **LSLOT-08** (last-pin retirement/reclamation) is unexecuted for the same
  template-identity reason. Observed and matching on LSLOT-04: every committed update increments
  the epoch by one, same-slot and unrelated stale writers `stale-reject`, a deleted-and-recreated
  slot uses incarnation 8 without ABA, and rollback publishes a new epoch/generation without
  replaying prior roots.
- [ ] [serial] Execute every LSLOT case and adversarial variants including cross-slot pinning, retry after last owner release and hostile frozen-replay bindings against actual runtime; record separately bound trace, evidence, proof and trust results without inheriting LIVE acceptance. r[EV-LSLOT-01] r[VC-LSLOT-01] r[DX-LSLOT-01]
  Progress 2026-11-20 (round 2): seven of nine cases are projected; LSLOT-05 and LSLOT-08 wait on
  runtime template identity (P-ID-03/P-ID-04), and LSLOT-03/04/06/07 are refused on the fields named
  above (03/06 after their decision-19 amendments, 04/07 on evidence-carrying publication). No case
  inherits LIVE acceptance, and no promoted case claims more than `proof: open` / `trust: explicit`.
  This task stays open.
