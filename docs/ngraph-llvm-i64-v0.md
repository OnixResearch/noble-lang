# Opt-in total-I64 graph-to-LLVM lowering, `ngraph-llvm-i64/0`

Status: **opt-in experiment** (issue #107). The selected production backend is unchanged; see
[Production backend](#production-backend-unchanged).

This document fixes the ABI, the operation inventory, the emitted text form, the refusal
diagnostics, the observation relation and the trusted components of one lowering edge: an admitted
`ngraph-i64/0` graph becomes **one** straight-line, memory-free, call-free LLVM IR function, and an
independent validator reads the emitted bytes back against the admitted graph. It is the document
the code and the Lean model are reviewed against:

| Unit | Path |
|---|---|
| lowering, parser, validator, concrete interpreter | [`crates/noble-ngraph/src/llvm.rs`](../crates/noble-ngraph/src/llvm.rs) |
| semantic model and `validate_sound` | [`proofs/foundation/NobleFoundation/LlvmLowering.lean`](../proofs/foundation/NobleFoundation/LlvmLowering.lean) |
| kernel-checked corpus generated from the real artifacts | [`proofs/foundation/NobleFoundation/LlvmLoweringCorpus.lean`](../proofs/foundation/NobleFoundation/LlvmLoweringCorpus.lean) |
| differential tests, controls, corpus exporter | [`crates/noble-ngraph/tests/cert/llvm/`](../crates/noble-ngraph/tests/cert/llvm/) |
| retained bundle, compiler-lab request, fresh-process replay | [`verification/llvm-lowering/`](../verification/llvm-lowering) |

Bound revisions: `ngraph-ir/0` ([`ir.rs`](../crates/noble-ngraph/src/ir.rs)), `ngraph-sem/0`,
`ngraph-facts/0`, and the lowering profile `ngraph-llvm-i64/0` itself. The graph meaning is the one
the certificate lane already uses: `NobleFoundation.PackedSchedule.value`, restricted to input,
exact `i64` constant, wrapping add/sub/mul and one selected output.

## Profile name and interface

`ngraph-llvm-i64/0` numbers its lowering separate from the graph IR: the artifact is target-specific
text, so it cannot carry `ngraph-ir/0` alone.

| Interface fact | Value |
|---|---|
| function | `define i64 @noble_i64(i64 noundef %a0, i64 noundef %a1)` |
| parameters | exactly **two**, `i64`, each carrying `noundef` and nothing else; names `%a0`, `%a1` positionally |
| return | exactly one `i64`, spelled by one `ret` |
| calling convention | `ccc`, which is the default and is therefore **absent** from the text; any other convention or `cc <n>` is refused |
| function attributes | none (no `#0`, no attribute group, no alignment) |
| target triple | `x86_64-unknown-linux-gnu` |
| data layout | `e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128` |
| memory, calls, globals, metadata | none; no declaration, `undef`, `poison` or intrinsic |

Two parameters and one output, not "any arity": the only consumer of these modules is compiler-lab's
`llvm-pair-v1` lane, which reads the two-`i64`-to-`i64` shape only
([`llvm_pair.rs`](https://github.com/brittonr/compiler-lab/blob/407ce5eded2bc428630a3b196c380f8ac614627a/src/validation/llvm_pair.rs)
`shape_reasons`). A wider interface would widen the ABI claim without a consumer.

`noundef` is part of the obligation, not decoration: the Lean statement quantifies over every pair
of concrete `BitVec 64` inputs, which is the `all-u64-pairs` domain compiler-lab checks. Nothing the
function computes can be `undef` or `poison`, because the profile has no symbolic value, no flag, no
division, no shift and no memory.

## Operation inventory

The lowering admits the graph's whole node table; it never drops an unsupported node to make a graph
eligible.

| `ngraph-ir/0` node | emitted text | note |
|---|---|---|
| `input k`, `k ∈ {0,1}` | `%a<k>` | the parameter is the value; no instruction is emitted |
| `const i64 v` | `<v>` | canonical signed decimal, exact 64 bits; no instruction is emitted |
| `i64.add a b` | `  %v<i> = add i64 <a>, <b>` | operand order preserved, no flag |
| `i64.sub a b` | `  %v<i> = sub i64 <a>, <b>` | operand order preserved, no flag |
| `i64.mul a b` | `  %v<i> = mul i64 <a>, <b>` | operand order preserved, no flag |
| the single output | `  ret i64 <v>` | one return |

`<i>` is the node's canonical index, so each arithmetic node is computed **once** and a shared
subexpression is a single `%v<i>` reused by every consumer. An input is the parameter name and a
constant is its literal, both of which LLVM already treats as values, so sharing a constant
recomputes nothing.

Ordinary `add`/`sub`/`mul` on `i64` are two's-complement wrapping modulo `2^64`; `nsw` and `nuw`
would make overflow immediate undefined behaviour, which would change the meaning the graph's
`K-NUM-01` semantics fixes, so this profile never emits them.

## Emitted text form

```
; ngraph-llvm-i64/0 module: one straight-line function, no memory, no calls
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

define i64 @noble_i64(i64 noundef %a0, i64 noundef %a1) {
entry:
  %v2 = add i64 %a0, %a1
  %v3 = mul i64 %v2, -3
  ret i64 %v3
}
```

Node order is canonical node order, spacing and the signed-decimal spelling are canonical, and the
body holds one line per arithmetic node followed by exactly one `ret`. The text is deterministic:
the same admitted graph emits the same bytes.

## Refusal diagnostics

Admission refusals are raised by `admit_graph`/`admit`; module refusals by `parse`/`verify`. The
number is the stable name in `Refusal::name` and the string a report records.
`admit_graph` screens only profile exclusions and does not establish topology or general graph
well-formedness. Emission takes a `CheckedGraph` from `check::check` through `admit`.

| Name | Refused input |
|---|---|
| `L-PROFILE` | a graph whose sub-profile is not `ngraph-i64/0` (an `F32` graph has no image here) |
| `L-INTERFACE` | an interface that is not exactly two `I64` `Input` slots; a module whose `define` line is not the pinned signature (return type, name, parameter names or types, attributes, a named calling convention) |
| `L-PARAM` | a `Param` slot or `param` node (only `ngraph-f32/0` has them) |
| `L-SHAPE` | an interface or node shape containing a non-scalar (compound) shape |
| `L-FACT` | a graph carrying a fact record |
| `L-GUARD` | a `guard` root |
| `L-EQUALITY` | an `i64.eq` node, which yields `Bool` and has no `I64` instruction |
| `L-OPERATION` | a node outside the inventory above, including every `F32` operation |
| `L-OUTPUT` | a graph that does not select exactly one output, or a `ret` operand that does not denote the selected output |
| `L-HEADER` | a text whose first line is not this profile's header |
| `L-TARGET` | absent, repeated or different target lines, or a noncanonical module-preamble line/position |
| `L-INSTRUCTION` | a body line that is not `  %v<n> = <op> i64 <a>, <b>` (including a `call` without a result assignment), or destinations that do not strictly increase |
| `L-OPCODE` | an opcode other than `add`, `sub`, `mul` in an otherwise well-formed arithmetic instruction (for example `xor` or `and`) |
| `L-FLAGS` | `nsw`, `nuw`, `exact`, `samesign`, `disjoint` or `nneg` anywhere on an instruction |
| `L-OPERAND` | an operand that is not `%a<0|1>`, `%v<n>` or a canonical signed `i64` literal |
| `L-RETURN` | no `ret` before the closing brace, a second `ret`, or text after the brace |
| `L-NODEMAP` | an instruction whose destination is not the arithmetic node at its position, an arithmetic node with no instruction, or an instruction for a parameter or a literal |
| `L-OPERANDS` | an operand that does not denote the node the admitted graph supplies at that position |

## Independent validator

`parse` tokenizes the bytes a caller supplies and checks the header, the two `target` lines, the
`define` signature, the `entry:` label, each instruction line's grammar, opcode, absence of flags and
operand spelling, strictly increasing destinations, exactly one `ret`, and the closing brace.
`verify` then maps each parsed instruction onto the admitted graph: the instruction at position `p`
must name the `p`-th arithmetic node in canonical order with that node's opcode, its operands must
denote that node's operand nodes, and the `ret` operand must denote the selected output. An operand
denotes a node exactly when a parameter names an `input` node's slot, a literal is that `constant`
node's exact bits, or `%v<n>` is the arithmetic node `n` itself.

The validator never calls the emitter. Comparing the bytes of a second emission would be an identity
check between two copies of the same code and would say nothing about the graph; the binding here is
structural on the parsed form and semantic through the theorem below.

## Observation relation

Three statements, kept apart on purpose. None of them substitutes for another.

1. **Semantic validation (Lean, universal).** The Lean model of the validator is a `Bool` function
   over the parsed instruction list and the `ret` operand. `LlvmLowering.validate_sound` states: for
   every well-formed graph, every instruction list and every `ret` operand the model accepts, the
   executed function returns the graph reference's value of the selected output **for every pair of
   `BitVec 64` inputs**. Allowed axioms are `propext`, `Classical.choice` and `Quot.sound`; no
   `bv_decide`, no `native_decide`, no `sorry`.
2. **Concrete agreement (finite).** `execute` runs the parsed artifact's own instructions, and the
   retained executions record the value it produces at named inputs next to the graph reference's
   value and the Lean kernel's evaluation of the model at the same inputs. This is agreement at
   finitely many inputs, not a proof.
3. **Artifact validation (bytes).** The retained module bytes are accepted by the Rust validator for
   the retained graph, and a fresh emission from the retained graph text reproduces them byte for
   byte. The corpus the kernel checks is generated from the *parser's* view of these bytes, not from
   the emitter's view.

Trusted, and listed so the reader can price it:

- **T1 — the tokenizer.** The Lean theorem quantifies over the parsed instruction list; the step from
  bytes to that list is `parse`, and the correspondence between `parse` and the Lean model's reading
  of an instruction is assumed, not proved. The corpus narrows it: every retained artifact's parsed
  form is what the kernel evaluates.
- **T2 — the checker.** `value`'s well-formedness premise (every operand strictly earlier) comes from
  `check`, and the node-inventory constraints (slots `< 2`, only `input`/`const`/arith nodes) come
  from `admit_graph`. The implementation-to-model correspondence of the Rust checker is the open
  NG-PO-B obligation, not a result of this lane.
- **T3 — the target.** The data layout and triple are the `x86_64-unknown-linux-gnu` values LLVM 18
  and later report, recorded by inspection. No pinned LLVM exists on this host, so compiler-lab owns
  confirming them against the LLVM it builds the pair with.
- **T4 — Lean.** The kernel, the pinned toolchain and Mathlib's downloaded oleans; the foundation
  lane's receipt records the exact source and tool identities.

## Stage list

The compiler-lab report distinguishes the stages; a stage that is unknown, unsupported or timed out
inherits no other stage's evidence.

| Stage | Owner | This lane |
|---|---|---|
| graph rewrite | #42 (`cert::emit`/`cert::check`) | retains the original and rewritten endpoints and their certificate verdict; the implementation-to-Lean correspondence of the rewrite stays open |
| graph → LLVM lowering | this document, issue #107 | emits, validates, and states the universal claim |
| LLVM optimisation (LLVM may fold `add i64 3, 5`) | compiler-lab #124 | **not run here**: reported unknown |
| native artifact and performance | compiler-lab #124 / #135 | **not run here**: reported unknown |

The compiler-lab request is
[`verification/llvm-lowering/request.json`](../verification/llvm-lowering/request.json): `schema 1`,
`kind compiler-lab-llvm-pair-request` (which selects the consumer's `llvm-pair-v1` profile),
relation `equivalence` in both
directions, `arguments ["i64","i64"]`, `result "i64"`, `calling_convention "ccc"`,
`attributes []`, `input_domain "all-u64-pairs"`, `poison_policy "exclude"`,
`permitted_externals []`. The attributes `llvm-pair-v1` admits are `nounwind`, `noundef`, `signext`
and `zeroext`, and the profile writes **only** `noundef` on the two parameters (**T3**-adjacent
assumption A2: admission was checked against the admitted-attribute list of the reviewed
implementation; the lane itself was not run here).

`verification/llvm-lowering/bundle.json` retains these assumptions, the exact revision labels, and
path-bound identities for the Rust profile/parser/validator, graph checker and semantics, graph
formatter, rewrite checker, test identity implementation, corpus generator, fresh-process replayer
and Lean model. The generated corpus is bound separately by its artifact identity. `replay.mjs`
re-reads the implementation source files in a fresh process and checks their identities along with
the retained graphs, modules, certificate, request and corpus. These use the deterministic
test-only `TestIds` scheme, not a cryptographic digest or an authentication mechanism; the source
bindings are replay/freshness evidence only.

## Production backend unchanged

Item 7 of the issue is satisfied by construction, and stated explicitly: nothing in this lane is
reachable from the selected production backend. `crates/noble-ngraph/src/compiled.rs`
(`ngraph-layered/0`) and `crates/noble-ngraph/src/schedule.rs` are untouched, no call site selects
the lowering, and the CLI has no flag for it. A later adoption at run time — for example making an
executor select an emitted module — needs its own decision record with its own evidence, including
native artifact validation and performance measurement, and it cannot cite this document's semantic
claim as covering the LLVM optimiser or the machine code.

## Non-claims

- Not a correspondence theorem between the Rust emitter, the Rust parser or the Rust checker and the
  Lean model. The universal statement is about the Lean model of the parsed form (**T1**, **T2**).
- Not a statement about the LLVM optimiser, the backend, the generated machine code or any native
  artifact. Only compiler-lab's stages can say anything there, and only for the pairs it reads.
- Not a statement about the graph rewriter: the certificate's implementation-to-Lean correspondence
  stays open, and the retained pair inherits #42's premises, not its conclusion.
- Not a performance result. No measurement of lowering cost, of LLVM's optimisation or of runtime is
  claimed here.
