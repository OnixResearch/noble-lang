# Wiring laws and known-graph optimization authority

## ADDED Requirements

### Requirement: WIRE-LAW-01
r[WIRE-LAW-01]

Every wiring law MUST name:

- its observation relation: normal result, recipe, ordered finite-prefix trace, ownership, failure and cleanup, quota, or identity and proof subject;
- its supported profile;
- its typing and eligibility premises;
- its unsupported cases.

A law about normal returns MUST NOT be reported as finite-prefix safety, abnormal cleanup, termination or contextual equivalence. This includes any context where `reflect` or identity can distinguish the Programs.

Laws MUST be stated modulo quota. Where either side exhausts a quota, no equality is claimed.

Effect-set union MUST NOT justify any of the following:

- commuting effects;
- swapping effects;
- duplicating effects;
- parallelizing effects.

The evaluation order of product, fanout and choice combinators MUST be fixed.

Behavioral equivalence MUST NOT establish recipe, definition or proof-subject identity. It MUST NOT transfer evidence between distinct subjects. Such transfer requires a checked derivation bound to the new subject.

A packaged law bundle MUST NOT grant `Data` or `Capture` eligibility, commute ordered effects, or transfer exact-subject evidence. Resource-frame laws MUST stay unpackaged until their ownership premises and evidence exist.

### Requirement: WIRE-GRAPH-01
r[WIRE-GRAPH-01]

An optional known-graph analysis MUST admit only Programs that meet all of the following:

- the closure of immutable named definitions is fully resolved;
- every invoked operand is statically known;
- no control flow depends on `reflect`;
- captures are `Capture` data;
- recipe size and depth are within the declared limits.

The analysis MUST run statically after acceptance, or as an explicit host analysis of a constructed Program's recipe. It MUST NOT run inside ordinary `quote`, `compose` or `run`. Unsupported Programs MUST remain ordinary Programs.

Graph shape MUST NOT establish termination, GPU eligibility or profitable parallelism.

An optimization that retains the logical recipe MUST preserve:

- reflection output;
- effect order;
- abort propagation;
- the declared failure classification.

It requires semantic and artifact correspondence evidence for the executed artifact.

A rewrite that changes the source recipe creates a new subject. It requires fresh evidence or a checked transformation that names its relation. Equal results or hashes MUST NOT transfer proof authority.
