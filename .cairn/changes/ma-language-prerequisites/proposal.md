## Why

The accepted exact calculator requires source-checked modules, bounded iteration
or recursion, and guest-native text processing. Public main already has
`Declared-Modules-v1` and a separate `Text-Byte-Cursor-v1`, but rejects their
combination; it also rejects recursive definitions. This change selects a
separate, opt-in language prerequisite, not an exact numeric library or a
completed calculator. r[B-CHECK-02] r[K-CHECK-02] r[DX-MODULE-04] r[CALC-GATE-01]

## What Changes

- Existing `--declared-modules --text-byte-cursor` selects pure declared
  modules with the existing disjoint, allocation-free `text.byte` semantics
  at builtin 26 and one new pure signed-I64 `<` at builtin 27.
  Its pure profile needs no fake host binding file and cannot link a protected
  host operation. Ordinary declared modules, standalone cursor and unflagged
  `run`/`session` remain unchanged. r[DX-MODULE-04] r[CALC-GATE-01]
- New `--signed-self-recursion`, valid only for explicitly declared modules,
  permits an exported definition to reference *itself* only under its exact
  declared signature. The source resolver and independent checked Core bind
  the recursive occurrence to one immutable owner and one shared
  substitution; mutual recursion and polymorphic recursion still reject.
  Compiled managed-memory Wasm executes the recursion under existing bounded
  admission and runtime invocation policies. r[B-CHECK-02] r[K-CHECK-02]
  r[K-DEF-03] r[DX-MODULE-04]
- In the combined declared-Text profile, `let name` binds the current top
  value immutably for the enclosing definition or quotation. Initial `let`
  forms bind ordered function parameters; a reference copies its lexical
  value through checked stack operations, and a nested quotation captures
  checked values at construction. Duplicate names in one scope reject,
  nested shadowing is lexical, and `Data`/Capture eligibility is enforced by
  the original kernel operations. Neither ordinary modules nor standalone
  cursor acquire local syntax. r[DX-LOCAL-01] r[DX-LOCAL-02]
- Demonstrate a real compiled decimal-byte scanner with checked errors,
  opt-off/on, a bounded successful recursive path, and a source-checked
  local-parameter/capture guest beside malformed source and wrong-profile
  failures that preserve session state. This is not
  `CALC-01`–`CALC-13` execution or MA1/MA2 acceptance. r[CALC-GATE-01]

## Impact

- **Files**: Native deltas for `core-bootstrap`, `language`,
  `developer-experience`, and `calculator`; source, checked kernel, Wasm
  compiler, CLI, executable guest fixture, behavior tests and source-bound
  receipts. No canonical spec/status promotion in this change.
- **Testing**: Scoped signed recursion, lexical parameters/captures, resource
  ownership, source profile and compiler tests,
  actual `noble session --framed` compiled Wasm positive and hostile
  transcripts in both optimization modes, independent interface checks and
  final native Cairn proposal/design/tasks/validate and sync preview.
