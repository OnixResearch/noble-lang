# Artifact correspondence trust labeling

## ADDED Requirements

### Requirement: ACORR-TB-01
r[ACORR-TB-01]

The proof-bound artifact admission envelope MUST be reported as a strengthened trusted-build correspondence: exact final-byte equality with a consumer rebuild by policy-pinned tools. It MUST NOT be reported as translation validation, semantic preservation or a verified backend.

Reports MUST keep artifact identity (matching hashes and bytes) separate from these open obligations:
- lowering preservation (PO-17 slice);
- optimizer preservation;
- loading binding (PO-18 slice);
- guard correspondence including wrapper Wasm (PO-21);
- erasure (PO-20);
- consumer decision refinement (PO-16);
- their composition into a semantic claim about the executed artifact.

Hashes, finite control executions, Lean acceptance of generated statements and extraction success MUST NOT discharge any of them.

Every receipt MUST list its residual assumptions:
- compiler and helpers;
- assembler and optimizer;
- engine and OS loader;
- CLI binary integrity;
- checker soundness;
- observation authenticity;
- worker non-malice;
- the pin file;
- the absence of cross-host provenance.

A new backend MUST supply its own correspondence evidence and MUST NOT inherit Wasm claims.
