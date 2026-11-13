## Phase 0: design (this change)

- [x] [serial] Record the reviewed contract in `design.md` (D1–D11, effect exclusions, budgets and H-LIMIT accounting, residual representation, test plan with the frozen measurement protocol, risks, open questions) and the `NGRAPH-PE-01..07` delta with its scenario blocks, then pass the pinned Cairn `proposal`, `design` and `tasks` gates and `validate`. Evidence: the three gates returned `{"verdict":"PASS"}` with no issues and the pinned `validate` reported no findings (base `fed77835`). These are document checks; they are not implementation, execution or proof evidence. r[NGRAPH-PE-01] r[NGRAPH-PE-02] r[NGRAPH-PE-03] r[NGRAPH-PE-04] r[NGRAPH-PE-05] r[NGRAPH-PE-06] r[NGRAPH-PE-07]

## Phase 1: library entry point

- [ ] [serial] Implement `crates/noble-ngraph/src/specialize.rs`: the admitted slice (D1), the static map and `merge` (D2), substitution and the `add`/`sub`/`mul`/`eq` fold (D3), pruning and renumbering plus the independent re-check (D4), the derived residual subject and the derivation record (D5, D11), the accounting revision and caps (D7), and the residual source emitter with its re-admission helper (D9). Add the two budget kinds to `diag.rs`. Evidence: unit tests for the model fixtures, the prefix-merge control, the fold whitelist, wraparound, the refused slice, the arity refusal and the identity record. r[NGRAPH-PE-01] r[NGRAPH-PE-02] r[NGRAPH-PE-03] r[NGRAPH-PE-04] r[NGRAPH-PE-05] r[NGRAPH-PE-06] r[NGRAPH-PE-07]
- [ ] [serial] Add the budget ladder to the same module: below, at and above the residual-node cap, the zero variant cap, and a raised cap refused with `E-PROFILE`. Evidence: the ladder test names each boundary and its code. r[NGRAPH-PE-05]

## Phase 2: command

- [ ] [serial] Add `noble ngraph specialize` to `crates/noble-cli/src/ngraph.rs` with `--static`, `--out`, `--graph-out` and the existing budget flags, the existing report schema and exit mapping, the stage field of D8, create-new publication, and publication only after independent re-admission. Evidence: a manual smoke of the command on a fixture and the black-box tests below. r[NGRAPH-PE-01] r[NGRAPH-PE-06] r[NGRAPH-PE-07]
- [ ] [serial] Black-box tests in `crates/noble-cli/tests/ngraph.rs`: the admitted-slice report fields, the refused slice, the arity refusal, the budget ladder, the residual source acceptance and the derivation record. Evidence: the test names and their assertions. r[NGRAPH-PE-01] r[NGRAPH-PE-02] r[NGRAPH-PE-05] r[NGRAPH-PE-06] r[NGRAPH-PE-07]

## Phase 3: differential, negative and measurement evidence

- [ ] [serial] Add the differential leg to `crates/noble-ngraph-experiment` (`b2i64` plus the existing `extract_b2` target module): original under merged vectors, residual, re-admitted residual, L3, L4, and the Node BigInt oracle when `NOBLE_NODE` is set, over the fixture graphs and seeded Core programs with the frozen vector count. Evidence: the executed test and its pass count; agreement is finite differential evidence, not proof. r[NGRAPH-PE-02] r[NGRAPH-PE-04] r[NGRAPH-PE-06]
- [ ] [serial] Add the zero-host-request controls: the effectful, resource and recursive B3 subjects refused at the admission stage with no output file, and the same sources' ordinary-path reports recording zero guest and zero host requests. Evidence: the executed test and the report fields it asserts. r[NGRAPH-PE-06]
- [ ] [serial] Produce the measurement receipt: per fixture, the two node counts, folded and pruned counts, per-side L3 and L4 node visits over the frozen vector count, retained bytes of both compiled graphs, and specialization elapsed time measured only in the harness. Bound it to the clean source commit. Evidence: the receipt path, its source commit and the reproduced numbers. r[NGRAPH-PE-05]
- [ ] [serial] Edit only the #32 row of `proofs/foundation/README.md` to separate the model proof, the absent Rust correspondence claim, the finite differential tests and the measured runtime counts, and record the unproved `eq` fold and the unmodelled pruning step. Evidence: the rendered row and the document checks. r[NGRAPH-PE-03] r[NGRAPH-PE-04]

## Phase 4: verification (this change)

- [ ] [serial] Run `cargo test -p noble-ngraph -p noble-ngraph-experiment -p noble-cli --all-targets --all-features`, the ignored BigInt oracle leg with the pinned Node, workspace `clippy -D warnings`, `fmt --check`, the Octet deny-all and architecture gate, Cairn `validate`, and the document checks. Evidence: each command and its result line. r[NGRAPH-PE-01]

## Phase 5: integration (separate integrator, not this change)

- [ ] [serial] Canonical sync of `NGRAPH-PE-01..07`, registration of the fourteen scenarios as case records with their state and evidence fields, generated views and the requirement ledger. This change does not touch canonical specs, registered cases, generated views or `.cairn/VALIDATION.json`; it records the draft scenario IDs so promotion does not have to invent them. r[NGRAPH-PE-01] r[NGRAPH-PE-02] r[NGRAPH-PE-03] r[NGRAPH-PE-04] r[NGRAPH-PE-05] r[NGRAPH-PE-06] r[NGRAPH-PE-07]
- [ ] [serial] Source-inventory renewal, reviewed-file re-pin and Octet capacity accounting for the touched Rust sources, then the repository flake gate run. This change leaves all of them to the integrator. r[NGRAPH-PE-01]
