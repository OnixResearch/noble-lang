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

## Refusal-class controls (2026-10-30)

[`refusals.mjs`](refusals.mjs) runs the pinned exporter (`lean4export`, sha256 `9f7b3dbb…`) and
the pinned checker (`nanoda_bin`, `27388e46…`, one thread) as one pipeline, with the pinned Lean
(`8d4a77a0…`). It refuses to start if any input digest differs from its pin, and it classifies
every run into a named class.

A run is `accepted` only when all of these hold:
- the exporter exits zero without a panic;
- every requested root is declared in the export, resolved from the export's own name records;
- the checker exits zero and reports a positive number of checked declarations;
- every axiom the export declares is `propext`, `Classical.choice` or `Quot.sound`, whatever the
  checker was configured to permit.

The receipt is [`receipts/2026-10-30/refusals.json`](receipts/2026-10-30/refusals.json),
`holds: true`. It records the harness digest and every control's export and checker records.

| Control | Change | Exporter | Checker | Verdict |
|---|---|---|---|---|
| `baseline-exported` | none: `Init` export of `Nat.add_comm` | exit 0 | accepted, 52 declarations | **accepted** |
| `baseline-retained` | none: the retained small export | — | accepted, 16 declarations | **accepted** |
| `baseline-large` | none: the pinned `toInt_add` export | — | accepted, 2,773 declarations | **accepted** |
| `export-missing-module` | module `NoSuchModule` | exit 1, `unknown module prefix` | not run | `export-failure` |
| `export-missing-root` | root `Nat.no_such_declaration` | **exit 0**, `PANIC … not found in environment`; the export is only the meta record | not run | `export-failure` |
| `export-partial-roots` | roots `Nat.add_comm` and `Nat.no_such_declaration` | **exit 0**, the same panic; the export holds `Nat.add_comm`'s closure | not run | `export-failure` |
| `unsupported-format-above` | meta format version `3.2.0` | — | exit 1, `greater than the maximum supported version` | `unsupported-format` |
| `unsupported-format-below` | meta format version `3.0.0` | — | exit 1, `less than the minimum supported version` | `unsupported-format` |
| `unsupported-nat-literal` | `nat_extension: false` on the large export (56 `natVal` records) | — | exit 1, `Nat lit extension disallowed` | `unsupported-extension` |
| `unsupported-string-literal` | `string_extension: false` on the large export (37 `strVal` records) | — | exit 1, `String lit extension disallowed` | `unsupported-extension` |
| `checker-forbidden-axiom` | checker allow-list without `propext`, which the large export declares | — | exit 1, `declares unpermitted axiom "propext"` | `forbidden-axiom` |
| `custom-axiom-checker-refuses` | constructed `axiom NobleReplayControl.customAxiom.{u} : Sort u` appended to the small export | — | exit 1, `declares unpermitted axiom` | `forbidden-axiom` |
| `custom-axiom-checker-permits` | the same export, with the checker also permitting that axiom | — | **accepted**, 17 declarations | `forbidden-axiom` |

**The exporter reports a missing root only by a panic.** `lean4export` prints `PANIC` for a root
that is not in the environment, then continues and exits zero; Lean's compiled `panic!` does not
abort. The receipt also records what a pipeline reading exit statuses alone would have seen:
- for the missing root, the checker exits zero with `Checked 0 declarations with no errors`;
- for the partial root list, it exits zero with `Checked 52 declarations with no errors`.

So a run whose second requested theorem does not exist would have been reported as a success. The
panic check and the root-presence check refuse both runs, and the positive-count rule alone already
refuses the empty one. `limits.mjs` uses the weaker acceptance pattern, which matches
`Checked 0 declarations`. Its retained receipt is unaffected, because its two inputs are
digest-pinned exports of 2,773 and 16 declarations. A gate must use the strict rule.

**The custom axiom.** The checker accepts the custom axiom once it is told to permit it, and the
lane still refuses it. This meets the issue's requirement that forbidden or custom axioms stay
rejected even when an external checker accepts them as declared assumptions. The axiom is a
constructed control typed by the export's own `Sort u` expression, not a Noble declaration.

**Each rule is load-bearing.** Three throwaway copies of the harness each removed one rule and
reran the same controls. Each run failed (`holds: false`):

| Rule removed | Controls then accepted |
|---|---|
| exporter-panic and root-presence checks | `export-partial-roots` (`export-missing-root` became `empty-check`) |
| the lane's own axiom audit | `custom-axiom-checker-permits` |
| both of the above, with `Checked 0` also counted as acceptance | `export-missing-root`, `export-partial-roots` |

## Requirements before this becomes a gate

1. ~~A checker implemented independently of Lean's kernel with a compatible
   declaration-export format~~ — satisfied for this bounded pilot by the
   pinned `nanoda_lib`/`lean4export` pair. A gate would still need to freeze
   and rebuild both tools rather than relying on these local binaries.
2. Exact claim binding beyond the two explicit root names: the pilot records
   the names and closure hashes, but does not independently audit that the
   exported closure is the complete dependency closure for a higher-level
   claim. `refusals.mjs` now refuses an export that does not declare every
   requested root, but it binds names, not statements.
3. ~~Artifact integrity for the replayed `.olean` set~~ — implemented above: the frozen digest
   manifest refuses a corrupted, removed or extra artifact before the checker runs.
4. ~~Explicit refusal classes for unsupported constructs, timeout, export failure
   and rejection — none of which may be reported as success~~ — all exercised by
   pilot controls:
   - timeout and resource exhaustion by `limits.mjs`;
   - rejection and parse refusal by the 2026-10-29 controls;
   - unsupported format versions, disabled literal extensions, export failure and
     forbidden or custom axioms by `refusals.mjs`.

   A gate would run these classes under its own frozen tools.
5. Owner review before promotion to CI, a flake check, or a policy/tool-selection
   change; this pilot intentionally makes none of those changes.

## Non-claims

- Nanoda is a separately implemented checker, but this two-root observation is
  not an audit of its implementation or a second-kernel correctness result.
- No complete independent-kernel replay of the foundation corpus was run; only
  the two named roots and their exported closures were exercised.
- The semantic and parse controls here are minimal pilot controls, not a complete
  declaration-level negative-control suite (no dropped-dependency or
  statement-level claim-binding control; `refusals.mjs` checks root names only).
  The broader declaration-level equivalents
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
