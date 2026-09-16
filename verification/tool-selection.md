# Tool selection boundary

## Scope

`policy/tool-selection.ncl` owns the reviewed tool configuration for `x86_64-linux`.
Its generated JSON is configuration, not a runtime receipt or compatibility result.
`standards/policies/registry.ncl` registers this configuration family.

The configuration selects production Rust separately from extraction Rust.
Miri uses Charon's extraction toolchain, which includes the Miri component.
The required target is `x86_64-unknown-linux-gnu`, with 64-bit words and all current features.
The Cargo profiles use unwind and explicit overflow checks.
The configuration also names translation arguments and resource limits.
Future Wasmtime, WIT, Verus, and byte-view components remain unselected.

## Ownership

The Nix observation module reads actual flake source identities and repository files.
The pure checker compares those observations with the reviewed policy.
It performs no process execution and emits no execution receipt.
Every flake output forces its rejection decision before exposing a package or check.

The checker requires eight immutable source revisions with NAR hashes.
It also compares ten file digests, including the flake declarations, Nix lock, Cargo configuration, hook, and Lake files.
The expected digests remain in reviewed Nickel source. A check does not refresh its own expected values.
Nix SHA-256 interfaces own these interoperability hashes. Execution evidence uses BLAKE3 separately.

Ten selected tool recipes and their coerced output paths also match reviewed Nix identities.
This binds their transitive build inputs without evaluating every unused optional provider.
A matching top-level Git revision alone cannot hide a changed selected tool recipe or output.
These Nix identities assume store integrity and do not independently authenticate builds or caches.

The selected Aeneas source supplies its required Charon revision and Lean toolchain.
The checker compares the resolved Lake dependencies with that source's backend manifest.
The comparison preserves the complete dependency set but ignores package ordering and inherited placement metadata.
It rejects duplicate packages, floating resolved revisions, missing dependencies, path overrides, and noncanonical package directories.
A resolved Git revision remains mandatory even when an upstream `inputRev` names a branch or release tag.

Noble reuses Nix, the pinned Octet components, and the upstream Aeneas pin check.
No provider implementation enters a Noble production crate.
A new Rust receipt crate does not fit this build-configuration boundary.
Noble owns the exact selection policy and the Nix composition code.

## Lean release packaging

The selected Nixpkgs Lean is 4.28.0. It does not satisfy this backend's 4.31.0 requirement.
`nix/lean-release.nix` therefore consumes the upstream 4.31.0 Linux release archive with a fixed SHA-256 digest.
The GitHub release API supplied the archive URL and digest; Nix checks the downloaded bytes.
The declared Lean commit is `68218e876d2a38b1985b8590fff244a83c321783`.
The tool check compares the executable's version output with that commit.

The package retains the upstream distribution and notices.
Nix patches ELF interpreter paths and library search paths for the selected host.
Those patched bytes require fresh execution evidence; the historical Elan result does not validate them.
The packaging is an external tool dependency, not Noble-owned extracted Rust or an audited upstream proof.

## Checks

Run from the dedicated Noble worktree:

```sh
nix build .#checks.x86_64-linux.tool-selection
nix build .#checks.x86_64-linux.tool-versions .#checks.x86_64-linux.tool-overrides
nix run .#toolchain-check
```

The selection check runs pure positive and rejection controls and checks Nickel export freshness.
The version check invokes the selected binaries and records their BLAKE3 digests.
The override check requires specific diagnostics and exit status 2 for every tested override.
It retains separate logs for 16 environment overrides, one manual override argument, and one backend-directory symlink.
It does not accept an unrelated process failure as a rejection result.

`noble-toolchain-check` accepts no arguments.
It rejects the named ambient tool overrides before invoking a tool.
A real Nix `--override-input` control also rejects a path-only Charon input with identical source bytes.
These checks assume the host and Nix evaluator operate as observed. They do not authenticate a hostile host or mutable build caches.

A passing version check reports tool availability only.
Compatibility additionally requires extraction of the actual workspace subject and compilation of its generated Lean module.
`noble-lean-dependencies-check` compares all ten working Git dependencies before and after compilation.
It rejects symlinked source directories and changed Git sources. It does not authenticate compiled caches.
Miri support requires an executed, explicitly scoped run with the selected driver and memory-model flags.
The complete M1 source inventory, native controls, CI, and independently bound acceptance remain separate tasks.

## Evidence

The primary repository retains this work under `.pi/m1-tool-selection/`.
The directory contains source observations, the release metadata, control logs, and failed build attempts.
The path-override red test caught a descriptor with valid Git fields plus an additional path field.
The checker now requires exact backend descriptor fields.
These synthetic controls test structural consistency, not authenticated compiler observations.

The historical extraction records remain under `.pi/m1-extraction-probe/` and `.pi/m1-quality/`.
The published-provider quality record remains under `.pi/m1-test-ownership/adoption/`.
No selection result alone completes M1 or establishes refinement.
