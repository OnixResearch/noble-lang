# Decision: retain VT-PIN-02 exception for #97

**Decision: blocked; retain the VT-PIN-02 library exception.** The source and Aeneas Lean library pins currently match at `1238879f`, and the reviewed Charon invocation includes `--inline-anon-consts`, but Aeneas still cannot translate two minimal, source-bound subjects. The failure records are [`vec-new-failure.json`](vec-new-failure.json) and [`iterator-find-failure.json`](iterator-find-failure.json).

## Evidence

- `Vec::new` used as a generic function item: Charon succeeds; Aeneas exits 2 with `Unimplemented` in `symbolic/SymbolicToPureTypes.ml:411` while translating `alloc/src/vec/mod.rs:464:4-464:30`.
- `Iterator::find` with a higher-ranked predicate lifetime: Charon succeeds; Aeneas exits 2 with `Unimplemented: found an occurrence of a lifetime constraint relating a higher-ranked lifetime to a free lifetime` in `llbc/TypesAnalysis.ml:994` at `core/src/slice/iter/macros.rs:338:12-341:46`.
- The attempted reviewed opaque-item fallback for `crate::contracts::nominal::validation::bound_definition_matches` made its LLBC body `Opaque`, but the full kernel Aeneas translation still exited 2 with other interpreter failures (first: `Could not read a borrow` at `crates/noble-kernel/src/shapes.rs:251:17-251:22`; also a context mismatch at `crates/noble-kernel/src/contracts/nominal.rs:300:25-300:51`). It did not produce a complete subject or reach downstream Lean checks, so no opaque exception is adopted.

`--inline-anon-consts` was present in both standalone Charon runs. It does not resolve either failure above. The separate promoted-`'static` negative control passed: pinned Charon extracted the fixture, and Aeneas exited 2 with `There should be no bottoms in the value` for both escaping constants. That is the expected refusal and addresses the `'static` caveat; it does not resolve the two independent translation blockers documented here (unit-3 record, commit `048c3971`).

## Exact reopen condition

Reopen the exception only after a pinned extraction revision and command configuration satisfy all of the following on the fixed subject inputs:

1. Both committed reproducers pass Charon and Aeneas, including the generic `Vec::new` function item and the `Iterator::find` higher-ranked lifetime case. The promoted-`'static` negative control must continue to refuse escaping constants under the reviewed `--inline-anon-consts` argument (or a pinned LLBC scan must establish that none occur in the fixed subjects).
2. Every checked-in generated subject is freshly produced by that extractor. If an opaque-item exception is proposed, the complete real subject extraction must succeed and every downstream Lean proof that consumes the subject must still check; the body hash and reason must be recorded.
3. The affected M2, M3, M3-Wasm, and MC1 proof/build, coverage, and extraction-evidence lanes pass with fresh receipts.
4. Only then remove the compatibility block and all VT-PIN-02 exception restatements, regenerate the Cairn views, and run the required Lean lanes and repository flake checks.

Until those conditions hold, do not claim #97 is complete or remove the VT-PIN-02 exception. This is the recorded hold decision; the current canonical spec and generated views must be reconciled with it before closure. These probes are translation diagnostics only; they establish neither a Lean proof nor a Rust defect.
