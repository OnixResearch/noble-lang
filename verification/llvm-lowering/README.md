# `ngraph-llvm-i64/0` retained lane

This directory retains issue #107's opt-in graph-to-LLVM experiment. The profile,
operation inventory, ABI, refusals, observation relation and trust boundaries are
specified in [`docs/ngraph-llvm-i64-v0.md`](../../docs/ngraph-llvm-i64-v0.md).

## Files and replay

- `bundle.json` binds six graph/module artifacts, their logical IDs, exact-text
  identities, finite observations, controls, explicit assumptions, exact revision
  labels, implementation-source identities and stage evidence.
- `artifacts/` holds the original and rewritten graph endpoints and their emitted
  LLVM modules. `pair/` repeats the pair passed to compiler-lab and retains the
  `ngraph-cert/0` derivation that binds the graph endpoints.
- `controls/` holds the actual LLVM text for each refusal mutation.
- `foundation-check-2026-11-13.json` is the foundation gate's receipt, generated
  from the clean committed source tree; it records the checked theorem statements,
  allowed axioms, source commit and source hashes.
- `request.json` uses the compiler-lab schema-1 pair-request kind, which selects
  the consumer's `llvm-pair-v1` profile, with a two-`i64` interface, one `i64`
  result, `ccc`, no function attributes, the `all-u64-pairs` input domain, and no
  permitted external declarations.
- [`replay.mjs`](replay.mjs) runs in a fresh Node process. It reconstructs the
  profile and target bindings, checks every retained text identity, ties the
  certificate endpoints to the graph IDs, rechecks the source identities and
  assumptions, replays the retained constant fold,
  compares the pair modules with the named artifact modules, binds finite
  agreement rows to the generated Lean corpus, and checks that every stage has
  its own evidence/status. The `cert_differential` integration test invokes it
  in a child process and checks its success.

Text identities in this fixture use the integration target's deterministic
`TestIds` scheme. They detect stale or substituted retained bytes in this lane;
they are **not cryptographic digests, signatures, or authenticity claims**. The
graph IDs are likewise the graph checker's logical IDs under `TestIds`. The
fresh-process replay does not replace the Rust `llvm::validate` tests or Lean's
kernel checking.

Regenerate the corpus and retained files after a deliberate source change with:

```sh
cargo test -p noble-ngraph --test cert_differential -- --ignored write_the_artifacts
```

Then run the pinned foundation proof gate. The ordinary Rust test checks that
all generated files still equal their source-derived export and executes
`node verification/llvm-lowering/replay.mjs` in a fresh process.

## Evidence classes and stages

The lane intentionally does not collapse distinct claims:

| Evidence | Current status | What it means |
|---|---|---|
| Graph rewrite | accepted | Rust `cert::check` accepts the retained derivation for the graph pair; the Rust-to-Lean certificate correspondence remains open. |
| Graph-to-LLVM lowering | validated | Rust's independent parser/validator accepts the actual module bytes against each admitted graph. |
| Semantic validation | proved | The Lean model's `validate_sound` theorem is universal over all pairs of `BitVec 64` inputs. The parser-to-model and Rust-checker-to-model links remain trusted, not proved. |
| Concrete agreement | checked | The parsed LLVM subset interpreter agrees with the graph reference on the finite inputs retained in `bundle.json`; this is not a universal proof. |
| LLVM optimization | not run | No pinned LLVM is available in this lane. Compiler-lab #124 owns this external stage. |
| Native artifact | not run | No native code or performance result is claimed; compiler-lab owns that stage. |

When compiler-lab reports arrive, they must keep these categories distinct:

- **semantic validation**: independent relation-checker result, if one is present;
- **concrete agreement**: values observed on the request's finite boundary inputs;
- **unknown**: a stage could not establish its relation or artifact result, including
  a missing checker or an inconclusive run;
- **unsupported**: the pair or operation was outside that consumer's admitted
  profile.

An `unknown`, `unsupported`, timeout or harness error at one stage does not inherit
the Lean model theorem, the Rust validator result, or another stage's result. No
compiler-lab report is retained here, so the LLVM-optimization and native stages
remain **not run** rather than being described as successful or unsupported.

## Production backend

This lane is opt-in and does not change the selected production backend. Nothing
in `compiled.rs` or `schedule.rs` calls the LLVM emitter, and there is no runtime
selection path. Adopting the lowering in production requires its own decision and
evidence, including LLVM/native artifact validation and performance measurements;
the Lean theorem in this lane says nothing about optimized or native code.
