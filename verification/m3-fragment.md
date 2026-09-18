# M3 checking fragment v1: audit, word table, and control map

**Scope label:** `bootstrap fragment v1`. This document fixes the named
fragment that the M3 change implements; it supersedes
[m2-fragment.md](m2-fragment.md) for v1 while citing it for everything
v1 carries over unchanged. It does not claim the whole language,
inference, recursion support, live resources, evaluation, or Wasm
(V-MODEL-01, V-GATE-05). The audit and word-table decisions are
[design.md](../.cairn/changes/m3-checker-coverage/design.md) "Extend the named
fragment by audit" and "Complete the word table".

## B-CHECK-01..07 audit (M2 evidence versus v1 scope)

| Check | M2 fragment v0 status | Fragment v1 scope | v1 controls |
|---|---|---|---|
| B-CHECK-01 untrusted candidate, independent acceptance against the explicit expected interface and environment | Covered: `noble-candidate` request/candidate split, untrusted witnesses, five-way outcomes (m2-fragment "Candidate and request schema") | Retained; the candidate schema gains reference bindings and is re-stamped `noble-candidate/v1`, both revisions controlled | `decode` revision controls (`tests/docexamples/decode.rs`: v1 accepted, every other format unsupported); all v1 suites re-run |
| B-CHECK-02 exact environment identities; external data separately validated; recursive dependencies and user-declared recursive schemas unsupported; bootstrap `List` supported | Partial: 22-entry table binds identities; recursion only silently omitted | **In:** explicit unsupported rejections for (a) recursive definition dependencies and (b) user-declared recursive schemas; external data validated before any body check | `recursion_dependency_*`, `external_dependency_data_is_validated_before_any_body_check`, `recursive_schema_*`, `nonrecursive_schemas_*` (`tests/recursion-schema-cycle.rs`) |
| B-CHECK-03 declarative per-node derivation, ordered stack joins, quotation effect separation, fresh instantiation, duplication shares one interface | Covered at control level; proof depth one end-to-end function | **In:** duplication-sharing negative; per-word and per-rule theorem depth (proof root) | `duplicated_program_value_shares_one_interface` (invalid); `every_word_has_a_positive_and_a_rejection_control` |
| B-CHECK-04 eligibility and effect inclusion from derivations; empty bound cannot hide `test.emit`; host denial does not repair | Covered: ELIG-DATA/EFF-INC families with hidden-emit and narrowed-bound negatives | **In:** the decision paths become separately proved subjects (`eligibility_iff`, `inclusion_iff`) | `resource_eligibility_rejects_duplication`, `hidden_emit_rejects_against_empty_bound` (invalid) |
| B-CHECK-05 finite well-founded candidates and substitutions; rejects cyclic type equations; fail-closed limits | Partial: work budget fail-closed; no cyclic-equation rejection control | **In:** named rejections for self- and mutually-referential substitution witnesses; every v1 path charges work before allocation or traversal | `cyclic_witness_self/mutual_references_reject_invalid`, `resolvable_witness_chains_accept_and_resolve` + boundary/exhausted companions, `kind_crossed_reference_rejects_as_instantiation_kind`, `witness_references_resolve_and_cycles_reject` (`tests/fragment.rs`) |
| B-CHECK-06 branch-local types from checked eliminator derivations; complete common output stack; no trusted refinements or implicit union joins; `Sum<Resource<R>,I64>` exposes the payload at its own type, sum stays non-`Data` | Partial: eliminators implemented with join controls; pool/oracle omit `case`/`list.case` | **In:** all three eliminators in pool + oracle + per-eliminator proofs; payload-exposure positive with non-`Data`-sum negative; implicit-union-join and advertised-refinement negatives | `sum_resource_payload_exposes_at_its_own_type`, `eliminator_union_is_explicit_not_implicit`, `advertised_refinement_is_not_trusted` (acceptance); pool/oracle arms for `=`, `case`, `list.case` (`tests/property/`) |
| B-CHECK-07 request binding, limits, boundary and exhausted cases | Covered: LIM-*/OUT-* families | Retained; every new v1 path declares and charges its limits with boundary/exhausted pairs | `limits_boundaries_and_exhaustion`, `acyclic_dependencies_accept_at_the_walk_boundary` + exhausted companions, `resolvable_chain_at_the_walk_boundary_and_exhausted_below_it`, `nonrecursive_schemas_scan_at_the_boundary_and_exhaust_below_it` |

Every audited `In` row names its controls above; the v1 theorem
obligations per family are in "Theorem obligations" below.

## Word table (23 entries)

The full bootstrap word table in `Definition` order, each with its
contract source (`.cairn/specs/language/spec.md`), at least one positive
and one rejection fixture, a property-pool entry, an independent oracle
arm, and a per-word theorem obligation. Schemes are v0's
([m2-fragment.md](m2-fragment.md) "Environment contracts") plus `=`;
the bootstrap `List` schema is the table's `nil`/`cons`/`list.case`
vocabulary.

| # | Word | Scheme | Contract source | v1 fixture control (per-word, `word_controls()`) |
|---|---|---|---|---|
| 0 | `dup` | `S a -- S a a ! {}` where `Data(a)` | 7.1 | reject: resource value, `Eligibility` |
| 1 | `drop` | `S a -- S ! {}` where `Data(a)` | 7.1 | reject: resource value, `Eligibility` |
| 2 | `swap` | `S a b -- S b a ! {}` | 7.1 | reject: `I64 Bool` shape, `StackJoin` |
| 3 | `dip` | `S a Program<S,T,e> -- T a ! e` | 7.1 | reject: branch stack-tail mismatch, `StackJoin` |
| 4 | `+` | `S I64 I64 -- S I64 ! {}` | 4.4 K-NUM-01 | reject: `I64 Bool`, `StackJoin` |
| 5 | `-` | `S I64 I64 -- S I64 ! {}` | 4.4 K-NUM-01 | reject: `I64 Bool`, `StackJoin` |
| 6 | `*` | `S I64 I64 -- S I64 ! {}` | 4.4 K-NUM-01 | reject: `I64 Bool`, `StackJoin` |
| 7 | `=` | `S I64 I64 -- S Bool ! {}` | 4.4 K-NUM-01 (`=` paragraph) | reject: `I64 Bool`, `StackJoin`; substitution control `equals_substitutes_i64_i64_to_bool` |
| 8 | `quote` | `R a -- R Program<S,S a,{}> ! {}` where `Data(a)` | 6.5 | reject: resource value, `Eligibility` |
| 9 | `compose` | `R Program<A,B,e> Program<B,C,f> -- R Program<A,C,union(e,f)> ! {}` | 6.4 | reject: middle-stack mismatch, `StackJoin` |
| 10 | `run` | `S Program<S,T,e> -- T ! e` | 6.3 | reject: body stack mismatch, `StackJoin` |
| 11 | `reflect` | `R Program<S,T,e> -- R Syntax ! {}` | 11.3 | reject: non-program value, `StackJoin` |
| 12 | `unit` | `S -- S Unit ! {}` | 7.2 | reject: claimed output mismatch, `StackJoin` |
| 13 | `pair` | `S a b -- S Pair<a,b> ! {}` | 7.2 | reject: arity/shape mismatch, `StackJoin` |
| 14 | `unpair` | `S Pair<a,b> -- S a b ! {}` | 7.2 | reject: non-pair value, `StackJoin` |
| 15 | `inl` | `S a -- S Sum<a,b> ! {}` | 7.2 | reject: value kind mismatch, `StackJoin` |
| 16 | `inr` | `S b -- S Sum<a,b> ! {}` | 7.2 | reject: value kind mismatch, `StackJoin` |
| 17 | `case` | `S Sum<a,b> Program<S a,T,e> Program<S b,T,f> -- T ! union(e,f)` | 7.2 | reject: unequal branch joins, `StackJoin` |
| 18 | `if` | `S Bool Program<S,T,e> Program<S,T,f> -- T ! union(e,f)` | 7.3 | reject: branches claim different latent bounds, `StackJoin` |
| 19 | `nil` | `S -- S List<a> ! {}` | 7.4 | reject: witness arity, `InstantiationArity` |
| 20 | `cons` | `S a List<a> -- S List<a> ! {}` | 7.4 | reject: element/list mismatch, `StackJoin` |
| 21 | `list.case` | `S List<a> Program<S,T,e> Program<S a List<a>,T,f> -- T ! union(e,f)` | 7.4 | reject: cons-branch input mismatch, `StackJoin` |
| 22 | `test.emit` | `S Text -- S Unit ! {test.emit}` | B-SCOPE-01 item 5 | reject: hidden emit against empty bound, `EffectInclusion` |

The `test.counter` resource-maker fixture (pool id 23) is a
negative-eligibility fixture, not a language word.

## Control map (v1)

Positive, boundary, exhausted, and rejection control per family, with
named fixtures and expected outcomes. "→" states the fixture's expected
outcome.

| Family | Positive | Boundary | Exhausted | Rejection |
|---|---|---|---|---|
| FRAG-LIT/SEQ | `sequence_and_literals_accept_arithmetic` → accepted | empty sequence, empty stack-variable mapping (`schemes_instantiate_and_reject_mismatches` boundary) — accepted | — | join mismatch / order swap / arity: `Constraint::StackJoin` — invalid |
| FRAG-QUOTE | `quotation_construction_checks_body_and_runs` → accepted | `constructor_patterns_substitute_in_documented_order` (pair/sum/list patterns) → accepted | — | unchecked body / latent-as-construction — invalid |
| FRAG-BRANCH | `if`/`case`/`list.case` equal-join positives (`every_word_has…` def 17/18/21; pool fires case=171 if=173 list.case=161) → accepted | branch order swap under equal joins → accepted | — | unequal output stacks, missing union effect, resource payload join (`eliminator_union_is_explicit_not_implicit`, DX-01 branch control) — invalid |
| ELIG-DATA | `dup`/`drop`/`quote` on `Data` values → accepted | nested `Pair`/`List` of `Data` (`data_is_recursive_over_resource_payloads`) → accepted | — | `Resource<k>` payloads (`resource_eligibility_rejects_duplication`, `duplicated_program_value_shares_one_interface`) — invalid |
| EFF-INC | bound included in allowed effects → accepted | empty sets both sides → accepted | — | `hidden_emit_rejects_against_empty_bound`, narrowed bound — invalid |
| PAYLOAD (B-CHECK-06) | `sum_resource_payload_exposes_at_its_own_type` (`Sum<Resource<R>,I64>` eliminated at `R`) → accepted | — | — | non-`Data` sum rejected as eligibility/`Data` violation — invalid |
| REFINEMENT-BOUND (B-CHECK-06) | explicit union joins (`effect_sets_union_sorted_deduplicated_and_ordered`) → accepted | — | — | `eliminator_union_is_explicit_not_implicit`, `advertised_refinement_is_not_trusted` — invalid |
| REJ-REC (B-CHECK-02) | `external_dependency_data_is_validated_before_any_body_check` → accepted | `acyclic_dependencies_accept_at_the_walk_boundary` → accepted | dependency walk over limit → exhausted | `recursive_dependency_self/mutual_names_identity` → unsupported, diagnostic names the definition identity |
| REJ-SCHEMA (B-CHECK-02) | non-recursive schema accepted | `nonrecursive_schemas_scan_at_the_boundary…` → accepted | schema scan over limit → exhausted | `recursive_schema_rejects_unsupported_naming_the_declaration` → unsupported, names the declaration |
| REJ-CYCLE (B-CHECK-05) | `resolvable_witness_chains_accept_and_resolve` → accepted (resolved) | `resolvable_chain_at_the_walk_boundary…` → accepted | witness walk below hop count → exhausted | `cyclic_witness_self_reference_rejects_invalid` / `_mutual_…` → invalid; kind-crossed reference → invalid `InstantiationKind` |
| LIM-* | at-limit candidate accepted (`limits_boundaries_and_exhaustion`) | exactly at each limit → accepted | one over each limit → exhausted | negative sizes, malformed counts — rejected |
| OUT-* / DIAG-* | each outcome distinguishable; `diagnostics_report_order_shape_and_truncation` → accepted | exact-shape diagnostic | diagnostic budget exhausted | unavailable provenance reported as unavailable, never invented (DX-01 resource control) |
| SCHEMA-REV | `noble-candidate/v1` decodes and checks → accepted | — | — | every other format string (incl. `v0`) → unsupported (`tests/docexamples/decode.rs`) |
| Property agreement | agreement lane over the 24-id pool, seeds `0x0B1E_5EED_0000_0001/0002`, 1000+200 candidates | — | — | malformed lane: 0 accepted, 0 panics (`generated_candidates_agree_with_independent_oracle`, `malformed_candidates_never_panic_or_accept`) |

## Theorem obligations (scoped-in families)

Obligations owned by the `proofs/m3` root (tasks 5.1–5.4); the inventory
is recorded in `verification/m3-proof-evidence.md` when they land.

| Family | Theorem obligation |
|---|---|
| Word table | per-word scheme refinement for each of the 23 entries (`apply_refines`: the extracted instantiation path yields exactly the reference interface; `table_refines`, `wordTable_length = 23`) |
| Per-rule soundness | one theorem per rule family {EMPTY, LITERAL, WORD, SEQUENCE, QUOTATION, CASE, IF, LISTCASE}: acceptance through the rule implies well-formedness and a `TypingDerivation` at that node |
| ELIG-DATA / EFF-INC | `eligibility_iff` (per constrained word) and `inclusion_iff`; branch-union conservativity for the three eliminators |
| REJ-REC / REJ-SCHEMA / REJ-CYCLE | rejection judgments in the reference model; rejection agreements in the refinement matrix; the walks fold into `foldBody_work_bounded_v1` |
| Termination / work | `check_total_v1`, `foldBody_work_bounded_v1` over fragment v1 including the rejection walks; per-word cost positivity |
| Coverage | `coverage_positive_word` instantiated for every table entry (an accepted, derivable fixture exists per word) |
| Refinement matrix | per-word and per-rule representative-input agreements; rejection agreements for the B-CHECK-02/05/06 negatives; `accepted_via_refinement` generalized to v1 |
| B-CHECK-03 | duplication shares one instantiated interface at both extracted and reference levels |
| LIM-* / B-CHECK-07 | the new walks' declared limits charge before allocation/traversal, fail closed (boundary/exhausted companions mirrored in Lean) |

## Frozen per-word fixture list

Each word's v1 fixtures (task 1.3 freeze): positive = `word_controls()`
entry `def` with its accepting binding vector, asserted against
`checked.interface.stack_out`; rejection = the same table's negative
binding vector, asserted against the row's named constraint (column
"v1 fixture control" above); property-pool entry = pool id equal to the
table index (24 ids including the resource-maker fixture); oracle arm =
`word_face(def, …)`; doc example = the per-word `noble-check` example
in [m3-docexamples.md](m3-docexamples.md) (task 3.2). The freeze is
enforced mechanically by
`every_word_has_a_positive_and_a_rejection_control` (23/23) and the
doc-example coverage assertion.

## Omissions

Inference/unification, generalization, recursion support (only its
explicit rejection is claimed), imports, preparation services, live
resources, evaluation and recipe semantics, Wasm lowering and execution,
concurrency, contracts, and whole-kernel verification. A result about
fragment v1 MUST NOT be presented as a result for the whole language
(V-MODEL-01).
