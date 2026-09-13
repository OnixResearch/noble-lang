# Current decision register

Document: DEC-N001  
Revision: 0.1.0-draft.5  
Status: Selected direction for a new implementation

This register consolidates current decisions. The old registers remain historical sources under [SOURCES.md](SOURCES.md). Their identifiers do not imply missing active documents.

## Foundation

| ID | Decision | Consequence |
|---|---|---|
| ND-01 | Noble starts from scratch, as confirmed by the user | No repository recovery task or assumed prior implementation |
| ND-02 | This directory is the canonical working family | Archived previews do not govern new work |
| ND-03 | Use `run`, `reflect`, and `#` | No automatic `call`/`reify` aliases or `//` comments |
| ND-04 | Preserve first-class checked programs, latent effects, and resolved recipes | No literal-only implementation or hidden source compilation |
| ND-05 | Begin with a finite rank-1 checker subset | Advanced types remain possible later, not permanently rejected |

## Safety and identity

| ID | Decision | Consequence |
|---|---|---|
| ND-06 | Preserve no guest unsafe, no language UB, and recursive ownership | Host representations require checked boundaries |
| ND-07 | Every guest-requested host operation belongs to the effect bound, including denied requests and reviewed deterministic WIT imports | Neither authority denial nor a purity review erases a boundary request |
| ND-08 | Resolved versioned WIT operations are semantic dependencies | Their changes affect dependent definition/program identity |
| ND-09 | Build-only adapters remain separate from semantic identity | Identical WIT shapes alone do not prove identical Noble semantics |
| ND-10 | The first borrow support is synchronous, adapter-local, and owner-threaded | Borrowed exports, escape, same-owner reentrancy, and suspension are unsupported |

ND-08/09 replace the blanket `WD-14` build-only statement. An unrelated WIT world input can change the build key without changing definitions that do not reference it.

Canonical program encoding and stable digest vectors remain open. New stack-owned content identity uses BLAKE3 unless an interoperability contract requires another algorithm. The snapshot SHA-256 manifest is only an integrity record. Algorithm selection alone does not close the encoding gate.

## Delivery and assurance

| ID | Decision | Consequence |
|---|---|---|
| ND-11 | Prototype checker extraction and dynamic Wasm builders early | Toolchain feasibility precedes broad component implementation |
| ND-12 | Deliver synchronous components before full native async | Each partial profile names its exclusions |
| ND-13 | Keep implementation, execution, proof, and trust separate | A failed executed test never counts as acceptance |
| ND-14 | Give every numbered requirement an evidence route | An unimplemented route remains visibly open |
| ND-15 | Preserve original sources and requirement IDs | The new revision records changes without replacing historical evidence |

## External-study adaptations

The user requested integration of the [external study](../review/EXTERNAL-DESIGN-STUDY.md). These decisions select practices and experiments, not the external projects' entire designs.

| ID | Decision | Consequence |
|---|---|---|
| ND-16 | Compare WasmGC and managed linear memory during M3 | SPEC-BE001 requires bounded evidence before representation selection |
| ND-17 | Adopt cited native-boundary arguments and scoped Miri checks | VT-NATIVE-01/02 supplement, not replace, the existing proof routes |
| ND-18 | Separate byte validity from semantic acceptance | S-DECODE-01/03 keep zerocopy optional and canonical encoding independent |
| ND-19 | Use checked branch facts and Rust domain types | Better diagnostics without implicit union joins or a new guest type system |
| ND-20 | Preserve ownership and explicit contract snapshots | No unmanaged guest escape regime, GC cleanup substitution, or disabled boundary checks |

WIT marshaling remains explicit. General guest borrows, scoped futures, general runtime assertion syntax, guest nominal declarations, and units of measure remain later work. Self-hosting and a Zena compiler dependency are not selected.

## Aeneas-first implementation

| ID | Decision | Consequence |
|---|---|---|
| ND-21 | Target Charon → Aeneas → Lean 4 for all Noble-owned production Rust | Verification shapes implementation from the start, beyond selected checker utilities |
| ND-22 | Make this route mandatory for the entire semantic kernel | Safe sequential Rust, explicit state, no kernel exceptions for unsupported code |
| ND-23 | Separate deterministic resource/runtime decisions from external effects | Handle-table transitions use Aeneas; host release and synchronization require concrete boundary evidence |
| ND-24 | Retain Verus only for reviewed non-kernel exceptions | No default Verus component or mandatory Verus toolchain |
| ND-25 | Gate verification claims on complete source and proof coverage | Extraction, refinement, external models, exceptions, and open work remain separate counts |

The user's whole-project direction replaces the earlier checker-only preference and mandatory Verus resource-table assignment. [IMPL-V001](VERIFICATION-TOOLCHAIN.md) defines scope and exceptions. Lean remains the proof language. Aeneas does not replace Noble syntax, Rust compilation, or the Noble-to-Wasm correctness obligations.

## First-class verification

| ID | Decision | Consequence |
|---|---|---|
| ND-26 | Make typed contracts and first-class evidence companions a supported optional profile | Contract declarations are compiler inputs, not comments or host-only reports |
| ND-27 | Preserve ordinary `Program<S,T,e>` and explicit admission | No implicit behavioral coercions, new dependent kernel, or mandatory application proofs |
| ND-28 | Compose evidence and instantiate runtime builder families through proved finite rules | No proof search inside ordinary builders or runtime rule replay |
| ND-29 | Integrate verification, explanation, and proof-required builds | Exact claims, distinct outcomes, untrusted proof producers, and independent consumer policy |
| ND-30 | Deliver pure contracts before resource protocols | MC1 follows M2; MC2 follows MC1 and M4, with no M5 dependency |

[PROGRAM-CONTRACTS.md](PROGRAM-CONTRACTS.md) owns the logical and API contracts. Concrete grammar, companion representation, and encoding remain entry gates, not frozen features. This direction supersedes the metadata-only starting scope. Applicability checks and executable guards remain separate from erased logical snapshots and optional proof terms.

## Developer-experience amendment

| ID | Decision | Consequence |
|---|---|---|
| ND-31 | Require stack-aware diagnostics; select non-executable editor holes | Diagnostics join M2; hole grammar remains open and holes cannot pass admission |
| ND-32 | Select opaque domain types and exhaustive variants within the existing data mechanism | Declaration/module gates remain open; wrappers preserve resource eligibility |
| ND-33 | Provide Result composition through ordinary library programs | No exception effect, implicit resource discard, or new propagation syntax |
| ND-34 | Use resolved recipes for dependency tools, semantic diffs, and evidence caches | Cache reuse binds exact claims and assumptions; hashes do not grant authority |
| ND-35 | Select explicit scripted test-host substitution | Preserve effects and authorization; no real-host fallback or general effect handlers |

[DEVELOPER-EXPERIENCE.md](DEVELOPER-EXPERIENCE.md) defines the contracts and delivery gates. These are design adaptations, not upstream compatibility or implementation claims. ND-32 selects the later guest-type direction without moving it into bootstrap.

## Language-workflow amendment

| ID | Decision | Consequence |
|---|---|---|
| ND-36 | Select optional lexical local names lowered to checked stack operations | Syntax, shadowing, capture identity, and lowering gates remain open; no mutable cells |
| ND-37 | Select capability-aware module interfaces | Linking fixes dependencies without guest execution; required effects do not grant authority |
| ND-38 | Require bounded property tests with reproducible shrinking | Well-typed generation uses independent acceptance; malformed fuzzing and proof claims stay separate |
| ND-39 | Select later resource protocol types | Transitions account for every owner and retain runtime checks; ambiguous outcomes do not imply retry safety |
| ND-40 | Require compiler-backed documentation examples | Static and runtime observations remain separate; failures and unexecuted examples stay visible |

SPEC-DX001 sections 6–10 define these additions. Local bindings and modules follow M4. Protocol types also require M5 and declaration interfaces. Property and documentation harnesses accompany the applicable M2/M4 checks. No concrete binding, module, or protocol syntax is selected.

## Exact-calculator amendment

| ID | Decision | Consequence |
|---|---|---|
| ND-41 | Select a programmable calculator as the first AI-authoring reference application | Noble library code owns the mathematical engine; the shell owns terminal effects |
| ND-42 | Use arbitrary-precision integers, normalized rationals, and exact decimal input | Division is exact; zero divisors produce errors; `I64` wrapping remains unchanged |
| ND-43 | Select explicit precedence, lexical nonrecursive functions, and state preservation after failure | Calculator syntax is application syntax; declaration commands and library interfaces remain gates |
| ND-44 | Require independent, bounded evaluation of AI-authored changes | Record costs and failures; producers cannot change acceptance criteria or grant authority |
| ND-45 | Keep approximation and symbolic mathematics separate from exact calculator v1 | No silent float conversion, expanded bootstrap, or unearned proof claim |

[CALCULATOR.md](CALCULATOR.md) owns these application contracts and the MA1–MA3 delivery gates. The calculator selects integer exponentiation, including negative exponents and `0^0 = 1`. This convention does not define real exponentiation. No runtime or benchmark result is claimed.

## Worker-contract amendment

| ID | Decision | Consequence |
|---|---|---|
| ND-46 | Normalize runtime-selected programs through typed interfaces and explicit effect widening | No `Any`, implicit schema conversion, or new agent syntax |
| ND-47 | Require unchanged-context round-trip laws and explicit package dependency closure | Encoding remains open, and import reestablishes local admission |
| ND-48 | Bound candidate decoding, resolution, acceptance, and diagnostics under one request budget | Exhaustion fails closed without candidate execution or partial publication |
| ND-49 | Select explicit task ownership and serialized cancellation/delivery transitions | Late completion cannot restore revoked owners or imply rollback |
| ND-50 | Require bounded execution and distinct correlated outcome records | Pure code has limits, and incomplete observations do not prove absence of effects |

[Worker conformance](WORKER-CONFORMANCE.md) describes MW1/MW2 and their unexecuted controls. This slice does not replace the calculator or close the full checker, package encoding, async ABI, or Syndicate profile gates.

## Octet contract amendment

| ID | Decision | Consequence |
|---|---|---|
| ND-51 | Separate plans, host-established one-shot witnesses, attempts, observations, and receipts | Pure allow data cannot mint authority, and success receipts require applicable observations |
| ND-52 | Require nominal domain identities, units, and invariant-preserving construction paths | No aliases, defaults, or decoders can silently bypass the declared invariant |
| ND-53 | Require explicit state/event coverage for designated finite lifecycle protocols | Invalid transitions remain explicit, and changed schemas invalidate old coverage |
| ND-54 | Use resolved contracts for Noble effects and explicit Octet architecture policy for Rust | Typed higher-order calls remain supported, and source-token evidence cannot replace semantic checking |
| ND-55 | Bind imported evidence to exact inputs and preserve its original role | Partial checks, artifact integrity, and fixture analysis cannot become whole-kernel proof |

[Octet adoption](OCTET-ADOPTION.md) records the inspected revision and delivery gates. This amendment leaves concrete policy files and compatible pins open. It does not import Octet implementation code or move Noble to Verus.

## Platform direction

Component Model and WIT remain the standard external boundary. WASI 0.3 remains the preferred host family, with exact versions selected through executed compatibility work.

Syndicate/Synit and Preserves remain the concurrency and protocol direction. BEAM/OTP supplies secondary operational lessons, not primary concurrency semantics.

Choreography must target the selected concurrency layer. Durability must record observable nondeterminism and ambiguous outcomes. Neither is part of the first core milestone.

## Open decisions

1. Complete inference, candidate encoding, recursive checking, and declaration syntax.
2. Canonical semantic bytes, recursion identity, and portable package transport.
3. Internal Wasm calling convention, generic lowering, memory management, and session recovery.
4. Exact component type mapping, borrowed exports, async lifetimes, and tested toolchain pins.
5. Full Syndicate transitions, bounded Preserves adaptation, choreography projection, and durability contracts.
