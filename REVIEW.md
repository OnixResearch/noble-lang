# Noble handoff review — historical record

**Current direction: [canonical specs](specs/README.md) and [review resolutions](specs/REVIEW-RESOLUTION.md).**

The user confirmed that Noble starts from scratch. Repository-recovery recommendations below are superseded. The original findings remain here as review evidence.

The architecture has clear boundaries. This archive is a design handoff, not evidence of a working language.

## Observed state

| Item | Result |
|---|---|
| Extraction | 35 files extracted into `noble-project-handoff-2026-09-12/` without overwriting files |
| Integrity | ZIP checks passed for all entries. All 34 listed SHA-256 checks passed. |
| Review coverage | All 35 files examined. Shared preview text reviewed through complete diffs against the baseline. |
| Implementation | No compiler source, Lean source, build manifest, or executable test runner in this directory. It is not a Git worktree. |
| Assurance | 30 new scenario descriptions, all unexecuted. Nine safety obligations are open. The baseline lists 18 further open obligations. |

The original ZIP and extracted files remain unchanged. This review does not select new language semantics or mark draft requirements complete.

[File-by-file inventory](review/INVENTORY.md) · [Checks and evidence limits](review/EVIDENCE.md)

## Keep these architectural decisions

1. Keep checked `Program<S,T,e>` separate from inert `Syntax` and explicit preparation.
2. Keep runtime program construction separate from execution, with observable recipes independent of optimizer layout.
3. Keep effects separate from authority, with recursive resource ownership and host-side validation.
4. Keep WIT at component boundaries and Syndicate outside the language kernel.
5. Keep language proofs, Rust proofs, program proofs, backend correspondence, and host assumptions as separate claims.

These decisions fit a functional core with explicit host adapters. The main risk is incomplete contracts and evidence, not the need for another architecture.

## Resolve before the affected implementation work

### R1 — High: WIT version identity contradicts the core identity contract

**Finding:** `WD-14` says exact WIT/WASI versions affect `BuildKey`, not `DefinitionId`. However, `WI-WIT-03` includes the exact package version in `WitOpId`. Core identity includes resolved dependencies and host semantics.

A definition that changes from one versioned host operation to another therefore changes its resolved recipe. The blanket build-only rule cannot cover that case.

**Impact:** A literal implementation of `WD-14` can reuse a definition identity for a different resolved dependency. Evidence and cache applicability then become ambiguous.

**Recommended resolution:** Distinguish semantic dependencies from build-only adapters. A changed resolved operation or schema changes definition identity. If the resolved Noble contract stays unchanged, an adapter/toolchain change can affect only build identity.

**Acceptance:** Add separate vectors for source renaming, optimizer changes, adapter-only changes, and resolved WIT operation/version changes.

Sources, relative to the extracted directory:
- `reference/working-wit-wasi/WIT-WASI-DECISIONS.md:25`
- `reference/working-wit-wasi/WIT-WASI.md:54–60`
- `reference/baseline/SPEC-0001-baseline.md:649–668`

### R2 — High: one safety scenario accepts an effect-soundness failure

**Finding:** `S-CASE-05` permits `deny-or-static-reject` when an empty-effect program attempts a guest host operation. `S-EFFECT-01/02` forbid that request. `SO-03` explicitly includes denied requests.

**Counterexample:** An incorrect checker accepts an empty-effect body that requests `fs.read`. The host denies it. The scenario can pass while the finite-prefix effect theorem is false.

**Recommended resolution:** Separate static effect rejection, forged-metadata admission rejection, and runtime authority denial. Authority denial belongs to a program whose effect bound already includes the request.

**Acceptance:** The static/admission cases record zero candidate-body host requests. The authority case records a denied declared request and no protected operation.

Sources:
- `reference/working-safety/conformance/safety-cases.json:31–40`
- `reference/working-safety/SAFETY.md:64–68,120`
- `reference/baseline/VERIFICATION.md:77–79`

### R3 — Blocker: the handoff cannot establish the implementation baseline

**Finding:** The archive contains no implementation. Several referenced supporting documents are also absent. This is consistent with the handoff's warning, but prevents its P0 task from completing here.

Missing material includes the original core conformance fixtures, the verification decision addendum, the baseline obligation ledger, and the toolchain-lock template. The historical concurrency and choreography files are summaries, not their full packages.

There are also relocated or absent links, including `SPEC-AMENDMENT.md`, `DECISIONS.md`, `SOURCES.md`, and amendment patch paths. Some preview links assume a future combined directory. A missing reference is not evidence that the original document never existed.

**Recommended resolution:** Record the real repository URL/path and immutable revision. Recover the missing packages or mark each omission explicitly. Add a source map without rewriting the archived files.

**Acceptance:** A fresh checkout has documented build/test commands and a baseline report. Every normative reference resolves locally or names an explicit unavailable dependency.

Sources:
- `NEXT-WORK.md:3–13`
- `SOURCE-OF-TRUTH.md:68–81`
- `reference/baseline/VERIFICATION.md:16,140,202`
- `reference/baseline/VERIFICATION-TOOLCHAIN.md:132`
- `reference/baseline/VERIFICATION-AMENDMENT.md:13–17`

### R4 — High planning risk: the work order delays foundational decisions

**Finding:** P1 requires the full merge plan, which includes new concurrency and choreography specifications. P6 later schedules the missing concurrency specification. This creates an ordering conflict if P1 must finish before P2.

P2 also asks for a checked Wasm slice before P3 completes its acceptance-checker contract. P5 postpones the first Rust-to-Lean experiment until after the broad component slice. The verification toolchain document places that experiment in the first vertical slice.

**Impact:** Core progress waits for optional platform design, while checker and extraction risks surface after substantial implementation work.

**Recommended resolution:** Publish a canonical core draft with explicit profile dependencies and open statuses. Develop the checker contract, narrow extraction experiment, and dynamic Wasm prototype together. Complete concurrency and choreography later.

**Acceptance:** Every milestone has explicit dependencies and an observable exit criterion. No milestone requires a deliverable scheduled after it.

Sources:
- `NEXT-WORK.md:15–23,42–84`
- `MERGE-PLAN.md:54–75`
- `reference/baseline/VERIFICATION-TOOLCHAIN.md:144–148`

### R5 — Declared design blocker: borrow and async lifetime contracts remain incomplete

**Finding:** The WIT profile requires scoped, nonescaping borrows without defining their checker representation or their interaction with suspension. Async cleanup remains an obligation to specify, rather than a transition contract.

The core currently describes owned resources and immutable data. It does not provide a complete borrow-scope judgment. This is acknowledged open work, not evidence of an existing runtime defect.

**Recommended resolution:** Specify one owned resource and one bounded borrow adapter first. Define scope creation, permitted moves, scope exit, cancellation, and reentrant access. Explicitly reject unsupported lifetime patterns.

**Acceptance:** Cover aggregate/quotation escape, owner retirement during a pending call, normal error returns, cancellation, and stale callback results. Every live obligation has one accountable owner at each transition.

Sources:
- `reference/working-wit-wasi/WIT-WASI.md:68–76,100–112,173–178`
- `reference/baseline/VERIFICATION.md:85–93`
- `reference/baseline/VERIFICATION-TOOLCHAIN.md:80–86`

## Correct in the new documentation revision

### R6 — Medium: the stated preview ancestry is inaccurate

`MERGE-PLAN.md:9` describes different amendment paths. The complete diff shows that the WIT preview already retains the safety and verification additions. Its own amendment explicitly names the safety/verification preview as its base.

The warning against declaring a preview canonical remains valid. The claim that these two files require an independent safety remerge is misleading. A blind additive merge can duplicate requirements.

**Action:** Record the observed chain: baseline → verification/safety preview → WIT preview. Preserve a requirement-level comparison and review remaining gaps before publication.

Sources:
- `reference/working-wit-wasi/WIT-WASI-AMENDMENT.md:6,40`
- `reference/working-wit-wasi/SPEC-0001-wit-wasi-preview.md:116–124,841–847,904–905`

### R7 — Medium: the selected surface is not consistent in either preview

The current decisions select `#`, `run`, and `reflect`. Both previews retain the old `//` lexical rules and the list example `[ drop call ]`.

A lexer implemented from the preview will reject the handoff's comment syntax. The list example also requires an undocumented compatibility alias.

**Action:** Fix the new canonical revision, not the historical baseline. Define whether old spellings are aliases or migration errors. Add lexer tests for comments, strings, token boundaries, and operators.

The WIT preview also retains the broad statement that no async ABI is frozen. Replace it with the precise distinction: native component async is selected, while Noble lifetime and scheduling rules remain open.

Sources:
- `CURRENT-DECISIONS.md:10–14`
- `reference/working-wit-wasi/SPEC-0001-wit-wasi-preview.md:142–144,851,988`
- `reference/working-safety/SPEC-0001-safety-preview.md:141–143,938`

### R8 — Medium: scenario counts do not establish conformance coverage

The 15 safety scenarios have no requirement-reference fields or per-case execution statuses. Their package status is correctly `expected-not-run`. Several scenarios combine distinct operations or leave the failure contract unspecified.

The 15 WIT scenarios explicitly reference 19 of 40 profile requirements. All supplied references resolve. The remaining 21 requirements have no explicit scenario link in that file.

This is a traceability gap, not proof that every unlinked requirement lacks conceptual coverage. Some architectural rules need review evidence rather than a runtime case.

**Action:** Give each requirement an evidence route. Turn runtime scenarios into concrete source/WIT inputs, host setup, observations, and failure stages. Keep static rejection, authority denial, domain errors, and release-policy checks separate.

Important explicit gaps include `WI-ASYNC-04` live-value capture restrictions, `WI-RES-04` cleanup, and `WI-SAFE-01..03` boundary assurance.

Sources:
- `reference/working-safety/conformance/safety-cases.json:5–95`
- `reference/working-wit-wasi/conformance/wit-wasi-cases.json:5–144`
- `reference/working-wit-wasi/VALIDATION.json:5–14`

### R9 — Medium: the proposed status ledger mixes independent facts

`MERGE-PLAN.md:83–92` requires one status from a list that mixes implementation, execution, proof, and trust. It has no explicit failed-test status.

A requirement can be implemented, have a failed test, and retain an open proof obligation simultaneously. `implemented + executed` must not imply acceptance.

**Action:** Use separate dimensions, with evidence references:

```text
implementation: absent | partial | implemented | unsupported
execution:     not-run | passed | failed | timeout | unsupported
proof:         not-applicable | open | accepted | failed
trust:         explicit assumptions and correspondence boundaries
scope:         spec revision + requirement + implementation revision + configuration
```

A release decision derives from these facts. It does not replace them.

## Workstream readiness

| Workstream | Review judgment | Next bounded deliverable |
|---|---|---|
| Core language and checker | Coherent contracts, incomplete acceptance algorithm | Finite candidate schema and declarative checking rules for a named subset |
| Programs-as-data and identity | Strong staging rules, identity conflict and open encoding | R1 decision plus experimental normalization/identity vectors |
| Wasm backend | Requirements only in this archive | Internal calling convention with runtime capture/composition and recipe retention |
| Safety and verification | Strong claim boundaries, no executed or mechanized evidence here | Narrow Rust-to-Lean refinement experiment and explicit host assumptions |
| WIT/WASI and resources | Selected architecture, unpinned implementation profile | One world, exact type mapping, owned/borrowed adapter, tested toolchain pins |

### Later platform work

| Workstream | Review judgment | Required boundary |
|---|---|---|
| Syndicate/Synit/Preserves | Selected model, no full normative profile in this archive | Dataspace transitions, assertion lifetimes, observation order, authority, quotas, and failure cleanup |
| Choreography | Historical summary only | Recover the full draft, then define projection onto Syndicate |
| Durability | Optional design direction | Replay observations and ambiguous external outcomes, not automatic retries |
| Bend/Ambient adaptations | Useful references, not dependencies | Preserve sequential composition, source identity, and explicit host authority |

The historical Lunatic comparison describes possible live-socket capture by a quotation. That example conflicts with the selected core and must not enter a new normative draft.

The proposed `SPEC-CH002` identifier also appears in the historical choreography summary for a communication/session document. Reserve identifiers before publishing a replacement family.

## Original implementation sequence — superseded

The current greenfield sequence is in [specs/ROADMAP.md](specs/ROADMAP.md). These original estimates assumed access to prior source and are not the current work plan.

1. **Recover and measure — 0.5–1 day after access.** Record the revision, inventory the code, and run existing tests. Keep unavailable features explicit.
2. **Freeze a core draft — 1–2 days.** Resolve R1/R2, fix the new surface, restore traceability, and define the first checker subset. Do not wait for choreography.
3. **Run feasibility experiments — 3–5 days.** Try the chosen Rust checker representation through pinned Charon/Aeneas. Prototype typed Wasm runtime builders in the same period.
4. **Complete one end-to-end core slice — estimate after step 3.** Execute dynamic builders, retain recipes, reject invalid candidates, and record proof/test boundaries separately.
5. **Add platform slices incrementally.** Start with synchronous WIT import/export and one resource adapter. Add native async next, then Syndicate. Keep choreography and durability optional.

The Wasm experiment must address dynamically long compositions, interface compatibility, allocation limits, and dispatch to compiled operations. A recipe interpreter is not substitute evidence.

### First core exit criteria

1. Run the arithmetic, composition, `twice`, runtime capture, and homogeneous-program-list examples on Wasm.
2. Supply captures and choose program operands after compilation. Disable the Noble preparation service during their execution.
3. Compare execution and reflected recipes with optimization on and off, including programs nested inside returned data.
4. Reject incompatible composition, malformed witnesses, invalid unused quotations, and forbidden resource capture before candidate-body effects.
5. Preserve effect order and whole-configuration ownership across success, domain-error, and supported abnormal paths.

No result from a reference evaluator alone closes the Wasm milestone. No document check closes a runtime or proof milestone.

## Subsequent decision

The user selected a fresh implementation and confirmed that no prior repository exists. The next milestone creates the Rust/Nix workspace under the current specification family.
