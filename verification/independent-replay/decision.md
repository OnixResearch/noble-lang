# Issue #62 follow-up decisions

## (a) Language-diverse checker survey and selection

**Decision:** add source-built OCaml4Lean as the additional independent implementation. The current
independent-design set is nanoda (Rust), con-leche (Lean), and OCaml4Lean (OCaml). The Rust con-ron
is a con-leche port and Lean4Lean is derived from Lean's C++ kernel; neither is counted toward the
language-diverse set. OCaml4Lean has its own parser, term/environment representation, and source
revision, and is not a port of either checker. It deliberately follows the reference kernel's
algorithmic behavior, so this is implementation and language diversity, not an assertion of
algorithmic independence.

The survey used the Lean Kernel Arena checker registry at commit
[`6de4317e0e4c7c34d4c474fda1628a8dc64b34ee`](https://github.com/leanprover/lean-kernel-arena/tree/6de4317e0e4c7c34d4c474fda1628a8dc64b34ee/checkers).
Candidates and the selection boundary:

| Candidate | Language / pinned source revision | Finding |
|---|---|---|
| [OCaml4Lean](https://github.com/yoonwoojeong/ocaml4lean/tree/6f94226df5b91e51af692d05e00b001ee8e0f2fd) | OCaml, `6f94226df5b91e51af692d05e00b001ee8e0f2fd` | Selected. Its Arena entry identifies the Lean-export NDJSON input, and its reader documents format 3.x. The pinned Nix derivation supplies the OCaml compiler and the upstream shell build; compatibility with this repository's Lean 4.35.0 / 3.1.0 exports is established by the clean-source receipt, not inferred from the registry claim. |
| [Tenet](https://github.com/keithadler/tenet/tree/026a35ce8c433b4785a8be8ae15952803ddfacc5) | C#, `026a35ce8c433b4785a8be8ae15952803ddfacc5` | Independent implementation and reads lean4export NDJSON. Not built or tested against 4.35.0 here; compatibility is therefore unestablished, not declared false. Not selected because OCaml4Lean supplies the needed additional language with a smaller source-build closure. |
| [tenet-hs](https://github.com/keithadler/tenet-hs/tree/c1dfa0eece89bdffe9e4018a9ade8b58e14066e3) | Haskell, `c1dfa0eece89bdffe9e4018a9ade8b58e14066e3` | Arena describes an independent NDJSON checker, maintained as a Tenet sibling. No local 4.35.0 compatibility run; not needed after the selected checker passed. |
| [tenet-jvm](https://github.com/keithadler/tenet-jvm/tree/264ddbd4e409569d8c0d28140f387a06eda68ca7) | Java, `264ddbd4e409569d8c0d28140f387a06eda68ca7` | Arena describes an independent NDJSON checker, maintained in the Tenet family. No local 4.35.0 compatibility run; not needed after the selected checker passed. |
| [eink0rn](https://github.com/Timeroot/eink0rn/tree/ca5c1d24691011daba4c064f989f18d2d16cabd1) | Haskell, `ca5c1d24691011daba4c064f989f18d2d16cabd1` | Arena describes a clean-room checker with an NDJSON input and a large documented memory profile. Not built or tested against 4.35.0 here; kept as a viable future candidate, not marked incompatible. |
| [Vow Lean kernel](https://github.com/pmatos/vow-lean-kernel/tree/da680383c6a797cba1a356ed3ae533a3f8d5b507) | Vow, `da680383c6a797cba1a356ed3ae533a3f8d5b507` | Reads NDJSON and has an `Init` result in the Arena. The pinned build requires bootstrapping a separate Vow compiler revision and its Rust dependencies. Not built here; no 4.35.0 format claim. |
| [EVMLean](https://github.com/FawadHa1der/evm_proof_checker/tree/7157f684854897ec46a5e7c76ead46e3bfcd4ba3) | Solidity, `7157f684854897ec46a5e7c76ead46e3bfcd4ba3` | Arena marks module-scale `Init` inputs as resource declines because of its EVM gas budget. It cannot establish the required large-root baseline, so it is not a compatible candidate for this corpus. |
| [rpylean](https://github.com/Julian/rpylean/tree/463a3101197fa7db733cc6e2d4f55767ef3b3713) | Python/RPython, `463a3101197fa7db733cc6e2d4f55767ef3b3713` | Arena explicitly marks Mathlib as declined and records a multi-hour/incomplete run for a large test. The required corpus compatibility and bounded run are not demonstrated; not selected. |
| [Pestka](https://github.com/jasisz/pestka/tree/f8e48a7ddee20ebfe9bfeb3a81ae2b87763c374c) | Aver compiled to Rust, `f8e48a7ddee20ebfe9bfeb3a81ae2b87763c374c` | Separate type-theory implementation, but its executable and NDJSON-to-Susz conversion are Rust-based. It does not provide a clear implementation-language distinction from nanoda for this criterion, so it is not counted. |
| [nanoclo-fortran](https://github.com/kim-em/nanoclo-fortran/tree/b3a13ef5f0ae98a56087bef11ee1ec80dde0ef30) | Fortran, `b3a13ef5f0ae98a56087bef11ee1ec80dde0ef30` | Arena describes it as a Fortran translation of nanoclo; excluded as a port for language-diverse independence. |
| [lean4cobol](https://github.com/kim-em/lean4cobol/tree/d2e7b79c014ec3fa9aa8fb42f2b34d0ca479e3c8) | COBOL, `d2e7b79c014ec3fa9aa8fb42f2b34d0ca479e3c8` | Arena describes it as a port of lean4lean, which is itself derived from the C++ kernel; excluded as a port/derived checker. |
| Arena `parse-only` | Lean | Validates NDJSON parsing only and does not check proofs; not a proof checker. |

This is a source-grounded candidate survey, not a claim that every Arena checker was built or tested.
The unbuilt candidates are not declared incompatible unless the table gives a concrete limitation.
Reopen if the selected checker ceases to build from its exact pins, a 4.35.0 baseline fails, or the
lane requires a second newly built implementation rather than the one needed to complete the
three-language set.

## (b) Export-format compatibility

The selected checker is pinned by source revision, a Nixpkgs source revision, and executable SHA-256
in `checkers.json`. The clean-source replay generates or consumes the same Lean 4.35.0 `leanexport`
NDJSON 3.1.0 artifacts used by the other checkers. Its acceptance of the fresh `Init` closure and
both Noble roots is the compatibility evidence; parser behavior on malformed and changed-version
controls is recorded separately from checker acceptance.

## (c) Per-checker controls and resource records

The source-bound multi-checker receipt includes every baseline and control row for every configured
checker. The semantic proof mutation and both dropped-dependency controls are executed by every
checker. The receipt also separates kernel rejection, parse failure, forbidden-axiom refusal,
unsupported capability, and resource exhaustion; unsupported controls without a checker-facing
option are explicit `unsupported` rows, not passes. The 1 MiB address-space control can terminate a
checker without diagnostics; its silent exit 139 is classified as resource exhaustion only under
that explicit limit. The unlimited baseline establishes that the same checker accepts the input.
Each executed run records wall
time and maximum RSS. Lane-level claim/format/axiom refusals remain separate from the checker's own
result.

## (d) Remaining `native_decide` conversions

**Decision (2026-11-14): keep the currently strength-flagged fixture declarations native; no
additional conversion passed.** The brief identifies Aeneas re-extraction commit `1238879f` as
blocked by two upstream limitations, with the VT-PIN-02 exception and current generated artifacts
unchanged. Therefore this measurement uses the current checked-in M2/M3 sources, not a presumed
post-#97 extraction.

The M2 and M3 coverage records identify 12 and 26 `native` citations, respectively, for
`noble_kernel.acceptance.check`. Each cited theorem was temporarily tested with the same statement
and `decide +kernel`, under Lean 4.35.0 with `maxHeartbeats = 1,000,000` and `maxRecDepth = 4,096`.
Only the proof tactic and these trial options changed; no statement was edited. Both module builds
reported a tactic reduction failure for every candidate, not a heartbeat timeout. In each case the
diagnostic remained at the `match` over the extracted `project_result (acceptance.check …)` and the
reference `check …`; the extracted `Result` computation did not reduce to a concrete outcome. The
current modules provide conditional refinement handoffs, not a pointwise proof that closes these
fixture equalities. Replacing the native proof would therefore require new checker-refinement
lemmas, not more kernel fuel. Existing measured conversions and non-cited native fixtures remain
documented in the README.

Reproduce the bounded candidate run by temporarily replacing only the named theorem proofs with
`decide +kernel`, setting the two Lean options above in each module, and running the corresponding
`lake build` command under the policy-selected Lean:

- `cd proofs/m2 && lake build NobleM2 NobleM2.Refinement NobleKernel` — exit 1 after all 12
  candidates failed to reduce (11.78 s command wall time with warm dependencies).
- `cd proofs/m3 && lake build NobleM3 NobleM2 NobleM2.Refinement NobleKernel` — exit 1 after all
  14 candidates failed to reduce (156.96 s command wall time including dependent builds under
  concurrent load).

| Strength-flagged theorem | Trial source location | Measured result |
|---|---|---|
| `NobleM2.Refinement.refinement_sequence_and_literals_accept_arithmetic` | `proofs/m2/NobleM2/Refinement.lean:72` | `decide +kernel` failed to reduce at the extracted/reference outcome match. |
| `NobleM2.Refinement.refinement_quotation_construction_checks_body_and_runs` | `proofs/m2/NobleM2/Refinement.lean:80` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM2.Refinement.refinement_limits_nodes_exact` | `proofs/m2/NobleM2/Refinement.lean:90` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM2.Refinement.refinement_limits_nodes_over` | `proofs/m2/NobleM2/Refinement.lean:100` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM2.Refinement.refinement_limits_depth_deep` | `proofs/m2/NobleM2/Refinement.lean:110` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM2.Refinement.refinement_limits_depth_shallow` | `proofs/m2/NobleM2/Refinement.lean:120` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM2.Refinement.refinement_limits_work_eight` | `proofs/m2/NobleM2/Refinement.lean:131` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM2.Refinement.refinement_limits_work_four` | `proofs/m2/NobleM2/Refinement.lean:142` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM2.Refinement.refinement_hidden_emit_rejects_against_empty_bound` | `proofs/m2/NobleM2/Refinement.lean:153` | Same reduction failure after provenance erasure. |
| `NobleM2.Refinement.refinement_diagnostics_report_order` | `proofs/m2/NobleM2/Refinement.lean:160` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM2.Refinement.refinement_diagnostics_truncation` | `proofs/m2/NobleM2/Refinement.lean:172` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM2.Refinement.refinement_resource_eligibility_rejects_duplication` | `proofs/m2/NobleM2/Refinement.lean:185` | Same reduction failure after provenance erasure. |
| `NobleM3.Refine.refinement_recursive_dependency_self` | `proofs/m3/NobleM3/Refinement.lean:184` | `decide +kernel` failed to reduce at the extracted/reference outcome match. |
| `NobleM3.Refine.refinement_recursive_dependency_mutual` | `proofs/m3/NobleM3/Refinement.lean:192` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM3.Refine.refinement_dependency_boundary` | `proofs/m3/NobleM3/Refinement.lean:202` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM3.Refine.refinement_dependency_exhausted` | `proofs/m3/NobleM3/Refinement.lean:212` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM3.Refine.refinement_dependency_before_body` | `proofs/m3/NobleM3/Refinement.lean:223` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM3.Refine.refinement_recursive_schema` | `proofs/m3/NobleM3/Refinement.lean:231` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM3.Refine.refinement_schema_boundary` | `proofs/m3/NobleM3/Refinement.lean:244` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM3.Refine.refinement_schema_exhausted` | `proofs/m3/NobleM3/Refinement.lean:253` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM3.Refine.refinement_cyclic_witness_self` | `proofs/m3/NobleM3/Refinement.lean:267` | Same reduction failure after provenance erasure. |
| `NobleM3.Refine.refinement_cyclic_witness_mutual` | `proofs/m3/NobleM3/Refinement.lean:279` | Same reduction failure after provenance erasure. |
| `NobleM3.Refine.refinement_resolvable_chain` | `proofs/m3/NobleM3/Refinement.lean:293` | Same reduction failure; no concrete `Outcome` from `acceptance.check`. |
| `NobleM3.Refine.refinement_kind_crossed_reference` | `proofs/m3/NobleM3/Refinement.lean:306` | Same reduction failure after provenance erasure. |
| `NobleM3.Refine.refinement_duplication_negative` | `proofs/m3/NobleM3/Refinement.lean:328` | Same reduction failure after provenance erasure. |
| `NobleM3.Refine.refinement_implicit_union_negative` | `proofs/m3/NobleM3/Refinement.lean:353` | Same reduction failure after provenance erasure. |

No statement became strict in this follow-up: the clean strength join is expected to retain M2
`citations=13, strict=1, native=12` and M3 `citations=29, strict=3, native=26`. No new theorem
became eligible for the independent-replay corpus; `statements.json` remains bound to its existing
foundation roots. The follow-up strength-join receipt is
[`receipts/2026-11-14/strength-join.json`](receipts/2026-11-14/strength-join.json).

**Reopen condition:** when a corrected Aeneas extraction is available, or a structural proof of
`acceptance.check` is added for these exact checker behaviors, rerun the 26 named conversions
against that source with the same statement and budget, then rerun both coverage gates. Any
successful conversion must be committed before its clean-source receipt is generated; replay roots
remain frozen absent a separate claim-selection decision.

## (e) Community axiom-audit comparison

**Scoped decision: defer the comparison; retain Noble's inventory-bound theorem audit as the
deciding reference.** The proposed upstream `axiom-audit` revision
`0b885d88a69982cd76990b502742690b80b1a685` declares Lean 4.32.1, while the repository selects Lean
4.35.0. Compatibility has not been demonstrated. The issue also records that the upstream audit
imports compiled environments at `trustLevel := 1024`, audits assumptions rather than validating
proof terms, and documents possible undercounting in cyclic dependency clusters. The foundation
package builds multiple libraries, including vendored InteractionNets, so a default first-library
run would not satisfy the requested full module/root coverage.

No external audit was built or run, and no performance or coverage comparison is claimed. A partial
or toolchain-mismatched result would be weaker than the existing per-root audit and could not
establish equivalence. The comparison may reopen only when a source revision builds with the selected
4.35.0 toolchain and an explicit roster covers every foundation library/module and independently
selected inventoried root. The experiment must compare exact per-root transitive axioms, selected
policy violations, private helpers, imported custom axioms, empty/unimported roots, `sorryAx`, native
extensions, and cyclic fixtures against the existing checker; it must report incomplete coverage,
wall time and peak memory, and retain negative controls. Until that work passes, the existing
independent inventory-bound checker remains authoritative and no audit-only result is proof replay or
an executable-correctness claim.
