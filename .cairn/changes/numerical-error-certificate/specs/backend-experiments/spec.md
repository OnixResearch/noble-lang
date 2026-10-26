## ADDED Requirements

### Requirement: NGRAPH-ERR-01
r[NGRAPH-ERR-01]

A numerical-error certificate MUST cover a finite acyclic add/multiply fragment
over explicit finite inputs, with no overflow, division, branch discontinuity or
training-derivative claim. Every constant and input bit pattern MUST be
interpreted exactly, and a separately defined intended real graph MUST preserve
the stored machine operation order. The ordered affine fold and the piecewise
operations MUST be excluded from the first fragment because their rounding
layers remain open under #38. The input relation MUST be stated explicitly:
either real inputs are the exact dyadic values of the supplied binary32 inputs,
or the certificate includes a separately bounded input-rounding error. Initial
representation error MUST NOT be silently omitted.

#### Scenario: input representation error is not omitted

- GIVEN a certificate whose input relation names only intermediate rounding
- WHEN the certificate is checked
- THEN it MUST be refused, because a bound over already-rounded inputs MUST NOT
  be reported as a bound from the reals

#### Scenario: unsupported operation set

- GIVEN a graph containing the ordered affine fold or a piecewise operation
- WHEN a certificate is proposed for it
- THEN the certificate MUST be refused as unsupported for this fragment rather
  than checked with an implicit assumption

### Requirement: NGRAPH-ERR-02
r[NGRAPH-ERR-02]

An untrusted analyzer MAY propose rational input ranges, per-node real and
machine range enclosures and absolute-error bounds. A total Lean checker MUST
accept or refuse the certificate, and the proved statement MUST be that an
accepted certificate implies the stated per-output absolute-error bound for every
input satisfying the checked premises. The checker's abstract rounding and range
model MUST be related to #38's binary32 reference by a proved bridge statement for
the covered fragment; the bridge MUST NOT be assumed, and analyzer search MUST
NOT stand in for the proof.

#### Scenario: analyzer search is not the proof

- GIVEN an analyzer that reports a plausible bound but whose certificate the
  checker refuses
- WHEN the result is reported
- THEN the outcome MUST be a refusal rather than a bound, and the analyzer's
  report MUST NOT be cited as evidence

#### Scenario: unchecked bridge

- GIVEN a checker statement that relies on the binary32 rounding bridge while the
  bridge is not established for the covered fragment
- WHEN the obligation is registered
- THEN it MUST remain open rather than being recorded as proved

### Requirement: NGRAPH-ERR-03
r[NGRAPH-ERR-03]

Subnormal and gradual-underflow behavior MUST be handled by a proved
absolute-error rule, or the certificate MUST establish the stronger
no-subnormal premise mechanically for the declared domain and refuse unsupported
domains. A real-range interval alone MUST NOT be treated as evidence that every
rounded intermediate satisfies that premise. Flushing subnormals to zero,
weakening the execution contract or silently widening the premise MUST NOT occur
in either case.

#### Scenario: unproved underflow premise

- GIVEN a domain that admits subnormal intermediates with no proved absolute-error
  rule
- WHEN applicability is checked
- THEN the certificate MUST be refused as unsupported for that domain rather than
  certified under an unproved premise

#### Scenario: premise checked mechanically

- GIVEN a declared no-subnormal domain
- WHEN applicability is checked
- THEN the premise MUST be established by checked enclosure reasoning over every
  intermediate rather than by sampling

### Requirement: NGRAPH-ERR-04
r[NGRAPH-ERR-04]

A certificate MUST bind the exact graph and parameter snapshot, ordered
interfaces and outputs, constants, input domain, input-error model,
round-to-nearest-even and overflow policy, and the semantic, model and checker
revisions together with its assumptions. A consumer MUST establish applicability
for the actual inputs before advertising a certified bound, and any graph or
snapshot edit MUST invalidate the previously issued certificate.

#### Scenario: snapshot edit invalidates the certificate

- GIVEN a certified graph whose constants or outputs change
- WHEN the consumer presents the old certificate for the edited snapshot
- THEN the certificate MUST be refused rather than re-interpreted for the new
  bytes

#### Scenario: applicability before advertisement

- GIVEN inputs outside the certified domain
- WHEN a consumer advertises a bound
- THEN the advertisement MUST be refused because applicability was not
  established for those inputs

### Requirement: NGRAPH-ERR-05
r[NGRAPH-ERR-05]

Certificate parsing, rational sizes, range operations, checking work and
retained evidence MUST be bounded. The `unknown`, `unsupported` and `exhausted`
outcomes MUST remain distinct from a disproved bound, and an oversized or
truncated certificate MUST be refused rather than partially checked.

#### Scenario: unknowns are not successes

- GIVEN a checker run that exhausts its declared work budget
- WHEN the outcome is reported
- THEN it MUST be `exhausted` rather than a verified or disproved bound

#### Scenario: oversized certificate

- GIVEN a certificate exceeding the declared rational or parsing bounds
- WHEN it is submitted
- THEN it MUST be refused without partially checking the admissible prefix
