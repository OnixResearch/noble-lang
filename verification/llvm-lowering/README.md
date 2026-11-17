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
- `compiler-lab-pilot-2026-11-16.json` is the retained compiler-lab
  optimization-pilot receipt for this pair (see
  [Compiler-lab optimization pilot](#compiler-lab-optimization-pilot)). It is
  immutable; `bundle.json` binds it by SHA-256 and by the compiler-lab commit.
- [`replay.mjs`](replay.mjs) runs in a fresh Node process. It reconstructs the
  profile and target bindings, checks every retained text identity, ties the
  certificate endpoints to the graph IDs, rechecks the source identities and
  assumptions, replays the retained constant fold,
  compares the pair modules with the named artifact modules, binds finite
  agreement rows to the generated Lean corpus, and checks that every stage has
  its own evidence/status. For the compiler-lab receipt it checks the SHA-256
  and commit binding, that every graph, module and certificate digest the
  report consumed is the SHA-256 of the retained file here, the per-stage
  statuses of the retained pair, that the four evidence classes stay distinct
  across all candidates and controls, and that no eligible candidate has a stage
  below the policy requirement. The `cert_differential` integration test invokes
  it in a child process and checks its success.

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
| LLVM optimization | semantic-validation (compiler-lab) | Compiler-lab's pinned Alive2 (LLVM 21.1.8) accepts `opt default<O2>`'s output of each pair module as a correct transformation for every concrete `u64` input pair. This is compiler-lab's Alive2 result, not a Noble or Lean proof. |
| Native artifact | concrete-agreement (compiler-lab) | Native code from the pinned `llc`/`clang` agrees with the graph reference on the 24 frozen workload inputs. No machine-code checker runs, so this is not a proof. Both sides compile to identical machine code, so there is no runtime improvement. |

Compiler-lab's reports keep these categories distinct:

- **semantic validation**: independent relation-checker result, if one is present;
- **concrete agreement**: values observed on the request's finite boundary inputs;
- **unknown**: a stage could not establish its relation or artifact result, including
  a missing checker or an inconclusive run;
- **unsupported**: the pair or operation was outside that consumer's admitted
  profile.

An `unknown`, `unsupported`, timeout or harness error at one stage does not inherit
the Lean model theorem, the Rust validator result, or another stage's result.

## Compiler-lab optimization pilot

`compiler-lab-pilot-2026-11-16.json` retains the report of compiler-lab commit
`2ff04a542a43fe635385b9b23b6a7b99daf1a6ec`'s `validate optimization-pilot`
command (compiler-lab #124/#176). It ran on this lane's unchanged `bundle.json`
at clean Noble commit `6248573077ea647372918b493ff20ef7b9d685b3` (bundle SHA-256
`fc70619b…`). Compiler-lab's frozen contract `noble-i64.json` (SHA-256
`aa875728…`) fixes the five original graph texts by SHA-256, Noble's revisions,
the interface, the target, LLVM 21.1.8, the `opt` and `llc` flags, 24 workload
inputs, the evidence policy, the continue/stop rule and 20 controls. The tools
come from compiler-lab's `nix/llvm-translation.nix` pin (LLVM 21.1.8, Alive2
`913e1556`).

Results:

- The run exited 0 with `pass`: 23 candidates attempted, 6 eligible, 20 of 20
  controls met their declared outcome, and no admission refusal.
- A fresh `--replay` of the retained run directory exited 0 with `pass`,
  `matches: true` and no differences.
- The pinned clang reports exactly the data layout this profile assumes
  (`data_layout_confirmed: true`).
- The decision is **stop**. Both sides of the retained pair optimize to the same
  module and compile to the same 9-byte machine code. The rewrite therefore
  supplies no native improvement, and the timing samples are descriptive only.

For the retained pair (`foldable` → `foldable-rewritten`), compiler-lab reports
each stage with its own semantic and concrete status:

| Compiler-lab stage | Side | Semantic | Concrete | Method |
|---|---|---|---|---|
| graph-rewrite | pair | semantic-validation | concrete-agreement | compiler-lab's own certificate replay; Noble's checker verdict is separate evidence |
| graph-to-llvm-lowering | baseline, candidate | semantic-validation | concrete-agreement | Noble's validator and Lean claim, consumed by SHA-256 and not rechecked |
| llvm-pair | pair | semantic-validation | concrete-agreement | pinned Alive2 refinement in both directions; the `llvm-pair` lane agrees on 16 boundary pairs |
| llvm-optimisation | baseline, candidate | semantic-validation | concrete-agreement | pinned Alive2 on each `opt default<O2>` step |
| native-artifact | baseline, candidate | not-run | concrete-agreement | no machine-code checker; native agreement on 24 inputs |

Across the controls the same report records `refuted`, `unknown` (for example
graph-to-LLVM under a changed triple, data layout or ABI), `unsupported` (the
`llvm-pair` lane under a changed triple or data layout), a `timeout` semantic
result (the Alive2 check of the optimized module in the `boundary-agreement`
control, whose concrete status stays separate) and `not-run`. None of these makes
a candidate eligible.

The receipt's `report` member is a projection of the run report. The projection
drops per-process command records, the bundle's build-host path and tool-store
paths. It keeps the stage report, every claim, every artifact digest, coverage,
costs, tool versions and BLAKE3 digests, pins, target, workload, measurement
protocol, trust boundaries and decision. The receipt records the jq filter and
the SHA-256 of the unprojected run and replay reports, which stay outside the
repository.

To rerun, at compiler-lab commit `2ff04a54` with the tools built from
`nix/llvm-translation.nix`:

```sh
compiler-lab validate optimization-pilot --contract src/validation/optimization_pilot/noble-i64.json \
  --bundle <noble checkout at the recorded commit>/verification/llvm-lowering/bundle.json \
  --alive-tv <tools>/bin/alive-tv --opt <tools>/bin/opt --llc <tools>/bin/llc --clang <tools>/bin/clang \
  --out <new run directory>
compiler-lab validate optimization-pilot --replay <new run directory> \
  --alive-tv <tools>/bin/alive-tv --opt <tools>/bin/opt --llc <tools>/bin/llc --clang <tools>/bin/clang \
  --out <new replay directory>
```

The contract binds graph texts and revisions, not `bundle.json`'s bytes. Changing
a graph text or revision makes compiler-lab refuse the bundle as stale until the
contract is re-frozen there.

### Producer-export gaps the pilot lists

The report's `producer_gaps` names four gaps in Noble's export. None is needed
for an acceptance item of #107, so this lane leaves them open:

- The `TestIds` identities are not cryptographic. Compiler-lab binds every
  consumed file by its own SHA-256, and `replay.mjs` checks those digests
  against the retained files, so the retained pair is checked without them.
- The bundle records no proposal, emission or validation cost, so the report's
  `proposal_ms` is `null` with that reason. Cost accounting belongs to #64's
  optional measurements and is not one of #107's acceptance criteria.
- One nonidentity candidate and no chained `ngraph-cert/2` candidate are
  exported. The criterion asks for one real retained pair.
- Noble's checker verdicts carry no retained process records. Compiler-lab
  states that it consumes them as claims; that trust boundary is in the report.

## #107 acceptance mapping

Each row keeps its own stage scope; no row borrows another row's evidence.

| Acceptance criterion | Evidence | Scope and limits |
|---|---|---|
| Named opt-in profile, exact ABI/operation inventory and explicit refusal diagnostics | [`docs/ngraph-llvm-i64-v0.md`](../../docs/ngraph-llvm-i64-v0.md); `crates/noble-ngraph/src/llvm.rs`; profile tests in `crates/noble-ngraph/tests/cert/llvm/` | Profile `ngraph-llvm-i64/0`, opt-in only. |
| A real retained graph pair is exported and independently checked under a universally quantified graph-to-LLVM claim | `artifacts/`, `pair/`, `request.json`, `bundle.json`; Lean `LlvmLowering.validate_sound` and the generated corpus, receipt `foundation-check-2026-11-13.json`; Rust `llvm::validate` | The universal claim is about the Lean model of the parsed form. Parser-to-model and checker-to-model correspondence are trusted (T1, T2). |
| Fixtures cover MIN/MAX wrapping, negative constants, subtraction order, sharing and a fold LLVM may already perform | `artifacts/wrapping`, `negative`, `subtractive`, `shared`, `foldable` and `foldable-rewritten`; the pilot's `llvm-optimisation` stage shows that LLVM performs the fold itself (identical optimized module) | Fixture coverage, not a proof by itself. |
| Controls reject changed operands/constants/return values, overflow flags, unsupported instructions, altered interface/target and substituted artifact bytes | `controls/` (17 retained refusals checked by Rust and `replay.mjs`); the pilot's 20 controls, all met | Noble's controls test the Rust validator; compiler-lab's controls test its own stage policy. |
| Fresh-process replay reconstructs the retained bindings and each stage's evidence | `replay.mjs`, run by the `cert_differential` test in a child process; compiler-lab's own `--replay` (`matches: true`, no differences) | Binding and stage replay. Neither replay reruns the Lean kernel. |
| Compiler-lab reports distinguish semantic validation, concrete agreement, unknown and unsupported results | `compiler-lab-pilot-2026-11-16.json` (bound in `bundle.json`); per-stage `semantic`/`concrete`/`status` fields; `replay.mjs` requires all four classes to be present and kept distinct | Compiler-lab's statuses are compiler-lab's. Alive2, LLVM code generation and the host processor stay trusted. |
| The experiment preserves the selected production backend; any later runtime adoption has its own decision and evidence | [Production backend](#production-backend); the pilot's decision **stop** (identical machine code, no speedup) | No runtime adoption is proposed. |

## Production backend

This lane is opt-in and does not change the selected production backend. Nothing
in `compiled.rs` or `schedule.rs` calls the LLVM emitter, and there is no runtime
selection path. Adopting the lowering in production requires its own decision and
evidence, including LLVM/native artifact validation and performance measurements;
the Lean theorem in this lane says nothing about optimized or native code. The
compiler-lab pilot found no runtime improvement for the retained pair (identical
machine code; decision stop), so this lane gives no reason to adopt it.
