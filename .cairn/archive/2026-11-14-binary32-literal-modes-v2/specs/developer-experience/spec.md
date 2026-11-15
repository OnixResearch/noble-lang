# Binary32 literal modes v2

## MODIFIED Requirements

### Requirement: NGRAPH-FORMAT-01
r[NGRAPH-FORMAT-01]

**NGRAPH-FORMAT-01.**

Graph, vector and training-data files MUST each begin with a schema identifier line: `ngraph-ir/0 <sub-profile>` for graphs, `noble-ngraph-vectors/0` for evaluation vectors and `noble-ngraph-data/0` for training data. A missing or unknown identifier MUST be refused before any further parsing: a graph file without the `ngraph-ir/0` schema line with `E-GRAPH-MALFORMED`, a graph file naming an unknown sub-profile with `E-PROFILE`, and any other file with `E-FORMAT`.

**Graph files.**

- A graph file MUST record the sub-profile, the ordered slots, the nodes in canonical order, the facts with their provenance, the outputs, the input and output shapes, and the root subject kind, identity and closure. `GraphLogicalId` MUST then be recomputed from the parsed records alone.
- Comments, whitespace, file names and paths MUST NOT be identity-bearing. Producer origins are not identity-bearing and MAY be omitted.
- Printing MUST be canonical and deterministic, and every encoding MUST be symmetric between printing and parsing, including `evidence` provenance. Parsing a printed graph MUST reproduce the same graph apart from origins, with the same `GraphLogicalId`, fact records and checker outcome.
- Parsing MUST only build producer data; admission MUST still be decided by the independent graph checker. `F32` constants MUST obey the exact-literal rules of NGRAPH-F32-01, or the explicitly selected v2 rule below.
- An interface shape tree MUST nest at most 127 pairs, the deepest root interface item that source extraction can produce; a deeper tree MUST be refused with `E-GRAPH-MALFORMED` before it is walked.

**Numbers in every file.** Integers MUST be written as ASCII digits, with a leading `-` only on signed values, and bit patterns as `0x` plus hex digits; a leading `+` MUST be refused like any other malformed token. Deciding whether a decimal is exact MUST take time linear in its length.

**Vector and data files.**

- Values MUST be typed by a declared type line. It MUST equal the graph's ordered input slot types and, for data, the loss graph's target slot types; otherwise the file MUST be refused with `E-INTERFACE` before evaluation.
- An `F32` value MUST be given either as its exact binary32 encoding, which MAY be any encoding including NaN and infinities, or as an exactly representable finite decimal. An inexact unmarked decimal MUST be refused with `E-LITERAL-INEXACT`.
- Graph, vector and data files MAY select `noble-graph-literal-modes/v2` with the single line `literal-modes noble-graph-literal-modes/v2` immediately after the schema line. Without that declaration, existing exact-only behavior and diagnostics MUST remain unchanged. In v2, `rne:<decimal>` MUST parse the existing finite decimal grammar as an exact base-ten rational, round that rational exactly once to binary32 using round-to-nearest, ties-to-even, and store the resulting bits. The marker MUST NOT authorize implicit rounding of any unmarked decimal. If rounding produces infinity, the literal MUST be refused with `E-LITERAL-OVERFLOW`; an RNE decimal longer than 4096 bytes MUST be refused with `E-BUDGET-LITERAL-BYTES`; an `rne:` token without the v2 declaration MUST be refused with `E-PROFILE`. NaN and infinity words remain refused; their encodings MAY be supplied with exact bits. Canonical printing MUST emit exact bits rather than the original decimal spelling.
- The scalar boundary API MUST expose only explicit conversions with these signatures and results: `i64_to_f32_exact(i64) -> Result<u32, ConversionError>` refuses discarded integer bits with `Inexact`; `i64_to_f32_rne(i64) -> u32` rounds once to nearest, ties-to-even; `f32_to_i64_exact(u32) -> Result<i64, ConversionError>` requires a finite integral value in `[-2^63, 2^63)`; and `f32_to_i64_trunc(u32) -> Result<i64, ConversionError>` truncates toward zero before checking that range. Both F32-to-I64 functions MUST return `NotFinite` for NaN or infinity and `Overflow` when the resulting integer is outside the range; the exact conversion MUST return `Inexact` for a finite fractional value. Both signed zero encodings MUST convert to integer zero. These operations MUST NOT use saturating or wrapping casts and MUST NOT introduce implicit coercions.
- Parameters MUST NOT be supplied through vector or data files.
- Training-data order MUST be preserved, because the training-data digest binds it.

<!-- cairn:scenario-links:start -->
#### Scenario: NGRAPH-CASE-03 for NGRAPH-FORMAT-01

- GIVEN the `Numeric-Graph-v1` profile and every field of `input` in [NGRAPH-CASE-03](../../../../../specs/conformance/numeric-graph-cases.json)
- WHEN the `admission` procedure for case `NGRAPH-CASE-03` runs against those inputs
- THEN the observations match every field of `expected` in case `NGRAPH-CASE-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: NGRAPH-CASE-04 for NGRAPH-FORMAT-01

- GIVEN the `Numeric-Graph-v1` profile and every field of `input` in [NGRAPH-CASE-04](../../../../../specs/conformance/numeric-graph-cases.json)
- WHEN the `admission` procedure for case `NGRAPH-CASE-04` runs against those inputs
- THEN the observations match every field of `expected` in case `NGRAPH-CASE-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: NGRAPH-CASE-06 for NGRAPH-FORMAT-01

- GIVEN the `Numeric-Graph-v1` profile and every field of `input` in [NGRAPH-CASE-06](../../../../../specs/conformance/numeric-graph-cases.json)
- WHEN the `identity` procedure for case `NGRAPH-CASE-06` runs against those inputs
- THEN the observations match every field of `expected` in case `NGRAPH-CASE-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: NGRAPH-CASE-09 for NGRAPH-FORMAT-01

- GIVEN the `Numeric-Graph-v1` profile and every field of `input` in [NGRAPH-CASE-09](../../../../../specs/conformance/numeric-graph-cases.json)
- WHEN the `static` procedure for case `NGRAPH-CASE-09` runs against those inputs
- THEN the observations match every field of `expected` in case `NGRAPH-CASE-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.
<!-- cairn:scenario-links:end -->

#### Scenario: NGRAPH-CASE-06 identity-preserving graph round trip

- GIVEN every authored, workload, random and extracted graph and text variant specified by `NGRAPH-CASE-06`
- WHEN each graph is printed, parsed and checked again
- THEN the parsed graph MUST equal the original apart from origins, with the same `GraphLogicalId`, fact-binding digest and checker outcome

#### Scenario: NGRAPH-CASE-09 vector and data value encodings

- GIVEN every schema, value and row variant specified by `NGRAPH-CASE-09`
- WHEN the vector and data parsers read each file
- THEN each MUST be accepted as the recorded encoding or refused with the recorded code
