# Octet contract adoption

Revision: 0.1.0-draft.5  
Status: Selected amendment with bounded M5 host-authority execution; broader adoption remains open

## Scope

Noble already requires a pinned, deny-all Octet lint gate for its Rust workspace. This amendment adds explicit architecture, authority, lifecycle, nominal-domain, and evidence contracts. It does not translate the entire Rust lint catalog into Noble language rules.

The [M5 receipt](../verification/m5/acceptance.json) executes OCTET-01 through
OCTET-04 and every declared variant for `Component-Sync-Bootstrap`, with explicit
[host-operation classifications](../.cairn/specs/wit-wasi/spec.md#bootstrap-host-operation-classification).
This is not general worker/native-async delivery, a universal authority/receipt
proof, or selection of an external Octet policy service.

| Adopted contract | Noble owner | Requirement IDs |
|---|---|---|
| Protected plan, witness, attempt, observation, and receipt boundaries | [Language host contracts](SPEC-0001.md) | H-AUTH-01–04, H-RECEIPT-01–02 |
| Nominal identities, units, and validated constructors | [Developer experience](DEVELOPER-EXPERIENCE.md) | DX-TYPE-03–04 |
| Explicit finite lifecycle matrices | [Protocol types](DEVELOPER-EXPERIENCE.md) | DX-PROTOCOL-03 |
| Resolved dependency/effect admission | [Module contracts](DEVELOPER-EXPERIENCE.md) | DX-MODULE-04 |
| Rust architecture enforcement and scoped tool evidence | [Toolchain](VERIFICATION-TOOLCHAIN.md), [evidence](EVIDENCE.md) | VT-OCTET-01–03, EV-BIND-01, EV-TIER-01 |

These are additions to existing owners, not a separate language profile or competing source of semantics.

## Protected effect walkthrough

```text
explicit facts + plan
  -> pure policy decision (data only)
  -> authorized host witness establishment
  -> fresh binding checks and one-shot admission
  -> attempt
  -> validated outcome observation
  -> receipt for that exact claim
```

The host validates authority facts independently of the worker. An allow decision, a plan hash, or a typed record is not a live authorization witness. Any guest-visible witness follows Noble's existing resource rules.

Admission binds the operation, exact plan and arguments, actor/owner context, policy revision, and required current validity facts. A successful commit consumes witness availability before the protected operation starts. A preflight failure preserves valid unconsumed obligations. A later failure does not restore a consumed witness.

A receipt constructor needs an applicable observation, not merely an attempt. The claim distinguishes external operation success from worker invocation success. A cancelled worker can have a confirmed external effect without regaining a successful return or a live owner.

The [worker slice](WORKER-CONFORMANCE.md) retains a pure transition function. Its shell now has explicit authorization and receipt-constructor obligations. Candidate authorization is not an undocumented step inside builders or preparation. Compiler-service authorization remains explicit and separate from candidate execution.

## Language and Rust boundaries

Nominal domain declarations distinguish attempts, task generations, budget units, durations, and byte counts. Checked constructors and import validation establish their declared invariants. These are ordinary library/module contracts, not general refinement inference.

Finite lifecycle coverage names every state/event constructor pair. It includes explicit invalid-transition outcomes. This rule does not require a closed schema for every external protocol or impose FSM syntax on ordinary Noble programs.

Noble effect admission uses resolved references and trusted program interfaces. A higher-order call can retain a complete effect bound without enumerating all future program values. Unknown contracts cannot silently become pure.

Octet's Rust architecture policy supplements the full lint gate. It declares roles, providers, ports, protected executors, and constructor ownership. Compiler facts and required unknowns bind the exact build configuration. Unsupported binaries, targets, or features remain explicit coverage gaps.

Source-token effect closure, function-address baselines, and artifact checks retain their own evidence limits. An identity baseline is not a finding allowance. A verified artifact hash does not establish authorization, runtime determinism, or proof of the claimed behavior.

## Delivery

| Milestone | Added obligation |
|---|---|
| M1 | Activate scoped architecture policy and its negative controls. Preserve full deny-all lint scope and the Aeneas route. |
| M5 | Establish protected host operation and observation/receipt boundaries for the implemented component subset. |
| M6 | Cover every declared async lifecycle pair, including invalid transitions and cancellation races. |
| MW1 | Execute nominal-domain, resolved-effect, and protected-effect controls after the existing declaration/service gates. |
| MW2 | Connect cancellation, consumed witnesses, late observations, and task-result restrictions in the worker harness. |

These obligations do not add dependencies to ordinary M4 delivery or make behavioral proofs mandatory for ordinary programs. Concrete witness APIs, policy files, evidence adapters, and compatible pins remain implementation gates. The calculator remains the first reference application.

The [Octet adoption scenarios](conformance/octet-adoption-cases.json) remain unexecuted. Validator mutation tests protect their expectations but do not execute Octet, Noble, Charon, Aeneas, or Lean.

## Source and reuse boundary

The inspected Octet documentation comes from revision `cf04e894e53eb0947230118a086ef6066ddba38c`. The selected local documentation files were unchanged relative to that revision during inspection.

- [Project architecture policy](https://github.com/OnixResearch/octet/blob/cf04e894e53eb0947230118a086ef6066ddba38c/docs/project-architecture-policy.md)
- [Type-driven domain modeling](https://github.com/OnixResearch/octet/blob/cf04e894e53eb0947230118a086ef6066ddba38c/docs/type-driven-domain-modeling.md)
- [Concurrency and state review](https://github.com/OnixResearch/octet/blob/cf04e894e53eb0947230118a086ef6066ddba38c/docs/concurrency-state-review.md)
- [Effect-closure admission](https://github.com/OnixResearch/octet/blob/cf04e894e53eb0947230118a086ef6066ddba38c/docs/effect-closure-admission.md)
- [Evidence boundaries](https://github.com/OnixResearch/octet/blob/cf04e894e53eb0947230118a086ef6066ddba38c/docs/trust-boundary-evidence.md)

The [strict profiles](https://github.com/OnixResearch/octet/blob/cf04e894e53eb0947230118a086ef6066ddba38c/docs/strict-rust-profiles.md) remain Rust source-shape policies. The [Charon semantic rail](https://github.com/OnixResearch/octet/blob/cf04e894e53eb0947230118a086ef6066ddba38c/docs/charon-semantic-analysis.md) remains fixture-scoped, recorded analysis evidence.

This revision identifies inspected design material, not a selected or tested Noble toolchain. No Octet implementation was copied or linked. Future component reuse requires a published immutable contract, compatible pins, and separate execution evidence. An ambient sibling checkout is not a durable product dependency.
