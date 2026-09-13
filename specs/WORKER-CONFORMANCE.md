# Typed-worker conformance slice

Revision: 0.1.0-draft.5  
Status: Unexecuted harness design, not a language profile or an agent runtime

## Purpose and ownership

This slice connects five existing contracts without agent-specific kernel syntax:

1. Runtime-selected typed interfaces and checked effect widening.
2. Recipe round trips and explicit package dependencies.
3. Bounded admission of generated code.
4. Async ownership through cancellation and completion.
5. Bounded execution and distinct failure observations.

[SPEC-0001](SPEC-0001.md) owns language, admission, identity, and execution-boundary rules. [Resource adapters](RESOURCE-ADAPTERS.md) own local task transitions. [Module contracts](DEVELOPER-EXPERIENCE.md) own exported schemas and adapters. This document describes tests of those rules, not an additional semantic authority.

The [worker cases](conformance/worker-cases.json) use the `Worker-Design` test lane. Their schemas and operations are symbolic harness inputs. They do not select concrete Noble declarations, candidate bytes, task APIs, or portable package framing.

## Worker interface

The harness admits a program with this instantiated interface:

```text
State Event -- State List<Action> ! {}
```

The harness supplies exact, application-owned schema identities for these conceptual variants:

| Schema | Payload |
|---|---|
| `State` | Submission count as `I64`, plus an optional pending attempt identity |
| `Event` | `Submit(attempt, text)` or `Cancelled(attempt)` |
| `Action` | `Send(attempt, text)` |

These types contain only capturable data. An attempt identity is ordinary data, not a credential or a task handle. The shell creates identities and supplies them explicitly. A separate move-only capability remains in the shell's resource context.

The worker contract uses this exhaustive transition table over the declared input schemas:

| Input state and event | Output state and actions |
|---|---|
| No pending attempt, `Submit(id, text)` | Increment count with `I64` wrapping, retain `id`, return one `Send(id, text)` |
| Pending attempt, another `Submit` | Return unchanged state and no actions |
| Matching pending attempt, `Cancelled(id)` | Clear the pending attempt, preserve count, return no actions |
| No pending attempt or mismatched `Cancelled` | Return unchanged state and no actions |

The count measures submissions, not successful external sends. This is deliberately a cancellation-focused test application. It is not a complete service protocol or a termination proof about its implementation.

## Main scenario: WORKER-01

1. Supply the candidate syntax after the coordinator compiles. Prepare it against the independent worker interface and exact schema environment.
2. Start from count zero and no pending attempt. Supply `Submit(attempt-1, "ping")` and observe count one plus one inert action.
3. Admit that action under an independently authorized shell capability. Transfer the owned resource to a pending async task.
4. Commit cancellation before result delivery. Supply the corresponding `Cancelled(attempt-1)` event and observe count one with no pending attempt.
5. Deliver a late native completion. Observe retirement progress, no restored guest owner, and no second worker event or successful result delivery.

Preparation has a separate service trace. Worker evaluation emits zero host requests. The shell action has its own operation-attempt record. Cancellation does not roll back an external send or prove that the send failed.

The shell keeps the native pin charged after cancellation acknowledgement. It releases that pin only after native access ends. The cleared application state does not erase the shell's retirement obligation.

## Protected-effect extension

The shell action follows H-AUTH-01–04 and H-RECEIPT-01–02. The pure worker's action does not contain a live witness. An authorized host establishes the witness from independently validated facts and binds it to the exact action and context.

Admission consumes witness availability before the protected send. A late external observation can support a scoped operation receipt without changing the cancelled task outcome. [Octet adoption controls](conformance/octet-adoption-cases.json) cover denial, stale bindings, replay, and invalid success-receipt construction.

Nominal attempt/generation types and explicit lifecycle matrices supplement the current worker controls. They do not turn application attempt identities into credentials or add a new concurrency mechanism.

## Required controls

| Cases | Discriminating observations |
|---|---|
| WORKER-02/03 | Reject incompatible interfaces and capture of resources. Admit explicit effect widening without source compilation during dispatch. |
| WORKER-04/05/12 | Preserve unchanged recipes and exact dependencies. Reject invalid imports and unauthorized export. |
| WORKER-06/07 | Accept within-limit candidates. Fail closed on each exceeded limit, malformed witness, or unsupported input. |
| WORKER-08 | Cover both cancellation/delivery orders, cancellation of buffered results, domain errors, quota denial, and stale callbacks. |
| WORKER-09/10/11 | Bound pure computation, preserve failure/observation distinctions, and retain incomplete implementation status. |

Each matrix entry needs a separate result. An absent or unsupported entry is not passed coverage. The document validator checks fixed expectations and mutation controls. It does not implement any harness or execute Noble.

## Delivery gates

MW1 follows M4 and M5. It requires full worker-interface checking, declared schemas/modules, an explicit preparation service, package encoding, and a bounded execution profile. Its pure and admission cases retain those gates even though some scalar precursors fit bootstrap.

MW2 follows MW1 and M6. It executes the integrated cancellation scenario and the async race matrix with the actual resource adapter. M6 retains its independent native-async ABI and ownership obligations.

Neither milestone blocks M1 through M4 or replaces the calculator as the first reference application. A single-worker harness does not implement a swarm scheduler. Syndicate/Synit remains the selected concurrency direction. Supervision, task leases, distributed delegation, durability, and safe remote retries require separate profiles.

All cases remain `implementation: absent`, `execution: not-run`, `proof: open`, and `trust: unassessed` until evidence exists.
