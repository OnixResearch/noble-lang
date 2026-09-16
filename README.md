# Noble

**Start with [the Cairn specs](.cairn/README.md).** Noble is a new implementation project. No prior compiler or runtime repository exists.

Noble is a statically typed concatenative language with first-class, inspectable programs and Wasm execution.

**Implementation direction: Rust → Charon → Aeneas → Lean 4.** This route is mandatory for the entire semantic kernel and the target for all Noble-owned production Rust. Host effects require explicit proof boundaries. See [the toolchain policy](.cairn/specs/verification-toolchain/spec.md).

**Application verification:** [typed contracts and first-class evidence companions](.cairn/specs/program-contracts/spec.md) support compositional proofs and proof-required admission. This optional profile preserves ordinary program execution. Its implementation and concrete syntax remain open.

```text
41 [ 1 + ] run
# expected: 42
```

This is a specification example, not an executed result.

**First reference application:** [an exact programmable calculator](.cairn/specs/calculator/spec.md) with rational division and exact decimal input. It will evaluate AI-authored changes under independent acceptance criteria. This application follows the core and library gates. No calculator exists yet.

**Worker substrate contracts:** [the typed-worker conformance slice](specs/WORKER-CONFORMANCE.md) connects generated-code admission, explicit state, and cancellation-safe resource ownership. It adds no agent syntax or completed swarm runtime.

**Octet adoption:** [the boundary-contract map](specs/OCTET-ADOPTION.md) adds typed authorization/receipt rules and explicit Rust architecture gates. Octet evidence stays separate from Noble semantic proofs.

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
| [specs/STATUS.json](specs/STATUS.json) | Greenfield state and separate assurance dimensions |
| [specs/REVIEW-RESOLUTION.md](specs/REVIEW-RESOLUTION.md) | Disposition of the nine review findings |
| [REVIEW.md](REVIEW.md) and `review/` | Original review and its evidence |
| `noble-project-handoff-2026-09-12/` and the ZIP | Preserved historical snapshot, not current instructions |

No runtime feature or proof is complete. M1 is active.
The [budget extraction probe](proofs/m1/README.md) covers one internal Rust function and its generated Lean module.
The [component review](verification/component-review.md) records the published Octet repairs, pinned tools, and passing incremental workspace gates.
The [tool-selection boundary](verification/tool-selection.md) separates immutable configuration checks from executed extraction and scoped Miri observations.
The [M1 runbook](verification/m1-runbook.md) separates available procedures from reserved inventory, native, and acceptance commands.
The [control matrix](verification/m1-controls.md) records observed, historical, and planned scopes. Full M1 acceptance remains open.
The [boundary controls](verification/boundary-controls.md) retain the successful body-owner repair. A new randomness control exposes a missing compiler effect. Task 2.2 and M1 acceptance remain open.
