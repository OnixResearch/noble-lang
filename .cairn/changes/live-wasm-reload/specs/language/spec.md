# Opt-in live Wasm namespace

## ADDED Requirements

### Requirement: K-LIVE-01
r[K-LIVE-01]

Reload MUST build a new immutable namespace and transitively rebuild affected checked dependent definitions. Only subsequent *new top-level* name resolution sees new identities. Old frames, saved Programs and lexical captures MUST keep exact old dependencies; no live slot inside prepared code or in-place Program mutation is introduced. Failed or incompatible rebuild publishes nothing.

### Requirement: H-LIVE-01
r[H-LIVE-01]

Separate opt-in persistent REPL and watch MUST use stable bounded host-selected source/module snapshots and a complete-file transaction. Success gives next-top-level freshness without process restart or external assembly/optimizer, not zero preparation latency. Existing run/session remains unchanged.

### Requirement: H-LIVE-02
r[H-LIVE-02]

Only fully accepted, binary-validated, interface-compatible generations MAY publish atomically at a top-level safepoint; shadow staging MUST NOT instantiate against mutable live VM imports or execute guest bodies. No-start/no-active-initializer bytes MUST install into fresh bounded slots of the same shared live Wasm arena without executing bodies; failure leaves old namespace, stack and host authorizations unchanged.

### Requirement: H-LIVE-03
r[H-LIVE-03]

The session MUST keep only ABI-compatible typed stack and immutable old Programs; it MUST prohibit implicit state/resource migration, bound retained generations and enforce independent current host authorization. Runtime traps report actual effects and poison the first live session.
