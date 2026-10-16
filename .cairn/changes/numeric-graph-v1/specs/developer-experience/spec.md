# Numeric graph tools and file formats

## ADDED Requirements

### Requirement: NGRAPH-CLI-01
r[NGRAPH-CLI-01]

`Numeric-Graph-v1` MUST expose exactly the `noble ngraph` subcommands `check`, `extract`, `eval`, `train` and `verify`.

**Effects.**

- Each subcommand MUST read only the files named by its arguments, each under a stated byte bound checked before parsing, and MUST write only its explicitly named output file.
- It MUST NOT spawn a process, execute Wasm or open a network connection.
- The `noble-ngraph` library MUST NOT execute any filesystem, process, environment, clock, thread or output effect, and MUST NOT depend on a third-party package.
- Identity digests MUST be computed through a hashing scheme that the composition root supplies.

**Reports.**

- Every outcome other than a usage error MUST be reported as a JSON line with the schema `noble-ngraph-report/v1`. A usage error MUST print its usage on standard error and nothing on standard output.
- A refusal MUST carry one stable code and a stable site. The codes are the D6 codes of `NumericGraph-Design`, `E-FORMAT`, `E-SNAPSHOT-INTEGRITY`, `E-SNAPSHOT-ID` and `E-SNAPSHOT-REPLAY`. Run-time observations that have no site (host-fuel exhaustion and cancellation) and host conditions MAY report a null site.
- A refusal that belongs to admission, snapshot binding or input validation MUST occur before any graph node is evaluated.

**Exit status.** The exit status MUST follow the outcome class:

- 0 for `normal`;
- 1 for `trap` (a guard failure or a non-finite update);
- 2 for `reject` (malformed or inconsistent inputs, snapshot binding refusals, usage errors, host I/O failures, exceeded input bounds and an existing output path);
- 4 for `unsupported` (effect, resource, dynamic, control, aggregate, reflection, Program-output, recursion and default-refused derivative classes);
- 5 for `exhausted` (every `E-BUDGET-<kind>`).

For `eval`, the exit status MUST be that of the first non-normal vector in file order.

**Budgets.** Every D5 budget MUST be settable by an explicit flag whose default is the `/0` value. A flag MAY only lower a budget; a raised value MUST be refused with `E-PROFILE`.

**Output.**

- For identical arguments and input bytes, standard output MUST be byte-identical across runs.
- An `F32` value MUST be rendered as its exact binary32 encoding, except that every NaN encoding MUST be rendered as the single NaN class.
- The compiled evaluator MUST be the default, and the reference interpreter MUST be selectable. Both MUST produce identical output except for the evaluator-specific point of host-fuel exhaustion.

#### Scenario: NGRAPH-CASE-03 graph admission report

- GIVEN the `Numeric-Graph-v1` profile and the `NGRAPH-CASE-03` graphs in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN `noble ngraph check` admits each graph file
- THEN the report MUST match every recorded `expected` field, including the recorded `GraphLogicalId` and exit 0; this active delta does not change that case's `state` or `evidence`

#### Scenario: NGRAPH-CASE-04 check refusal classes

- GIVEN every `NGRAPH-CASE-04` malformed, inexact, unchecked-fact, budget, oversized, missing and usage variant in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN `noble ngraph check` runs each variant
- THEN each MUST report its recorded code and exit status with zero evaluated nodes; this active delta does not change that case's `state` or `evidence`

#### Scenario: NGRAPH-CASE-07 bitwise evaluation in both evaluators

- GIVEN the `NGRAPH-CASE-07` graphs and vectors in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN `noble ngraph eval` runs them with the default evaluator and with `--reference`
- THEN every output MUST match its recorded encoding and both runs MUST produce byte-identical standard output; this active delta does not change that case's `state` or `evidence`

#### Scenario: NGRAPH-CASE-08 evaluation outcome classes

- GIVEN the `NGRAPH-CASE-08` guard, input-fact, interface, missing-snapshot, fuel and mixed-file inputs in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN `noble ngraph eval` runs each input
- THEN each vector line, code and process exit status MUST match the recorded `expected` fields; this active delta does not change that case's `state` or `evidence`

#### Scenario: NGRAPH-CASE-02 effect-free library boundary

- GIVEN the `NGRAPH-CASE-02` package checks in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN the `noble-ngraph` crate attributes, dependencies and collected capability facts are inspected
- THEN the library MUST be `no_std`, depend only on `noble-kernel` and `noble-contracts`, use no forbidden capability and take identity hashing as an injected scheme; this active delta does not change that case's `state` or `evidence`

#### Scenario: NGRAPH-CASE-15 claims, determinism and documented exits

- GIVEN the `NGRAPH-CASE-15` artifacts and repeated runs in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN the user documentation, help text and reports are reviewed and every command is run twice
- THEN no performance, GPU, backend-selection or proof claim MAY appear, the exit-code table MUST be documented and repeated runs MUST be byte-identical; this active delta does not change that case's `state` or `evidence`

### Requirement: NGRAPH-FORMAT-01
r[NGRAPH-FORMAT-01]

Graph, vector and training-data files MUST each begin with a schema identifier line: `ngraph-ir/0 <sub-profile>` for graphs, `noble-ngraph-vectors/0` for evaluation vectors and `noble-ngraph-data/0` for training data. A missing or unknown identifier MUST be refused before any further parsing: a graph file without the `ngraph-ir/0` schema line with `E-GRAPH-MALFORMED`, a graph file naming an unknown sub-profile with `E-PROFILE`, and any other file with `E-FORMAT`.

**Graph files.**

- A graph file MUST record the sub-profile, the ordered slots, the nodes in canonical order, the facts with their provenance, the outputs, the input and output shapes, and the root subject kind, identity and closure. `GraphLogicalId` MUST then be recomputed from the parsed records alone.
- Comments, whitespace, file names and paths MUST NOT be identity-bearing. Producer origins are not identity-bearing and MAY be omitted.
- Printing MUST be canonical and deterministic, and every encoding MUST be symmetric between printing and parsing, including `evidence` provenance. Parsing a printed graph MUST reproduce the same graph apart from origins, with the same `GraphLogicalId`, fact records and checker outcome.
- Parsing MUST only build producer data; admission MUST still be decided by the independent graph checker. `F32` constants MUST obey the exact-literal rules of NGRAPH-F32-01.
- An interface shape tree MUST nest at most 127 pairs, the deepest root interface item that source extraction can produce; a deeper tree MUST be refused with `E-GRAPH-MALFORMED` before it is walked.

**Numbers in every file.** Integers MUST be written as ASCII digits, with a leading `-` only on signed values, and bit patterns as `0x` plus hex digits; a leading `+` MUST be refused like any other malformed token. Deciding whether a decimal is exact MUST take time linear in its length.

**Vector and data files.**

- Values MUST be typed by a declared type line. It MUST equal the graph's ordered input slot types and, for data, the loss graph's target slot types; otherwise the file MUST be refused with `E-INTERFACE` before evaluation.
- An `F32` value MUST be given either as its exact binary32 encoding, which MAY be any encoding including NaN and infinities, or as an exactly representable finite decimal. An inexact decimal MUST be refused with `E-LITERAL-INEXACT`.
- Parameters MUST NOT be supplied through vector or data files.
- Training-data order MUST be preserved, because the training-data digest binds it.

#### Scenario: NGRAPH-CASE-06 identity-preserving graph round trip

- GIVEN every `NGRAPH-CASE-06` authored, workload, random and extracted graph and text variant in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN each graph is printed, parsed and checked again
- THEN the parsed graph MUST equal the original apart from origins, with the same `GraphLogicalId`, fact-binding digest and checker outcome; this active delta does not change that case's `state` or `evidence`

#### Scenario: NGRAPH-CASE-09 vector and data value encodings

- GIVEN every `NGRAPH-CASE-09` schema, value and row variant in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN the vector and data parsers read each file
- THEN each MUST be accepted as the recorded encoding or refused with the recorded code; this active delta does not change that case's `state` or `evidence`
