# M1 workspace design

## Context

The repository contains twelve accepted draft specs, document validators, and unexecuted scenario designs.
[The roadmap](../../../specs/ROADMAP.md) defines M1 as the Rust/Nix workspace and its assurance gates.
[The toolchain policy](../../specs/verification-toolchain/spec.md) requires Charon → Aeneas → Lean for the semantic kernel.
[The evidence policy](../../specs/evidence/spec.md) separates document results, test results, extraction, and proofs.

This design adds no implementation evidence. The existing toolchain-lock template still contains unselected fields.
All work below belongs to the open tasks in this change.

## Goals and Non-goals

M1 provides a reproducible workspace, explicit ownership boundaries, actual extraction, and enforced acceptance checks.
M1 does not deliver a checker, executable Noble, a Wasm backend, or M2's nontrivial refinement proof.
A compiled generated Lean module establishes only the named translation and compilation result.

## Decisions

### Decision: Isolate a small kernel from the CLI shell

**Choice:** Start with `crates/noble-kernel` and `crates/noble-cli`. Keep dependencies directed from the shell toward the kernel.

The kernel uses safe sequential Rust with explicit owned inputs and typed results.
It uses `no_std` for host independence, with `alloc` only when the confirmed extraction subset requires owned collections.
The CLI composition root owns arguments, files, process execution, environment access, and output.
No generic `common` crate or speculative port abstraction enters the workspace.
Real external capabilities use application-owned contracts and separate adapters when they appear.

The initial extraction subject is a bounded budget transition in the kernel, with accepted and exhausted outcomes.
Its exact internal types depend on the compatibility experiment. It adds no public Noble operation or syntax.
The shell invokes the actual function through an internal smoke-test route, not a fake language evaluator.
A positive budget decrements once. A zero budget returns exhaustion without underflow or an external effect.

**Rationale:** A real production function establishes the extraction path without prematurely selecting the M2 checker representation.
A host-independent crate helps enforce boundaries, but `no_std` alone proves neither purity nor extractability.

### Decision: Select pins through a bounded compatibility experiment

**Choice:** Use a Nix flake with immutable inputs and let Nix generate `flake.lock`.
Select the Charon revision, extraction rustc, and Lean backend required by the chosen Aeneas revision.
Record production Rust, Lean/Lake, proof libraries, Miri, Nixpkgs, Octet, and relevant build dependencies.
Different lanes can select different Rust versions when their compatibility contracts require them.

Start with `x86_64-linux` and `x86_64-unknown-linux-gnu`.
Record targets, word size, panic behavior, overflow behavior, features, generation arguments, and resource limits.
Do not invent a Wasmtime or WIT pin before M3 or M5 selects that boundary.
Keep future components explicit and unselected in the inventory.

**Rationale:** Tool names and version strings alone do not establish a compatible chain.
An actual extraction and Lean build must support the selected pins before M1 acceptance.
Ambient sibling checkouts and manual pin-check overrides are not product inputs.

### Decision: Reuse published components only after a contract review

**Choice:** Review a published immutable Octet revision before selection.
Compare its deny-all template, architecture policy, compiler-fact coverage, and supported crate kinds with Noble's requirements.
Consume that revision as a pinned input instead of copying its implementation.
Review upstream Charon and Aeneas compatibility files and their Lean support at exact revisions.

Record each component's repository, revision, consumed interface, assumptions, and compatibility result.
The existing Octet adoption reference is design evidence, not the selected Noble dependency.
Do not add a byte-view library, bounded executor, or receipt framework without a concrete matching boundary.

**Rationale:** Reuse must preserve semantics and evidence ownership, not merely provide a similarly named command.
A missing required compiler-fact surface remains a blocker, not permission to use an advisory result.

### Decision: Derive coverage from the configured build

**Choice:** Keep a reviewed inventory under `verification/` with a compiler-derived source and function comparison.
Classify all production Rust, generated and macro bodies, dependencies, tests, non-Rust tools, and future components.
The inventory distinguishes extracted functions, external models, reviewed exceptions, and explicit open work.
Extraction and refinement have independent statuses and counts.

Keep complete project accounting separate from the small scope claimed by the smoke test.
An explicitly open future component does not fail a smoke-only claim, but an omitted current function fails coverage.
Kernel functions cannot acquire exceptions through relocation into an adapter crate.
Non-kernel exceptions require exact symbols, configuration, owner, diagnostic, contract, assumptions, evidence, and reassessment conditions.

**Rationale:** A hand-maintained list of successful functions cannot detect an omitted or configuration-dependent body.
Unknown, stale, ambiguous, or over-limit required facts must block the corresponding claim.

### Decision: Regenerate the actual Rust extraction

**Choice:** Add a bounded build entry point that runs Charon, Aeneas, and Lean over the actual workspace function.
Store generated modules separately from handwritten reference contracts and future refinement proofs.
Bind the output to the source, dependency closure, toolchain, target, features, arguments, and resource limits.
Regenerate affected outputs after any relevant input change.

The M1 lane compiles the generated Lean module and checks exact extraction coverage for its named subject.
It does not substitute a handwritten Lean implementation for the extracted Rust function.
Proof-required claims remain blocked until the corresponding refinement evidence exists.
M2 owns the nontrivial proof and complete finite checker experiment.

**Rationale:** A tool-version check or an unrelated upstream fixture cannot establish extraction of Noble's implementation.
Successful translation cannot establish refinement or Wasm preservation.

### Decision: Apply the full quality gate to the workspace

**Choice:** Use the selected Octet revision for both its pre-commit hook and the Nix gate.
Declare `[workspace.metadata.octet]` and `dylint.toml` with the actual core source scopes.
Use `args: ["--workspace", "--", "--all-targets", "--all-features"]` for the deny-all hook.
Do not add disabled lints, warning budgets, finding baselines, or broad suppressions.
Any narrow source allowance requires its exact scope, reason, and named owner.

Keep the architecture policy in Nickel with a checked export and freshness manifest.
Declare roles, packages, source scopes, ports, adapters, providers, allowed dependencies, targets, and feature coverage.
Cover production binaries, libraries, and tests. A library-only collector result cannot satisfy the complete gate.
Do not treat an empty policy, inventory mode, or unsupported configuration as enforcement.
Use a complementary compiler-backed check only when it covers the same required contract.

**Rationale:** Pure-core linting and architecture policy enforce different contracts.
The same required checks must run locally and through `nix flake check -L` in CI.
Incompatible feature combinations require an explicit matrix instead of silent exclusions.

### Decision: Separate native-boundary and evidence claims

**Choice:** Record native boundaries, external dependencies, generated code, and any Noble-owned unsafe sites.
An empty Noble-owned unsafe inventory is valid only after source accounting establishes that it is empty.
It does not claim that dependencies contain no unsafe code.
Pin Miri's Rust, target, and memory-model configuration for scoped positive and negative runs.
An unsupported required Miri run remains a blocker.

Use BLAKE3 for new Noble-owned evidence identities unless an existing format requires another digest.
Keep existing validator formats unchanged. Keep external Octet formats behind their versioned interface.
Consumers select the expected subject, claim, source, policy, and configuration independently of producer receipts.
Record each phase result and each incomplete scope without promotion to a stronger evidence role.

**Rationale:** An effect plan is not execution evidence. An artifact digest is not authenticity or correctness.
A passing lint phase cannot hide failed extraction, missing collection, or open proof obligations.

## Requirement Traceability

The [delta](specs/verification-toolchain/spec.md) contains every new task-linked requirement.
The following map preserves every requirement assigned to M1 in `specs/roadmap.json`.

| New requirement | Existing contracts | Planned acceptance |
|---|---|---|
| VT-M1-01 | VT-PIN-01, VT-PIN-02 | Immutable compatible inputs and actual toolchain execution |
| VT-M1-02 | VT-SCOPE-02, VT-SCOPE-03 | Inward dependencies, deterministic kernel, separate effects |
| VT-M1-03 | VT-SCOPE-01, VT-SCOPE-04, VT-SCOPE-05 | Complete source accounting and exact exceptions |
| VT-M1-04 | VT-CI-05, VT-SCOPE-05 | Actual extraction, coverage comparison, independent proof status |
| VT-M1-05 | VT-OCTET-01, VT-OCTET-02 | Full deny-all catalog and compiler-backed architecture policy |
| VT-M1-06 | VT-NATIVE-01, VT-NATIVE-02, VT-NATIVE-03 | Native inventory, dependency records, scoped Miri runs |
| VT-M1-07 | VT-OCTET-03, EV-BIND-01, EV-TIER-01 | Bound phase results and scoped acceptance without evidence promotion |

## Planned Artifacts

These paths are proposed implementation outputs, not present artifacts or completed work.

| Area | Paths and contents |
|---|---|
| Workspace | `Cargo.toml`, `Cargo.lock`, `crates/noble-kernel/`, `crates/noble-cli/` |
| Reproducible tools | `flake.nix`, Nix-generated `flake.lock`, `verification/toolchain-lock.json` |
| Source and native scope | `verification/` inventories, exceptions, dependency records, evidence schemas, and gate tests |
| Architecture policy | `policy/architecture.ncl`, checked export, freshness manifest, `dylint.toml` |
| Extraction | `proofs/` build configuration, generated-module destination, and bounded extraction entry point |
| Local and CI checks | `.pre-commit-config.yaml`, `.github/workflows/ci.yml`, and Nix check outputs |

Human-authored policy uses Nickel. Rust owns any new runtime receipt types, while build tools retain their own scope classification.
Generated extraction artifacts need not enter Git if deterministic builds retain their exact identities and evidence.
Durable implementation receipts belong outside disposable worktrees, in the main repository's ignored `.pi/` or a secure home path.

## Verification Plan

### Planning-package checks

Run the existing Bun conversion checks, regression tests, and document-validator self-tests.
Run the proposal, design, and tasks gates for `m1-workspace` with the explicit worktree root.
Run Cairn validation. A sync dry-run can inspect delta applicability, but no sync or archive executes during planning.

### Implementation checks

The workspace will expose the following required checks:

```sh
cargo fmt --all -- --check
cargo test --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
pre-commit run octet-deny-all --all-files
nix flake check -L
```

The Nix checks also run document regressions, policy freshness, architecture enforcement, source coverage, extraction, and the declared native-assurance lane.
The first implementation task must name their exact commands before the corresponding tasks can close.
No unavailable future command counts as an executed check in this proposal.

| Control | Positive observation | Required rejection |
|---|---|---|
| Toolchain | The selected chain extracts the workspace subject and compiles generated Lean | Missing pin, mismatched Charon/backend, stale lock binding, or manual override |
| Core and shell | Positive budget decrements once, zero budget returns exhaustion | Kernel I/O, unsafe body, inward host type, or reverse crate dependency |
| Source coverage | Every configured current body has an explicit classification | Missing source, macro body, target, feature, stale mapping, or kernel exception |
| Extraction | Fresh output names the actual Rust subject and expected generated module | Missing function, unsupported body, unrelated fixture, edited output, or unexplained opaque model |
| Proof boundary | Smoke output remains extraction-only with open refinement status | Proof-required acceptance with missing proof, proof hole, timeout, or reference-only theorem |
| Octet | The full catalog and reviewed architecture policy pass for all required targets | Lint finding, stale export, empty policy, missing compiler facts, or advisory-only output |
| Native boundary | The scoped inventory and pinned Miri controls pass | Unrecorded unsafe site, missing safety argument, invalid boundary, or unsupported required run |
| Evidence | Independently expected bindings match each required phase | Changed subject, source, policy, target, feature, missing phase, or promotion of lint results to proof |

Existing VERIFY-01 through VERIFY-05 and OCTET-08 through OCTET-10 remain design references until their own recorded procedures execute.
M1 can execute applicable gate controls without marking the whole source scenario or any Noble runtime case as passed.

## Risks / Trade-offs

- Compatible Charon, Aeneas, Lean, production Rust, and Miri inputs can require separate Rust versions and substantial builds.
- Octet can lack required facts for a crate kind or configuration. That required scope stays blocked until equivalent coverage exists.
- A small smoke test can encourage an overstated assurance claim. Separate counts and negative controls prevent that promotion.
- Nix builds require disk space and external source access. Resource failure remains operational failure, not a passing unsupported lane.
- The first host target limits initial evidence. Additional platforms remain unsupported until their explicit matrix passes.

## Rollout and Recovery

Implementation proceeds in task order from compatible pins through workspace, inventories, gates, and retained evidence.
A failed probe leaves the change active with its exact diagnostic and configuration.
No accepted requirement becomes optional because a tool cannot implement it.

After the implementation acceptance checks pass, inspect the sync plan before execution.
After sync, regenerate compatibility views and requirement ledgers for the accepted delta.
Check that every new native requirement lands in the accepted specification.
After the final post-sync checks pass, mark the implementation checklist complete.
Archive follows the completed checklist and its unblocked dry-run plan.
This planning commit leaves M1 `not-started` and all implementation tasks unchecked.
