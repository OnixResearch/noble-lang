# Editor-only incomplete syntax transport

## MODIFIED Requirements

### Requirement: DX-HOLE-01
r[DX-HOLE-01]

Editor holes MUST remain incomplete syntax, not executable values or trusted witnesses. Analysis MUST report known input/output stack constraints and effect constraints, including unresolved variables. Every admission path MUST reject a candidate containing a hole. Analysis MUST NOT execute the candidate body.

The selected `Editor-Draft` profile transports incomplete syntax only as versioned `noble-editor/v1` structured nodes for signed I64 integer, Boolean, single source word, nested quotation and hole. Its JSON envelope and typed AST are editor-only; hole punctuation is NOT selected for executable `.noble` source, and a hole MUST NOT be represented as a kernel node or executable candidate. Transport MUST reject unsupported versions, missing/unknown/duplicate object fields, incorrect JSON types, invalid word encoding, trailing data and exceeded byte/node/depth/work budgets before admission. Rendering a hole-free AST MUST produce single lexically valid source tokens for ordinary parse, source preparation, independent kernel acceptance and compiled managed-Wasm execution, rather than use a second trusted backend.

Editor analysis MUST use the same resolved source namespace and operator scheme constraints as ordinary source inference. At each hole it MUST preserve the incoming stack, the constrained yet unresolved outgoing stack and the unknown possible effects, not default the hole to a pure/empty operation. The finite `1, hole, +` witness MUST report the `I64 I64` required outgoing suffix induced by the actual `+` scheme, and the minimal additional `I64` suffix conditional on retaining the known incoming `I64`, while retaining unresolved stack/effect variables. This conditional suffix MUST NOT assert that the hole preserves or does not consume its input. Direct, nested-quotation and serialized hole-bearing admission MUST reject before an accepted program, guest/host requests or protected operations. The complete editor tree `1, 2, +` MUST remain admissible to the ordinary compiled path and produce `I64(3)`.

These selected finite observations neither execute a hole nor establish universal frontend, backend, host or proof refinement; all nonselected editor syntax and application authoring tools remain open.

#### Scenario: DX-02 editor analysis and admission

- GIVEN DX-02's exact structured nodes, four variants, bounded versioned transport, a hole-free positive control and hostile malformed/collision/oversized controls
- WHEN editor analysis, direct and nested admission, serialized JSON ingress and ordinary source/kernel/Wasm admission run against their exact inputs
- THEN source-derived hole stack and unresolved effect constraints are reported without execution, every hole-bearing admission rejects before accepted candidate or guest/host requests, and the hole-free positive control executes to `I64(3)`; neither transcript nor status substitutes for source-bound observation
