## Context

Core source `Session::prepare` parses, resolves and infers before emitting a kernel candidate; the kernel has only Literal, Invocation and Quotation. `inference::Term::Hole` is an internal unification variable, not editor syntax. The untrusted CLI JSON reader needs duplicate-key rejection before its values can identify editor node kinds. DX-02 names analysis, direct admission, nested quotation admission and serialized candidate admission. r[DX-HOLE-01]

## Decisions

### Decision: Editor-only versioned AST, never executable holes

**Choice:** The transport has exact keys `{format:1,nodes:[...]}` and tagged integer/boolean/word/quotation/hole nodes. Integer is signed I64, quotation owns nested nodes, and source words must classify as actual single source words. Byte, node, depth and work limits are checked before building an editor tree. Duplicate or unknown JSON keys, wrong types and unsupported versions reject before source preparation. No kernel Hole constructor and no `.noble` punctuation are added. r[DX-HOLE-01]

**Rationale:** The editor may preserve incomplete syntax without entrusting a frontend-only type variable to executable acceptance. Hole-free syntax lexically encodes into ordinary source and is re-parsed and independently checked before compiled Wasm execution.

### Decision: Reuse source inference and preserve unknown effects

**Choice:** A narrowly scoped EditorAnalysis mode shares the actual namespace resolution and inference traversal. At a hole, record its incoming stack, a fresh unresolved output-stack term and a fresh effect hole; subsequent operations unify the output against their real environment word schemes. Report the solved symbolic stack shapes and the structural known/unknown effect formula, without equating hole effects to empty or constructing a kernel candidate. r[DX-HOLE-01]

**Rationale:** `+`'s actual scheme forces an `I64 I64` suffix after the hole; the recorded effect remains unresolved even in a host-free environment. This is a constrained partial interface, not a witness or execution.

## Risks / Trade-offs

- The selected AST intentionally supports a bounded core subset, not all source literals/modules. Unsupported forms reject rather than silently acquiring alternate semantics.
- The diagnostic renderer bounds its text length; truncated descriptions remain diagnostic and confer no admission authority.
- Compiler/backend correctness remains an explicit assumption; tests do not discharge general Rust refinement, host trust or owner-law proof.
