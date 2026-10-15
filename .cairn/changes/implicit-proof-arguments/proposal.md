## Why

Explicit Noble source proofs repeat information that the checker could reconstruct. Instantiating the accepted polymorphic reflexivity proof at `(List I64)` today requires `(apply (apply (use poly-refl) (List I64)) x)`, although `(List I64)` is already fixed by the sort of `x`. The CONTRACT-18 template likewise restates the subject's literal in `(exec-literal 1)`. Issue #12 asks for a bounded design and conformance plan, not an implementation. The design covers an authoring layer that omits only uniquely reconstructible arguments, exposes the fully explicit term, and leaves acceptance with the existing explicit checker and the independently pinned Lean consumer. r[IMPL-ELAB-01]

## What Changes

- Select inference fragment `implicit-args/v1`. It covers two things:
  - Argument positions that the author explicitly marks for inference, and the implicit `Type0` prefix of an explicit-spine application of an exactly resolved proof.
  - Structural parameters of reviewed template rules that the exact subject fixes.

  Constraints are first-order equations over `PureTyCode` terms, value levels and `Pi`/`Eq` propositions. They are solved by syntactic unification with an occurs check. The only definitional equality used is the checker's existing structural identity. A reconstruction is unique exactly when the most general unifier assigns every metavariable a ground, well-scoped term. r[IMPL-ELAB-01]
- Never infer proof witnesses, motives, bundles or theorem identities, and never search the local context, imports or any global instance table. Underconstrained, conflicting, unsupported and exhausted elaborations request an explicit argument at their source span and report the unresolved parameter, its constraints and any listed candidates. Listed candidates are never chosen. r[IMPL-ELAB-01] r[IMPL-TRUST-01]
- Expose every inserted argument, the selected theorem/rule identity, the instantiated claim, assumptions and dependencies through an inspect/copy-expanded form. Elaboration is deterministic and carries a recorded `implicit-args/v1` revision and stable source mappings. Fully explicit authoring stays available unchanged. r[IMPL-EXPOSE-01]
- Treat the elaborator as an untrusted producer. Its explicit output passes the existing source checker and the pinned Lean consumer against the separately generated exact claim. Its work is charged to the shared checker limits. It performs no search during ordinary `quote`/`compose`/`run`. r[IMPL-TRUST-01]

## Impact

This change adds a design-only active change with ADDED `IMPL-*` requirement deltas for `program-contracts` and `developer-experience`. The unexecuted scenario designs `IMPL-CASE-01` through `IMPL-CASE-20` live in `design.md`. Nothing else changes:

- No parser, checker, CLI or Lean library.
- No conformance JSON or canonical spec.
- No status, roadmap, generated view or ledger.
- No concrete syntax is selected.
- No proof, implementation or execution result is claimed.

The change depends on the goal view of `proof-automation-diagnostics` (#6) to display inserted arguments. It coordinates with #7 law bundles only by refusing any implicit bundle selection.
