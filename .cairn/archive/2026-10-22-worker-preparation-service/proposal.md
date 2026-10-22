# Worker-Typed preparation service

## Why

MW1 requires an explicit preparation service. The worker conformance slice prepares candidate syntax against the independent worker interface and exact schema environment, and keeps the preparation trace separate from evaluation. The local Worker-Typed v1 profile has no production preparer: its packages come only from a test-only builder and the WORKER-02 gate's own encoder, and `noble worker typed` re-checks and evaluates in one invocation. r[P-PREPARE-03]

## What Changes

- Add `noble worker prepare SOURCE --schema SCHEMA --out PACKAGE`. Only the source bytes come from the candidate. The host selects the WorkerData@1 schema module, the exact `State Event -- State List<Action> ! {}` interface, the `transition` root export, finite limits, the package encoding and a new destination. r[P-PREPARE-03]
- Share one implementation of the schema catalog, root module, closure and transition checks between preparation and `typed` admission, and move the package encoder from test code into production. A refused candidate publishes nothing. The package is written create-new only after every check and one worker compilation succeed. r[P-PREPARE-03]
- Report a separate `noble-worker-preparation/v1` service trace. Its effect counters are incremented where the service reads an input, enters a checker stage, walks a closure, compiles and writes. The service never starts a worker engine; `typed` stays a separate invocation that re-admits the untrusted package. r[P-PREPARE-03]

## Impact

- **Files**: `crates/noble-cli/src/core/worker_typed.rs`, CLI dispatch and usage text in `crates/noble-cli/src/main.rs` and `crates/noble-cli/src/core.rs`, CLI integration tests in `crates/noble-cli/tests/worker_typed.rs`, and the roadmap entry-gate record.
- **Testing**: scoped `noble-cli` unit and CLI tests, the unchanged WORKER-02 gate, Cairn validation, and the source-bound flake gates before publication.
- **Non-claims**: no guest-visible `code.prepare` word or compiler capability value (P-PREPARE-04), no v2 package preparation or export, no portable or remote package identity, no WORKER case promotion. WORKER-11 keeps `preparation-service` among its open gates until milestone review.
