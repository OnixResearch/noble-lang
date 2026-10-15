# Wiring-law evidence plan

## ADDED Requirements

### Requirement: WIRE-EVID-01
r[WIRE-EVID-01]

Each executable wiring-law case MUST record:

- the exact sources, bindings and inputs;
- the pinned toolchain and runtime;
- expected observations stated independently from the specification before execution;
- the compared relation and any ignored fields;
- independent implementation, execution, proof and trust fields.

The corpus MUST cover:

- left and right identity;
- nested associativity;
- frame preservation;
- pair and unpair routing;
- both choice branches;
- ordered effects;
- rejected resource fanout, discard and capture;
- incompatible intermediate stacks;
- wrong branch joins;
- stale or wrong-subject evidence;
- ordinary errors, traps, cancellation and quota exhaustion, wherever the selected profile supports them, with guest and host cleanup responsibilities stated.

Each law MUST map to its reused reviewed Lean rules. It MUST also map to the additional theorem, model, implementation-refinement and artifact-correspondence obligations it needs. Passing finite cases MUST NOT be reported as a general law proof.
