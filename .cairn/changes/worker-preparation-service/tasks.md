## Phase 1: Implementation

- [ ] [serial] Split typed admission into shared schema, root, closure and transition checks; move the package encoder into production and cut the tests over to it. r[P-PREPARE-03]
- [ ] [serial] Add `noble worker prepare` with create-new publication after every check and the `noble-worker-preparation/v1` service trace. r[P-PREPARE-03]

## Phase 2: Verification

- [ ] [serial] Scoped tests: the prepared baseline is byte-identical to the accepted WORKER-02 baseline package and runs through `typed`; each refusal stage publishes nothing and enters no later stage; an existing destination is never overwritten; preparation and evaluation are separate CLI invocations. r[P-PREPARE-03]
- [ ] [serial] Validate Cairn; record the entry gate as partial in the roadmap without promoting a WORKER case or removing WORKER-11's open gate. r[P-PREPARE-03]
