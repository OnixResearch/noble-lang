## Why

MC1 is the selected next milestone after checker feasibility. There is no contract frontend or application proof path. Kernel typing evidence alone cannot establish behavioral claims for an accepted program.

## What Changes

- Add optional, versioned pure contract declarations with resolved input/output snapshots, typed scalar and structural predicates, explicit quantified builder parameters, and exact logical-definition references: VC-SCOPE-02, VC-INPUT-01, VC-INPUT-02, VC-INPUT-04.
- Export actual kernel-accepted programs and complete typed claims into Lean obligations; retain the Rust extraction/refinement path separately from application proof acceptance: VC-INPUT-03, VT-CONTRACT-01.
- Provide strict Lean primitive, sequencing, quotation, structural/branch and builder-family rules, including wrapping increment and composed increments: VC-LIB-01, VC-LIB-02.
- Select an experimental grammar, IR, companion boundary and finite rule/API design. Implement verify and explain-proof CLI outcomes without claiming MC2 Wasm companion or runtime admission conformance: r[VC-GATE-01].

## Impact

- **Files**: new `crates/noble-contracts`, existing `crates/noble-cli`, workspace/policy records, `proofs/mc1`, verification controls, canonical program-contract specifications and generated views, conformance/evidence ledgers.
- **Testing**: real CLI positive and rejection scenarios; Lean strict proofs and axiom/type checks; actual Charon/Aeneas extraction and correspondence checks; existing kernel/CLI, quality and specification gates. No test result substitutes for a theorem. MC2 requirements remain open.
