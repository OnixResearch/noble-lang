## Phase 1: Specification (complete; change remains active)

- [x] [serial] Record opt-in Wasm live reload, immutable dependent rebuild and next-top-level freshness without changing old Program semantics. r[K-LIVE-01] r[H-LIVE-02]
- [x] [serial] Design explicit REPL/watch entrypoints, bounded file snapshots, in-process binary validation, staged publication and fail-closed interpreter selection. r[DX-LIVE-01] r[BE-LIVE-01] r[S-LIVE-01]
- [x] [serial] Add unexecuted scenarios and separate roadmap/status entries without promoting historical evidence. r[EV-LIVE-01] r[V-LIVE-01]
- [x] [serial] Validate the spec with pinned native Cairn: after the Nix-backed wrapper unexpectedly attempted shared auto-GC, `/nix/store/bvz6kl88qkfi15n8ml1xr7p22syykggd-cairn-0.1.0/bin/cairn validate --root . --policy /nix/store/jqz083ahq10fdrakci9xzvf7bl4nh075-source/cairn-policy/generated/cairn-policy.json` ran directly with immutable policy SHA-256 `62b9ade2bc8af10b2c68776dba8abb374a6c7331aae7ad8bb41ca938ef100d27`, no Nix invocation, 15 GiB free disk and 33 GiB available memory; returned `valid: true`, empty issues/findings on 13 changes and 41 specs. This validates specification structure, not any LIVE implementation, execution, proof or acceptance. r[EV-LIVE-01] r[V-LIVE-01]

## Phase 2: Implementation and acceptance (not started)

- [ ] [serial] Implement and test in-process Wasm encoder/verifier with independently accepted source, exact selected ABI and isolated staging. r[BE-LIVE-01] r[S-LIVE-01]
- [ ] [serial] Implement resident live CLI/reload/watch, complete dependent rebuild, atomic safepoint publication, stable typed stack and bounded code retirement. r[K-LIVE-01] r[H-LIVE-01] r[H-LIVE-02] r[H-LIVE-03] r[DX-LIVE-01]
- [ ] [serial] Exercise every LIVE case against actual source, selected VM and hostile controls with source-bound receipts and separately classify proof/assumptions before acceptance. r[V-LIVE-01] r[EV-LIVE-01]
