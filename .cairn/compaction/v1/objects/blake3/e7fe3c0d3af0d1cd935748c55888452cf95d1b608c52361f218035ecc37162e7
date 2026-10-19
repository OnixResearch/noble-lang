# Task 1.3: immutable tool selection

Task 1.3 has a passing scoped result. M1 acceptance remains open. r[VT-M1-01]

## Implementation

- Reviewed Nickel configuration selects eight source revisions and NAR hashes, ten file digests, and ten tool recipe/output pairs.
- A pure Nix checker validates the observed inputs before any flake output becomes available.
- The checker compares Aeneas's Charon pin, Lean requirement, and complete resolved backend dependency set.
- The shell preflight invokes immutable executable paths and records versions and BLAKE3 hashes.
- Lean 4.31.0 comes from the hash-pinned upstream release, with Nix ELF path patches.
- A read-only helper checks ten Lake Git dependencies before and after compilation. It rejects changed sources and directory symlinks.
- Nix generated the lock. A later `flake lock --no-update-lock-file` passed without changing it.
- Rust production code, Cargo inputs, architecture policy, extraction manifests, and the published Octet pin remain unchanged.

See [tool selection](../../../verification/tool-selection.md) for ownership, interfaces, and claim limits.
The [probe runbook](../../../proofs/m1/README.md) names the executed extraction and Miri commands.
Its full M1 inventory and rejection matrix remain task 1.4 work.

## Observed checks

| Check | Result and scope |
|---|---|
| Pure selection controls | 96 passed: two positive cases and 94 rejected mutations |
| Executed override controls | 16 environment overrides, one manual argument, and one backend-directory symlink rejected with required diagnostics and status 2 |
| Real Nix override | Same-byte, path-only Charon input rejected; lock unchanged |
| Tool preflight | Selected Rust, Miri, Charon, Aeneas, Lean, and Lake commands executed |
| Extraction | Complete current kernel library translated: one function and one enum; not a project-wide inventory |
| Lean | Explicit selected executable compiled the byte-checked generated module into a fresh object |
| Backend sources | Ten Git dependencies matched before and after compilation |
| Miri | Two kernel budget integration tests passed with an explicit selected driver and strict provenance |
| Full Nix check | All ten checks passed, including the unchanged full-scope deny-all gate |
| Artifact replay | The Nix Octet bundle passed full stored-artifact verification |

The Miri setup downloaded Rust sysroot dependencies. The subsequent subject run used `--locked --offline`.
The earlier run omitted an explicit driver binding; the retained replacement run supplies it and uses a fresh sysroot cache.
No CLI or native-FFI Miri claim follows from these two tests.

## Retained failures

The primary repository retains evidence under `.pi/m1-tool-selection/`.
The directory includes command logs, failed probes, source and tool observations, generated outputs, and hash ledgers.

- A red mutation exposed a backend descriptor with valid Git fields plus an ambient path. Exact descriptor fields now reject it.
- A remote Lean transfer stalled. Local attempts failed on private-home traversal and Nix's world-writable ancestor check.
- The build succeeded in the existing Nix-owned build directory. No home permissions or directory security checks changed.
- The first version runner used incorrect Miri and Charon invocation forms. It now uses `cargo miri --version` and `charon version`.
- An external Lean source path failed the compiler's working-root check. The retry compiled the byte-identical workspace copy.
- Full flake evaluation caught duplicate dynamic `apps` attributes. All three apps now share one system attribute set.

## Non-claims

Configuration consistency and artifact replay do not independently authenticate observations, hosts, builds, or caches.
The probe reused upstream compiled Lean dependencies and a native linker.
It does not establish hermeticity, refinement, complete source accounting, native-boundary assurance, CI completion, or M1 acceptance.
Noble has not been synced, archived, committed, or pushed.
