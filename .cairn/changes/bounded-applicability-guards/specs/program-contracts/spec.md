# Bounded applicability guards

## ADDED Requirements

### Requirement: GUARD-BIND-01
r[GUARD-BIND-01]

An applicability witness MUST be a nonexecuting logical judgment bound to one exact accepted contract (full independently generated statement, not only a digest), its whole elaborated precondition, the exact subject identity and retained recipe events, the exact ordered input/output types, and the current semantic and policy revisions. It MUST be introduced only by a checked caller proof, by one execution of a selected guard wrapper for that binding, or by an existing explicitly policy-accepted environmental premise. A guard MUST be selected only when the root of the checked precondition matches a selected template exactly on the exact input interface. Checking one conjunct, a nested fragment, a similar predicate, another literal, another contract or a stale revision MUST NOT establish the whole precondition, and an unmatched precondition MUST remain `unsupported` rather than assumed. A dynamic witness establishes applicability only for the single invocation it wraps. It MUST NOT become a guest value, stack type, stored token or `Program` parameter, and composition or derivation MUST NOT transfer it. Refined input types are not selected by this requirement.

### Requirement: GUARD-CORR-01
r[GUARD-CORR-01]

Each selectable guard template MUST have separately stated obligations:
- a recognizer link: the template's precondition holds for a typed input iff the contract's own MC1 `requires` evaluates to true for it;
- an execution law for the fixed guard program;
- soundness and exactness of acceptance;
- the source and live wrapper semantics;
- renderer layout equations for the actual production code;
- a separate backend lowering/loading obligation.

Successful guard execution MUST establish the precondition for the exact values the protected body subsequently consumes, on the untouched stack. Rejection MUST occur before the body's first instruction, with zero guest requests and protected operations. It MUST consume the argument (and the supplied `Program` in the live wrapper) and return `inl (I64 1)`. A trap or quota exhaustion during the guard MUST be reported as such, never as rejection or acceptance. A predicate that can be undefined MUST NOT be selected as a guard and MUST NOT be treated as true. Signed `I64` wrapping comparisons MUST NOT be presented as mathematical-integer bounds. None of these obligations is discharged by tests, matching hashes or extraction success.

### Requirement: GUARD-LIST-01
r[GUARD-LIST-01]

The first bounded extension MUST be a new versioned guard template set that adds exactly `NonEmptyList` and leaves the three v1 templates, their wrappers and their frozen gate unchanged. `NonEmptyList` applies only when:
- the checked input interface is exactly one `List A`, where `A` is a pure Type0 code (`Unit`, `Bool`, `I64`, `Text`, or `Pair`/`Sum`/`List` thereof);
- the precondition root is exactly `(not (is-nil (in x0)))`.

Its guard MUST be the `dup`/`list.case`/`if` wrapper selected in the change design. Length-based, double-negated, emptiness, conjunctive and selector-dependent variants MUST be refused as `unsupported`. Conjunction, sum-tag guards, resource-bearing inputs and derived guards for composed contracts are not selected.

### Requirement: GUARD-FACT-01
r[GUARD-FACT-01]

Static applicability facts MUST be properties of values in proof configurations. They survive `swap`, `dip` and `dup` of `Data` values. They MUST be re-established after construction, destructuring or arithmetic. They MUST transfer through composition only by a proved intermediate implication. They MUST be invalidated by any change to contract, precondition, subject, capture, or semantic or policy revision. Erasing proof metadata MUST retain executable guard code. It MUST NOT turn a statically justified or guarded call into an unconditional certified claim when the justifying metadata is absent.
