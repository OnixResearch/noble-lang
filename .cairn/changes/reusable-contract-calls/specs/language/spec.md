# Named-Contract-Calls-v3 declarations

## ADDED Requirements

### Requirement: PREUSE-GRAMMAR-01
r[PREUSE-GRAMMAR-01]

The opt-in `Named-Contract-Calls-v3` profile MUST add `contract 3` and `proof 3 Name for QualifiedContract3 [ ProofTerm ]` and nothing else. `contract 3` uses exactly the revision-1 fields, field order, binder grammar and MC1 logical vocabulary. The profile MUST refuse a `proof 3 ... :` colon form, a source `params` field, and any `for` reference that crosses revisions.

A revision-3 subject MUST be a closed, nonrecursive, resource-free and pure definition. Its accepted body is a flat sequence of the following elements only:

- `I64`/`Bool` literals
- the pure builtins `+ - * = dup drop swap`
- named calls

Every call interface MUST be `I64`/`Bool` only. The profile MUST refuse quotations, `run`, `compose`, `dip`, `case`, `if`, `reflect`, effects, resources and Program-typed slots as unsupported. It MUST NOT inline or weaken them.

Revision 1 and revision 2 declarations, their checkers, and their accepted semantics MUST remain unchanged. Proof declarations of every revision MUST be erased before code emission. They MUST NOT change any recipe, DefinitionId or runtime behavior.
