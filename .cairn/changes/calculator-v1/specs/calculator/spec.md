# Selected calculator application boundary

## MODIFIED Requirements

### Requirement: CALC-SCOPE-01
r[CALC-SCOPE-01]

**CALC-SCOPE-01.** Noble MUST provide an exact programmable calculator as its first AI-authoring reference application. The calculator MUST use Noble library programs for parsing, validation, and evaluation. A thin host shell owns terminal input/output and session storage.

The selected implementation entry is the opt-in `Calculator-v1` application, not a change to Noble source grammar, Core-Bootstrap, or the arithmetic behavior of `I64`. Each calculator expression and its private guest phases MUST pass Noble source checking, independent program acceptance and selected managed-Wasm compilation before guest execution. The host MUST NOT substitute its own lexical parser, exact arithmetic, name resolver or formatting decisions for guest library programs. Source-module registration and a compiled arithmetic demonstration alone MUST NOT count as full application acceptance.

#### Scenario: CALC-13 selected application entry

- GIVEN the `Calculator-Design` profile and every `CALC-13` implementation-entry input in [the unexecuted case design](../../../../../specs/conformance/calculator-cases.json)
- WHEN the application is reviewed against its required gates and hostile host-only/bootstrap substitutes
- THEN only a genuinely compiled Noble/Wasm application could satisfy every recorded `expected` field; this active delta does not change that case's `state` or `evidence`

### Requirement: CALC-STATE-02
r[CALC-STATE-02]

**CALC-STATE-02.** A submission MUST produce either an accepted value and new session state or an explicit failure. Syntax errors, definition errors, numeric errors, and resource exhaustion MUST leave the prior calculator state unchanged. A function definition MUST resolve and validate its entire body before installation, without evaluating that body.

The shell serializes session submissions. It publishes a new state only after evaluation or definition admission succeeds and bounded formatting succeeds. Earlier successful submissions remain intact. Terminal output failure does not imply rollback of an already committed calculator state.

Concrete variable-assignment and function-definition commands remain a CLI design gate. Structured fixtures describe definitions without selecting their user-facing spelling. Calculator validation is not a substitute for Noble's program acceptance checker.

A selected implementation MAY keep one committed session and run all checked phases of the new line directly against its ModuleSession, compiler and Wasm worker, with its opaque state retained on the typed guest stack. It MUST commit only after the new guest result and bounded display are accepted. After a syntax, definition, numeric, guest quota, precommit native, host-bound or infrastructure failure it MUST restore the worker's whole arena (memory, function table, runtime globals and published signatures) and the host's namespace, compiler and session fields to the last commit, MUST verify the restored arena byte for byte, and MUST end the session if that verification fails. Restore is a trusted engine operation over committed bytes; no guest value selects or supplies it. A subsequent terminal write failure after a successful commit does not roll back that already accepted line.

#### Scenario: CALC-06 transactional lexical session

- GIVEN every `CALC-06` operation and expected prior-state observation in [the unexecuted case design](../../../../../specs/conformance/calculator-cases.json)
- WHEN valid and rejected submissions are serialized against the selected guest session
- THEN committed values and earlier function captures MUST match the case's `expected` fields and failed submissions MUST leave the accepted state unchanged; no execution is claimed here

### Requirement: CALC-BOUND-01
r[CALC-BOUND-01]

**CALC-BOUND-01.** The calculator core MUST receive the expression, immutable environment, and deterministic budget as explicit inputs. It MUST return values, diagnostics, and proposed state without host effects. The shell MUST NOT supply ambient filesystem, network, clock, or process authority to calculator expressions.

Only an explicitly selected calculator entry MAY use its disjoint builtins, checked source/compiler allowances and larger managed-linear-memory cap. Ordinary, Text-byte-cursor, declared-module and LIVE profiles MUST retain their existing identities and memory behavior. Before cloning untrusted structural witnesses or publishing a compiled program, the selected compiler MUST apply borrowed preclone structure checks, independent kernel acceptance for each reachable body, complete environment/identity checks and bounds on emitted programs. It MUST independently bound reachable named bodies at 512 and environment rows at 768, including derived nominal operations, without omitting any reachable row from checking. The selected guest runtime MAY grow from 16 to at most 64 pages with no more than 72,362 monotonically retained handles and distinct immutable Text and transient signature-observation regions; the per-invocation allocation ceiling remains 1,376,208 bytes. No memory reclamation, handle recycling, host numeric computation or ambient guest import is implied. Opaque parser, numeric and session state MUST remain private to the checked guest worker.

The selected `ModuleSession::new_calculator` MAY retain up to 524,288 bytes of checked module source across its registered modules. Each candidate module/request MUST still obey its individual `Limits.bytes` ceiling of 262,144 bytes, and the terminal input ceiling remains 65,536 bytes. The aggregate MUST be checked before parsing or cloning an overflowing candidate module. Ordinary and cursor profiles MUST keep their prior aggregate source ceiling; this selected allowance MUST NOT bypass source checks, node/depth/Work caps, source-bound module identities or opaque nominal identities.

#### Scenario: CALC-13 checked guest boundary

- GIVEN the `CALC-13` required library/runtime gates and hostile substitute variants in [the unexecuted case design](../../../../../specs/conformance/calculator-cases.json)
- WHEN selected and nonselected source, backend and compiled guest boundaries are assessed
- THEN host-only arithmetic, ambient effects, an application parser standing in for Noble acceptance and open prerequisite gates MUST be refused as complete calculator evidence; no CALC-13 review result is recorded by this delta

### Requirement: CALC-LIMIT-01
r[CALC-LIMIT-01]

**CALC-LIMIT-01.** The implementation MUST bound input bytes, numeric digits, intermediate integer bits, syntax depth, call depth, evaluation work, and output size. Bounds MUST apply before unsafe allocation or unbounded work, including parsing, normalization, exponentiation, and formatting. Each exceeded bound MUST identify its category through `ResourceLimit`.

The gate must define budget units and accounting. A host timeout or internal failure remains distinct from a deterministic `ResourceLimit`. No failure can count as a successful calculation. Tests must cover each limit independently.

A selected calculator compiler preparation MUST reserve its request's original Work allowance for admission and kernel acceptance. Exclusively during borrowed structural preflight, before any owned kernel witness clone, every nonzero `Work::charge(amount)` MUST grant exactly that measured amount through the checked calculator ledger; this scoped grant MUST end before admission and MUST NOT enable the later plan grant across kernel cloning. Each borrowed path, type, nominal, witness and actual quotation appearance MUST remain independently bounded and checked before admission; granting during a scan does not certify unfinished checks. The existing bounded environment-work allowance after environment validation and any measured plan-lowering shortfall after independent kernel acceptance and identity MUST use that same ledger. The original allowance and all grants MUST NOT exceed 2,000,000,000 cumulative issued units per preparation. Ordinary and cursor profiles MUST retain their original ungranted preflight accounting. Structural type/stack, signature, source and output caps remain separate and MUST NOT be inflated or labeled guest `ResourceLimit` to conceal a compiler refusal. This preparation budget is charged structural work, not a physical CPU-time theorem. Native engine traps, assembly/compilation refusal, host transport limits and bounded accepted-history replay MUST remain distinct from the seven guest-produced `ResourceLimit` categories. A candidate line that fails any of these checks MUST NOT be reported as an accepted calculation.

#### Scenario: CALC-08 independent categories and failures

- GIVEN each independently exceeded category, adequate alternative budgets and the failed-definition state control in [the unexecuted case design](../../../../../specs/conformance/calculator-cases.json)
- WHEN the checked guest calculator and separately bounded host/compiler run the complete inputs
- THEN the seven guest refusals MUST have the matching typed `ResourceLimit` category, previous state MUST remain unchanged, and host/native failures MUST NOT count as guest quota results or success; no CALC-08 execution is claimed here

### Requirement: CALC-GATE-01
r[CALC-GATE-01]

**CALC-GATE-01.** Calculator acceptance MUST follow M4 and explicit library, declaration/module, iteration or recursion, text-processing, error-schema, and budget gates. The complete application MUST NOT block M1 through M4. Bootstrap arithmetic demonstrations MUST NOT count as an implemented exact calculator.

| Slice | Entry gate | Required evidence |
|---|---|---|
| Bootstrap precursor | M2/M4 | Existing checked `I64` arithmetic and runtime composition, labeled as a narrower slice |
| Exact library | After M4 and library/type gates | Big integers, normalized rationals, exact decimals, checked conversions, and independent limits |
| Calculator v1 | Exact library and expression/session gates | Actual Noble/Wasm parser, lexical functions, exact results, and unchanged state after failure |
| AI-authoring report | Calculator harness and declared authoring tools | Recorded change tasks, held-out acceptance, negative controls, costs, and visible failures |
| Optional proof report | Applicable MC1/MC2 support | Exact subject/model correspondence and accepted evidence under SPEC-V002 |

The [calculator scenarios](../../../../../specs/conformance/calculator-cases.json) remain unexecuted harness designs until each applicable input and expected field is independently observed and recorded. `Calculator-Design` and `AI-Authoring-Design` label application/test lanes, not Noble language conformance profiles. JSON exact-number fields use decimal strings, not portable Noble serialization. [Source provenance](../../../../../specs/SOURCES.md) retains the mathematical-computing references and their limits. Existing M1–M4 evidence is unchanged. This active, unselected change supplies neither a canonical CALC status promotion nor a proof/AI-authoring acceptance claim. Pinned `tools/cairn.mjs` proposal/design/tasks/validate gates are unrun under the explicit no-Nix low-space directive; a later offline local spec check would not substitute for those lifecycle gates.

#### Scenario: CALC-13 prerequisite and substitution refusal

- GIVEN every required gate and hostile substitute in [the unexecuted CALC-13 design](../../../../../specs/conformance/calculator-cases.json)
- WHEN independent review evaluates an actual compiled calculator against that full entry checklist
- THEN each host-only, `I64`-only, ambient-authority and open-library-gate substitute MUST be rejected as a complete calculator; this delta does not change CALC-13's absent/not-run evidence state
