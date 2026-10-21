# Keep signed owner recursion outside bootstrap

## MODIFIED Requirements

### Requirement: B-CHECK-02
r[B-CHECK-02]

The acceptance environment MUST bind exact builtin/host/schema/definition identities to checked contracts. External environment data requires separate validation before use. Recursive definition dependencies and user-declared recursive schemas are unsupported in Core-Bootstrap. The fixed bootstrap `List` schema is supported.

The separately selected signed declared-module profile MAY admit one explicit self-reference only as a distinguished owner-scoped node in an explicitly signed exported declaration, never as a generic cyclic dependency edge or from an untrusted candidate's advertised environment. Independent acceptance MUST recheck the complete instantiated owner signature and shared self-call substitution against its immutable checked definition; cross-owner, mutual, unsigned and polymorphically recursive calls MUST reject. The source, candidate and checked body MUST retain the resolved owner identity, and runtime execution remains subject to finite invocation and memory limits. This extension does not change bootstrap dependency acceptance or supply a general recursive schema.
