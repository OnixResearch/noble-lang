## Why

Issue #32 asks for bounded, effect-aware partial evaluation for checked Programs. The
mechanized part exists: `proofs/foundation/NobleFoundation/Specialize.lean` (18 theorems,
receipt `verification/foundation/receipts/84753c99e/`) proves, for every packed graph and
every static-input map, that a specialization which substitutes static inputs, renumbers
the dynamic ones and folds *fully static* `add`/`sub`/`mul` preserves the value of every
output under `merge σ d`. That is a model result about a graph. Nothing in the repository
turns it into a checked Program:

- `noble-ngraph` has no specialization entry point; its `extract::Request` carries no
  static-input map (`crates/noble-ngraph/src/extract/mod.rs`), so a subject is either
  extracted whole or refused;
- no code anywhere turns a graph back into a Program, so a specialized graph cannot be
  re-accepted by the kernel and has no ordinary execution path;
- the only differential evidence is the Lean fixture set (no/all/mixed static inputs, a
  non-prefix static slot, operand order, wraparound), and no executed differential exists;
- there is no negative test showing a refused subject performs zero host requests, and no
  measurement of the runtime work a residual actually saves.

Issues #33 (cache specialized Programs with exact dependency keys), #34 (Rust interpreter
specialization and operand forwarding) and #35 (source-level integration) are blocked on
this contract: #33 needs the manifest and build-key fields, #34 needs the entry point, and
#35 needs a residual Program at all. The owner reserved five decisions to this change: the
binding-time/representation contract, the observation relation, budgets and H-LIMIT
accounting, the public syntax, and the residual's representation.

## What Changes

- Select the contract `NGRAPH-PE-01..07` in a `developer-experience` delta: the admitted
  subject slice, the static-map representation, the folding whitelist, the observation
  relation, the accounting revision and caps, the residual Program and its independent
  re-acceptance, and the derivation record that a later cache binds.
- The library entry point is `noble_ngraph::specialize` in the pure domain-core crate: it
  takes a **checked** `ngraph-i64/0` graph plus an explicit static map and returns one
  re-checked residual graph with a derivation record and an accounting report. It performs
  no evaluation, no host request and no I/O.
- `noble ngraph specialize` is the only CLI surface, added to the existing
  `crates/noble-cli/src/ngraph.rs` with the existing report schema and exit mapping. The
  command emits the residual as `let`-based Noble source and **re-prepares and re-admits
  it through the ordinary frontend and kernel before publishing anything**. It also
  publishes the residual graph file (existing `ngraph-ir/0` form) when asked, and never
  overwrites a file (the existing create-new plus hard-link publication).
- The residual is a **new** Program and a **new** graph identity. The original graph,
  Program, its recipe and its identity are untouched; there is no in-place optimization.
- Two new budget kinds in the published `E-BUDGET-<kind>` family: `RESIDUAL-NODES`
  (residual nodes created, charged before pruning) and `VARIANTS` (residual variants per
  request; the v1 entry point supports exactly one). Both default to the `/0` values and a
  flag may only lower them, exactly like every other budget in this profile.
- No source syntax change. A Program is never reinterpreted, no kernel type, word or effect
  changes, and the ordinary path is byte-identical when `specialize` is not invoked.

**Non-claims.**

- **No new proof claim and no correspondence claim.** `specialize_sound` is a model result
  about graph values. This change adds a Rust implementation and finite differential tests;
  it does not claim Rust↔model correspondence, neither for the implementation nor for the
  residual Program's re-extraction.
- **The `I64Eq` fold is an extension of the model.** `specialize_sound` covers
  `add`/`sub`/`mul`; folding `eq` when both operands are static is implemented and tested
  here and is recorded as an unproved extension.
- **No equal-budget claim.** The original and the residual do not have equal fuel or equal
  accounting: `merge σ d` reconstructs the original's inputs and the residual consumes only
  the dynamic ones. Per H-LIMIT-03 the accounting revision is named and the two sides are
  reported separately.
- **No universal runtime claim.** Measurements report evaluator node visits, residual size,
  retained bytes and wall time for the named fixtures; they do not establish that every
  graph shrinks, and the Phase C stop recommendation for the compiled backend stands.
- **No pairs/`Unit`/non-flat residual interface and no guard, fact or effect support in
  v1.** Those graphs are refused with a code, not silently specialized.

## Impact

- **Files (this change):**
  - `.cairn/changes/partial-evaluation-v1/{proposal,design,tasks}.md` and `metadata.json`;
  - a MODIFIED `NGRAPH-CLI-01` requirement and ADDED `NGRAPH-PE-01..07` requirements in
    `specs/developer-experience/spec.md`, with scenario blocks naming the case IDs this change
    will register at sync.
  - No canonical requirement, registered scenario, generated view or ledger row changes on
    this design turn. The canonical sync, the case records and the generated views belong to
    the integrator's promotion.
- **Implementation (this change, after the design commit):**
  - `crates/noble-ngraph/src/specialize.rs` (new module: entry point, static map,
    substitution, folding, pruning, re-check, residual source emission, re-admission
    helper, accounting report) and its unit tests;
  - `crates/noble-ngraph/src/diag.rs` (two budget kinds) and `lib.rs` (re-exports);
  - `crates/noble-cli/src/ngraph.rs` plus `ngraph/arguments.rs` and `ngraph/files.rs` (the
    `specialize` subcommand, `--static`, `--out`, `--graph-out`);
  - `crates/noble-cli/tests/ngraph.rs` (black-box tests, including the zero-host-request
    controls);
  - `crates/noble-ngraph-experiment/src/b2i64/mod.rs` and
    `crates/noble-ngraph-experiment/tests/extract_b2.rs` (the differential leg: original
    with merged inputs, residual, residual re-admitted from source, L3, L4 and the Node
    BigInt oracle);
  - `proofs/foundation/README.md` (only the #32 ledger row's status text).
- **Testing:** the design turn itself is checked by the pinned Cairn `proposal`, `design`
  and `tasks` gates and `validate`, and by the repository's document checks. Those are
  document checks; they are not implementation, execution or proof evidence. The planned
  Rust, CLI and measurement evidence is listed in design "Test plan" and is produced (or
  explicitly reported as unproduced) in the later commits of this change.
- **Coordination:** #33 consumes the derivation record; #34 consumes the entry point and
  the whitelist; #35 is explicitly out of scope (it needs `Bool`/branch composition and was
  recorded NO-GO in the triage). #42's certificates are optional and not used. #15
  (`F32` algebra) is untouched: v1 is `I64` only and folds no `F32`.
