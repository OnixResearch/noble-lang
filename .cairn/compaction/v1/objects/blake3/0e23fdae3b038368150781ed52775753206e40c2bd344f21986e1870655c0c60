# M2 checker feasibility acceptance delta

## ADDED Requirements

### Requirement: VT-M2-01
r[VT-M2-01]

M2 MUST define a versioned finite candidate schema and finite acceptance rules for a named Core-Bootstrap checking fragment.
The candidate MUST carry resolved nodes, claimed node interfaces, instantiations, schema references, effect and eligibility witnesses, and its format and semantic revisions (K-CHECK-06).
The rules MUST check stack interfaces in order at their full declared arity, quotation construction separation, latent effect propagation and inclusion, recursive eligibility, and branch joins, deriving each conclusion from its premises rather than trusting a claimed result.
Acceptance MUST distinguish accepted, invalid, unsupported, resource-exhausted, and internal-failure outcomes, and MUST enforce every declared size and work limit before an allocation or traversal exceeds it (B-CHECK-07, B-RESULT-01).
A rejected submission MUST NOT execute a candidate body or issue a candidate-body host request (B-RESULT-02).

#### Scenario: Accepted candidate derivation

- GIVEN a finite candidate within its limits and an independently supplied expected interface, environment, and allowed effects
- WHEN the acceptance checker checks every node against the declared rules
- THEN acceptance records the derived interfaces, effect bound, and eligibility conclusions for the named fragment

#### Scenario: Rejected and unsupported candidates

- GIVEN a malformed join, a mis-shaped stack, a hidden effect, a noncapturable capture, and an unsupported node or type form as separate mutations
- WHEN the checker evaluates each mutation
- THEN it returns the matching non-accepted outcome, identifies the failing join with expected-versus-actual stack shapes, and executes no body

#### Scenario: Exhausted limits

- GIVEN candidates that exceed each declared size or work limit
- WHEN the checker evaluates them
- THEN it fails closed with the exhausted outcome before the limit is exceeded

### Requirement: VT-M2-02
r[VT-M2-02]

M2 MUST extract the actual acceptance representation and at least one nontrivial validation function through the mandatory Charon → Aeneas → Lean route.
A proof about a separate handwritten checker MUST NOT establish Rust correspondence (B-IMPL-02).
Rust interfaces MUST distinguish semantic identities, build identities, owner contexts, untrusted candidates, and accepted programs with domain-specific types, and construction or decoding MUST NOT bypass acceptance (B-IMPL-03).
The M2 checker MUST use ordinary owned semantic data within the confirmed extraction subset; byte-layout views stay in decoding adapters (VT-AENEAS-01, B-IMPL-01).
One proved validation function MUST NOT close the whole-kernel inventory started here (VT-SCOPE-05).

#### Scenario: Extracted acceptance representation

- GIVEN the selected compatible toolchain and the named M2 kernel functions
- WHEN the extraction entry point regenerates the representation and the selected validation function
- THEN the generated module compiles and its record binds the actual Rust source, inventory, and configuration identities

#### Scenario: Handwritten substitute

- GIVEN a theorem stated about a separate handwritten checker or a stale extraction artifact
- WHEN the extraction or refinement gate evaluates the claim
- THEN it rejects the claim without crediting the extracted implementation

#### Scenario: Identity boundaries

- GIVEN an untrusted candidate byte view, a decoded candidate, and an accepted program
- WHEN Rust construction, decoding, or public conversion paths are inspected and tested
- THEN no path admits a value into the accepted-program role without acceptance, and representation equality alone does not make the roles interchangeable

### Requirement: VT-M2-03
r[VT-M2-03]

M2 MUST provide a Lean reference model for the named fragment and at least one nontrivial refinement theorem that connects the extracted Rust functions to the intended contract (PO-11, VT-AENEAS-04).
The reference model MUST state the declarative judgments independently of the extracted implementation; the refinement MUST relate the extracted functions' observable outcomes to those judgments for the full named fragment, not a reduced or rewritten body.
The required-refinement gate MUST reject a missing proof, a proof hole, an unexplained external model, a timed-out check, and a theorem about a substituted subject.
The theorem MUST be reported as refinement of the named fragment; it MUST NOT close whole-kernel verification, Wasm correspondence, or unrelated obligations.

#### Scenario: Fragment refinement

- GIVEN the extracted acceptance functions and the independent Lean reference judgments for the named fragment
- WHEN the refinement target builds with the pinned toolchain
- THEN the theorem relates accepted outcomes to derivable judgments and non-accepted outcomes to their declared rejections within the fragment

#### Scenario: Unfinished or substituted proof

- GIVEN a missing proof, a proof hole, a stale subject binding, or a theorem about a handwritten or reduced substitute
- WHEN the proof-required gate evaluates the claim
- THEN it rejects the claim and the obligation remains open

### Requirement: VT-M2-04
r[VT-M2-04]

M2 MUST extend the compiler-derived inventory and extraction gate to cover every new kernel function with separate extracted, proved, modeled, excepted, and open reporting (VT-SCOPE-05).
The M2 gates MUST reject a missing function, an unsupported kernel body, an unexplained external model, an incomplete required refinement, and a required coverage case that is not executed (VT-CI-05).
Coverage statuses MUST NOT be promoted from tests, extraction, or document checks alone.

#### Scenario: Complete M2 coverage

- GIVEN the updated reviewed classification and compiler-derived inventory for the M2 functions
- WHEN the coverage gate compares body sets, extraction records, and proof records
- THEN each M2 function reports its extracted, proved, modeled, excepted, and open status separately

#### Scenario: Incomplete coverage rejection

- GIVEN an omitted function, a macro or generated body, an unsupported required body, or a refinement marked proved without a building proof target as separate mutations
- WHEN the coverage or extraction gate evaluates each mutation
- THEN it rejects the mutation without weakening the required scope
