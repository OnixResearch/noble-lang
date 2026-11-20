# Opt-in live commands

## ADDED Requirements

### Requirement: DX-LIVE-01
r[DX-LIVE-01]

The tool MUST expose separate `noble live repl` and `noble live watch SOURCE` with explicit `:reload FILE` and `:generation`, source spans, checked stack/engine/generation report and no ambient guest reload/file/compiler authority. The only exception is K-LIVE-02/H-LIVE-04's narrowly host-granted guest proposal enqueue, never direct guest file reload, compiler access or permission grant. Strict interpreter selection MUST refuse until accepted; ordinary run/session semantics do not change.

The shipped commands accept `--allow-effects test.emit,test.abort`: an exact,
nonempty subset of the two resource-free Core test hosts, refused together with
`--declared-modules`. It is a host selection, never read from source text: an
ungranted live session still refuses both words as unbound. Checked admission
retains Core's effect identities (0 and 1); emission is limited to the selected
subset; every actual request is still authorized at execution. `test.emit` is a
protected test operation. `test.abort` is a declared abnormal termination: it
performs no protected operation, retains the prior effect prefix and poisons the
session. OWNER DECISION 2026-11-20 (Decision 22): "The live REPL grants
`test.emit`/`test.abort` only through an explicit flag `--allow-effects
test.emit,test.abort` (exact non-empty subset; refused together with declared
modules); DX-LIVE-01's usage line is amended to include it. LIVE-05 runs on the
shipped CLI. Live-slot sessions are unchanged."
