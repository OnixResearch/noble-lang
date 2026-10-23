# Typed Worker Program v2

The selected `worker typed` local pure v1 is intentionally closed: its package cannot carry captures, effectful host contracts, artifact authority, or runtime-selected Programs. Keep that entry and its receipts unchanged. Add an explicitly selected MW1 `worker program` v2 boundary for actual source-checked, owner-bound Program values, runtime typed builders, finite dependency packages, explicit export authority, and bounded task observations. MW2's independent worker-send WIT/native shell is not redefined here.

## Scope and authority

Use the existing ModuleSession/source checker and accepted candidate/recipe as the only semantic source. Guest-provided IDs, WIT bytes, digests, flags, and compiled artifacts are claims, never acceptance. Host-selected schema, dependencies, destination grants, effect ceilings and invocation budgets are independent inputs. Checked runtime operations may adapt a program to a *wider* effect witness without modifying its recipe; neither adaptation nor reflection grants an effect. Import performs fresh local checks. No new kernel syntax, implicit Any, ambient fetch, fallback checker, unbounded work or unchecked executable artifact.

WORKER-02 through WORKER-07, WORKER-10 and WORKER-12 have exact retained execution evidence; WORKER-09's MW1 rows execute and its native-work/suspension rows remain MW2. MW1 completion additionally requires the schema-selected transition to run against a genuine guest/component host and retained entry-gate evidence. An API or document gate alone does not promote a case or close a gate.
