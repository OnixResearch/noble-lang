# Worker-Typed bounded execution profile

## MODIFIED Requirements

### Requirement: H-LIMIT-02
r[H-LIMIT-02]

**H-LIMIT-02.** A bounded execution profile MUST define accounting units, interruption points, and a maximum guest-work interval between interruption checks. Coverage MUST include pure loops, calls, runtime builders, data traversal, and allocation. An empty effect bound MUST NOT exempt a computation from limits. Nested invocations MUST remain within their enclosing allocation and execution budgets.

Host operations require separate duration and outstanding-work limits. A host deadline does not prove that native work stopped. Unstoppable work retains an owner and consumes its declared quota until retirement completes. The profile MUST specify bounded diagnostics and observation storage. Exhaustion MUST NOT silently increase a budget or become an ordinary success.

For the selected local Worker-Typed profile, revision `worker-typed/step-4096/interrupt-8` charges one unit for each dispatched operation, initialization item and captured-data traversal node against a 4,096-unit execution ceiling, and one unit for each reflection or type-observation node against a separate 4,096-unit observation ceiling. Every eighth unit charged on either counter MUST be an interruption point: the guest reports its total charged work to a host check, and the host answers continue, cancel or deadline. The host MUST record the work between consecutive checks itself; the profile is violated, and the execution an internal failure, if it observes more than eight units between checks or after the last one. Nested `run` and runtime-built Programs MUST draw from the same counters and recipe limits as their caller. Deterministic work, allocation or recipe exhaustion, cancellation at a check, a host deadline at a check, a guest trap and a host internal failure MUST each be a distinct outcome, and none of them exposes a typed output. A host deadline is wall-clock time, checked only at interruption points, and never reads as work exhaustion.

#### Scenario: Worker-Typed interruptible execution

- GIVEN finite pure, domain-error, over-budget call-tree, over-limit builder and nested-`run` transitions, and host-selected cancel, zero-deadline, guest-fault and host-fault controls at chosen checks
- WHEN each runs once through the selected Worker-Typed engine
- THEN the finite and domain-error transitions return normally with typed output, every budget case exhausts while each part within budget returns normally, each control produces its own outcome at its check without typed output, and the host observes no more than eight units of guest work between checks
