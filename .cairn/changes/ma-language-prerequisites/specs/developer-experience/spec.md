# Additive signed, pure declared text profile

## MODIFIED Requirements

### Requirement: DX-MODULE-04
r[DX-MODULE-04]

Module admission MUST derive effect requirements from resolved definitions, host contracts, and trusted instantiated program interfaces. Each invocation MUST have an applicable interface with a conservative effect bound. An unresolved operation or unavailable contract MUST NOT become an empty effect set or a name-based purity exemption.

A typed indirect call can use its trusted `Program<S,T,e>` bound without enumerating every possible runtime program value. Supported recursive calls use their checked signatures. Effect closure does not prove termination. Dependency traversal and constraint work remain bounded under K-CHECK-07.

A provider summary cannot erase a known direct effect or override the admitted contract for another identity. Rust source-token analysis can supply separate implementation evidence. It cannot replace Noble's resolved effect judgments or backend correspondence.

Only explicit host selection of **both existing** `--declared-modules --text-byte-cursor` creates the new pure declared-text profile, distinct from standalone `Text-Byte-Cursor-v1` and ordinary `Declared-Modules-v1`. This profile uses the exact original disjoint 27-row cursor semantics at IDs 0..26 and adds only one pure signed-I64 `<` as Definition(27) with scheme `forall<S:stack> [ S I64 I64 -- S Bool ! {} ]` and signed mathematical comparison of the two I64 inputs. Standalone cursor and Core MUST NOT resolve or admit `<`. The combined profile rejects all bound host operations, host adapter files, ambient `test.emit`, and declared requirements for host effects, and does not require a fake empty bindings file. Other existing modes and their independent admission/refusal contracts MUST remain unchanged. Static parsing, the kernel, and the selected Wasm compiler MUST agree on the exact 28-row profile; no source unit or producer-supplied environment may opt into it.

New `--signed-self-recursion` MUST require `--declared-modules` and authorize only a prior explicit signature of the enclosing exported word with one shared owner-body substitution. The source registry MUST retain immutable module/version, definition and lexical dependencies, then independently rederive the candidate from original source at selected compiler ingress. Forged direct submissions, changed owner bytes, wrong source slot or signature, mutual cycles, mismatched generic instantiation and invalid effects MUST refuse before any Wasm publication. Each declared signature in the *pure combined* profile MAY name an already staged fixed local nominal or an exported fixed imported `module@version.Type` (including `Result<I64,Error>`), but unknown, unexported or wrong-version names MUST reject. Complete source work and compared signature keys are metered to the first matching key, with full miss charged before publication.

#### Scenario: Source-checked text digit fold and refusal boundaries

- GIVEN an exported fixed `ParseError`, an exact versioned Result interface and a signed recursive `Text I64 I64 -- Result<I64,ParseError>` fold using `text.byte`, plus malformed UTF-8, wrong-offset and wrong-version controls
- WHEN opt-off/on compile and execute real managed-Wasm children, followed by statically rejected unsigned/cross-owner/wrong-profile submissions and a subsequent valid expression in the same framed session
- THEN genuine bytes, nominal error values, immutable imported callee, zero host requests and state-preserving static refusal are observed; runtime depth/fuel exhaustion is a distinct terminal failure, not a typed calculator ResourceLimit or MA1 acceptance

### Requirement: DX-LOCAL-01
r[DX-LOCAL-01]

In the opt-in pure combined declared-Text profile, `let name` MUST consume the
top value as an immutable lexical binding scoped to the current root body,
definition or quotation. Initial `let` forms MUST preserve ordered function
parameters; duplicate names in one scope MUST refuse, while a nearer
quotation-local binding MAY shadow an enclosing spelling. Lexical reads MUST
lower to existing kernel-checked `dup`/`swap`/`dip` operations, and end-of-scope
discard MUST use checked `drop`; unchanged words still undergo the ordinary
resolver and type/effect check. No binding may introduce a mutable cell, a
runtime name lookup, or a new kernel/managed-Wasm primitive. A resource
without `Data` eligibility MUST NOT be copied or implicitly discarded.
Ordinary declared modules and standalone cursor MUST NOT acquire `let`.

#### Scenario: Ordered checked function parameters

- GIVEN a fixed declared signature `[S I64 I64 -- S I64 ! {}]` and function body `let right let left left right +`
- WHEN the source-derived checked owner compiles and executes that function with `5 6` in opt-off and opt-on guest sessions
- THEN the result is `11`, malformed or duplicate binders refuse before publication, and failed submissions preserve prior session stack and namespace

### Requirement: DX-LOCAL-02
r[DX-LOCAL-02]

Lexical references to enclosing values inside a quotation MUST capture the
original checked values at quotation construction, not reinterpret names
under a later global binding. Capture MUST compose the original `quote` and
`compose` operations so the retained program recipe visibly identifies the
checked captured values and ordinary kernel `Capture`/`Data` eligibility
still applies. A live resource MUST NOT be captured or duplicated. Source
re-derivation at the selected compiler ingress MUST reject provider-forged
local expansions and stale owner/module bytes.

#### Scenario: Scoped captures and resource refusal

- GIVEN immutable lexical shadowing, nested quotation captures, qualified immutable module references, and resource duplication/discard/capture counterexamples
- WHEN the checker compiles the valid cases and analyzes each counterexample in the same opt-in profile
- THEN quotations retain original captured values after shadowing/rebinding, while each ineligible resource action refuses without publishing a program or changing session state
