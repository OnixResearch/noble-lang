## Context

The selected backend is M4's managed-linear-memory Core-Bootstrap implementation. MC1 supplies immutable prepared contracts, exact statement export, strict independent Lean checking and reusable semantic rules. MC2 must preserve ordinary execution and exact compiled program observations while adding optional checked companions.

## Decisions

### Decision: Complete observations, not verifier callbacks

**Choice:** `Core::begin_admission` returns an opaque request borrowing the exact prepared contract. The application shell executes the isolated independent checker. `CheckObservation` contains the complete checked statement, source bytes and result; `Core::complete_admission` validates that observation against the request and current retained context before issuing evidence.

**Rationale:** The deterministic core remains `no_std`, bounded and free of process/I/O callbacks. Invalid classes, ineligible data, stale requests and mismatched observations fail closed. Arbitrary host Rust can construct an observation: authenticity and checker soundness are explicit host assumptions, not an unforgeability property of a public Rust constructor. Imported guest bytes and producer status flags never become such authority directly.

### Decision: Preserve live program and cell identity

**Choice:** Contract/Evidence/Certified use concrete runtime cells with retained metadata and compiled injection/observation. Composition and instantiation operate on live compiled programs. The host parks and restores the exact original operand cells on all operation outcomes, including intermediate preparation failures.

**Rationale:** Reconstructing a subject from source or a reflected recipe would not demonstrate first-class compiled transport. Preserving the original cells also preserves nested aggregate tails and handles. Reflection stays inert; it does not fetch evidence or invoke a prover.

### Decision: Bounded replay and explicit applicability

**Choice:** Replay uses a finite versioned rule set, checked premises and context, bounded acyclic traversal and exact derived descriptors. Logical quantities use named boundary records rather than interchangeable scalar parameters. Policy exhaustion is terminal; no revision wraparound can revive stale evidence. Invocation requires an installed guard for the exact retained contract and current evidence.

**Rationale:** Failed admission or applicability must not run the candidate body. Accepted partial-correctness evidence does not promise termination, execution fuel, resource ownership or host authority. Unsupported predicates/guards remain explicit refusals rather than implicit acceptance or proof search.

### Decision: Separate source and evidence limits

**Choice:** CLI source preparation keeps the 64 KiB bound; admission permits the declared 512 KiB proof-source envelope, still bounded by the independent consumer's own decoding/registry/work limits. File reads enforce their byte limit during reading, not only through prior metadata.

**Rationale:** A valid bounded proof wire must reach the consumer's real exhaustion and mismatch controls. Rejecting it at an unrelated smaller frontend limit would be a proxy test, not evidence for the named consumer control.

### Decision: Release only an exactly bound compiled artifact

**Choice:** A proof-required build requires a source submission representing the exact accepted inert quotation, successful backend production and applicable evidence. Release metadata binds the actual source/WAT/Wasm artifacts and labels the backend as trusted, not verified. A supported guard wrapper applies the retained program rather than silently rebuilding a different proof subject.

**Rationale:** A proof for one recipe cannot authorize unrelated Wasm with the same interface. All non-proved outcomes remain distinct and block release; failed checking is not disproof.

### Decision: Independent evidence lanes

**Choice:** Runtime acceptance exercises all canonical cases and variants and retains raw inputs, commands and observations. Each executable is copied into a unique run directory before use and rehashed at completion. Semantic Lean certificates, strict extracted correspondences, native closed source equations, generated-body accounting and full compiler-policy gates remain separately reported scopes.

**Rationale:** Expected answers cannot substitute for observations, and a mutable build target cannot identify all subprocesses in a receipt. Fresh discovery is a review candidate; independent check must bind the reviewed lock to current authored sources, actual generated code, models, declarations, dependencies and axioms. Native equation assumptions are disclosed separately from strict logical axioms.

## Risks / Trade-offs

- The pinned compiler collector requires explicit Result matches and bounded loops rather than unresolved `?`/`for` desugarings. The existing `attempt!` convention is retained; no callback trait or fabricated default body hides a required unknown.
- The pinned Aeneas route requires transparent fallible loop bodies and shared-only branch helpers. Source repairs preserve refusal ordering and work charges rather than weakening extraction coverage.
- Independent proof checking requires real sandbox services and can exhaust its configured wall-clock budget. There is no unsandboxed or cached-status acceptance fallback.
- Full source extraction and successful examples do not prove every production body or universal lowering correctness. The final ledger must state only the named proved fragments and leave broader PO-16/20/21, PO-17/18 and SO-07 claims open where unsupported.
