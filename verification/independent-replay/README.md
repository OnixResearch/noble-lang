# Independent replay of Lean declarations (issue #62, evaluation unit)

Bounded evaluation of a separately implemented Lean proof checker against Noble's
foundation corpus. Opt-in evidence only: nothing here replaces the current
consumer, changes MC1 admission, or claims compiler/runtime correspondence.

## Decision

**No genuinely separately implemented checker is available for Noble's pinned
toolchain, and none was adopted.** The reference workflow changed underneath the
issue: `lean4checker` is deprecated and merged into Lean itself, distributed as
`leanchecker` with every toolchain since v4.28.0, and its own documentation states
that it "is not an external verifier, as it uses the Lean kernel itself". The
exporter repository's current revision targets Lean `v4.36.0-rc1`, newer than
Noble's `4.31.0`.

What Noble *can* do today is a **producer-independent replay**: run the pinned
toolchain's `leanchecker` over the foundation's compiled `.olean` files, which
replays every declaration through the kernel in a fresh environment. That detects
environment hacking and non-kernel-checked declarations; it does not establish an
independent implementation. It is recorded here as an opt-in lane, not a gate.

## Pinned subjects

| Item | Value |
|---|---|
| `leanprover/lean4checker` revision inspected | `91a7f0e8e9dffe927089f5a6edcfeeb8a0e07709` (README: deprecated, merged into Lean, `lean-toolchain` `v4.29.0-rc8`) |
| `leanprover/lean4export` revision inspected | `df81913eee5519d5a88ae5b0fd701b5298cc29ab`, `lean-toolchain` `v4.36.0-rc1`, NDJSON export format |
| Checker actually used | `leanchecker` shipped with Noble's pinned Lean **4.31.0** |
| Noble source | `b6d2f88b` (the tree whose `.olean` files were replayed) |
| Corpus | module `NobleFoundation` — the foundation model modules, 314 inventoried theorems |

## Measured

| Lane | Command | Result |
|---|---|---|
| Producer-independent replay, default order | `leanchecker NobleFoundation` | silent, `status=0` (every declaration replayed and accepted) |
| Fresh-environment replay | `leanchecker --fresh NobleFoundation` | silent, `status=0` (imported constants replayed too; single-threaded, slower) |
| Control: random bytes in one `.olean` | `leanchecker NobleFoundation.ReverseAD` first on `LEAN_PATH` | `status=1`, `leanchecker found a problem in NobleFoundation.ReverseAD`, `invalid header` |
| Control: **one** flipped byte in the same `.olean` | as above | **`status=0`** — accepted |
| Control: missing module | `leanchecker NobleFoundation.NoSuchModule` | `status=1`, `Could not find any oleans` |

Logs and statuses are in `receipts/2026-10-26/` (`replay-baseline.out`,
`replay-fresh-NobleFoundation.log`, `control-random-bytes.out`,
`control-single-byte-flip.out`, `control-missing-module.out`).

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

## Requirements before this becomes a gate

1. A checker implemented independently of Lean's kernel (not shipped with the
   toolchain), with a declaration-export format compatible with the pinned
   toolchain — `lean4export`'s current revision targets 4.36.0-rc1 and does not
   match Noble's 4.31.0, so this needs either a pinned compatible revision or a
   toolchain decision that is not a repository-wide upgrade.
2. Exact claim binding: independently selected theorem identities and complete
   dependency closures, so that checking an unrelated valid declaration cannot
   satisfy the lane.
3. ~~Artifact integrity for the replayed `.olean` set~~ — implemented above: the frozen digest
   manifest refuses a corrupted, removed or extra artifact before the checker runs.
4. Explicit refusal classes for unsupported constructs, timeout, export failure
   and rejection — none of which may be reported as success.

## Non-claims

- No second kernel implementation was evaluated: none exists for this toolchain.
- No negative control was constructed at the *declaration* level in **this** lane (substituted valid
  claim, dropped dependency, corrupted proof body inside a parseable file): the controls here are
  artifact-level only. The declaration-level equivalents exist in the producer-consumer lane
  (`verification/mc1/gate.mjs`: duplicate/missing/cyclic dependency, forbidden axiom, free variable,
  wrong-theorem-type, trusted-name override, erased forbidden axiom) and are not evidence about a
  separately implemented checker.
- No external checker accepted anything about Noble; the replay is
  producer-independent, not implementation-independent.
- A passing replay is not a correctness claim about any Rust, Wasm or engine
  subject, and it says nothing about the retained Mathlib `.olean` trust
  assumption.
