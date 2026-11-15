## Context

NGRAPH-FORMAT-01's exact-decimal and exact-bits paths are normative and already implemented. The
binary32 literal table in the root README previously described RNE decimal import as proposed.
This package changes only the normative NGRAPH-FORMAT-01 contract. It neither claims that the
Rust implementation refines the mathematical model nor records a passing execution or proof.

## Decisions

### Decision: opt-in, per-literal RNE under a new file mode

**Choice:** A graph, vector or training-data file may declare
`literal-modes noble-graph-literal-modes/v2` after its schema line. Only a literal explicitly
prefixed with `rne:` uses round-to-nearest, ties-to-even; all unmarked decimals retain exact-only
semantics. Canonical output uses exact bit encodings.

**Rationale:** Existing exact syntax and refusal diagnostics are compatibility constraints. A
per-file opt-in would allow accidental RNE on a single value, while an implicit cast would make
policy depend on context. Requiring both a versioned declaration and a per-literal marker makes
the rounded operation visible and preserves every unmarked call site.

**Reopen if:** an existing input must acquire rounded semantics without an explicit v2 declaration
or marker, or a canonical format revision changes the graph/value identity contract.

### Decision: reject RNE overflow and bound decimal work

**Choice:** If one exact decimal-to-binary32 RNE rounds to infinity, reject it with
`E-LITERAL-OVERFLOW`. Refuse a decimal payload longer than 4096 bytes with
`E-BUDGET-LITERAL-BYTES`. Keep `nan` and `inf` words refused; encode non-finite values only through
exact bits where the file format permits them.

**Rationale:** The independent graph checker rejects non-finite graph constants. Treating overflow
as an input error is consistent with the checked finite-graph boundary and avoids silently
introducing an infinity through authoring. The fixed token bound caps exact big-integer work without
changing the existing exact path's diagnostic.

**Reopen if:** the checked graph contract intentionally admits non-finite constants, or measured
requirements justify a separately reviewed resource policy.

### Decision: explicit scalar adapter signatures

**Choice:** Provide `i64_to_f32_exact`, `i64_to_f32_rne`, `f32_to_i64_exact`, and
`f32_to_i64_trunc`. Exact I64-to-F32 rejects discarded integer bits. F32-to-I64 rejects non-finite
values, checks the signed I64 range after conversion, and the exact variant also rejects a
fractional value. Truncation is toward zero. Either F32 signed zero maps to I64 zero. No implicit
coercion, saturation, or wrapping is permitted.

**Rationale:** The two destination domains have different failure modes; naming exactness and
rounding in the function makes each information-losing boundary explicit. Checked integer logic
avoids host-cast saturation and implementation-dependent overflow behavior.

**Reopen if:** a caller needs another rounding direction or a different result/error type; such an
operation must be separately named and specified rather than weakening these contracts.

### Decision: immutable byte-bound import receipt

**Choice:** `noble ngraph import GRAPH --out RECEIPT` records v2 policy and semantic revision,
source bytes and digest, canonical exact-bit graph and digest, and each RNE spelling, line and
resulting bits. `export` verifies those bindings before writing either the canonical graph or the
original byte-identical source. Import input is capped at 1 MiB and the receipt at 16 MiB; outputs
use create-new publication.

**Rationale:** Canonical bits alone discard the spelling that selected RNE. A self-contained receipt
preserves that provenance while binding it to deterministic output. Bounds and immutable
publication prevent oversized reads and accidental replacement of prior evidence.

**Reopen if:** provenance must cover a different source format or the receipt needs a cryptographic
signer beyond its content digests.

## Risks / Trade-offs

- A BLAKE3 receipt detects accidental inconsistency but is not a signature or authenticated
  attestation; callers must retain the receipt as evidence and must not infer trust in its producer.
- The 1 MiB import bound is intentionally lower than the ordinary graph-file bound so embedded
  source and canonical bytes plus per-literal provenance remain bounded in a single receipt.
- Exactness, decimal interpretation, conversion range and round-to-even claims require their
  separate kernel-checked Lean model and Rust conformance evidence; this spec package itself proves
  none of them.
