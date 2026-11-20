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
> conditional on an executed, source-bound receipt for exactly that case. `specs/conformance/live-reference-cases.json`
> records only each case's `state`/`evidence`; it does not record what an unpromoted case waits
> for. The waits are recorded here: LSLOT-04's rollback-evidence conjunct, LSLOT-07's
> `publication-without-grant` and LSLOT-05 all need evidence-carrying publication, the capability
> tied to template identity, so decision 8's premise does not hold for 04 and 07 yet.

> OWNER DECISION 2026-11-20 (decision 21a, verbatim): "Decision 21's script `["20 twice swap run"]`
> was based on a wrong derivation: from the retained old Program P it yields [25] (`run` consumes 24
> and the old capture adds 1), not the case's unchanged expected stack [24, 21] (the integrator's
> question also misquoted that stack as [21, 22], LIVE-01's). The integrator, under the owner's
> delegation, replaces decision 21's script with `["20 swap run 20 twice swap"]`: [P] -> 20 ->
> [P,20] -> swap -> [20,P] -> run (old capture, +1) -> [21] -> 20 -> [21,20] -> twice (rebuilt
> dependent, +4) -> [21,24] -> swap -> [24,21]. Nothing else in the case changes; LIVE-02 is
> promoted only if a fresh receipt observes exactly [24, 21] and every other expected field." The
> LSLOT-09 projection therefore accepts exactly one LIVE contract change against the change's base
> commit `8075d3c0`: LIVE-02 `input.after_reload_ack` becoming `["20 swap run 20 twice swap"]`.

> OWNER DECISION 2026-11-20 (decision 19, verbatim): "LSLOT-03 and LSLOT-06 case designs are
> amended: drop or replace each impossible variant with the reviewer's recorded reason (LSLOT-03:
> unselectable `fs.write`; the input-only variant no checked candidate can have; the impossible
> under-declare variant. LSLOT-06: case-level `guest_requests: 0` contradicting its accepted
> valid-borrow variant; serialize/forge variants with no operations by design), record them as
> owner-approved amendments, and run both through the reviewed gate." The amendments, with each
> dropped variant and its reviewer reason, are recorded in the `amendments` block of
> `specs/conformance/live-reference-cases.json` dated 2026-11-20.

- [ ] [serial] Implement independently checked `Live-Slot-Design` admission, root-issued typed ref inputs/pass-through, root pinning and nested dispatch with current host authorization. r[K-LSLOT-02] r[S-LSLOT-01] r[B-LSLOT-01]
  Progress 2026-11-20 (round 3): executed, not yet complete. `verification/lslot-cases/gate.mjs`
  builds the selected CLI from a committed clean tree, requires one projection entry for every
  canonical leaf path and every variant, and refuses a case for each machine-expressible field
  whose canonical attempt did not run. Receipt: `verification/lslot-cases/acceptance.json`
  (sha256 `b62cf62b80c148add5e6126f06e854cad2ad44f0491573dc3d436cb96edf3cb7`, source commit
  `ae972fd0cce81a4d82c0dc03b1e5fa1d0bbd7fac`, source revision
  `sha256:9aa0b92c8314c6ba70f4d99e80cc0d6eb6780481677bb85366e4d3714a48fd2e`, binary sha256
  `422cfa959b53987480158ef4ed995f10e79788fff47ac70afe1d7e5cb05909bb`); raw lane evidence and
  command output are retained under `verification/lslot-cases/raw/r3f-151856/`. The receipt binds
  the runtime after merging `cairn/wave1`'s template identity (experimental local identities v4,
  `template_sha256` and template classes): no LSLOT expectation names the identity version or a
  descriptor field, and the companion origin-control lane now asserts the v4 identity directly.
  Two earlier round-3 runs of the same design are superseded by this receipt; their receipts stay
  in git history and their raw lane output is retained under `verification/lslot-cases/raw/r3c-150242/`
  and `verification/lslot-cases/raw/r3e-151546/`.
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
    compared with the change's base commit `8075d3c083474884f9a7c72b5b1a483f73eec314`; at
    `cairn/wave1` the LIVE-01..10 contract fields are still identical to it (only state/evidence
    transitions landed), and the only accepted difference is owner decision 21a's LIVE-02
    `input.after_reload_ack`. Later state/evidence transitions are recorded separately.
  - **LSLOT-03 (promoted, round 3):** the exact ordered candidate publishes at epoch 5 while the
    reversed-input, both-sides `Account@2` schema, unknown-effect and under-declare candidates are
    each refused with the epoch and map unchanged; the constructible generic caller's reflected
    site carries `["test.emit","live.dispatch"]`. The only dropped variant is decision 19's
    unselectable `fs.write`.
  - **LSLOT-06 (promoted, round 3):** the direct quotation, aggregate capture, aggregate storage,
    export/return and retain attempts are each refused at their earliest static or invocation
    boundary with zero guest requests; only the checked generic borrow executes (one guest request,
    result 21). `serialize`/`forge` are decision 19's by-design drops; `ref_authority` is observed
    from a host-issued ref whose dispatch grant is revoked.
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
  Progress 2026-11-20 (round 3): seven of nine cases are projected; LSLOT-01/02/03/06/09 are
  promotable from one receipt, LSLOT-04 and LSLOT-07 stay refused on the fields named above
  (evidence-carrying publication), and LSLOT-05/LSLOT-08 wait on runtime template identity
  (P-ID-03/P-ID-04). No case inherits LIVE acceptance, and no promoted case claims more than
  `proof: open` / `trust: explicit`. This task stays open.
