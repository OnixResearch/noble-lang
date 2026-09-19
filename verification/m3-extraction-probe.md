# M3 extraction probe: the ladder re-run on the extended kernel (2026-09-19)

Task 4.1 requires the extraction subject for the extended kernel to be
regenerated and every failed probe retained with its exact diagnostic.
This record carries the ladder re-run for `bootstrap fragment v1`, the
refusal classes it hit, the two repairs they forced, and the green
strict probe whose emitted files are byte-identical to the checked-in
`proofs/m3` root. None of these results is an acceptance claim; the
fragment label stays exactly `bootstrap fragment v1`
(V-MODEL-01 / V-GATE-05).

The v0 ladder and its four refusal classes are retained unchanged in
[m2-extraction-probe.md](m2-extraction-probe.md); this file records the
re-run after the kernel extension (tasks 2.1–2.3: the `=` contract, the
B-CHECK-02 environment-validation walks, the B-CHECK-05 witness
resolution walk).

## Route and configuration

| Input | Value |
|---|---|
| Regeneration loop | header of `proofs/m2/NobleKernel.lean` (the loop that produced the current root) |
| Subject | `crates/noble-kernel` lib, all features, `x86_64-unknown-linux-gnu` |
| Extraction Rust | `1.100.0-nightly (8fa1c96cf 2026-08-17)` at `/nix/store/wq92i79ivn1w1nrdhmzl1mqx0wcyrawi-rust-default-1.100.0-nightly-2026-08-18` |
| Charon | `0.1.254 (b104e24fea7d721b71e6c39fd70f26ff20bc0980)` at `/nix/store/ka61j6d6zy8zg91ynlbd6nmjssyynrpl-charon` |
| Aeneas | `505b6ca` at `/nix/store/4a6mwly7yrbpr675xq9r399nf75d7d71-ocaml5.2.1-aeneas-0.1.0` |
| Charon arguments | `--preset=aeneas --error-on-warnings` |
| Aeneas arguments | `-backend lean -abort-on-error -warnings-as-errors -no-progress-bar -split-files -gen-lib-entry -all-computable` |
| Re-run command | `bash /tmp/m3-extraction-probe.sh` (the loop from the root header, emitting to `/tmp/llbc-m3`, `/tmp/lean-m3`) |

The pins are the M2 pair (`m2-extraction-probe.md` "Route and
configuration"): the extension did not change the extraction toolchain,
so the ladder is comparable probe-for-probe with the v0 record.

## Probe outcomes (in order)

| Probe | Charon | Aeneas | Refusal / outcome |
|---|---|---|---|
| 1 | ok | fail | `InterpBorrowsCore` "Can't end abstraction 5 as it is set as non-endable" over the new `words/resolve.rs` substitution walk: the option-combinator closure `and_then(\|slot\| inst.bindings.get(slot))` over the borrowed arena. Same class as v0 probe 5 (option-combinator closures). |
| 2 | ok | fail | `don't know how to synthesize implicit argument T`: the loop condition `while … && failure.is_none()` in `acceptance/validate.rs` and `words/resolve.rs` writes `failure` without reading it in the body, so Aeneas emitted a bare `none` whose implicit type Lean cannot infer. Same class as v0 probe 4 (borrow interpretation) on the shape side. |
| 3 | ok | **ok** | none: every kernel function translates under the pinned flags; `Types.lean`, `Funs.lean`, `NobleKernel.lean`, `TypesExternal_Template.lean`, `FunsExternal_Template.lean` emitted. |

The two repairs the ladder forced are recorded with their source sites in
[m3-kernel-gates.md](m3-kernel-gates.md) "Regeneration re-binding":
the resolution walk became two explicit branches (the v0
"option-combinator closures became branches" discipline), and both loops
now exit by `break` so the condition stays numeric/state-only. Neither
repair changed a decision: the frozen-fixture matrix is identical before
and after (the twelve M2 refinement theorems still prove against the
regenerated checker — `m3-kernel-gates.md` "Regeneration re-binding"),
and the M2 refinement theorems bind the regenerated `acceptance.check`.

## Green strict probe (probe 3, final) and file identity

```console
$ charon cargo --preset=aeneas --error-on-warnings \
    --dest-file /tmp/llbc-m3/noble_kernel.llbc -- \
    --manifest-path crates/noble-kernel/Cargo.toml --lib --all-features \
    --target x86_64-unknown-linux-gnu --locked --offline
charon_exit=0
$ aeneas -backend lean -abort-on-error -warnings-as-errors -no-progress-bar \
    -split-files -gen-lib-entry -all-computable \
    -dest /tmp/lean-m3 /tmp/llbc-m3/noble_kernel.llbc
aeneas_exit=0
[Info ] Generated: /tmp/lean-m3/TypesExternal_Template.lean
[Info ] Generated: /tmp/lean-m3/Types.lean
[Info ] Generated: /tmp/lean-m3/FunsExternal_Template.lean
[Info ] Generated: /tmp/lean-m3/Funs.lean
[Info ] Generated: /tmp/lean-m3/NobleKernel.lean
```

The emitted files are bound to the checked-in root by byte comparison
(task 4.2's regeneration duty) — the checked-in module *is* the emission
from the current crate sources under the pinned pair:

```console
$ cmp /tmp/lean-m3/Types.lean proofs/m3/NobleKernel/Types.lean   # identical
$ cmp /tmp/lean-m3/Funs.lean  proofs/m3/NobleKernel/Funs.lean    # identical
$ cat /tmp/lean-m3/NobleKernel.lean
import NobleKernel.Funs
```

`NobleKernel.lean` differs from the checked-in file only by the
hand-written regeneration header (the loop, the flag set, and the
external-filling instruction), which is why the checked-in file carries
it: the emitted entry point is the single import line.

The emitted external templates are filled, not regenerated: the checked-in
`FunsExternal.lean` / `TypesExternal.lean` carry one implementation per
template axiom, name for name, with nothing extra and nothing missing
(23 + 1 axioms, verified by comparing `^axiom` names in the emitted
templates against `^def` names in the checked-in files — equal sets in
both directions). `verification/m3-proof-gate.sh` re-checks the policy
("no template axioms" / no unexplained external model) on every run, and
`verification/m3-coverage-gate.sh` checks the five
`#[charon::opaque]` disclosures against the crate sources.

## Claim limits

- The identity above is a *mechanical* binding: same sources, same pinned
  pair, same flags, same bytes. It is not a refinement proof — PO-11's
  refinement claim lives in the `proofs/m3` theorem set.
- Aeneas' `-abort-on-error -warnings-as-errors` strictness is a subset
  claim over `crates/noble-kernel` only; it says nothing about the CLI,
  the host boundaries, or the roadmap's `wasm-feasibility` milestone.
- The ladder is reproducibility evidence for this tree; it does not
  authenticate the Lean dependency cache (that is
  `lean-dependencies-check`'s claim).
