# Opt-in live Wasm namespace

## ADDED Requirements

### Requirement: K-LIVE-01
r[K-LIVE-01]

Reload MUST build a new immutable namespace and transitively rebuild affected checked dependent definitions. Only subsequent *new top-level* name resolution sees new identities. Old frames, saved Programs and lexical captures MUST keep exact old dependencies; no live slot inside prepared code or in-place Program mutation is introduced. Failed or incompatible rebuild publishes nothing.

### Requirement: K-LIVE-02
r[K-LIVE-02]

Only the opt-in live profile adds effectful `self.generation : S -- S I64 ! {live.observe-generation}` (fresh live ID 4) and `self.propose : S I64 Program<[I64] -> [I64] ! {}> -- S Unit ! {live.propose}` (fresh live ID 3). Generation observes the checked invocation's host source-generation snapshot, not a mutable untracked pure callback or Wasm submission count. A guest MAY construct the pure candidate through checked `quote`/`compose`; the import synchronously snapshots bounded reachable Program/recipe/capture cells and referenced bytes, with no Wasm reentry or candidate compilation/execution/publication. Post-return reflection/interface admission MUST compare original snapshotted bytes against the same live cells and refuse mismatch. Source-controlled material MUST NOT identify the authority-bearing originating definition.

A pure typed I64→I64 Program is necessary, not sufficient: the bounded host decoder admits only closed I64 literal/capture events and pure resolved arithmetic builtin IDs 4/5/6 (`+`, `-`, `*`) with checked root/invocation signature IDs and ordered stack transitions. Identity `[ ]`, checked recipes containing `dup`/`drop`/`swap`/`=`, arbitrary Syntax and unrecognized Program encodings refuse. No general PushGP code mutation/crossover/fitness semantics is selected.

### Requirement: H-LIVE-01
r[H-LIVE-01]

Separate opt-in persistent REPL and watch MUST use stable bounded host-selected source/module snapshots and a complete-file transaction. Success gives next-top-level freshness without process restart or external assembly/optimizer, not zero preparation latency. Existing run/session remains unchanged.

### Requirement: H-LIVE-02
r[H-LIVE-02]

Only fully accepted, binary-validated, interface-compatible generations MAY publish atomically at a top-level safepoint; shadow staging MUST NOT instantiate against mutable live VM imports or execute guest bodies. No-start/no-active-initializer bytes MUST install into fresh bounded slots of the same shared live Wasm arena without executing bodies; failure leaves old namespace, stack and host authorizations unchanged.

### Requirement: H-LIVE-03
r[H-LIVE-03]

The session MUST keep only ABI-compatible typed stack and immutable old Programs; it MUST prohibit implicit state/resource migration, bound retained generations and enforce independent current host authorization. Runtime traps report actual effects and poison the first live session.

### Requirement: H-LIVE-04
r[H-LIVE-04]

Guest self-edit is an explicitly host-granted narrow transaction. Initial `--self-edit NAME --expect-generation N` and later host-only `:grant-self-edit NAME N` MUST bind the selected existing definition name, its independently checked immutable originating definition identity and source generation. The trusted compiler embeds the already checked lexical named owner's identity in admitted executable import metadata, not guest source/argument/recipe fields; an unowned top-level self.propose refuses before execution and an old saved Program MUST fail against a grant rearmed for a newer owner even when it observes the current generation. `live_propose` validates grant and generation, enqueues at most one bounded pending candidate per top-level invocation (duplicates cannot overwrite it or commit twice), and reports pending separately from committed/refused. A grant supports at most one successful CAS; an unauthorized callback is an actual host denial subject to terminal runtime policy, not a static or post-return refusal. After normal top-level return the host alone decodes supported closed recipe to source, independently source/type/effect/kernel/byte-checks and shadow-stages, then CAS-publishes one generation or refuses before the next top-level submission. Publication never mutates the selected file: guest-derived source digest/origin and unchanged file bytes/hash are separately recorded (equal source bytes may have equal digests). Later host `:reload FILE` independently checks the current effect ceiling; effectful file source cannot replace a pure guest successor without separate explicit authority. Post-return candidate refusal changes no namespace/grants or candidate-body effects, but does not roll back the completed invocation's stack or host-request prefix. A trap cancels pending candidate, preserves prior effect prefix and applies terminal runtime policy; static pre-invocation refusal remains unchanged. First bounded self-edit refuses dependents or incompatible changes without stale ACK while broader K-LIVE-01/LIVE-02 dependent rebuilding remains open.
