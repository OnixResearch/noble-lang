# M1 implementation checkpoint

## Result

**Blocked at toolchain selection.** Tasks 1.1 and 1.2 are complete. The other nineteen tasks remain open.
No production workspace, compatible pin set, extraction result, or M1 completion is claimed.
The change remains active. No spec sync or archive executed.

The required deny-all hook command fails with active architecture policy at the inspected published Octet revision.
A control with an explicit feature set passes on the same fixture.
Replacing the required flag is not an accepted workaround in this checkpoint.

## Baseline

Base commit: `3d77e573d67f73b6faf97d2cc86878b30d99457c`.
Implementation branch: `impl/m1-workspace-20260913`.
The baseline ran before any implementation edits and had no failing repository check.

| Check | Observed result |
|---|---|
| `bun tools/cairn-specs.mjs --self-test` | Passed, 26 self-tests |
| `bun test tools/cairn-specs.test.mjs tools/check-specs.test.mjs` | 97 passed, 0 failed |
| `bun tools/check-specs.mjs --self-test` | Passed, 226 self-tests |
| Cairn proposal, design, and tasks gates | All passed |
| Cairn validation | Valid, one active change |

Every lifecycle command used this explicit root:

```text
/home/brittonr/git/noble-worktrees/m1-workspace-implementation-20260913
```

The exact lifecycle commands were:

```sh
bun tools/cairn.mjs gate proposal m1-workspace --root "$ROOT"
bun tools/cairn.mjs gate design m1-workspace --root "$ROOT"
bun tools/cairn.mjs gate tasks m1-workspace --root "$ROOT"
bun tools/cairn.mjs validate --root "$ROOT"
```

The independent acceptance contract requires all seven delta requirements and every inherited M1 roadmap requirement.
Actual production extraction, complete configured coverage, full deny-all linting, architecture enforcement, scoped Miri controls, and bound phase evidence remain required.
The 475 accepted requirements, 149 existing scenario designs, and 30 open proof obligations remain unchanged.

## Published Dependency Review

### Octet candidate

Published revision: `42dc32a6fe921d35182d7aef03c2a83999d55771` from `git@github.com:OnixResearch/octet.git`.
Nix fetched the immutable source. The review did not use the modified state of a sibling checkout.

Inspected interfaces include `cargo-octet`, `hooks/octet-deny-all.sh`, the architecture templates, `octet-standards`, and the Aeneas provider contract.
The source declares Rust `nightly-2026-03-21` for its compiler tooling.
Its Nickel package is revision `1320a983e6c3d1e2fb53dd2464b084b4903b1426`.
The executed evaluator reported `nickel-lang-cli nickel 1.17.0 (rev 1320a98)`.

Nix successfully provided `cargo-octet`, `octet-standards`, and `nickel-1-17-0` from this revision.
These package results establish tool availability, not Noble acceptance.

[The architecture contract](https://github.com/OnixResearch/octet/blob/42dc32a6fe921d35182d7aef03c2a83999d55771/docs/project-architecture-policy.md) now covers explicit `--all-targets` selection, including binaries and tests.
It still rejects `--all-features` and requires explicit feature rows.
[The implementation](https://github.com/OnixResearch/octet/blob/42dc32a6fe921d35182d7aef03c2a83999d55771/cargo-octet/src/architecture_ir.rs#L854) rejects that flag before compiler collection.
The hook calls this same `cargo octet check` path. Its deny-level flags do not bypass architecture collection.

Octet owns its lint catalog, compiler facts, policy evaluator, and external artifact formats.
Noble owns its source scope, required configurations, policy intent, and acceptance claims.
No imported Octet result can establish Noble extraction or a semantic proof.

### Upstream-matched Aeneas candidate

Aeneas revision: `505b6ca35217e7be5c96c3e2f8045edfbdf47291`.
Its `charon-pin` requires Charon `b104e24fea7d721b71e6c39fd70f26ff20bc0980`.
The inspected Charon source selects `nightly-2026-08-18`, including Miri and `x86_64-unknown-linux-gnu` support.
The Aeneas Lean backend requires Lean `v4.31.0`.
Its Lake manifest pins Mathlib to `fabf563a7c95a166b8d7b6efca11c8b4dc9d911f` and records transitive dependency revisions.

The [upstream instructions](https://github.com/AeneasVerif/aeneas/blob/505b6ca35217e7be5c96c3e2f8045edfbdf47291/README.md) require actual Charon extraction before Aeneas translation.
The Lean backend library must match the selected Aeneas source.
Manual local symlinks that disable upstream pin checks are not acceptable Noble inputs.
This review establishes declared compatibility constraints only. It does not establish an executed compatible chain.

### Published packaging alternative

Octet's provider profile selects Aeneas `bc2eb6f941369fa3c695e30598b5758b11e4f0de` and Charon `cb50ff16b9f1066b8a97dc06da704de2da2fa41c`.
It packages Lean `v4.31.0` from source revision `68218e876d2a38b1985b8590fff244a83c321783`.
This older cohort is distinct from the upstream-matched candidate and cannot mix their components silently.

The provider build started, but it was stopped after the mandatory Octet invocation failed.
No successful provider build, actual Noble extraction, or generated Lean compilation is claimed.
The [consumer contract](https://github.com/OnixResearch/octet/blob/42dc32a6fe921d35182d7aef03c2a83999d55771/docs/aeneas-consumer-contract.md) binds exact source items, artifacts, theorem intent, and resource policy.
Its example theorem does not satisfy Noble's production-source requirements.

## Executed Compatibility Probe

The probe is an isolated fixture, not Noble production code.
It contains one Rust library, one `production` feature, and a valid Nickel architecture policy in `gate` mode.
The fixture declares a composition-root role to isolate invocation compatibility from kernel-policy design.
Fresh target directories prevent cached Cargo checks from omitting compiler shards.

The policy export and freshness manifest came from the pinned Nickel and Octet tools.
The manifest used the evaluator identity observed from the actual pinned binary.
No lint, architecture mode, or required flag was disabled for the negative probe.

| Invocation | Result |
|---|---|
| Deny-all hook with `--workspace -- --all-targets --features production` | Passed, zero lint findings and zero architecture findings |
| Same hook with `--workspace -- --all-targets --all-features` | Failed before collection |

The exact failure is:

```text
error: architecture rows require explicit --features rather than all-features or shorthand
```

The successful control's architecture receipt is:

```text
54ed67fcb2d9ac1eedcb3d70130adfcc208db091d4fd0e820cf4c13e810777e1
```

The explicit feature control covers the fixture's only feature.
It demonstrates the flag incompatibility, but it does not satisfy Noble's required hook arguments or close M1.

### Probe corrections and limits

The initial Cargo probe inherited a Rust wrapper without `rustc` on its path. The corrected probe used the pinned toolchain and process-local wrapper settings.
The manifest command required an explicit measured `--nickel-identity`. Merely placing Nickel on `PATH` did not supply that argument.
An earlier policy fixture lacked a composition root. A later run reused cached compilation and correctly failed for missing shards.
The final comparison used a valid composition-root policy and fresh target directories for both commands.
These earlier results remain retained and are not acceptance evidence.

## Evidence Retention

Raw evidence stays in the main repository, outside the disposable worktree:

```text
/home/brittonr/git/noble/.pi/m1-workspace-impl-1563/
```

It includes the independent acceptance contract, baseline JSON and test logs, immutable source roots, fixture inputs, generated policy, compiler artifacts, and command logs.
The strict explicit-feature control ran as Pueue task 1595. The final required-flag rejection ran as task 1596.
The provider build ran as task 1572 and was stopped without a compatibility claim.
`TMPDIR` used the home cache because the system temporary filesystem was full.

The search used two Aeneas cohorts and one published Octet revision with at most two concurrent build processes.
All reasoning passes ran serially in one session. They are correlated, not independent-agent review.

## Blocker and Next Check

**Owner:** Octet's Cargo-selection and architecture-collection boundary.
**Affected work:** M1 pin selection and every later phase that depends on the required quality gate.
**Next check:** publish a reviewed Octet revision that resolves `--all-features` through Cargo and binds the complete feature set to architecture facts.

The repair must retain mismatch rejection, complete target coverage, required unknown handling, and the full deny-all catalog.
Then rerun both fixture commands and the production workspace gates with the new immutable revision.
Changing Noble to warning-only mode, removing its architecture policy, or replacing the mandated hook arguments does not resolve this blocker.

No upstream repository was modified. No Noble requirement was relaxed.
