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

## Requirements before this becomes a gate

1. A checker implemented independently of Lean's kernel (not shipped with the
   toolchain), with a declaration-export format compatible with the pinned
   toolchain — `lean4export`'s current revision targets 4.36.0-rc1 and does not
   match Noble's 4.31.0, so this needs either a pinned compatible revision or a
   toolchain decision that is not a repository-wide upgrade.
2. Exact claim binding: independently selected theorem identities and complete
   dependency closures, so that checking an unrelated valid declaration cannot
   satisfy the lane.
3. Artifact integrity for the replayed `.olean` set, given the measured
   single-byte control.
4. Explicit refusal classes for unsupported constructs, timeout, export failure
   and rejection — none of which may be reported as success.

## Non-claims

- No second kernel implementation was evaluated: none exists for this toolchain.
- No negative control was constructed at the *declaration* level (substituted
  valid claim, dropped dependency, corrupted proof body inside a parseable file):
  the measured controls are artifact-level only.
- No external checker accepted anything about Noble; the replay is
  producer-independent, not implementation-independent.
- A passing replay is not a correctness claim about any Rust, Wasm or engine
  subject, and it says nothing about the retained Mathlib `.olean` trust
  assumption.
