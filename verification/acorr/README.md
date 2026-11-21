# ACORR-CASE acceptance gate

`gate.mjs` executes ACORR-CASE-01…12, the positive and negative controls for the opt-in PAA-1
proof-bound artifact admission envelope (`noble admit-proof-artifact`, change
`artifact-correspondence-extension`). It freezes the supplied CLI (copied to `NEW_DIR/bin`, mode
0500), records the clean source commit and tree, emits one pin document per build
(`pins-off.json`, `pins-on.json`, `pins-barrier-off.json`), and prepares every negative candidate
outside the admission code path with `prepare.mjs` (sealed wrapper and call renderings, wasm-tools
print/parse edits, byte-edit copies, JSON text edits). Every prepared input, emitted candidate and
admission report is hashed into `receipt.json`. The gate reports `passed` only when all twelve cases
executed and every observation held.

```sh
bun verification/acorr/gate.mjs NOBLE NEW_DIR --barrier-cli NOBLE_BARRIER
```

`NOBLE` is `cargo build --locked -p noble-cli --bin noble`; `NOBLE_BARRIER` is the same build with
`--features paa1-test-barrier`, the only build in which ACORR-CASE-09 can replace files between the
byte comparison and instantiation. `XDG_RUNTIME_DIR` must name the user runtime directory for the
sandboxed Lean consumer, and `proofs/mc1` must contain no symbolic links, because the v3 library pin
refuses them. `--only` rehearses a subset and never yields `passed`.

## Retained run

[`acceptance.json`](acceptance.json) (file sha256
`18534d32e0e4056ee6ee87e70b5855d8003aa24c830acc593040f4418ed93cfe`) is the receipt of the complete
run on clean commit `6caeb64b6ce5a604bb156649903575ea64f0823d` (tree `24428e2d…`): `result: passed`,
all twelve cases `passed`, 87 commands. The frozen CLI is
`735aca6776fc8919fc22752a06b405b53c635a17c684925afa12a2353c1895a7` and the barrier CLI
`87cdfb613e0228658e710e9e45c5a5b0267de9702ea61ae4c51580a939b1d8f7`; both were built from that
commit's Rust sources.

[`raw/20261121-6caeb64b`](raw/20261121-6caeb64b) mirrors the run's 38 admission reports
(`reports/`) and the 123 prepared inputs, emitted candidates and pin documents the receipt hashes,
under their paths relative to the run's output directory. The receipt names them by their original
absolute paths. Repository inputs (`gate.mjs`, `prepare.mjs`, fixtures, laws, the tool-selection
policy) are hashed at their tested-commit paths.

The receipt's `gate_sha256` and `prepare_sha256` fields hold the two script paths, not digests:
the gate records each script's digest in `inputs` (`gate.mjs`
`c1f574450e1d1ab521e42ace89e9099883af6096cf4e92295b9f5764fb17bb44`, `prepare.mjs`
`c197378ac37dda45d47f107345579ce0578bf9e4bb9f972eac120b128e865f74`) and returns the path. The
receipt stays byte-for-byte as written; the case records cite the digests.

An earlier complete run on `aabe2dd3` passed ten cases and failed ACORR-CASE-01 and -10 on two
harness defects (an exclusive input file re-created by the opt-on pass; the discarded manifest read
from inside the envelope instead of beside it). It promoted nothing and is not retained; `6caeb64b`
fixes both.

## What the run does not show

- ACORR-CASE-08's altered-helper control edits the candidate's emitted `$op_add` WAT. It does not
  rebuild a CLI with an altered `operations.wat`.
- Artifact identity is not semantic preservation. No translation validation, compiler correctness
  or verified backend is claimed, and obligations O1–O7 (PO-14/16/17/18/20/21) stay open.
