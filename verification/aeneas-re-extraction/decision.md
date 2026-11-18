# Decision: VT-PIN-02 is satisfied; the current subjects are re-extracted

**Decision: the VT-PIN-02 pin exception stays retired, and every checked-in
generated subject is the selected extractor's output for the current Rust
source.** The selected Aeneas source and Aeneas Lean library match at
`1238879f`, and the reviewed Charon `9e95cf31` invocation includes
`--inline-anon-consts`, so the canonical requirement has no pin-exception
paragraph. The old M1 probe's `505b6ca` extractor / `1238879f` library pair is
historical evidence, not the current selection; see
[the M1 probe record](../../proofs/m1/README.md).

## How the blockers were resolved

The selected translator still has the two limitations recorded in
[`vec-new-failure.json`](vec-new-failure.json) and
[`iterator-find-failure.json`](iterator-find-failure.json). The two committed
reproducers still pass Charon and fail Aeneas with exit 2:

- `Vec::new` used as a generic function item: `Unimplemented` in
  `symbolic/SymbolicToPureTypes.ml:411` at `alloc/src/vec/mod.rs:464:4-464:30`.
- `Iterator::find` with a higher-ranked predicate lifetime: `Unimplemented:
  found an occurrence of a lifetime constraint relating a higher-ranked
  lifetime to a free lifetime` in `llbc/TypesAnalysis.ml:994` at
  `core/src/slice/iter/macros.rs:338:12-341:46`.

The fix is in the subjects, not the extractor. Each construct that the selected
tools refused is rewritten into an equivalent form they accept. No
missing-body exemption is added. The remaining local opaque models are the
five inherited kernel copy/format helpers, plus ten frontend functions, four
host-handle types and one backend debug function. They are reviewed as the
earlier version of this decision required: each is bound to its complete
item bytes, extent and reason, the complete subjects extract, and every
consuming Lean proof checks. None of them is an extracted-body claim.

| Change | Commits | What it rewrites |
|---|---|---|
| Kernel restructuring | `b562952a`, `9bf57aa4`, `e1a14cb4` | A monomorphic `empty_dependency_list` replaces the generic `Vec::new` item. An explicit first-match walk (`first_bound_row`) replaces `Iterator::find`. By-value destructuring, borrows pushed while indexing, Prop-typed bindings that Lean cannot branch on, and the checked-I64 closure adapters become direct helpers. |
| Restructuring tests | `34ea0164`, `d1b71fc0` | In-crate tests pin every restructured site, found by a one-mutant-at-a-time sweep. |
| Format removal | `17de5c0a`, `3d280b62` | Every `alloc::fmt::format` and `ToString` call in the contracts subject is replaced by exact `String` building. The output is compared with the old rendering. |
| Reviewed annotations | `82b4ab13`, `f70e6e5c` | Every Charon rename, opaque item, exclusion and test-only `cfg` is bound to its complete item bytes and extent. Compiler closure instances are joined to their translated method bodies. |
| Literal-free checker | `1107343b`, `12be5375` | Adapter identities are compared as byte arrays, and the `parts::join` assertions lose their messages. The strict M4 bridge theorems therefore depend only on `propext`, `Classical.choice` and `Quot.sound`. |

The promoted-`'static` caveat is addressed by the separate negative control:
pinned Charon extracts the fixture with `--inline-anon-consts`, and Aeneas
refuses both escaping constants with `There should be no bottoms in the value`
(unit-3 record, commit `048c3971`). The kernel's `contracts::empty_dependency_list`
stays as the Aeneas-compatible form of the old `repeat_with(Vec::new)` rows:
`repeat_with` still calls the factory once per reserved row, `take` still
bounds the count, and the regression test in
`crates/noble-kernel/src/contracts.rs` checks that rows never share a vector.

## Exact condition for issue completion

#97 is complete when all of the following hold on one committed source tree:

1. `policy/tool-selection.json` names the same Aeneas revision for the
   extractor and the Lean library, and the reviewed Charon arguments include
   `--inline-anon-consts` with the negative control above still refusing.
2. Every checked-in generated `Funs.lean`/`Types.lean` (M2 and M3 kernel,
   MC1 contracts, M3-Wasm) is byte-identical to the selected tools' output
   for that tree. Only the documented M5 archive keeps the compatibility hook.
3. The M4 discovery passes, its reviewed candidate is committed as the
   extraction lock, and `check` mode passes against it. The M2/M3 coverage
   and proof gates, the MC1 and M3-Wasm gates, and the Lean lanes that consume
   these subjects pass with fresh records.
4. The repository flake checks pass on the integrated commit.

The two reproducers are not a completion condition. They document
translator limitations that the subjects avoid. If a future extractor
accepts them, the restructured forms may be simplified only with a fresh
extraction and the same gates.
