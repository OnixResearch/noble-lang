## Context

This package is grounded at worktree HEAD `dd856a83` (2026-10-29). Issue #73
was filed from the same day's `main` inspection; the cited design facts are
present at this worktree HEAD. The issue has no implementation or benchmark to
preserve. The following existing boundaries are normative inputs to this
design:

- `crates/noble-kernel/src/untrusted.rs` defines untrusted `Node` forms
  (`Literal`, `Invocation`, `SignedSelf`, `SlotInvoke` and `Quotation`), the
  finite `Candidate` arena and body, independently supplied `Expected`,
  `Limits` (`bytes`, `nodes`, `depth`, `type_size`, `stack_height`, `work` and
  `diagnostics`), and `Request`.
- `crates/noble-kernel/src/acceptance/mod.rs::check` is the ordinary admission
  entry point. It returns `Outcome::Accepted`, `Invalid`, `Unsupported`,
  `Exhausted` or `InternalFailure` through `outcome`;
  `check_signed_structure` is a distinct structural/compiler path and is not a
  credential for this producer.
- `crates/noble-contracts/src/source.rs::Session` owns the pure namespace and
  retained source history. `Session::prepare` creates a staged `Prepared`; its
  `Prepared::checked` and `Prepared::output` expose the checked interface, while
  `Session::commit` is the explicit publication transition. The source
  preparation path parses, resolves and checks before returning a preparation.
- `crates/noble-contracts/src/source/editor.rs` is editor-only transport. Its
  `Candidate` has `FORMAT = 1`, `Quotation` and `Hole`; `admitted_source` rejects
  remaining holes and `analyze_editor` produces `HoleConstraint`. In
  particular, `required_output_suffix` is only a suffix guaranteed when the
  incoming suffix is retained. It is not evidence that a completion leaves that
  suffix untouched.

Editor nodes are therefore never passed directly to kernel acceptance. A
completed tree is emitted, reparsed by normal source admission, converted to a
kernel `Candidate`, and independently checked. The package introduces no second
parser or authority.

## Decisions

### Decision: exact immutable recipes, not signature-indexed substitution

A library is a host-owned immutable snapshot. Every entry retains its canonical
body recipe, capture list (zero through eight immutable scalar captures),
resolved dependency records, instantiation witnesses, exact ordered interface,
empty effect set,
`format`, semantic revision and source revision. `chunk_id` and any digest are
lookup aids only: admission recomputes canonical bytes and independently checks
the retained body. Two chunks with the same interface remain separate entries,
even when their behavior differs.

### Decision: selected profile and chunk envelope

The only initial profile is `pure-i64-bool/1`, with envelope format
`typed-chunk/1` and semantic revision `0`. Its canonical field order is:

```
format: u32 = 1
semantic_revision: u32 = 0
profile: "pure-i64-bool/1"
chunk_id: 16 raw bytes
source_revision: 32 raw bytes
interface: { stack_in: [I64|Bool], stack_out: [I64|Bool], effects: [] }
captures: [Capture]
dependencies: [DefinitionRef]
instantiations: [Instantiation]
body: { nodes: [RecipeNode], entry: [NodeId] }
```

`DefinitionRef` is `{identity: u64, format: u32, semantic_revision: u32,
source_revision: [u8; 32]}`. `Capture` is `{ordinal: u16, ty: I64|Bool,
value: exact scalar}` and is an immutable closed value, never a resource,
reference or host capability. There are at most 8 captures, ordinals are
contiguous from zero, and a recipe may use only a retained capture with
`Capture { index: u16 }`. `Instantiation` is an ordered list of at most 16
exact type-variable bindings; each binding records variable id, kind and target
(`I64` or `Bool`). The binding list is retained, not regenerated from a
signature.

`RecipeNode` is exactly one of `I64Literal(i64)`, `BoolLiteral(bool)`,
`Capture { index: u16 }`, `Invoke { dependency: u16, instantiation: u16 }`, or
`Quotation { body: [NodeId], instantiation: u16 }`. Node IDs are unique `u32`
values; a recipe is canonical only when every reference is to an earlier node,
entry is nonempty and every quotation body is finite. `SignedSelf`, `SlotInvoke`,
text/unit literals, live references, reflection, dynamic lookup and unresolved
calls are outside the profile. The dependency index is into the retained,
resolved dependency list and MUST match its full `DefinitionRef`.

The semantic whitelist is pure ordered-stack routing plus these resolved
operations: `I64` and `Bool` literals and immutable scalar captures; `dup`,
`drop`, `swap` and `over` over
`I64`/`Bool`; `+`, `-`, `*`, `=`, `<` over `I64`; `not`, `and`, `or` over
`Bool`; and finite quotations executed only by an explicit `run` recipe. The
source spelling is resolved in the selected namespace; a spelling alone does
not admit a node. Every operation and every transitive dependency MUST derive
`EffSet::empty`. The profile permits no host capability, resource, text,
unit, syntax, live, reflection or dynamic dispatch. A dependency index MUST
refer to an earlier library entry, making the retained dependency graph
acyclic. Quotation nesting is bounded at 8.

### Decision: completion request and response

The host supplies this fixed request; the producer cannot increase any field:

```
CompletionRequest {
  format: 1,
  semantic_revision: 0,
  profile: "pure-i64-bool/1",
  task: { stack_in: [I64|Bool], stack_out: [I64|Bool], allowed_effects: [] },
  editor: { format: 1, nodes: [editor::Node], hole_path: [u32] },
  library_revision: [u8; 32],
  fixtures: HostFixtureSetId | none,
  shared_budget: HostBudgetToken
}
```

`editor.nodes` MUST contain exactly one `Hole`, and `hole_path` MUST identify
that hole. `task` is the independently supplied contract; the producer cannot
rewrite expected stacks, effects or fixture results. `HostFixtureSetId` names
host-retained expected values and contracts, including boundary I64 and Bool
cases; it is not candidate metadata. The response is:

```
CompletionResponse ::= Accepted { source: bytes, candidate: Candidate,
  selected_chunks: [16-byte id], checked: Interface, report: Report }
 | Invalid { location, diagnostic, report: Report }
 | Unsupported { location, kind, report: Report }
 | Exhausted { resource, observed, limit, report: Report }
 | NoResult { report: Report }
```

`Accepted` is possible only after the emitted source is reparsed through
`Session::prepare`, its `Prepared::checked` interface is compared with the
original task, the corresponding kernel `Candidate` is sent to
`acceptance::check`, and all selected chunk identities are revalidated. `NoResult`
means all proposals considered before the finite search ended were refused; it
never means that no completion exists. `InternalFailure` remains an
implementation failure and is fail-closed, not a `NoResult`.

### Decision: deterministic one-hole enumeration

The enumerator first runs `editor::Candidate::analyze_editor` only for context.
It then computes the hole's complete ordered input and output equation by
replaying the surrounding prefix, hole, suffix and stack tail with exact types.
`required_output_suffix` is a diagnostic hint only; no incoming suffix is
assumed preserved. A proposal is a sequence of zero through four library
chunks. The sequence order is shortlex: sequence length first, then each
position in the library's canonical order `(stack_in, stack_out, effects,
chunk_id)`; a chunk's recipe order is its canonical node order. The host may
supply a lower shared budget but cannot reorder or skip candidates.

For each sequence, the service checks ordered-stack compatibility at each
composition boundary, expands the selected recipes into a fresh explicit editor
 tree, emits source, reparses and checks the full result against the original
request. It charges expansion and dependency/instantiation work before each
bounded step. It does not infer behavioral equivalence from signatures, does
not merge equal-signature entries, and does not search ordinary `quote`,
`compose` or `run`; only the one explicit editor hole is replaced. A hole inside
an editor quotation is supported when the resulting quotation stays within the
same profile and depth budget.

### Fixed budgets and shared accounting

These are the profile maxima. A host-shared budget MAY be lower but MUST NOT be
raised by a request or a chunk. Each attempted candidate, including a refused
one, consumes its accounting; chunk boundaries never reset meters.

| Resource | Maximum | Measurement |
|---|---:|---|
| retained library bytes | 262,144 | canonical envelope and retained source/dependency bytes |
| library entries | 64 | admitted immutable chunk envelopes |
| entry recipe nodes | 64 | all nodes including literals and quotations |
| stack height | 32 | every interface and intermediate stack |
| dependency traversal steps | 256 | each dependency edge visited, transitive |
| instantiation bindings | 64 | every binding checked across a proposal |
| quotation depth | 8 | maximum nested recipe quotation |
| expanded nodes | 256 | emitted nodes after every selected recipe is expanded |
| composed chunks | 4 | maximum chunks replacing one hole |
| emitted source bytes | 4,096 | canonical completed source bytes |
| candidates | 128 | proposals enumerated and checked, accepted included |
| diagnostics | 64 | retained refusal entries, first offending location each |
| checking work | 4,096 | shared source plus kernel admission work units |
| evaluation steps | 256 | optional host fixture execution steps; no test run is unbounded |
| request/retained bytes | 262,144 | request, library snapshot and retained proposal evidence |

Library admission additionally applies the normal `Limits` supplied to
`Session::prepare` and `Request` supplied to `acceptance::check`; the smaller
remaining shared counter wins. Over-budget results are `Exhausted`, never
truncation or partial publication. The service reports candidate count,
expanded node count, dependency and instantiation work, checking work,
execution steps, total monotonic latency and retained bytes.

### Mutation and freshness controls

- The library snapshot and `library_revision` are immutable for one request;
  generation changes make the request stale and return `Invalid`.
- The service recomputes the canonical envelope from the retained body. A
  changed body, capture list, dependency identity, instantiation, source or
  semantic revision, forged `chunk_id`/digest, wrong ordered interface or
  effect widening is `Invalid` or `Unsupported` at the exact field.
- Every chunk and dependency is re-prepared and sent through normal
  `acceptance::check`; matching signatures do not transfer evidence or permit
  substitution into an existing Program. Stale definitions and revisions are
  `Invalid`; dynamic, recursive, live, resource, reflection and effectful
  constructs are `Unsupported`.
- The completion proposal is fresh source/candidate data. Existing prepared
  Programs and editor input remain unchanged; no `Session::commit` is performed
  by completion. `check_signed_structure` is never used as a producer authority.
- A malformed request, editor tree, node reference or envelope is `Invalid`;
  an out-of-profile form is `Unsupported`; an exhausted counter is
  `Exhausted`; exhausted-independent absence of an accepted proposal is
  `NoResult`. These classes are not collapsed.

## Fixtures and verification plan

Future implementation work must include the following host-retained fixtures.
They are expected behavior checks, not proofs:

1. Primitive literal/stack-routing chunks and fixed `I64` chunks, with one
   completion using two chunks and one nested quotation where the quotation
   stays within depth 8.
2. Wrapping-I64 values `i64::MIN`, `-1`, `0`, `1` and `i64::MAX`, explicit
   subtraction operand-order cases, and `Bool(false)`/`Bool(true)` cases.
   Expected values come from the independent fixture set, not the producer.
3. Fresh instantiation at two uses of one polymorphic recipe, one immutable
   scalar capture, full stack-tail constraints, and two equal-signature chunks
   with deliberately different behavior that remain distinct.
4. Mutation controls for body, capture, dependency, source/semantic revision,
   ordered interface, effect widening and forged metadata; unsupported
   constructs; malformed one-hole proposals; and each budget boundary.
5. Four outcome controls: malformed request (`Invalid`), dynamic/live recipe
   (`Unsupported`), 129th candidate or 257th evaluator step (`Exhausted`),
   and a finite search in which every proposal is rejected (`NoResult`).

The implementation must report fixture agreement separately from any model
proof or behavioral-contract claim. No benchmark result is implied here.

## Acceptance mapping

| Issue #73 criterion | Design location |
|---|---|
| whitelist, envelope and deterministic API | TCHUNK-01, TCHUNK-02 |
| primitive/fixed examples, two chunks, nested quotation | TCHUNK-04; fixtures 1 |
| wrapping-I64, operand order and Bool results | TCHUNK-04; fixtures 2 |
| mutation, stale, interface, effect, forged and unsupported controls | TCHUNK-03, TCHUNK-04; fixtures 3–4 |
| fresh instantiation and stack-tail constraints | TCHUNK-02, TCHUNK-04; fixture 3 |
| distinct invalid/unsupported/exhausted/no-result | TCHUNK-03; fixture 5 |
| measured enumeration/checking/latency/expansion/retention | TCHUNK-03 measurement table |

## Proof obligations

This package proves none of the above. Future implementation must separately
show that source preparation and `acceptance::check` are invoked for each final
candidate, that counters are shared rather than reset by expansion, and that
reported fixture agreement is not promoted to behavioral equivalence or a
universal correctness claim. Any Lean theorem, finite Rust comparison and
executed-artifact correspondence must be labelled as its own obligation.

## Non-claims

No learned ranking or discovery, speedup, completeness of enumeration,
termination beyond the stated finite budgets, semantic equivalence of
same-signature chunks, authority from a hash/index/signature, proof that an
accepted chunk is suitable for substitution into an existing Program, runtime
execution in production, or #74 progress is claimed.
