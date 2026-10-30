# Independent replay of Lean declarations (issue #62, evaluation unit)

Bounded evaluation of a separately implemented Lean proof checker against Noble's
foundation corpus. Opt-in evidence only: nothing here replaces the current
consumer, changes MC1 admission, or claims compiler/runtime correspondence.

## Decision

**A bounded independent-kernel replay is available and was measured as an
opt-in pilot.** The pinned `leanprover/lean4export` revision
`8554815c2dc6b7abe99ec1f08849c9759ba77947` builds with Noble's Lean `4.31.0`
toolchain and emits NDJSON format `3.1.0`. The Rust `nanoda_lib` checker at
`d0a02cc5defadd683f62f9d078f41688314fa31f` accepts that format (its parser
range is `>=3.1.0,<3.2.0`). Its `nanoda_bin` CLI accepted the separately
exported closures of exactly `NobleFoundation.toInt_add` and
`NobleFoundation.LinkBoundary.vertical_simulation_compose`, with exit status
zero and no type-checker errors. The complete receipt is
[`receipts/2026-10-29/receipt.json`](receipts/2026-10-29/receipt.json), and the
small retained export is
[`receipts/2026-10-29/vertical_simulation_compose.ndjson`](receipts/2026-10-29/vertical_simulation_compose.ndjson).

This is evidence that a separately implemented checker can consume this
Lean 4.31.0 export format and accept these two closures; it is not evidence
that the checker is correct or complete. The earlier reference workflow also
changed underneath the issue: `lean4checker` is deprecated and merged into
Lean itself, distributed as `leanchecker` with every toolchain since v4.28.0,
and its own documentation states that it "is not an external verifier, as it
uses the Lean kernel itself".

What Noble *can* do today is a **producer-independent replay**: run the pinned
toolchain's `leanchecker` over the foundation's compiled `.olean` files, which
replays every declaration through the kernel in a fresh environment. That detects
environment hacking and non-kernel-checked declarations; it does not establish an
independent implementation. It is recorded here as an opt-in lane, not a gate.

## Pinned subjects

| Item | Value |
|---|---|
| `leanprover/lean4checker` revision inspected | `91a7f0e8e9dffe927089f5a6edcfeeb8a0e07709` (README: deprecated, merged into Lean, `lean-toolchain` `v4.29.0-rc8`) |
| `leanprover/lean4export` revision built | `8554815c2dc6b7abe99ec1f08849c9759ba77947`, `lean-toolchain` `leanprover/lean4:v4.31.0`, NDJSON format `3.1.0`, Apache-2.0 |
| `ammkrn/nanoda_lib` revision built | `d0a02cc5defadd683f62f9d078f41688314fa31f`, crate `0.4.30`, binary `nanoda_bin`, Apache-2.0 |
| Independent checker actually used | `/var/tmp/.../nanoda_bin` (release binary SHA-256 is retained in the receipt) |
| Producer-independent checker also retained | `leanchecker` shipped with Noble's pinned Lean **4.31.0** |
| Fresh foundation source | `25276acd370e56a12ed491865f02f0a7ff1704d9`, clean build; latest retained foundation receipt is `1d4113e6c` |
| Pilot roots | exactly `NobleFoundation.toInt_add` and `NobleFoundation.LinkBoundary.vertical_simulation_compose` |

## Measured

| Lane | Command | Result |
|---|---|---|
| Export-format parser probe | `nanoda_bin` on its `3.1.0` empty export fixture | accepted, `status=0`, empty stdout/stderr |
| Pinned exporter build | `PATH=<Lean 4.31.0>/bin nice -n 10 lake build` at `lean4export@8554815c` | completed, `status=0`; exact binary digest is in the receipt |
| Pinned nanoda build | `CARGO_BUILD_JOBS=8 nice -n 10 cargo build --release` at `nanoda_lib@d0a02cc` | completed, `status=0`; CLI binary exists and its digest is in the receipt |
| Two-root export | `lake env lean4export NobleFoundation -- NobleFoundation.toInt_add NobleFoundation.LinkBoundary.vertical_simulation_compose` | `status=0`, format `3.1.0`, 7,542,664 bytes; hash in the receipt |
| `NobleFoundation.toInt_add` closure | `nanoda_bin` with the permitted foundation axioms | **accepted**, `status=0`, 2,773 declarations, no errors |
| `NobleFoundation.LinkBoundary.vertical_simulation_compose` closure | `nanoda_bin` with the permitted foundation axioms | **accepted**, `status=0`, 16 declarations, no errors |
| Combined two-root export | `nanoda_bin` with the permitted foundation axioms | **accepted**, `status=0`, 2,781 declarations, no errors |
| Semantic negative control | one root-proof reference in expression `ie=870`: `arg 334` (`simulationBC`) → `arg 302` (`simulationAB`) | **rejected**, `status=101`, nanoda assertion `self.def_eq(u, v)` |
| Parse negative control | retained vertical export truncated at byte 12,345 inside a JSON string | **parse-refused**, `status=1`, `EOF while parsing a string` |
| Unmutated replay after controls | retained vertical export | **accepted**, `status=0`, 16 declarations, no errors |
| Producer-independent replay, default order | `leanchecker NobleFoundation` | silent, `status=0` (every declaration replayed and accepted) |
| Fresh-environment replay | `leanchecker --fresh NobleFoundation` | silent, `status=0` (imported constants replayed too; single-threaded, slower) |
| Control: random bytes in one `.olean` | `leanchecker NobleFoundation.ReverseAD` first on `LEAN_PATH` | `status=1`, `leanchecker found a problem in NobleFoundation.ReverseAD`, `invalid header` |
| Control: **one** flipped byte in the same `.olean` | as above | **`status=0`** — accepted |
| Control: missing module | `leanchecker NobleFoundation.NoSuchModule` | `status=1`, `Could not find any oleans` |

Logs and statuses are in `receipts/2026-10-26/` (`replay-baseline.out`,
`replay-fresh-NobleFoundation.log`, `control-random-bytes.out`,
`control-single-byte-flip.out`, `control-missing-module.out`).

The nanoda runs set `print_axioms=false` so the recorded results contain only
the checker outcome; nanoda therefore does not independently report axioms in
these results. The axiom audit remains Lean's `#print axioms` from the latest
retained foundation receipt. With nanoda's default `print_axioms=true`, the
same runs still exited zero and typechecked successfully but reported one
unrelated pretty-printer diagnostic (`Unable to print axioms`); this is
recorded rather than hidden in the receipt.

**Consequence of the single-byte control.** `.olean` loading in this lane detects
header/parse failure, not byte integrity, and `leanchecker` does not consult the
`.olean.hash` file. Any lane promoted to CI therefore needs an explicit artifact
integrity step (digest of the retained `.olean` set) rather than relying on the
checker to notice corruption.

## Trust boundary

| Concern | Established by this lane | Not established |
|---|---|---|
| Declarations are kernel-accepted when replayed from the compiled environment | yes, for the corpus above | source authenticity of what was compiled |
| Environment hacking by metaprogramming is detectable | by construction of a replay in a fresh environment | that the *original* elaboration was honest |
| Downloaded Mathlib `.olean` files are trustworthy | **no** — the foundation README's trust assumption is untouched | an independent implementation of the kernel |
| The two named roots and their emitted closures were actually checked | yes, with separate hashes and nanoda exit statuses in the 2026-10-29 receipt | semantic claim binding, completeness of a higher-level dependency closure, or checker correctness |
| Node/subtree closure matches a specific claim | **no** — replay accepts whatever the environment contains | exact claim binding, per-theorem closure |
| Emitted artifact, runtime or compiler correspondence | **no** | anything about `wasm`, the engine or the loader |

## Artifact integrity step (2026-10-27)

The single-byte control above was the reason this lane could not be promoted: `leanchecker` accepted a
flipped `.olean` silently. [replay.mjs](replay.mjs) now refuses **before** invoking any checker when
the retained `.olean` set does not match a frozen digest manifest, and the
[olean-integrity.txt](olean-integrity.txt) manifest binds the exact set replay resolves through
`lake env`'s `LEAN_PATH`: 2032 artifacts, 507,453,848 bytes, one `<sha256> <bytes> <path>` row each.
The checker does not consult `.olean.hash`, so this manifest *is* the digest step.

Measured at `80736e7c` (`receipts/2026-10-27/`): the retained set verifies `artifact-intact` and then
replays `kernel-accepted`, `status=0`, silent, 7066 ms. Corruption controls run against a copy of the
whole search path, so they cannot damage the retained artifacts:

| Control | Result |
| --- | --- |
| Untouched copy of the search path | `artifact-intact`, 2032 files |
| **One flipped byte** in `NobleFoundation.olean` (byte 2284) | `artifact-integrity` refusal, `digest mismatch`, **checker never invoked** |
| One artifact removed | `artifact-integrity` refusal, `missing Batteries/Classes/Order.olean`, checker never invoked |
| One artifact added | `artifact-integrity` refusal, `unexpected Extra.olean`, checker never invoked |

So the previously silent corruption is now a named refusal class rather than an acceptance. What this
does **not** do: it does not make the checker independent (it still uses the Lean kernel), and it does
not bind a claim to a declaration — it binds *bytes* to a replayed environment.

## Limit controls (2026-10-30)

`limits.mjs` runs the pinned `nanoda_bin` (sha256 `27388e46…`, one thread) on the pinned
`NobleFoundation.toInt_add` export (sha256 `b4c50e65…`, 7,492,900 B, not retained) and on the
retained 16-declaration `vertical_simulation_compose` export. It refuses before running anything
if a digest differs from its pin, classifies each run from its exit status, signal and output
alone, and fails the control if a bound does not bind or a bounded run is accepted. The receipt
is [`receipts/2026-10-30/limits.json`](receipts/2026-10-30/limits.json), `holds: true`:

| Run | `toInt_add` | small export |
|---|---|---|
| unbounded | **accepted**, 2,773 declarations, 338 ms | **accepted**, 2 ms |
| wall-clock bound 33 ms (a tenth of the unbounded time) | **timeout**, killed at 33 ms | **accepted**, 2 ms |
| address-space bound 6 MiB (the smallest searched bound admitting the small export) | **resource-exhausted**: `SIGABRT`, `memory allocation of 589840 bytes failed`, after 108 ms of checking | **accepted** |

The small export is refused at 4 MiB and accepted at 6 MiB, so the 6 MiB refusal of `toInt_add`
comes from checking work, not from the process failing to start. Both bounds are chosen from this
run on one host; they are pilot evidence, not a portable resource contract.

## Requirements before this becomes a gate

1. ~~A checker implemented independently of Lean's kernel with a compatible
   declaration-export format~~ — satisfied for this bounded pilot by the
   pinned `nanoda_lib`/`lean4export` pair. A gate would still need to freeze
   and rebuild both tools rather than relying on these local binaries.
2. Exact claim binding beyond the two explicit root names: the pilot records
   the names and closure hashes, but does not independently audit that the
   exported closure is the complete dependency closure for a higher-level
   claim.
3. ~~Artifact integrity for the replayed `.olean` set~~ — implemented above: the frozen digest
   manifest refuses a corrupted, removed or extra artifact before the checker runs.
4. Explicit refusal classes for unsupported constructs, timeout, export failure
   and rejection — none of which may be reported as success. Timeout and resource
   exhaustion are now exercised by `limits.mjs`, and rejection and parse refusal
   by the pilot controls; unsupported constructs and export failure are not.
5. Owner review before promotion to CI, a flake check, or a policy/tool-selection
   change; this pilot intentionally makes none of those changes.

## Non-claims

- Nanoda is a separately implemented checker, but this two-root observation is
  not an audit of its implementation or a second-kernel correctness result.
- No complete independent-kernel replay of the foundation corpus was run; only
  the two named roots and their exported closures were exercised.
- The semantic and parse controls here are minimal pilot controls, not a complete
  declaration-level negative-control suite (no dropped-dependency or
  independent claim-binding control). The broader declaration-level equivalents
  exist in the producer-consumer lane (`verification/mc1/gate.mjs`: duplicate/
  missing/cyclic dependency, forbidden axiom, free variable, wrong-theorem-type,
  trusted-name override, erased forbidden axiom) and are not evidence about a
  separately implemented checker.
- Nanoda's acceptance does not establish source authenticity, exact claim
  semantics, or implementation correspondence; it is an observed checker
  result for two named closures.
- A passing replay is not a correctness claim about any Rust, Wasm or engine
  subject, and it says nothing about the retained Mathlib `.olean` trust
  assumption.
