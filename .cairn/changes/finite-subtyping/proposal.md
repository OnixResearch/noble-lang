## Why

Noble's current checker compares types by structural unification. `unify`
(`crates/noble-contracts/src/inference/unify.rs:100`) equates matching
constructors and Program effects; `apply_signature`
(`crates/noble-contracts/src/source/inference/mod.rs:233`) unifies the root
input and equates the latent effect, with a rigid goal for a declared
universal effect at completion (`source/inference/completion.rs:5`). There is
no value-subtyping engine, and the language deliberately excludes general
subtyping in this revision (`.cairn/specs/language/spec.md:487`).

Issue #29 asks for a bounded language-extension study: when a single best type
is unavailable, represent a finite family of valid candidate types, and explore
target-directed backward checking over that family. The research paper
supplies a starting point for an explicitly annotated F-sub-top calculus; its
All-Top rule and complexity results do not transfer to Noble's ordered stacks,
wrapping `I64`, ownership, and latent effect bounds. K-SAFE-05 and S-GATE-03
permit future principled subtyping only with rules that preserve or strengthen
the safety invariants and with corresponding proof obligations.

The design must be honest about the likely outcome. The only safe,
representation-preserving relation available in a first fragment is effect-bound
inclusion, which is a directed finite lattice; every finite candidate set has a
least upper bound, so a principal candidate may exist and the finite-family
machinery may add nothing. The issue explicitly permits documenting that result.
This change selects the fragment, the declarative relation, the candidate-family
semantics, the backward-checking obligation, the differential evaluation and the
go/no-go criteria, and implements a test-only prototype. The prototype confirms
a principal candidate in every supported family; the selected decision is
**no-go** for production adoption. It does not adopt a type system, change
source acceptance, or alter kernel types or the ABI.

## What Changes

- Select opt-in fragment `finite-bases/v0`: a small type grammar over `Unit`,
  `Bool`, `I64`, `Text`, `Pair`, `Sum`, `List`, `Program`, and exact nominal
  identities; no `Top`, no bounded universals, no higher rank, and no
  resource/LiveRef subtyping. Current source acceptance is unchanged.
  r[FSUB-FRAG-01]
- Specify formation, declarative subtyping, representation-preserving
  subsumption, ordered-stack variance, effect inclusion versus equality, Program
  input/output variance, exact nominal/generic identity, and the unchanged
  resource, ownership, capture and borrowed-reference restrictions.
  r[FSUB-REL-01]
- Specify whole-candidate families, shared variable identities, sound
  generation, coverage, finite covers, compositional operations, termination
  versus bounded exhaustion, ambiguity, and the backward-checking elimination
  interface and inverse substitutions. r[FSUB-CAND-01]
- Specify identity, versioning and compatibility: candidate ordering and
  internal IDs MUST NOT affect public identity; an effect widening creates an
  adapted Program witness while retaining the recipe; no kernel/ABI change is
  assumed. r[FSUB-ID-01]
- Specify a separate explicit-family reference checker and differential
  evaluation against the production checker, with metered construction and
  queries, a distinct exhaustion outcome, deterministic/seeded corpus, negative
  controls, benchmarks and receipts. r[FSUB-EVAL-01]
- Implement the test-only `crates/noble-contracts/tests/models/finite-bases.rs`
  prototype and `verification/finite-subtyping/run.mjs` receipt lane. The
  prototype compares the explicit-family and backward checkers over 277 finite
  cases, exercises real `result.noble`-style interfaces and records the no-go
  decision. No production checker or source acceptance changes.
  r[FSUB-CAND-01] r[FSUB-EVAL-01]
- Specify the proof obligations (decidable subtyping, sound generation,
  coverage, finite covers, compositional completeness, termination, backward
  elimination, algorithm/declarative equivalence) and the explicit go/no-go
  criteria. A principal-type result, if confirmed, selects **no-go** for
  production adoption and retains the current checker plus K-EFFECT-05.
  r[FSUB-PROOF-01]

## Impact

- **Files**: `.cairn/changes/finite-subtyping/`, the test-only
  `crates/noble-contracts/tests/models/finite-bases.rs` prototype and
  `verification/finite-subtyping/run.mjs` plus its receipt. ADDED `language`
  deltas `FSUB-FRAG-01`, `FSUB-REL-01`, `FSUB-CAND-01`, `FSUB-ID-01` and ADDED
  `verification` deltas `FSUB-EVAL-01`, `FSUB-PROOF-01`.
- **Code**: test-only. No production checker, kernel type, candidate encoding,
  CLI, ABI, canonical hash or persistence change.
- **Testing**: `cargo test -p noble-contracts --test source -- finite_bases` passes. The
  receipt records 277 cases, 277 principal families, zero ambiguity, maximum
  family size 16, and the `no-go` decision. Existing source regressions are
  retained unchanged.
- **Coordination**: #15 owns the correctness foundation. #28 supplies the
  independent oracle for the existing effect solver and does not depend on this
  extension. #3 owns ownership and lifetime design; #13's physical-size
  safeguards are retained. #30 is an independent merge relation.
