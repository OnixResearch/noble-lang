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
checker. The receipt also separates kernel rejection, parse failure, unsupported capability, and
resource exhaustion; unsupported controls without a checker-facing option are explicit `unsupported`
rows, not passes. The 1 MiB address-space control can terminate a checker without diagnostics; its
silent exit 139 is classified as resource exhaustion only under that explicit limit, while the
unlimited baseline establishes the same checker accepts the input. Each executed run records wall
time and maximum RSS. Lane-level claim/format/axiom refusals remain separate from the checker's own
result.

## (d) Remaining `native_decide` conversions

**Deferred for this slice by the integration scope change.** No proof source, axiom inventory, M2/M3
coverage list, or strength join is changed here. Reopen after the #97 re-extraction merges: re-inventory
the then-current candidates, apply only conversions that finish under the declared heartbeat budget,
and rerun the M2/M3 proof and refusal gates and the strength join. This prevents measurements of
pre-extraction files from being mistaken for current-source results.
