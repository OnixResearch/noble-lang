# Opt-in live commands

## ADDED Requirements

### Requirement: DX-LIVE-01
r[DX-LIVE-01]

The tool MUST expose separate `noble live repl` and `noble live watch SOURCE` with explicit `:reload FILE` and `:generation`, source spans, checked stack/engine/generation report and no ambient guest reload/file/compiler authority. Strict interpreter selection MUST refuse until accepted; ordinary run/session semantics do not change.
