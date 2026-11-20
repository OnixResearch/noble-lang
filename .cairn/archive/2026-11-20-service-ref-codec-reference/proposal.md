## Why

The dataspace admits exactly one closed Preserves record, \`<service "NAME" #t>\`
or \`<service "NAME" #f>\`, and its Rust encoder and decoder in
\`crates/noble-kernel/src/dataspace/wire.rs\` are covered by finite tests only. No
kernel-checked reference names the admitted value type, the byte grammar, the
name alphabet, the wire and name budgets, the configured depth or the decision
precedence, so boundary behaviour and canonicality are pinned by whatever the
codec currently does. Ordinary JSON has several valid byte representations, so a
canonicality claim is only meaningful for a closed schema like this one. This
change is that reference layer plus an honest differential comparison (issue
#48); it does not invent a new artifact format or replace the frontend.

## What Changes

- \`proofs/foundation/NobleFoundation/ServiceWire.lean\` specifies the admitted
  value, the exact byte grammar, the name alphabet, the 64-byte wire budget, the
  32-byte name budget and the \`1..=4\` configured depth, and proves the named
  theorems: value round trip, canonicality, full-input consumption, the exact
  lengths and the decision precedence. r[VC-REF-01]
- \`verification/service-ref/cases.mjs\` is an independent decoder written from
  the Lean model, and \`crates/noble-kernel/tests/service_ref_differential.rs\`
  compares the Rust codec against its generated cases: alphabet edges, name
  lengths 0/1/32/33, wire lengths 64/65, configured depths 0/1/4/5, truncation at
  every offset, appended bytes, one-byte substitutions and seeded generated
  values. r[VC-REF-02]
- The receipts \`verification/foundation/receipts/b25314b1/\` (234 theorems,
  clean source, axiom sets inside the allow-list) and
  \`verification/service-ref/differential.json\` (269 cases, one documented
  divergence) bind the model, the toolchain and the tested implementation.
  r[VC-REF-03]

## Impact

- **Files**: \`proofs/foundation/NobleFoundation/ServiceWire.lean\`,
  \`proofs/foundation/NobleFoundation.lean\`, \`proofs/foundation/inventory.json\`,
  \`verification/service-ref/{cases.mjs,cases.json,cases.txt,differential.mjs,differential.json}\`,
  \`crates/noble-kernel/tests/service_ref_differential.rs\`.
- **Not claimed**: no extraction or refinement proof relates the Rust functions
  to the model; the differential is finite evidence on one host and target. The
  model counts opening brackets for its depth scan, which is stricter than the
  Rust scanner's saturating decrement, and the divergence case records exactly
  where. No whole-parser security, zero-copy performance or whole-compiler claim
  follows.
