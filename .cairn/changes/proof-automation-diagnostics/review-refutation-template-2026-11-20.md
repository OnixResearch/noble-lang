# Review record: tactic-free unary-`I64` refutation template (PAUTO-FO-4)

- **Change and task:** `.cairn/changes/proof-automation-diagnostics`, task 4 ("Review and prove a tactic-free unary-`I64` Lean refutation template. …"). This record covers the template half only. The compiled-replay harness (PAUTO-FO-5), PAUTO-CASE-02 to 06 and the correspondence report are outside it.
- **Reviewed commit:** `6a34895045a02dada1b10ecd5e0ee420718a0bfa` on branch `cairn/refutation-template` ("proof(mc1): add tactic-free unary-I64 refutation template (PAUTO-FO-4)"). Its parent is `fd16b77172769e52e0119f5c1612ce5b34b1126f`. The reviewed bytes are `git show 6a348950:<path>`, and the worktree had no differences from the commit during the review.
- **Reviewer:** fresh-context reviewer agent, not the implementer, per owner decision 2026-11-20. The owner keeps a veto.
- **Date:** 2026-11-20
- **Verdict:** **REVISE**. One required revision is mechanical (R-1: the verdict pin in `verification/mc1/implementation.mjs`). The Lean content is accepted as committed: the template, the helpers, both instances and the gate pins.
- **Labels:** MEASURED means the result was observed in a run listed under "Evidence". INFERENCE means reasoning from source that was not executed.

## Reviewed files (changed by the commit)

| File | sha256 at `6a348950` |
|---|---|
| `proofs/mc1/NobleContracts/Refutation.lean` (new, 164 lines) | `c2f1b76379a380ff266120989c0106c9bb7b96882a74a26957710e88f361c9e7` |
| `proofs/mc1/ContractGate.lean` (adds the import and 10 pins) | `0941f3b9a9e3241eee21c4f1da8c2172cd959436e9927f79bb6930a2bc4fe19c` |
| `proofs/mc1/lakefile.toml` (`globs`) | `b86e50a2e991f51e0bb079fa4f83f5e51ed34ed2ca2adec9b0055932c5fba917` |
| `.cairn/changes/proof-automation-diagnostics/design.md` (L377-441, "Refutation template accounting (PAUTO-FO-4)") | `9014159351434eb9b2fdcf9e157a7f89a3afd735c80457a76f34cfcc6d059918` |
| `.cairn/changes/proof-automation-diagnostics/tasks.md` (L18-20, progress note) | `d949aaa66afd2ce2ad8ddba08bf1850801d31220a81377884f7eb656e7533509` |

## Comparison, model and consumer files read (unchanged by the commit)

| File | sha256 at `6a348950` |
|---|---|
| `verification/mc1/monotonic-refutation.lean` | `eb41bfa283fef0df5988f642b3ffb8dc05e7b108aa6cd9db3d798eba1578e726` |
| `verification/mc1/monotonic.noble-contract` | `862adc84aa469b7ba3b51156a51f94de61a08fe258bac432487e1cafcc3f74b1` |
| `verification/mc1/implementation.mjs` | `6434cb235580dbea5d30638828cd518540eab5b5ee7f8b94fb62eb5e4a1c00a2` |
| `verification/mc1/gate.mjs` | `df1f556c2085fcd752aa7a21e19889cf63d55f333072df6a5edceaa53596f840` |
| `crates/noble-cli/src/workflow/verification.rs` | `7ad38c5b1d5da74b40372283ed812db07cc0d25aeda8cf5a2562388a5ed3f683` |
| `crates/noble-cli/src/workflow/verification/compilation.rs` | `d583e93a772d4bdbb442cf531b26163b13ac41edd1cdb7abf391b136f185c76e` |
| `crates/noble-cli/src/workflow/verification/evidence.rs` | `4cb181af6be0cfb5ad65f7d27c3c18cb9e116685439b8535acc4c34a2e601249` |
| `crates/noble-cli/src/consumer/replay.lean` | `3c0992a02adee4a3127a9eef62a438fa7ea4b467e9f28af224c904c850c52e1e` |
| `crates/noble-cli/src/producer.lean` | `47ff027c39e486953a7d5eb9720b425e1c79ad269d45c0c2a9e6bfae285017ec` |
| `crates/noble-cli/src/workflow/rules.rs` | `09f300b6bd8c6b6d68a1ac3ec5a019f7c50e1bf2a126c48e802fe7619f693b71` |
| `crates/noble-cli/src/workflow/intrinsic.rs` | `3e5b0723595718b8897c5b64cceda6b451312004e32d38c7b247d618148da229` |
| `crates/noble-contracts/src/rendering/mod.rs` | `7b2cfcd5271ce19bb4c39067bf2ce62cda4fe218201228173bf67a6be7770590` |
| `proofs/mc1/NobleContracts.lean` | `f975ac8220689ea0e9937480d60ad2c252d1cce2b151874c18ee618930981fc2` |
| `proofs/mc1/NobleContracts/Obligation.lean` | `13dddb8d47346ba648eb2dd372031ccd2f2e090f3312f1797a1ca567ab9cd616` |
| `proofs/mc1/NobleContracts/Expression.lean` | `4638c9b146da69b5d0a545734fd5bdc690288f86d1c3a2a40a235007a70561db` |
| `proofs/mc1/NobleContracts/Rules.lean` | `1d5802ddec13fcfcbba70e6c75f46ab051ea569e6f30fe92d61e3b5dec8d877b` |
| `proofs/mc1/NobleContracts/Model.lean` | `368cad7d6d86db4b182bd6b9e2bd0d3a354559c04579e3e27b789bb69313a7c3` |
| `policy/tool-selection.json` | `8ed766cdfa42f8228bfba41c28c6aee91afd97652418ae7f16952cdc51875e9b` |
| `policy/tool-selection-files.ncl` | `f64a2e5c67194d2a86d0f9ead23609c1dae63d9673fa435a439535aee313db7d` |
| `specs/verification/obligations.json` | `faa80fb0f2657228043dbdc9f87a3ad0eae267b0dcdd8e3a012b8cd8469793f0` |
| `.cairn/specs/verification-toolchain/spec.md` | `2ff24801f3e505da9701278f1acc466798f99b9731f516c8f11a2a910e316a25` |

## Findings

| ID | Severity | Location | Finding | Action |
|---|---|---|---|---|
| F-1 | **Major (required)** | `verification/mc1/implementation.mjs:170-177` | The commit raises the gate's required list from 90 to 100 theorems (`ContractGate.lean:70-82`). The gate now prints `MC1-CONTRACT-LIBRARY: 100 required theorems; strict axioms accepted` (MEASURED). The exact-string pin in the MC1 implementation gate still reads `'MC1-CONTRACT-LIBRARY: 90 required theorems; strict axioms accepted'` (L175), and its comment still says 90 (L170-174). That predicate is false on the measured gate output (MEASURED), so once past the bound-file check (F-2) the gate throws `missing strict contract-library verdict`. This is the gate that PO-19's evidence names (`specs/verification/obligations.json:275-276`). It also produces the `implementation.json` receipt that PO-15 cites (L195-196). Precedent: commit `3bb7b5f2` (the Wire laws, 67 to 90 theorems) changed this pin in the same commit as `ContractGate.lean`. `implementation.mjs` is not a bound file, because it is absent from `policy/tool-selection.json` `file_sha256` (MEASURED), so this fix needs no re-pin. | R-1 |
| F-2 | Integration (disclosed) | `proofs/mc1/lakefile.toml`; `policy/tool-selection.json:347`; `policy/tool-selection-files.ncl:98` | The bound file now hashes to `b86e50a2…`, but both policy files still pin `7518fd24…` (MEASURED). Until the re-pin, consumers of the `file_sha256` pins refuse the tree. For example, `implementation.mjs:43-44` throws `stale selected configuration: proofs/mc1/lakefile.toml` (MEASURED: its predicate fails on the actual hash). Other consumers, such as the tool-selection checks, are INFERENCE. The commit message and the tasks.md note disclose this. Campaign rules assign re-pins to the integrator. | Integrator re-pin. This is not a slice defect. |
| F-3 | Minor | `proofs/mc1/lakefile.toml:11-15` | The comment says a plain `lake build` "produces the oleans the consumer and `ContractGate.lean` use". The CLI consumer never reads Lake's oleans. `read_library` snapshots `.lean` sources only (`rules.rs:56-104`, L88). The workspace rewrites those sources (`verification.rs:64-69`). `compile_rule` compiles each module in the sandbox with only `/inputs` and `/out` mounted (`compilation.rs:68-109`). The report states "exact-source-snapshot; rebuilt without producer caches" (`rules.rs:32`). The comment misstates a trust-relevant property in a bound file. | R-2 (recommended; do it in the F-2 re-pin). |
| F-4 | Minor | `proofs/mc1/lakefile.toml:16` vs `verification/mc1/implementation.mjs:159-166` | Two mechanisms now define the Lake build set. With the globs, `lake build NobleContracts` builds all 16 modules (MEASURED). That makes the explicit out-of-closure targets in `implementation.mjs` redundant, along with its comment "Build every MC1 library module, including … outside the root import closure". One of the two mechanisms is required: with the parent lakefile and the explicit list, `Refutation.olean` is never built and `ContractGate.lean` fails with `object file … NobleContracts/Refutation.olean … does not exist` (MEASURED). | R-3 (recommended): keep one convention. |
| F-5 | Note | `Refutation.lean:107-109, 152-155`; `Expression.lean:58-61, 65, 81` | `not_holds_of_evaluate_false rfl` makes the elaborator and the kernel evaluate `evaluate (.lt (.input 0) (.output 0)) ⟨…⟩`. That evaluation is a structural unfolding of `evaluate` through the private `compareI64`, then `BitVec.ofInt`/`BitVec.toInt` on 64-bit literals, the wrapping `+`, and `decide (x < y)` through `Int.decLt`. No `decide` tactic is used and there is no `Lean.ofReduceBool`. Still, this is `Decidable.decide` evaluated by kernel reduction, the same kernel work the `decide` tactic requests. The work is small, uses only GMP-accelerated `Nat` literal reduction and fails closed: an elaboration or kernel error, never acceptance, if `evaluate` or the pinned core `BitVec`/`Int` definitions stop reducing. The design text ("a definitional `evaluate` result (`rfl`)") is accurate but does not say this. | R-4 (recommended): add one disclosure sentence. |
| F-6 | Note | `rendering/mod.rs:21`; `producer.lean:234`; `replay.lean:31, 56, 105, 198, 205-210` | The consumer does not treat the template as a trusted library rule. `MC1Obligation` imports only `NobleContracts` and `NobleContracts.Expression`. The exporter ships every dependency missing from that environment. The fresh consumer re-adds those dependencies and kernel-checks them; producer definitions get `hints := .opaque`, which the kernel still δ-unfolds. Measured wire: 12 declarations for the instance form and 11 for direct application, including `exportedClaim_unaryI64_refutation.match_1_1`. Consequence (a): a `disproved` result is sound without this review, which matches "no new trusted component". Consequence (b): accepted evidence does not bind the reviewed template's identity, so a producer may ship different bodies under these names. `design.md:425-428` already defers that digest to PAUTO-FO-5. | No action now. FO-5 acceptance must bind the digest. |
| F-7 | Note | `rules.rs:56-104`; `build/report/toolchain.rs:283-306` | Adding the file changes every CLI run against the default library; the globs do not. Effects: (1) one more module is compiled inside the shared proof budget (the Lake build of the module took 723 ms, MEASURED; the CLI cost is INFERENCE). (2) `rule_library_revision.sources` lists 16 modules, including `NobleContracts.Refutation` (MEASURED). (3) The proof-required build lane's `library_tree` blake3 over `proofs/mc1` changes (INFERENCE). No existing acceptance changes, and no script compares a retained library digest with the current tree (grep, MEASURED). | None. |
| F-8 | Nit | `Refutation.lean:17`; `design.md:379` | Both places say "two reviewed instances" while the same paragraph says review is pending. The wording becomes true only once this record is retained. | R-5 (optional). |
| F-9 | Note | `.cairn/specs/verification-toolchain/spec.md:1046` (view `specs/VERIFICATION-TOOLCHAIN.md:328`) | Canonical prose says the implementation gate "checks the 90 required strict contract-library theorems". After R-1 it checks 100. Precedent: `5f1797e4` updated this prose in a separate docs commit. | Integrator docs sync. |
| F-10 | Note | `Refutation.lean:48-51` | The helper closes only postconditions that evaluate to `some (.bool false)`. A counterexample whose postcondition evaluates to `none` also refutes the claim, and the template accepts one through a `none ≠ some` proof (MEASURED probe). No helper exists for that case. PAUTO-CASE-03 to 06 do not need one. | For the FO-5 producer, if it is needed. |

No finding concerns soundness. Nothing in the commit claims a discharge, a promotion or a ticked box that it did not earn.

## Answers

### 1. Soundness, generality, and exact match with the consumer's claim

- **Statement** (`Refutation.lean:67-72`). The premises are `run : Exec code [.i64 input] [.i64 output]`, `preHolds : Holds pre [.i64 input] [] []` and `postFails : ¬ Holds post [.i64 input] [.i64 output] []`. The conclusion is `¬ exportedClaim code [.i64] [.i64] [] (Holds pre) (Holds post)`. `#check` prints exactly this (MEASURED).
- **Proof** (`Refutation.lean:73-76`). The proof applies the claim once, with `tail := []`, `before := [.i64 input]`, `params := []`, the typing witnesses `StackTyped.cons HasType.i64 StackTyped.nil` and `StackTyped.nil`, then `preHolds`, `final := [.i64 output]` and `run`. `run` is accepted at `Exec code ([] ++ [.i64 input]) _` by `List.append` reduction. The proof then destructures `⟨_after, hfinal, _, hpost⟩` and closes `False` with `(hfinal ▸ postFails) hpost`. Each step follows `exportedClaim` field by field (`Obligation.lean:21-28`): `pre before [] params` (L25), `Exec code (tail ++ before) final` (L26), and `final = tail ++ after ∧ StackTyped after outputTypes ∧ post before after params` (L27-28).
- **Sound.** The kernel checks the proof. `#print axioms` gives `[propext, Classical.choice, Quot.sound]` for the template and both `*_refutation` instances (MEASURED). `Classical.choice` comes from `evaluate`'s `Classical.propDecidable` (`Expression.lean`, the `.eq`/`.maps` clauses), which `Holds` brings into the statement. The helpers use subsets: `[propext]` for the two constructor-conflict lemmas and `[propext, Quot.sound]` for the two executions (MEASURED). The consumer re-checks all of these declarations from the wire (F-6). Soundness of `disproved` therefore never rests on this module being reviewed.
- **Generality.** The template covers exactly the following: one `I64` input, one `I64` output, `paramTypes = []`, the empty tail, and a normal return. That matches what `design.md:381-395` and the docstring (`Refutation.lean:56-66`, "empty prefix, the single typed input, and no parameters") claim. It is program-generic: a probe instantiating it for `[ 2 * ]` at `x = 0` was accepted (MEASURED). It is also postcondition-generic: `postFails` is any `¬ Holds`, and a probe with an undefined postcondition `(head nil)` was accepted (MEASURED). The design correctly says it cannot refute claims with logical parameters, several inputs or outputs, closed programs such as `wrap.noble-contract` (`design.md:393-395`), traps or non-termination.
- **Exact claim.** I regenerated the obligation for `verification/mc1/monotonic.noble-contract` with `noble explain-proof`. It is byte-identical to the implementer's copy (MEASURED). Its definitions are `claim := exportedClaim program inputTypes outputTypes paramTypes (Holds precondition) (Holds postcondition)` with `program := [node_0, node_1]`, `node_0 := .lit (.i64 (BitVec.ofInt 64 (1)))`, `node_1 := .word 4`, `inputTypes := [.i64]`, `outputTypes := [.i64]`, `paramTypes := []`, `precondition := .bool true` and `postcondition := .lt (.input 0) (.output 0)` (renderer: `rendering/mod.rs:21-37`). The type of `monotonic_refutation` (`Refutation.lean:112-114`) is that claim's negation up to δ-unfolding of the generated definitions and of `monotonic_program` (`Refutation.lean:88`). The literal is in the same `BitVec.ofInt 64 1` form, so the match needs no evaluation, only unfolding. Results for the consumer's comparison (`replay.lean:205-210`):
  - `Kernel.isDefEq (type of monotonic_refutation) (Not MC1Obligation.claim)` returns `true` (MEASURED).
  - Negative control: `wrap_constant_refutation` returns `false` and fails to elaborate against that claim (MEASURED).
  - Applying the template directly to the generated `MC1Obligation.program/precondition/postcondition` also checks (MEASURED).
  - End to end, `noble verify … --refutation` returned `outcome: disproved`, `independent_recheck: true`, accepted axioms `{propext, Quot.sound, Classical.choice}` and no diagnostics. This held for both the landed instance and the direct application (MEASURED).

  So the conclusion is the negation of the exact exported claim, not a weaker statement. The identity is definitional, not syntactic, and the consumer accepts definitional identity by design. The wrap-constant instance also matches the frontend's generated claim for a scratch contract `[ 9223372036854775807 + ]` with `(ensures (lt (in x) (out y)))`: `isDefEq` returned `true` (MEASURED). That program is not a repository fixture.

### 2. Tactic-free, and reliance on kernel defeq

- **Source.** With comments and strings stripped, the module's code contains only `import`, `fun`, `match`, `rfl`, `▸`, `|>.mpr`, anonymous constructors and named arguments. It contains none of `by`, `simp`, `decide`, `native_decide`, `omega`, `exact`, `macro`, `elab`, `syntax`, `run_cmd`, `#eval`, `set_option`, `attribute`, `instance`, `axiom`, `sorry`, `opaque`, `partial`, `unsafe`, `extern` or `implemented_by` (MEASURED scan).
- **Elaborated environment.** The module adds 10 theorems and 6 `def`s (`monotonic_{program,input,output}` and `wrap_constant_{program,input,output}`), plus one matcher, `exportedClaim_unaryI64_refutation.match_1_1`, which the elaborator generates for the term-mode `match`. That is a match-compiler artefact, not a tactic. There are no `_proof_`, `_simp` or `_auxLemma` constants. The proof terms reference no `Decidable.decide`, `of_decide_eq_true`, `Lean.ofReduceBool` or `sorryAx` (MEASURED inventory and used-constant listing). The task's requirement is met.
- **What the reviewer should know** (none of these is a defect):
  - "Tactic-free" describes this module's terms, not its transitive closure. The instances use the MODEL lemmas `exec_append_iff`, `exec_lit_iff` and `exec_add_iff` (`Rules.lean:219, 230, 248`), which are proved with `simp`. Those lemmas sit in the consumer's trusted import closure and the gate pins them, so they do not travel on the wire (MEASURED wire list). This matches `design.md:283`.
  - The `rfl` in the postcondition lemmas hides `Decidable.decide` evaluation (F-5).
  - The `rfl` in `exec_*_iff … |>.mpr rfl` (`Refutation.lean:102-103, 146-148`) closes `List.append` equations and unfolds `monotonic_output`/`wrap_constant_output`. This is trivial.
  - `▸` (`Refutation.lean:76`) relies on the elaborator's motive selection in the pinned Lean 4.35.0 release and on `[] ++ _after ≡ _after`.
  - On the consumer side, producer definitions are re-added with `.opaque` hints (`replay.lean:31`), and the kernel still unfolds them (MEASURED acceptance).

  Every one of these fails closed. A toolchain bump or a MODEL change that broke reducibility would produce an elaboration or kernel error, never an acceptance.

### 3. MODEL closure, the lakefile globs, the rule library and `implementation.mjs`

- **MODEL and `model_revision()` are unchanged.** `MODEL` (`intrinsic.rs:9-58`) lists `NobleContracts`, `.Model`, `.Rules`, `.Expression`, `.Obligation`, `.Examples` and `.Composition`. `model_revision()` (`intrinsic.rs:156-168`) hashes only those modules plus `IntrinsicTypeWitness.lean`. The commit touches none of them, and `NobleContracts.lean` does not import `Refutation` (MEASURED diff stat). `NAMED_MODEL` and the checker revision are unchanged for the same reason. The generated `MC1Obligation` imports only `NobleContracts` and `NobleContracts.Expression` (`rendering/mod.rs:21`), so no existing proof's trusted environment changes. `intrinsic.rs:6-8` states this intent for extra modules.
- **The globs do not change the CLI.** The CLI never reads `lakefile.toml` or `.lake`. `read_library` (`rules.rs:56-104`) and `compile_library`/`compile_rule` (`compilation.rs:36-109`) already compiled every `.lean` under `NobleContracts/` from a source snapshot before this commit. The new file is what changes the CLI library (F-7). No existing acceptance decision changes.
- **The globs change what Lake builds, and that matters for `implementation.mjs`.** With the globs, the `lean-build` leg (`implementation.mjs:162-166`) builds `NobleContracts.Refutation` through the `NobleContracts` target (MEASURED). Without them, `ContractGate.lean` fails (MEASURED, F-4). The `contract-library` leg (L169-177) then fails on the 90-theorem pin (F-1). Before that, the tree is refused as `stale selected configuration` until the lakefile is re-pinned (F-2). The receipt's `source_revision` changes, as it does for any `proofs/mc1` edit (snapshot at L31-41).
- **Accepted evidence.** No previously accepted evidence changes. Future `implementation.mjs` evidence cannot be produced until R-1 is made and the lakefile is re-pinned.

### 4. PO-15/PO-19 accounting

The accounting is honest, and the commit claims no discharge.

- `specs/verification/obligations.json` is unchanged. PO-15 (L180: route `lean-kernel`, status `open`) and PO-19 (L260: route `lean-and-aeneas-contract-export`, status `open`) receive no new evidence entry.
- `design.md:407-415` says the template "does not discharge PO-15" and that "PO-15 stays `open`". It lists the normal-return-only scope, traps, non-termination, `test.emit`, runtime execution and companions.
- `design.md:417-428` says PO-19 "remains `open` and unaffected". It says the one-fixture definitional check "is not PO-19 evidence and must not be cited as such". It defers the template's digest to FO-5.
- `design.md:430-436` describes the trust base, and that description agrees with the consumer code (F-6).
- `design.md:438-441` says no review has been performed.
- The tasks.md note is headed "Progress notes (not acceptance)". Task 4 stays `- [ ]` (`tasks.md:16`). Its local CLI run is described as "local tooling evidence, not a retained conformance receipt", and I reproduced that run independently (MEASURED).
- One caveat links to F-1: the accounting says the theorems are "pinned … in `ContractGate.lean`". The gate that would record new PO-15/PO-19 evidence (`implementation.mjs`) is broken by the unchanged pin.

### Comparison with the hand-written refutation

`verification/mc1/monotonic-refutation.lean:7-22` proves the same proposition, `Not MC1Obligation.claim`, with tactics: `change`, `intro`, `let`, `have`, `obtain`, `simpa`, `subst` and `simp [Holds, evaluate, maximum]`. It reuses `Examples.increment` and `increment_normal` (`Examples.lean:7, 28-30`). `increment` writes its literal as `.i64 1`, a `BitVec` numeral. The template instance instead uses the frontend's own `BitVec.ofInt 64 1` form, so its program identity is pure δ-unfolding. Matching the hand proof's literal against the generated `node_0` needs literal evaluation (INFERENCE). Both proofs go through the same consumer path: `gate.mjs:123` runs the hand proof, and my runs exercised the template. The hand-written file is untouched, which keeps the two separate as `design.md:372` requires.

## Verdict

**REVISE.** The only required revision is mechanical. The template, the helpers, both instances and the ten `ContractGate.lean` pins are accepted as committed (sha256 above).

Required:

- **R-1** In `verification/mc1/implementation.mjs:170-175`, change the pinned string to `'MC1-CONTRACT-LIBRARY: 100 required theorems; strict axioms accepted'`. Extend the comment to name the ten PAUTO-FO-4 refutation-template theorems. Do this in this change's commit series, following precedent `3bb7b5f2`. No tool-selection re-pin is needed for this file.

Recommended (non-blocking):

- **R-2** Correct the `lakefile.toml` comment. The CLI consumer compiles the same module set from a source snapshot and never uses these oleans; only `ContractGate.lean`/`implementation.mjs` use the Lake build. Do this with the integrator's re-pin.
- **R-3** Keep one build-set convention. Option (a): keep the globs and drop the now-redundant explicit `NobleContracts.*` targets and comment in `implementation.mjs`. Option (b): revert the globs and add `'NobleContracts.Refutation'` to the explicit list. Option (b) also removes the bound-file re-pin.
- **R-4** In `design.md` "Tactic-free", state that the instances' `rfl` makes the kernel evaluate the model's `decide (x < y)` (`Int.decLt` on `BitVec.toInt` values). No `decide` tactic or native evaluation is involved.
- **R-5** Replace "reviewed instances" (`Refutation.lean:17`, `design.md:379`) with a reference to this record once it is retained.

Needs the integrator:

- Re-pin `proofs/mc1/lakefile.toml` (F-2).
- Sync the "90 required strict contract-library theorems" prose to 100 (F-9).
- Commit this record with private paths sanitized.

After R-1, a check limited to the R-1 diff turns this verdict into ACCEPT WITH NOTES. Any change to `Refutation.lean`, its `ContractGate.lean` pins or `lakefile.toml` beyond R-2/R-3 voids it.

## Not run by this review

- `verification/mc1/implementation.mjs` itself. It needs the pinned Charon/Aeneas/extraction toolchain, and today it stops at the bound-file check (F-2). F-1 is shown instead by evaluating its exact predicate (L175) against the measured gate output.
- The full `verification/mc1/gate.mjs`.
- `nix flake check`.
- Any compiled replay; that is FO-5 and not part of the commit.

## Evidence (private paths; sanitize before committing)

Scratch root: `<reviewer scratch outside the repository>`. The tools were Lean 4.35.0 (`/nix/store/kwvw84h9r7w0q40d3cm5xgfz0rpv7yzc-noble-lean-release-4.35.0`) and the main checkout's `proofs/mc1/.lake/packages`, used read-only through a symlink. The tree is `git archive 6a348950` of the mc1 sources, with no copied build products.

1. Cold build of the committed tree: `nlake tree/proofs/mc1 build`. Result: 18 jobs succeeded and `NobleContracts.Refutation` built in 723 ms with no warnings (`build.out`).
2. `nlake tree/proofs/mc1 env lean ContractGate.lean` printed `MC1-CONTRACT-LIBRARY: 100 required theorems; strict axioms accepted` (`contract-gate.out`).
3. `checks/ReviewChecks.lean`, run against the generated monotonic `MC1Obligation`, covered:
   - axioms of the ten theorems;
   - the template statement;
   - elaboration of `Not MC1Obligation.claim` from the instance and from direct template application;
   - a consumer-mirroring `Kernel.isDefEq` (`true` for monotonic, `false` for the wrap-constant control);
   - a `#check_failure` negative control;
   - the module inventory and the constants each proof uses;
   - the boundary values `(9223372036854775807, -9223372036854775808)` and `(1, -9223372036854775808)`;
   - the program and undefined-postcondition probes.

   Exit status 0 (`checks/ReviewChecks.out`).
4. `noble explain-proof` on the monotonic fixture produced a generated statement byte-identical to the implementer's. The binary was the owner-law debug build (sha256 `7deeb592bbd7f6363888eac535771a1f7f2b42fd381a8537e65ba6561c54fd19`). Its verify-path sources (`crates/noble-cli/src/{workflow,workflow.rs,consumer,producer.lean,compiler.lean,sandbox,sandbox.rs}` and `crates/noble-contracts/src`) are identical between `bc8586f5` and `6a348950` (MEASURED by `git diff`). That the binary was built from those sources is INFERENCE.
5. The scratch `wrap-constant.noble-contract` was run through `explain-proof`, and then `checks/WrapConstantCheck.lean`: `isDefEq` is `true` for `wrap_constant_refutation` and `false` for `monotonic_refutation`.
6. `run-cli.sh`: `noble verify <wt>/verification/mc1/monotonic.noble-contract --refutation <file> --timeout-ms 600000` with `NOBLE_CONTRACT_LIBRARY=<scratch tree>/proofs/mc1` and the pinned Lean, bubblewrap, prlimit and systemd-run. Both files returned exit 1 with `disproved`: the instance form in 180 s and the direct form in 177 s.
7. Experiment with the parent lakefile (`tree-base`). Running `implementation.mjs`'s explicit Lake targets produced no `Refutation.olean`, and `ContractGate.lean` failed with exit 1 (`base-lakefile-build.out`, `base-lakefile-gate.out`).
8. A node predicate check: the `implementation.mjs:175` string is absent from the gate output, and the lakefile policy pin `7518fd24…` does not match the actual `b86e50a2…`.
9. `scan.mjs` (strips comments and strings, then scans tokens) produced `scan.out`.
10. In the worktree: `ncairn validate --root .` reported 0 issues, 0 change issues and 0 findings across 21 changes. `bun tools/check-specs.mjs` (no write flags) printed `"result": "passed"`. `git status` was empty before and after.

| Artifact | sha256 |
|---|---|
| `build.out` | `28a9e496c123928912ce3a9bc343ac36390a2c088396c220204cc76825355fa6` |
| `contract-gate.out` | `af548f7370e435c5e15e3b78271979ca08a5503f36126d0d7a396e0a9b330721` |
| `checks/ReviewChecks.lean` | `37d0c37f888fde25d6b65d746a9f029bb95e53826a22f163de0b09d668274d1e` |
| `checks/ReviewChecks.out` | `1e5384392652891df26c234ab60bcda224346bf05e88af9c5acd7730f655ad49` |
| `checks/WrapConstantCheck.lean` | `57d3f70035c216b016759c3193363f1c544fee0c2307149f0b42b3f9c09a4d64` |
| `checks/WrapConstantCheck.out` | `da010442c254d6d24847f98fd20188672a248f529c279b43612af1b59c520fe2` |
| `MC1Obligation.monotonic.lean` | `bded7c7f7fb5deb79ff96790f81d0a91d767064913fa633f0c36f1b95eed0750` |
| `obl-wrap/MC1Obligation.lean` | `b7a11990463b1bd83d2bbfec94f848f50030504c27e4147c7fec50498cbb0c16` |
| `wrap-constant.noble-contract` | `b58d465aacbe78b0b780c509c16bf1168dd21561a3953ae5f47dff8e5d8a674c` |
| `explain-monotonic.json` | `a30242a1592995746ca142fedf83fbae901aab437b45fb5b86d84e14fbbd5e0a` |
| `explain-wrap-constant.json` | `f542bc4544b54a2fa22723f92f0b569a9aa0ed2691859d370a089f5624120771` |
| `monotonic-template-refutation.lean` | `717742b13801096aab863daa702b52fff10ad202be9680cf5ab63cc80d162df5` |
| `monotonic-template-refutation.report.json` | `1e1f8f6dd8f7dbee52ec9217324f28d73eae9cfd9bf950d2fd963dc9aede3543` |
| `monotonic-direct-template-refutation.lean` | `f4be15dc2dfbb3109a6ae6add45ef76f9afbfc7ebfee3c4ebb03bbc85b47738f` |
| `monotonic-direct-template-refutation.report.json` | `792a19b92f0315db10f7abe860bdca9b7ccb802527965c537ff2b2f1f497723c` |
| `run-cli.sh` | `d380be776e273798cfbb7f253cab1438ef68b9786e4eefef7f65a85c7c8ba769` |
| `base-lakefile-build.out` | `32802c3aaecbfbab54b76532dc3a57273f205650ab13584f4d200a6fd372cc0d` |
| `base-lakefile-gate.out` | `923d2f3140dc35edfeffe670e2df890673ab2aeb5af328a9d1faf7f80feafa90` |
| `scan.mjs` | `64ef1f21d6f6eefc2e60df6be8fe1a8876797a23ea7f745d658fbc444b2e0901` |
| `scan.out` | `ef5552ad72d60d807753636ec35516af292cf2323ce45c5cd2cf69e441a94fdd` |
| `cairn-validate.json` | `54e34088463b54d06009b659be41d5426bd74c8717c2808de6651e0bcd00dc75` |
| `check-specs.out` | `01d5527280fe65be64fef4d135bcb5f32acefcb9bb01abfeacb29c4390de44ed` |
