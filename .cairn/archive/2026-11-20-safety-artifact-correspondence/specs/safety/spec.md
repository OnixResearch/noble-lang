# Artifact correspondence admission

## MODIFIED Requirements

### Requirement: S-TYPE-02
r[S-TYPE-02]

`Program<S,T,e>` remains the executable interface. `run` MUST only execute a checked/prepared `Program`; `Syntax`, arbitrary bytes, untrusted artifacts, or claimed manifests MUST NOT be directly runnable. On the selected Core-only `noble admit-artifact` path of S-HOST-03, a submitted artifact that passes the pinned validator and engine compilation remains candidate data until its exact final bytes equal the host's pinned compilation of independently accepted, host-invoker-selected source under host-selected options; only then MAY it execute, in a fresh isolated host. Candidate-attached `source`, `digest`, `trusted_correspondence` and `allowed` fields MUST NOT establish correspondence, trust, authority or runnability. That selected path has no trusted-build attestation and no translation-validation input: S-CASE-07's `trusted_build:false` and `translation_validation:false` denote these structural absences, not Boolean inputs, and neither MAY be inferred from candidate metadata. A valid artifact compiled from another program, or whose bytes otherwise differ from that compilation, MUST refuse at admission as `correspondence-reject` before instantiation, with zero guest requests and protected operations. This finite selection does not establish backend lowering or artifact-loading correctness (PO-17/PO-18), cross-host provenance, signature or proof-object admission, declared-binding artifacts or general Wasm loading.

#### Scenario: S-CASE-07 forged correspondence metadata is not runnable

- GIVEN host-selected source `1 2 +` with pinned `--opt off`, and a pinned-validator-valid Core artifact compiled from `2 2 +` whose correct empty effect claim carries forged `source`, `digest`, `trusted_correspondence` and `allowed` metadata
- WHEN the selected admission path validates, reflects and compares its exact final bytes with the host's independent compilation before any instantiation
- THEN it refuses as `admission`/`correspondence-reject` with zero guest requests and protected operations; the same bytes are admitted only when the host selects their actual `2 2 +` source, and the unmodified `1 2 +` artifact executes to `I64(3)`
