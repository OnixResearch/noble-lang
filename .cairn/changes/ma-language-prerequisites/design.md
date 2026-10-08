## Context

`Declared-Modules-v1` owns immutable versioned nominal and word identities,
exact exported interfaces and optional explicitly linked host operations.
`Text-Byte-Cursor-v1` independently owns reserved builtins 23–25 and pure
`TextByte` builtin 26, retaining Text while returning the byte/status. Its
active change explicitly denies conjunction with declared modules, so the
existing standalone contract must not be silently widened. Core bootstrap
forbids recursive definition dependencies under B-CHECK-02; general K-CHECK-02,
K-DEF-03 and DX-MODULE-04 describe signature-checked recursion but leave
concrete self-reference syntax and identity unresolved.
r[B-CHECK-02] r[K-CHECK-02] r[K-DEF-03] r[DX-MODULE-04]

## Decisions

### Decision: Explicit additive language profile, never calculator authority

**Choice:** In the new profile only, the existing `--declared-modules
--text-byte-cursor` pair installs the canonical 27-row pure cursor prefix plus
one profile-private pure signed-I64 `<` row at Definition(27), *before* declared
module registration. Its `[S I64 I64 -- S Bool ! {}]` compares left below
right using signed Wasm `i64.lt_s`, with no allocation or host request; it
does not alter Core's wrapping arithmetic. Unflagged Core, standalone cursor,
ordinary declared modules and LIVE preserve their existing prefixes and
admissions. The combined profile forbids any bound host operation, host
adapter file, ambient test operation or source-selected profile switch.
An empty host-file path is not required or a credential. Independently
reconstruct the exact 28-row prefix and actual declared graph before Wasm
emission; a public Env or forged candidate cannot grant access. Keep the
original `Text I64 -- Text I64` and negative offset/EOF statuses; neither
append a substring nor treat cursor arithmetic as exact BigInt.
r[DX-MODULE-04] r[CALC-GATE-01]

**Rationale:** The new combination provides the smallest guest-side byte
scanner required by a declared exact-number library without redefining
accepted `Text-Byte-Cursor-v1` or weakening host/effect boundaries.

### Decision: Signed monomorphic self owner

**Choice:** New `--signed-self-recursion` requires `--declared-modules`.
Only a declared *exported* definition with a complete explicit signature may
resolve its own unqualified name as a distinguished self occurrence. At each
use, the frontend reuses the exact owner-body signature substitution, not a
fresh specialization; the independent kernel accepts a checked owner
substitution and exact self-node instance. A source-derived private owner
must rederive the prepared submission from immutable whole-module source,
authenticate lexical owner, signature and occurrence, then enter the
selected compiler. Ordinary producer-supplied candidate/Env, cross-owner
call, unsigned/missing signature, mutual dependency or published generic
recursive value cannot acquire this path. Reflect/recipes carry finite owner
identity without expanding the body. The same compiled Wasm call uses
existing bounded invocation depth, fuel and memory; exhaustion/trap is not
a successful numeric `ResourceLimit`. r[B-CHECK-02] r[K-CHECK-02]
r[K-DEF-03] r[DX-MODULE-04]

**Rationale:** A self-token in ordinary source is ambiguous after display
rebinding; the resolved immutable owner plus its exact signature rules out
polymorphic recursion without making it a Core-Bootstrap primitive. A
non-recursive definition keeps its old inference/generalization.

### Decision: Finite application precursor with visible failure

**Choice:** Execute an actual Noble decimal-byte helper in a real managed
Wasm session using `text.byte` and signed self with a nominal
`Result<I64,ParseError>` output, UTF-8/malformed/EOF boundaries and a bounded
success case. Compare opt-off/on and retain the raw source/stdin/stdout,
compiled Wasm/WAT and code/receipt hashes. Reject wrong module version,
unsigned self, wrong fixed signature, forged profiles and malformed source
*before* execution; submit a valid expression afterward to prove static
failure preserves namespace/stack. Distinguish runtime exhaustion from
state-preserving static refusal. A finite wrapping-I64 fold is not exact
decimal parsing or any accepted CALC case. r[CALC-GATE-01]

**Rationale:** Contract tests alone cannot establish the compiled guest
path, and host-only decimal parsing would not satisfy the calculator gate.

## Risks / Trade-offs

- Combining two existing flags is a *new* host-selected contract and needs
  negative cross-profile tests; the active cursor change remains unchanged.
- Existing source-work and signature table meters must charge actual visited
  nodes/keys, not all historic keys on an early match. A compiler exhaustion
  is an explicit failure and cannot publish partial state.
- This does not implement BigInt/Rat, seven independently typed calculator
  budgets, full calculator grammar/session commands, local-name authoring
  syntax, general termination/proof, or MA1/MA2 acceptance. Source-bound
  structural admission and observed execution are separate from refinement.
