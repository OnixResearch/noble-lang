## Selected model

Raw wire bytes are \`List Nat\` values below 256. The admitted value is a name
(nonempty, at most 32 bytes of ASCII alphanumerics, \`_\` or \`-\`) and a readiness
tag. Encoding is a function: ten prefix bytes \`<service "\`, the name, then the
five-byte suffix \`" #t>\` or \`" #f>\`. A canonical encoding is therefore fifteen
bytes longer than its name, and the longest admitted value is 47 bytes inside the
64-byte budget.

## Decoder shape

The decoder is written as a consuming parser: it matches the prefix byte by byte,
collects name bytes up to the closing quote, and requires the readiness suffix to
consume the rest of the input. Full-input consumption is therefore structural
rather than a checked postcondition, and the accept branch additionally checks the
canonical bytes, so canonicality follows from the same construction. Decision
precedence mirrors the Rust decoder: byte budget, configured depth, depth scan,
then the closed-schema match, which subsumes the UTF-8 and name-alphabet checks
because every admitted byte is ASCII.

## Proof inventory

\`nameByte_excludes_structure\`, \`recordPrefix_length\`, \`recordSuffix_length\`,
\`encode_length\`, \`openBrackets_cons\`, \`openBrackets_of_name\`,
\`depthExceeds_encode\`, \`takeName_encode\`, \`parseService_encode\`,
\`decode_encode\` (round trip), \`encode_of_accepted\` (canonicality),
\`longest_name_fits_wire_budget\`, and the precedence and boundary theorems
\`byte_limit_precedes_depth_scan\`, \`depth_configuration_precedes_schema\`,
\`depth_scan_precedes_schema\`, \`altered_readiness_is_schema\`,
\`trailing_bytes_are_schema\`, \`empty_name_is_schema\`,
\`foreign_name_byte_is_schema\`, \`overlong_name_is_rejected\`,
\`longest_name_round_trips\`, \`shortest_name_round_trips\`. All are inventoried in
\`proofs/foundation/inventory.json\` and kernel-checked by
\`verification/foundation/check.mjs\`; the receipt is
\`verification/foundation/receipts/b25314b1/\`.

## Correspondence and its open edge

The comparison route is handwritten: \`verification/service-ref/cases.mjs\` is an
independent decoder written from the Lean model, never from the Rust codec under
test, and \`crates/noble-kernel/tests/service_ref_differential.rs\` asserts the Rust
decision for every generated case. The remaining Rust-to-model edge is open and
labelled: there is no Charon/Aeneas extraction or refinement proof for
\`wire.rs\`, and the differential is finite evidence on one host and target.

The model's depth scan counts opening brackets while the Rust scanner decrements
saturating at zero. Both agree on every admitted record; the case-family
\`divergence-bracket-runs\` records an input where the model rejects by bracket
count and the Rust scanner never reaches the configured depth, and the Rust test
asserts that divergence rather than hiding it.
