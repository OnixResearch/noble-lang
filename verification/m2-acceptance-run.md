# M2 acceptance control run (updated 2026-09-18)

Scope: the M2 feasibility milestone — the finite fragment checker, its
extraction, the reference model proofs, and the developer-experience
controls (tasks 5.1–5.3). Every gate below was run against this worktree
after the 5.1–5.3 controls landed and the kernel substitution fix they
forced was re-extracted (see "Kernel bug found by the property harness").

## Kernel gates (implementation worktree root)

Environment: `PATH=/nix/store/1yvh3d6y3fj3xk2dgwczrp1dj5svd92c-rust-default-1.96.0-nightly-2026-03-21/bin`,
`CARGO_HOME=/tmp/cargo-home`, `CARGO_TARGET_DIR=<repo>/target`;
octet at `/nix/store/vj0sg5rn291z78im7gd2cqzqzd0bq3zy-cargo-octet-0.1.0/bin`.

| Gate | Command | Result |
|---|---|---|
| Build | `cargo build --offline --workspace --all-features` | 0 errors |
| Tests | `cargo test --offline --workspace --all-targets --all-features` | 9 suites, 22 tests, all pass (cli smoke 3, kernel acceptance 6, budget 2, fragment 4, property 2, docexamples 2, dx01 3) |
| Clippy | `cargo clippy --offline --workspace --all-targets --all-features -- -D warnings` | 0 errors, 0 warnings |
| rustfmt | `cargo fmt --check` | clean |
| Octet architecture | `rm -rf .octet/compiler-architecture/shards && cargo-octet check --workspace -- --all-targets --all-features` | Architecture findings: 0 (catalog clean). Lint phase: warning-only — the 12 pre-existing production findings (10 formatter-inherent `mutating_input_in_pure` sites on the hand-written `Debug` impls, 2 `module_file_count` notes on `src/`) plus style warnings inside the new DX test files only (`tests/property`, `tests/docexamples`, `tests/dx01.rs`), which the design keeps outside the kernel catalog scope; no test-file warning is a production finding. |

## Kernel bug found by the property harness (fixed)

The 5.1 agreement lane found the iterative substitution walk broken for
constructor patterns in `words/subst.rs`; the harness disagreement reports
minimized to `inr`-over-`[Bool]` (generated cases 2 and 11):

1. **Pair/Sum children swapped.** `part_task` queued the pair/sum children
   in left-then-right order on the last-in-first-out work stack, so the
   finish step popped them swapped: `inr` with `a=Unit, b=Bool` produced
   `Sum(Bool, Unit)`, contradicting the documented `inr : S b -- S Sum<a,b>`
   and the Lean reference model (`Words.lean substPattern` has the correct
   order). None of the twelve refinement fixtures exercised a pair/sum
   pattern, so every prior gate was blind to it.
2. **List patterns never substituted.** The completion step popped two
   segments unconditionally, so every `list` pattern (one child) failed on a
   phantom second segment — surfaced as `InstantiationKind` rejections for
   every `nil`/`cons` witness.

Fix: children queued right-first (`subst.rs`), list completion from its
single segment via the new `segments::list_segment` (`segments.rs`).
Regression control: `constructor_patterns_substitute_in_documented_order`
(`tests/fragment.rs`) — it fails on the pre-fix kernel and caught a
reintroduced swap during the fix's own compression. After the fix the full
gate matrix below was re-run, and the extraction was regenerated (next
section) so the generated module matches the fixed source.

## Extraction regeneration (after the substitution fix)

The pinned regeneration loop from the generated header was re-executed:
charon `b104e24f…` (`--preset=aeneas --error-on-warnings`, exit 0), aeneas
`505b6ca3…` (`-split-files -gen-lib-entry -all-computable
-warnings-as-errors`, exit 0 — every kernel function translates). The
regenerated `NobleKernel/Types.lean` and `NobleKernel/Funs.lean` are
installed (Types identical; Funs carries the re-ordered pushes and the new
`words.segments.list_segment`); the checked-in `FunsExternal.lean` /
`TypesExternal.lean` still cover the template surface (same 22 external
names, no new externals); `NobleKernel.lean` keeps its documented
regeneration-loop header. `verification/m2-coverage.json` was re-synced to
the generated facts (474 functions: 451 extracted, 1 proved, 17 modeled,
5 excepted, 0 open) and both verification gates pass on the regenerated
module:

- `verification/m2-proof-gate.sh` (pinned lean 4.31.0 on PATH): PASS all
  checks — build, no-sorry, generated kernel, required theorems with axiom
  policy and subject binding, external models.
- `verification/m2-coverage-gate.sh`: PASS all checks — coverage derive/join
  (474/474), excepted disclosure (5 == `#[charon::opaque]`), open-subject 0,
  env binding (474 constants; 96 elaborator matchers tolerated; 13
  refinement citations verified).
- `lake build NobleM2 NobleM2.Refinement NobleKernel`: 1690 jobs, 0 errors,
  `sorry` count 0.

## Developer-experience controls (tasks 5.1–5.3)

**5.1 property harness** (`crates/noble-kernel/tests/property.rs` +
`tests/property/`): deterministic splitmix64 streams, fixed seeds, no
time/env input. Agreement lane: 1000 generated candidates over a
19-word pool (dup, drop, swap, dip, +, quote, compose, run, reflect, unit,
pair, unpair, inl, inr, if, nil, cons, test.emit, and the resource-maker
fixture), nesting to depth 2, plus witness-corruption knobs —
**303 accepted by both sides, 697 rejected by both sides, 0 disagreements**,
against the separately written oracle (`tests/property/oracle.rs`,
`otypes.rs`, `table.rs`: own type mirror, own contract table from the
documented rules, own join/effect/eligibility logic — no kernel decision
code). On a disagreement the harness shrinks (bound 64 steps: body/quote
entry removal, stack-binding truncation, type flattening, expectation
truncation) while the failure predicate holds and reports the minimized
case. Malformed lane: 200 structurally corrupted candidates (foreign
format/semantic revisions, dangling and `u32::MAX` references, unknown
definitions, arity/kind/oversized witnesses, over-deep quotation chains,
truncated arenas, raw identifier bits) — **0 accepted, 0 panics**, under the
workspace's `overflow-checks = true` debug profile; outcome mix
invalid=115, exhausted=32, unsupported-or-failure=53.

**5.2 documentation examples** (`crates/noble-kernel/tests/docexamples.rs` +
`tests/docexamples/`): the `noble-check` fence format is defined in
[m2-fragment.md](m2-fragment.md) "Executed documentation examples". The
scanner covers `crates/noble-kernel/src/**/*.rs` doc comments (currently
none tagged) and the fragment document: **8 examples executed against the
actual kernel checker** (3 accepted, 3 invalid, 1 exhausted, 1 unsupported),
each asserting its stated outcome; **3 illustrative fenced blocks ignored**.
The illustrative control proves the candidate-schema JSON block (which names
an environment definition `add` the bootstrap checker does not provide) is
skipped purely for lacking the tag and would not decode as evidence.

**5.3 DX-01 controls** (`crates/noble-kernel/tests/dx01.rs`), from
`specs/conformance/developer-experience-cases.json` case DX-01 verbatim —
all three pass, each asserting `word_or_join` (`Diagnostic::def`),
`required_stack`/`actual_stack` (`expected`/`actual`), `constraint`,
`source_span` (`node`), and `value_origin_or_unavailable`
(`provenance_available == false`):

- `dx01_word_input_type_names_word_and_both_stacks` — `1 true +`: the `+`
  word requires `I64 I64`, the stack holds `I64 Bool`.
- `dx01_branch_output_type_names_the_unequal_join` — `true [ 1 ] [ false ] if`:
  the `if` join requires both branch programs at the claimed equal
  interface; the two quoted branches differ.
- `dx01_resource_duplication_reports_unavailable_provenance` — `dup` over
  `[Resource<test.counter>]`: eligibility rejection with provenance
  explicitly unavailable.

`guest_requests: 0` / `protected_operations: 0` are structural for this
fragment: checking is static and rejection issues no candidate-body host
request (B-RESULT-01).

## Fragment rule coverage (task 1.3 controls)

| Fragment rule | Control |
|---|---|
| Node derivation, positive | `crates/noble-kernel/src/acceptance` unit suite (6 tests) |
| Instantiation arity/kind rejection | fragment suite + `Constraint::InstantiationArity/Kind` paths |
| Constructor-pattern substitution order | fragment suite `constructor_patterns_substitute_in_documented_order` (regression for the property-harness find) |
| Stack join, wrong order vs wrong shape | `Constraint::StackOrder` / `StackJoin` via `same_multiset` |
| Effect inclusion | acceptance suite inclusion negatives (`Constraint::EffectInclusion`) |
| Recursive eligibility | acceptance suite eligibility negatives (`Constraint::Eligibility`) |
| Branch joins (B-CHECK-06) | fragment suite branch-join cases + DX-01 branch control |
| Limits (stack height, type size, work) | budget suite (`Fail::Exhausted`, fail-closed accounting) + malformed-lane exhaustion mix |
| Rejection without execution | CLI smoke suite (3 tests) |
| Bounded agreement + malformed input | property suite (2 tests) |
| Executed documentation evidence | docexamples suite (2 tests incl. the illustrative control) |

## Open obligations (kept open on purpose)

The lifecycle (spec sync, archive, push, integration — task 6.3) remains
open; the obligation-ledger entries for PO-09/PO-10/PO-11 move with the
parent acceptance sequence. Every other obligation in the ledger stays
open.
