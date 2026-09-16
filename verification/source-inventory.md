# Compiler-derived source inventory

**Tasks 2.3 and 2.4 are complete.** The `source-coverage` check collects the compiler-derived inventory and compares it with a reviewed classification.

This is a named-scope accounting control, not M1 acceptance and not a refinement proof.

## Commands

Reserved interface, now implemented:

```sh
source-inventory collect --root DIR --selection FILE --artifact-dir DIR
source-inventory check --root DIR --selection FILE --inventory FILE --policy FILE --artifact-dir DIR
```

The check runs both:

```sh
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' \
  build .#checks.x86_64-linux.source-coverage -L --out-link "$EVIDENCE/source-coverage"
nix run .#source-inventory -- collect …
nix run .#source-inventory -- check …
```

## Ownership and mechanisms

[The reviewed policy](../policy/source-inventory.ncl) owns the expected classification.
Its JSON export is checked, not refreshed, by the `policy` check.
`nix/source-inventory-derive.nix` derives the inventory from collector artifacts and performs no I/O.
`nix/source-inventory.nix` is the pure comparator over reviewed expectations and compiler-derived data.
`nix/source-inventory-app.nix` is the thin execution and file adapter.
Octet owns the compiler facts, coverage report, and Cargo graph.

The inventory comes from `cargo-octet check` on the real workspace with all targets and features.
Nothing in the inventory is hand-written: units, subjects, macro origins, dependency edges, and the compiler coverage report are compiler-derived.

## Classification

Each compiler-derived production subject is reviewed as one of three categories.

| Category | Meaning | Disposition |
|---|---|---|
| `body` | A Noble-authored production body | `extracted`, `modeled`, `excepted`, or `open` |
| `generated` | A harness or closure item with no independent Noble obligation | `generated` |
| `structural` | A type declaration or import marker | `structural` |

Bodies additionally carry a refinement status: `proved` or `open`.
Only bodies enter the extraction, modeling, exception, proof, and open counts.

The current named M1 scope is 6 bodies, 4 generated items, and 6 structural subjects across 16 reviewed paths and 17 compiler items.
All six bodies are `open`: the kernel transition has a compatibility probe but no source-bound integrated extraction gate, and the shell is not an M1 extraction subject.
Zero bodies are extracted, modeled, excepted, or proved.

Tests, macro origins, dependencies, tools, and future components are classified separately.
Test-labelled subjects keep the test role and cannot discharge a production obligation.

## Rejection controls

The comparator rejects, with a named diagnostic:

- incomplete compiler coverage, missing, unexpected, or duplicated units;
- a changed required target, feature set, or default-feature state;
- an undeclared production package or source scope;
- an omitted or stale production subject;
- a subject bound to the wrong package;
- an invalid disposition or refinement status;
- an unclassified or stale test subject, macro origin, dependency edge, or tool;
- an excepted body without a record, an incomplete record, or any kernel exception;
- a future component claimed as verified, or listed as a production subject.

Twenty-five synthetic self-tests cover these rules, including separate target, feature, and default-feature mutations.
They are harness self-tests, not compiler evidence.

## Limits

The reviewed policy is a static expectation. The build never refreshes it.
Renewal is an explicit reviewed action bound to the compiler-derived set at review time.
The inventory covers the selected target and feature configuration only.
Structural and generated categories are accounting, not Noble-authored bodies.
No entry establishes refinement, native safety, dependency safety, or whole-project verification.
