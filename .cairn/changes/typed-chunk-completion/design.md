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

`Library::new(revision)` stores the host-supplied 32-byte snapshot generation;
it does not derive a digest from the entries or make that revision an admission
credential. The host MUST issue a fresh revision and build a fresh library
snapshot whenever its contents change, and MUST keep a published snapshot
unchanged while requests use that revision. Every request carries the exact
generation pin, and a mismatch is `Invalid` before enumeration. This leaves
revision assignment with the host rather than duplicating the separate
content-addressed procedure-library design.

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
exact value-variable bindings; each binding records variable id, kind and
target (`I64` or `Bool`). Stack/effect-variable witnesses are unsupported in
this scalar profile. The binding list is retained, not regenerated from a
signature.

`RecipeNode` is one of `I64Literal(i64)`, `BoolLiteral(bool)`,
`Capture { index: u16 }`, `Operation { op, instantiation: u16 }`,
`Invoke { dependency: u16, instantiation: u16 }`, or `Quotation { body:
[NodeId], instantiation: u16 }`. Node IDs are unique `u32` values; a recipe is
canonical only when every reference is to an earlier node, entry is nonempty
and every quotation body is finite. `Operation` is a closed enum, never an
untrusted source spelling. Its witness is checked against the concrete ordered
stack before the emitted word is independently prepared and kernel-checked.
`SignedSelf`, `SlotInvoke`, text/unit literals, live references, reflection,
dynamic lookup and unresolved calls are outside the profile. The dependency
index is into the retained, resolved dependency list and MUST match its full
`DefinitionRef`.

The semantic whitelist is pure ordered-stack routing plus these resolved
operations: `I64` and `Bool` literals and immutable scalar captures; `dup`,
`drop` and `swap` over `I64`/`Bool`; `+`, `-`, `*` and `=` over `I64`; and finite
quotations executed only by an explicit `run` recipe. This is the exact
bootstrap stack-word surface that the current source resolver can re-admit.
The originally proposed `over`, I64 `<`, and Bool `not`/`and`/`or` are not
stack words in this worktree: `over` has no bootstrap definition, `<` is only
available in the declared-text path, and the Boolean operations belong to the
predicate grammar. Adding those words would require changing the source/kernel
language contract, not merely widening this opt-in library, so this slice
refuses them rather than weakening the normal `Session::prepare` boundary.
The source spelling is resolved in the selected namespace; a spelling alone
does not admit a node. Every operation and every transitive dependency MUST
derive `EffSet::empty`. The profile permits no host capability, resource,
text, unit, syntax, live, reflection or dynamic dispatch. A dependency index
MUST refer to an earlier library entry, making the retained dependency graph
acyclic. Quotation nesting is bounded at 8.

#### Implementation adjustment: closed operation leaves and concrete witnesses

The original recipe-node list had no leaf capable of representing a primitive
stack word: `Invoke` only names an earlier retained library entry, so seeding a
primitive through `Invoke` would require an already-retained primitive chunk.
The implementation therefore adds the closed `Operation { op, instantiation }`
leaf above. The operation enum maps only to the bootstrap spellings in the
fixed whitelist; it adds no parser syntax or dynamic name lookup. Its retained
value-variable witness is checked against the replayed ordered stack, while
ordinary source preparation and `acceptance::check` remain the acceptance
authority.

The envelope stores exact `I64`/`Bool` stack interfaces rather than open
schemes, so a single retained entry cannot be instantiated at multiple type
interfaces. Fresh-instantiation evidence is therefore a single source recipe
(`dup`) admitted and rechecked under both concrete value types, with distinct
retained witnesses and identities; it is not evidence for polymorphic chunk
substitution.

#### Implementation adjustment: shared checking-work accounting

The first implementation charged source bytes while passing a fresh compiler
work allowance to every admission. That did not make the design's 4,096
checking-work counter cumulative across source-aware editor analysis, source
preparation and independent kernel rechecks. The implementation passes the
remaining ledger allowance to `editor::Candidate::analyze_editor` and
`Session::prepare`, and debits their reported consumed work. Ordinary and
signed-owner source-path kernel checks report their consumed work, which the
source meter charges instead of reserving twice the declared checker
allowance. A kernel request-preflight refusal conservatively charges its full
declared allowance, because that path does not expose its local remaining
counter. Each independent chunk recheck then receives the ledger's new
remainder and debits its reported work; ordered-stack task replay debits the
same ledger, keeping the fixed 4,096 maximum cumulative so no chunk or checker
boundary can restart it. The
reported unit is a bounded checker-work charge, not a CPU-time or
instruction-count measurement.

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
| checking work | 4,096 | shared compiler-meter charges (including its kernel-work reservation), independent kernel recheck work and ordered-stack replay steps |
| evaluation steps | 256 | optional host fixture execution steps; no test run is unbounded |
| request/retained bytes | 262,144 | request, library snapshot and retained proposal evidence |

Diagnostics retain only the first refusal for each `(class, exact location)`;
repeated refusals still increment candidate and rejection counts but consume no
additional diagnostic or retained-byte entries. The two independent limits
otherwise conflict: storing a diagnostic for each refused proposal would
exhaust the 64-entry diagnostic budget before a high-count candidate-budget
control could run, making the candidate counter's own boundary untestable.

The 128-candidate value is an upper bound, not a guaranteed stopping point:
every selected chunk is independently re-admitted through source preparation
and kernel acceptance, and its work shares the 4,096 checking-work limit. A
request can therefore exhaust checking work before reaching 128 candidates. The
integration control lowers only the candidate budget to isolate that counter;
it does not claim that a default-budget search can always reach candidate 128.
The no-result witness uses one literal chunk repeated five times, so its five
bounded proposals fit within the shared work limit while a valid completion
still exists outside the four-chunk search.

Library admission additionally applies the normal `Limits` supplied to
`Session::prepare` and `Request` supplied to the independent kernel check. For
completion, each stage receives only the remaining shared `CheckingWork`
allowance; its reported work is charged before the next admission stage or
candidate begins. The compiler meter's existing reservation for its internal
kernel check is included, and the independent kernel recheck reports its
consumed work. Over-budget results are `Exhausted`, never truncation or partial
publication. The service reports candidate count, expanded node count,
dependency and instantiation work, checking work, execution steps, total
monotonic latency and retained bytes. This is bounded checker-work accounting,
not a CPU-time or instruction-count measurement.

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

The implementation test target retains the following host-supplied fixtures.
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
obligation.

## Addenda decisions and #74 exclusion

The Re-Pair/Sequitur discovery report proposed in the 10-31 issue comment is a
separate follow-up, not part of this fixed baseline. Reconsider it only after
all Phase 2 acceptance controls and the retained baseline measurement are
complete. That follow-up must remain report-only, exclude the synthetic stress
inputs, report per-module candidates and refusal reasons, and independently
prepare/check each rewritten candidate. A recurring candidate must show
positive token savings on checked real source before it is proposed for the
fixed library; no source rewrite or library adoption follows from frequency
alone.

Agent-cost word scoring from the 10-31 addendum is deferred. It requires a
separate LLM agent harness, a pinned model/sampling setup, a frozen train/held-out
task split and an explicit model/compute budget. Revisit only if the report-only
miner surfaces checked candidates worth evaluating. Any later claim of a useful
word must lower median held-out task cost without reducing pass rate, across
multiple seeds, and report variance; this slice ran no model task evaluation.

#74's learned ranking, learned discovery and leakage evaluation remain outside
this package. This implementation reports neither #74 progress nor a ranking
signal; #74 stays dependent on this completed fixed baseline.

## Non-claims

No learned ranking or discovery, speedup, completeness of enumeration,
termination beyond the stated finite budgets, semantic equivalence of
same-signature chunks, authority from a hash/index/signature, proof that an
accepted chunk is suitable for substitution into an existing Program, runtime
execution in production, or #74 progress is claimed.
