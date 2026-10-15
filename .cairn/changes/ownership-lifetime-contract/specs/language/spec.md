# Ownership-aware stack interfaces and saved Program lifetimes

## ADDED Requirements

### Requirement: OWN-PKG-01
r[OWN-PKG-01]

A saved, returned, captured or otherwise retained `Program` MUST keep its complete executable package alive for as long as any owner of that Program survives. The package comprises:

- compiled code and table entries reachable from its entry;
- every exact named dependency that compiled code reads through a module global, table entry or equivalent indirection, together with the storage holding it, transitively;
- its immutable capture graph;
- the recipe and Syntax cells needed for reflection and identity;
- static data and signature/interface entries;
- its interface, effect, identity, host-contract and artifact metadata.

Exit of the creating invocation, release of module roots, definition replacement, live reload, slot publication, compaction or arena reset MUST NOT remove, overwrite or relocate a package member reachable from a surviving owner without updating every reference to it. Copying or retaining only a Program cell or its capture graph MUST NOT be treated as retaining the package when code still reads external globals or table entries. The package excludes host implementations, live resources, `LiveRef` inputs, registry maps and authorization grants. Retaining a package grants no authority.

#### Scenario: OWN-CASE-06 saved named caller survives module-root retirement

- GIVEN a saved Program whose compiled body calls a named definition of module M through a module global, after M's version and replay owners are released
- WHEN M's module roots are released and the saved owner is later invoked
- THEN the invocation returns the checked result or retirement is refused while the owner lives, and a stale-handle trap or relocated read counts as failure

### Requirement: OWN-BORROW-01
r[OWN-BORROW-01]

Core profiles MUST keep the existing `Data`/`Capture` eligibility and move-threaded resource interfaces as their complete ownership vocabulary. A future profile that admits source-level borrows MUST obtain each borrow only from a lexical borrow scope that holds the owner outside the visible stack and restores it exactly once on normal return. Each scope MUST carry a fresh rigid scope tag. The checker MUST reject any scope body whose output stack, including nested `Program` interfaces, mentions that tag. A borrow MUST NOT satisfy `Data` or `Capture`. In such a profile a borrow MUST NOT enter an aggregate, recipe, capture, continuation, export, serialization or suspension, and MUST NOT be duplicated or discarded by generic `dup`/`drop`. The owner MUST NOT be moved, released or borrowed again through the visible stack while borrowed. Existing move-threaded interfaces MUST remain valid; a borrow form is only an additional, independently checked alternative.

#### Scenario: OWN-CASE-04 borrow escape and resource capture rejection

- GIVEN the resource `dup`/`drop`/`quote`/aggregate variants and the borrow-escape variants of OWN-CASE-04
- WHEN static checking runs
- THEN every variant is rejected before emission with zero guest requests
