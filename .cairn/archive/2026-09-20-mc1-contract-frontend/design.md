## Context

MC1 depends on the delivered bootstrap acceptance checker, not the not-yet-selected Wasm backend. Ordinary Program types and checking remain unchanged. Behavioral proof is a separate optional operation. The entire deterministic contract frontend/exporter remains first-party Rust on the Charon/Aeneas route.

## Decisions

### Decision: Experimental contract container version 1

**Choice:** Use a bounded UTF-8 s-expression contract container with `(contract 1 name ...)`, named ordered `input`/`output` stacks, optional `params`, nonrecursive pure `define` declarations, a `program [ ... ]` body, `requires` and `ensures`. References use explicit `(in name)`, `(out name)`, `(param name)` and `(def name)` forms. Nested program literals have explicit `(block (input types) (output types) [ body ])` interfaces. Scalar/structural predicates have fixed operators and typed operands. Wrap-add/sub/mul use 64-bit bitvectors; ordered comparison uses signed interpretation. Sum/list projections are partial logical terms: an undefined required term cannot prove a Boolean assertion. Quantified parameters and snapshots never supply erased runtime instructions.

**Rationale:** A small experimental grammar fixes binding and stage distinctions without pretending to freeze a full Noble module syntax. All scans, nodes, nesting and work have finite consumer-selected bounds. Unknown versions/operators are unsupported; binding/type errors are invalid. Neither outcome reclassifies an otherwise valid ordinary program.

### Decision: Reuse acceptance and export the retained representation

**Choice:** Elaborate source instructions and explicit block interfaces into existing kernel Candidate/Request values, call kernel acceptance, then derive the application proposition from the retained candidate, ordered interfaces and complete typed expression graph. The output binds exact word identities, literals, quotation bodies, logical definitions and pure semantic revision. Aeneas-generated implementation theorems and application proofs have separate evidence records.

**Rationale:** The final ordinary type authority is the existing checker. A valid proof about an independently rewritten program cannot pass for a mismatched emitted subject. There are no host effects in this MC1 fragment.

### Decision: Pure partial-correctness library

**Choice:** Relational normal-return semantics and strict Lean rules for primitives, sequencing, quotation, invocation, structural eliminators and branch joins. Composition requires the intermediate implication and compatible interfaces/context; family rules universally bind eligible captures and operand behavior. Increment claims use wrap64 and preserve arbitrary stack tails.

**Rationale:** This proves the requested normal-return properties without claiming termination, host availability, allocation success or Wasm correspondence. No test vector or solver exit code is a universal proof.

### Decision: Inert companion representation and finite rule interface

**Choice:** `Contract` is immutable typed IR plus semantic context; `Evidence` is an inert theorem/module reference and full subject/claim/dependency description; `CertifiedProgram` is an abstract pairing produced only by independent acceptance or finite rule replay. Constructors/imports cannot establish acceptance. MC1 selects experimental `contracts-draft/v1` with named rules `primitive`, `sequence`, `quote`, `invoke`, `branch`, `family-instantiate`; every record contains prior premise indices, exact subject and claim indices, and context/assumption references. Records must be bounded, topologically ordered and acyclic. Composition explicitly checks the intermediate implication; absent implication yields an obligation, not certification. Captures must satisfy recursive Data/Capture; no resources, service handles or authority in inert evidence. Inspection/projection is pure and cannot fetch evidence or invoke a prover. Serialization omits acceptance flags and requires revalidation under receiver policy. Exact byte transport and compiled replay implementation are MC2 gates, not claimed here.

**Rationale:** The first-class API construction boundary is fixed without adding dependent Program parameters or a production prover. MC1 host proof reports do not satisfy MC2 companion transport, applicability or Wasm conformance.

### Decision: Consumer-controlled independent proof checking

**Choice:** `noble verify CONTRACT [--emit DIR] [--proof FILE | --refutation FILE] [--timeout-ms N]` and `noble explain-proof CONTRACT`. Regenerate obligations from the independently selected source; user proofs cannot replace them. Compile proof producer source in a bounded isolated process and independently check the expected declaration and its transitive assumptions against the exact claim. Only `propext`, `Classical.choice` and `Quot.sound` are accepted. Distinguish proved, disproved, unknown, timeout, unsupported, error and not-run. A refutation must prove the negation of this exact claim. Missing isolation fails closed.

**Rationale:** Lean elaboration can execute metaprograms. Source strings, names and producer reports are not trust. Explanation and preparation never invoke Lean. No backend release or proof-required Wasm build is reported in MC1.

## Risks / Trade-offs

- The mandatory extraction pipeline can expose unsupported standard-library bodies. Refactor deterministic bodies rather than introduce kernel exceptions or unreviewed opaque success models.
- The existing checker has a scoped correspondence ledger, not complete language metatheory. MC1 must state the inherited assumptions rather than broaden that claim.
- Experimental grammar/encoding is version-scoped; no cross-version portable identity or evidence transport is promised.
- MC2 remains necessary for compiled companion replay, application guards, Wasm transport and artifact admission; MC1 completion must not close those obligations.
