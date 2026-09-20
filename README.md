# Noble

**Start with [the Cairn specs](.cairn/README.md).** Noble's implementation is staged; the specifications describe more than the currently delivered fragments.

Noble is a statically typed concatenative language designed for first-class, inspectable programs and Wasm execution.

**Implementation direction: Rust → Charon → Aeneas → Lean 4.** This route is mandatory for the entire semantic kernel and the target for all Noble-owned production Rust. Host effects require explicit proof boundaries. See [the toolchain policy](.cairn/specs/verification-toolchain/spec.md).

**Application verification:** the optional [contract profile](.cairn/specs/program-contracts/spec.md) now has an MC1 frontend, versioned source/IR, Lean rules, and the `noble verify` / `noble explain-proof` CLI. First-class runtime evidence companions and proof-required admission remain future work; MC1 does not implement a runtime or Wasm backend.

```text
(contract 1 increment
  (input (x I64))
  (output (y I64))
  (program [ 1 + ])
  (requires true)
  (ensures (eq (out y) (add (in x) 1))))
```

This is the actual [increment fixture](verification/mc1/increment.noble-contract), not general-purpose Noble runtime syntax. It asks for wrapping `I64` partial correctness on **every normal return**, not termination.

**First reference application:** [an exact programmable calculator](.cairn/specs/calculator/spec.md) with rational division and exact decimal input. It will evaluate AI-authored changes under independent acceptance criteria. This application follows the core and library gates. No calculator exists yet.

**Worker substrate contracts:** [the typed-worker conformance slice](specs/WORKER-CONFORMANCE.md) connects generated-code admission, explicit state, and cancellation-safe resource ownership. It adds no agent syntax or completed swarm runtime.

**Octet adoption:** [the boundary-contract map](specs/OCTET-ADOPTION.md) adds typed authorization/receipt rules and explicit Rust architecture gates. Octet evidence stays separate from Noble semantic proofs.

## Using MC1 contracts

From the repository root, build the `noble` binary with the pinned Rust environment:

```sh
nix develop
cargo build -p noble-cli --bin noble
export PATH="$PWD/target/debug:$PATH"
export NOBLE_CONTRACT_LIBRARY="$PWD/proofs/mc1"
export NOBLE_LEAN="$(nix build .#lean --no-link --print-out-paths)/bin/lean"
```

Proof checking requires Linux, working bubblewrap namespaces, `prlimit`, and an active user systemd manager with a usable `XDG_RUNTIME_DIR/bus`. The development shell alone does not provide that host isolation setup. Select trusted executables using absolute `NOBLE_BWRAP`, `NOBLE_PRLIMIT`, and `NOBLE_SYSTEMD_RUN` paths when they are not in the CLI's conventional `/run/current-system/sw/bin`, `/usr/bin`, or `/bin` locations. A Nix-installed Lean also requires `nix-store`; `NOBLE_NIX_STORE` can select its absolute path. `NOBLE_LEAN` must resolve to the release installation's `bin/lean`, not an elan dispatcher: Lean **4.31.0**, commit `68218e876d2a38b1985b8590fff244a83c321783`, is required. The selected contract library must contain the matching `lean-toolchain`, `NobleContracts.lean`, and its source modules.

```sh
# Prepare and explain the exact claim without launching Lean or a proof worker.
noble explain-proof verification/mc1/increment.noble-contract

# --emit must name a new directory; existing destinations are never overwritten.
noble verify verification/mc1/increment.noble-contract \
  --proof verification/mc1/increment-proof.lean \
  --emit mc1-increment --timeout-ms 120000

# An accepted refutation is "disproved" and exits 1, not a tool failure.
noble verify verification/mc1/monotonic.noble-contract \
  --refutation verification/mc1/monotonic-refutation.lean

# Statement generation only: emits artifacts, reports "unknown", exits 3.
noble verify verification/mc1/increment.noble-contract --emit mc1-statement
```

`verify` accepts one contract path followed by optional `--emit DIR`, either `--proof FILE` or `--refutation FILE`, and `--timeout-ms N`. Options cannot be repeated. `N` is 1–600000 ms; the default is 120000 ms for the entire proof workflow, including rebuilding the selected rule library. `explain-proof` accepts only the contract path. Neither explanation nor verification without supplied evidence runs proof tools; missing evidence is not automatic proof search.

Both commands write a `noble-mc1-report/v1` JSON report to stdout. Supplied Lean source imports the generated `MC1Obligation` and must define `MC1Proof.proof : MC1Obligation.claim`, or `MC1Proof.refutation : Not MC1Obligation.claim`. `--emit` retains `report.json` and, when available, `contract.noble`, `MC1Obligation.lean`, `MC1Proof.lean`, and accepted declaration data in `MC1Proof.json`.

| Outcome | Exit | Meaning |
|---|---:|---|
| `proved` | 0 | Independent consumer accepted a proof of the exact generated claim |
| `disproved` | 1 | Independent consumer accepted a refutation of that claim |
| `error` | 2 | Invalid source/options, rejected evidence, I/O failure, or a non-timeout checking failure |
| `unknown` | 3 | Prepared claim has no supplied evidence |
| `unsupported` | 4 | Unsupported contract feature or unavailable/incompatible required proof configuration |
| `timeout` | 124 | Proof workflow exhausted its wall-clock or recognized CPU-time budget |
| `not-run` | 0 | Explanation succeeded; no proof was checked |

Failed proof checking is not disproof. Missing tools, wrong Lean pins, and unavailable sandboxing fail closed; there is no unsandboxed fallback. Proof source runs without network or ambient credentials, with read-only inputs, fixed writable artifact files, and aggregate memory/task plus CPU, output, and file-size limits. The fresh trusted consumer decodes bounded inert JSON, reconstructs declarations, checks the exact expected theorem type and transitive assumptions, and **never loads producer `.olean` files**. Only `propext`, `Classical.choice`, and `Quot.sound` are allowed logical axioms; `sorry` and native-evaluation proof shortcuts do not satisfy this strict application lane.

### What MC1 establishes

- The production `no_std` [frontend](crates/noble-contracts/src/lib.rs) accepts `(contract 1 ...)` and exports contract IR revision 1. Ordinary typing comes from the inherited `noble-kernel` acceptance path, not witness inference. Export consumes an immutable `Prepared` containing that accepted candidate and resolved claim.
- The pure fragment supports explicit initial/final observations, universally quantified ghost parameters, total logical definitions, scalar/structural predicates, and higher-order `maps` claims. Ghost values cannot supply executable captures. `I64` addition, subtraction, and multiplication wrap at 64 bits; ordering is signed. Text literals, observable `Program`/`Syntax` equality, host/resource contracts, and total-correctness or prefix-safety claims are unsupported. See the [versioned grammar, logical partiality, and limits](.cairn/specs/program-contracts/spec.md#81-delivered-mc1-source-and-ir).
- Strict [application/rule proofs](proofs/mc1/NobleContracts.lean) concern the reviewed pure normal-return model, including increment, composition, and a universally quantified runtime-capture family. They do not prove current invocation preconditions, successful resource-bounded execution, host behavior, or termination.
- The separate [implementation correspondence lane](proofs/mc1/NobleContractImpl.lean) contains actual-source extraction, universal node/body projection preservation, and source-bound full preparation/export equations for the increment, composed, capture-family, structural, syntax, and signed-wrap fixtures. Those full-source equations use a separately reported native-evaluation assumption. They are **not universal frontend/compiler correctness**, and their modules are not imported by the strict application library.

The [increment](verification/mc1/increment.noble-contract), [capture-family](verification/mc1/family.noble-contract), and [structural frontend](crates/noble-contracts/src/fixtures/structural.noble) fixtures show the supported source forms. Durable run records belong in [MC1 evidence](verification/mc1/evidence.json) and [CLI acceptance](verification/mc1/acceptance.json); consult their exact source/tool identities and scopes rather than treating this overview as acceptance evidence. MC1 does not close runtime companion construction/replay, `noble build --require-proof`, MC2/Wasm, backend correspondence, or whole-language refinement.

Run the two evidence lanes independently:

```sh
# Re-extract both actual crates; compare generated code and audit compiled proofs.
# The artifact directory must not exist. Selected Nix tools and Lake dependencies
# must already be available; this gate does not substitute ambient tool versions.
bun verification/mc1/implementation.mjs .pi/mc1/implementation-run

# Exercise the real CLI, hostile proof transport, isolation, and timeout cleanup.
bun verification/mc1/gate.mjs target/debug/noble verification/mc1/acceptance.json
```

The implementation gate audits 61 strict bridge/projection theorems and six
source-equation cases separately from the 43 strict application/rule theorems.
Native computation also discharges generated string-bound obligations; its exact
axiom inventory is retained only in the implementation lane.

## Bounded Wasm experiment

M3 executes both WasmGC and managed-linear-memory representations, with optimization
off and on. Each configuration covers 25 declared workload variants and 27 boundary
controls: post-compilation captures, program-list selection, returned/reflection-only
programs, exact recipes, ordered interfaces, and bounded composition/cleanup.

```sh
# Uses the Nix-built Noble CLI and pinned Node/V8, wasm-tools, and Binaryen.
# The output directory must not exist; complete raw samples and artifacts are kept.
nix run .#m3-wasm -- --out /tmp/noble-wasm-comparison

# Separate actual-Rust extraction and compiled dependency/axiom audit.
bun verification/m3-wasm/implementation.mjs /tmp/noble-wasm-extraction
```

`noble wasm-experiment wasm-gc` and
`noble wasm-experiment managed-linear-memory` emit the checked experimental
vocabulary as WAT. They are not general source-compilation commands.

[M3 evidence](verification/m3-wasm/evidence.json) selects **managed linear memory
for M4** because its bounded arena is observable and does not require WasmGC,
not because it wins a speed or physical-memory comparison. Both candidates pass;
the selected layout retains one extra 64-KiB linear-memory page. Physical GC
allocation/reclamation and isolated engine peaks remain unknown.

The pure emitter's actual extraction has 112 local functions and no local opaque
bodies. Its 13 explicit external models and strict kernel bridge are audited
separately from runtime execution. Owned WAT, assembler, optimizer, engine and
loader remain trust boundaries; PO-17/18 and SO-07 are open. General end-to-end
core, runtime proof companions, resources and component interoperability remain
M4, MC2 and M5 work.

## Current work

1. Read [SPEC-0001](.cairn/specs/language/spec.md) and [the bootstrap scope](.cairn/specs/core-bootstrap/spec.md).
2. Follow [the implementation roadmap](specs/ROADMAP.md).
3. Run the document checks with Bun:

```sh
bun tools/cairn-specs.mjs --self-test
bun test tools/cairn-specs.test.mjs tools/check-specs.test.mjs
bun tools/check-specs.mjs --self-test
bun tools/cairn.mjs validate --root .
```

These checks do not run Noble or prove its safety.

## Source of truth

| Location | Role |
|---|---|
| `.cairn/specs/` | Canonical Cairn specifications, revision `0.1.0-draft.5` |
| `specs/` | Supporting ledgers, roadmap, scenario designs, and generated compatibility views |
| [specs/STATUS.json](specs/STATUS.json) | Milestone state and separate assurance dimensions |
| [specs/REVIEW-RESOLUTION.md](specs/REVIEW-RESOLUTION.md) | Disposition of the nine review findings |
| [REVIEW.md](REVIEW.md) and `review/` | Original review and its evidence |
| `noble-project-handoff-2026-09-12/` and the ZIP | Preserved historical snapshot, not current instructions |

The status ledger, not a successful example or document check, determines milestone acceptance. MC1 is a frontend/rules milestone, not a completed language runtime.
Earlier records retain their original scopes: the [budget extraction probe](proofs/m1/README.md), [component review](verification/component-review.md), [tool-selection boundary](verification/tool-selection.md), [M1 runbook](verification/m1-runbook.md), [control matrix](verification/m1-controls.md), and [boundary controls](verification/boundary-controls.md) document implementation experiments, observed controls, and historical open work. They do not substitute for current MC1 or M3 evidence.
