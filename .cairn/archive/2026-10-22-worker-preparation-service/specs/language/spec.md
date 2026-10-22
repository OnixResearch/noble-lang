# Worker-Typed preparation service

## MODIFIED Requirements

### Requirement: P-PREPARE-03
r[P-PREPARE-03]

**P-PREPARE-03.** Preparing code is an explicit host operation. Its permitted code-store access, compilation, allocation, and cache writes belong to the compiler capability's documented contract. These service effects are distinct from the candidate program's latent effects. The compiler service MUST NOT run arbitrary user code as an undocumented checking or macro-expansion step.

For the selected local Worker-Typed profile, the explicit host preparation service takes only candidate source bytes. The host selects the schema module, the exact `State Event -- State List<Action> ! {}` interface, the root export, finite limits, the package encoding and a new destination. The service MUST complete the same schema, root, closure and interface checks as admission, and one worker compilation, before it publishes; a refusal MUST publish nothing. It MUST report its input reads, checker stages, closure walks, compilations and package writes in a service trace separate from any evaluation report, and MUST NOT start a worker engine or run the candidate body. A published package remains an untrusted description: evaluation re-admits it locally and makes no preparation request.

#### Scenario: Worker-Typed local preparation service

- GIVEN the exact WorkerData@1 schema and transition source of the accepted WORKER-02 baseline, and alternatives that fail parsing, the closure walk, the interface check or the schema check
- WHEN each is prepared by the explicit host service, and the baseline package is then evaluated in a separate invocation
- THEN the baseline package is byte-identical to the accepted WORKER-02 baseline and evaluates normally with zero guest requests; each alternative publishes no package and its trace shows no stage after its refusal; the service trace reports no evaluation and the evaluation report no preparation
