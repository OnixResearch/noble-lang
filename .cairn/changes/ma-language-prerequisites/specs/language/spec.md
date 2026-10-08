# Explicit owner-bound recursion syntax

## MODIFIED Requirements

### Requirement: K-CHECK-02
r[K-CHECK-02]

Nonrecursive named definitions may generalize eligible variables not fixed by their environment. Recursive definitions require an explicit signature and check recursive calls against that signature without polymorphic recursion. The full inference algorithm and formal soundness argument are required follow-up work; this draft does not claim them established.

In the selected signed declared-module profile, `signature WORD forall<S:stack> [ ... ]` before `export WORD` and `def WORD [ ... WORD ... ]` binds the unqualified recursive occurrence to that exact enclosing immutable definition only. All occurrence instantiations share the owner's one checked signature substitution, including occurrences nested in quotations. Absent, duplicate, non-exported or mismatched signatures reject before publication; other unqualified words follow ordinary lexical resolution. No mutual recursion is selected.

### Requirement: K-DEF-03
r[K-DEF-03]

A definition declaration does not execute its body. Namespace changes are tooling operations outside ordinary pure guest evaluation.

For self-recursion, a checked declaration carries an explicit interface and a distinguished self reference within its identity scope. This revision permits no mutual-recursion groups. The signed declared-module syntax and owner-bound checked self node in K-CHECK-02 select concrete annotation and canonical self-reference encoding *only* for the new opt-in profile; unselected profiles and portable general reflection remain open. A reflected self reference that leaves its owner context must retain or be resolved with its owner identity; a bare self token cannot be rebound accidentally during preparation.

The source-name spelling of a definition is not the definition's semantic identity. Type schemas, builtin semantics, host contracts, and dependency identities participate in the identity rules in section 12. Rebinding a display name never changes a previously admitted owner or its recursive call.
