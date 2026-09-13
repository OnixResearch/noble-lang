# Review checks and evidence limits

Return to [the review](../REVIEW.md).

## Scope and completion contract

The goal was to extract the supplied ZIP and review all included specifications and work plans. Completion required archive checks, full document coverage, source-backed findings, and a ranked continuation plan.

A successful extraction, scenario count, expected result, or historical validation report does not establish a working compiler or a completed proof.

The review used one assistant with sequential, correlated review passes. No other agent participated. The passes covered semantic consistency, ownership/authority boundaries, evidence quality, and milestone dependencies.

The initial review budget was 15–25 minutes, scoped to the 35-file archive and three known first-party web pages. No broad repository search, toolchain installation, implementation build, or external-project audit was attempted.

## Observed filesystem state

Before extraction, `/home/brittonr/git/noble/` contained only:

```text
noble-project-handoff-2026-09-12.zip
```

The ZIP listed 35 regular files, all under one relative directory. No absolute paths, parent traversal entries, or symbolic links appeared in the listing.

The ZIP SHA-256 observed in task 345 is:

```text
62e0febf8f7f1d0dd73fcc711aa2d248edb2beac3a2cdc6d9dafea3b8a3bb20d
```

The archive expands to 472,325 bytes. Its 35 files comprise 25 Markdown files, three text summaries, six JSON files, and one checksum manifest.

A hidden/ignored-inclusive file inventory found no implementation files in the working directory. `git rev-parse --show-toplevel` reported:

```text
fatal: not a git repository (or any parent up to mount point /)
Stopping at filesystem boundary (GIT_DISCOVERY_ACROSS_FILESYSTEM not set).
```

This establishes only the state of the requested directory. It does not establish that no Noble repository exists elsewhere.

## Archive checks

Commands ran through the session-specific Pueue group `noble-review`.

| Task | Check | Observed result |
|---|---|---|
| 307 | `unzip -Z -l noble-project-handoff-2026-09-12.zip` | 35 regular files, one relative root |
| 308 | `unzip -t` followed by `unzip -n` | All entry checks passed. Extraction completed without overwrite. |
| 312, 346 | `sha256sum --check CHECKSUMS.sha256` | All 34 listed files passed before and after the review |
| 319 | `git rev-parse --show-toplevel` | Not a Git worktree in the inspected location |

Integrity checks establish agreement with the included manifest. They do not establish authorship, authenticity, semantic correctness, or trusted provenance.

## Complete preview comparisons

These comparisons read the complete diff, not selected keyword matches:

| Task | Base → preview | Result |
|---|---|---|
| 316 | `reference/baseline/SPEC-0001-baseline.md` → `reference/working-safety/SPEC-0001-safety-preview.md` | Verification and safety additions, partial surface renames |
| 317 | Safety core preview → `reference/working-wit-wasi/SPEC-0001-wit-wasi-preview.md` | Component additions retain the safety requirements and gate |
| 318 | `reference/baseline/VERIFICATION.md` → `reference/working-safety/VERIFICATION-safety-preview.md` | Safety obligations and four verification requirements added |

The commands used `diff -u`. They accepted status 0 for equality and 1 for differences. Higher statuses were not treated as successful comparisons.

Inherited text in the previews was reviewed in the complete baseline reads. These comparisons supplied every changed region.

## Requirement and scenario checks

Ripgrep counted declarations with this pattern:

```text
^\*\*(S|WI)-[A-Z]+-[0-9]+\.\*\*
```

| Material | Observed count or status |
|---|---|
| `SAFETY.md` | 37 numbered safety requirements |
| `WIT-WASI.md` | 40 numbered WIT/WASI requirements |
| Safety JSON | 15 scenario descriptions, package status `expected-not-run` |
| WIT/WASI JSON | 15 scenario descriptions, each status `not-run` |
| Safety obligation JSON | Nine obligations, each status `open` |

A Steel set comparison used the 40 requirement IDs and 19 references transcribed from the reviewed WIT files. It returned no unresolved references and 21 requirements without an explicit scenario link:

```text
WI-ARCH-01  WI-ARCH-02  WI-ARCH-03  WI-ARCH-04
WI-WIT-06  WI-RES-04   WI-WORLD-02
WI-AUTH-02 WI-AUTH-03  WI-WASI-01  WI-WASI-02
WI-ASYNC-04
WI-PROG-02 WI-PROG-03
WI-SYN-01  WI-SYN-03
WI-ID-02   WI-ID-03
WI-SAFE-01 WI-SAFE-02  WI-SAFE-03
```

This is an explicit-reference count. It is not a semantic coverage percentage or a complete fixture-schema validator. The JSON files were read as part of the review. The archived validation scripts were not available to rerun.

## Reference checks

Local Markdown links were searched and compared with the complete file inventory. Representative missing or relocated references include:

1. `reference/baseline/verification/obligations.json` and `verification/toolchain-lock.template.json`.
2. The verification `DECISIONS.md`, `SOURCES.md`, and `SPEC-AMENDMENT.md` names.
3. The verification amendment's patch, baseline manifest, and integration-preview paths.
4. Safety and WIT amendment patch directories.
5. Historical `sandbox:/mnt/data/...` packages and conversation-only citations.

Some preview-relative links intentionally assume later placement beside the baseline. That intention does not make the current handoff independently navigable. The original core fixture package was already absent from the verification amendment's inputs.

## First-party ecosystem checks

Three known URLs cited by the WIT workstream were fetched through `crw_scrape`:

| Source | Observation in this review |
|---|---|
| <https://wasi.dev/releases> | Lists WASI 0.3 as stable/current and describes runtime support as toolchain-dependent |
| <https://wasi.dev/releases/wasi-p3> | Lists 0.3.0 and 0.3.1 releases, native async, and concrete compatibility limits |
| <https://component-model.bytecodealliance.org/design/async.html> | Documents `async func`, `stream<T>`, and `future<T>` as component-boundary mechanisms |

The fetched pages corroborate the selected 0.3 direction. They do not establish a compatible Noble build.

The release page also distinguishes final 0.3 support from earlier release-candidate implementations. It lists additional Component Model requirements for 0.3.1. Therefore, a broad `0.3` label is not a substitute for an executed version matrix.

These are observations of live documentation, not immutable source snapshots or executed interoperability results. Other reference projects were not re-audited.

## Tool corrections during review

An initial ripgrep look-ahead expression failed because its default regex engine lacks look-around. The query succeeded with `--pcre2`.

An initial Steel comparison used string hash keys. This runner parsed JSON object keys as symbols. The corrected symbol-key comparison succeeded. Neither failed attempt counts as evidence.

## Final evidence boundary

| Review lens | State | Remaining limitation |
|---|---|---|
| Archive and source inventory | Checked | Integrity is not authenticity |
| Semantic and authority review | Findings documented | No mechanized counterexample or runtime execution |
| Traceability and work order | Findings documented | Missing original packages and implementation |
| Ecosystem assumptions | Three first-party pages checked | No pinned toolchain or component run |

**Terminal result:** extraction and archive review complete. Implementation readiness is blocked on source recovery and the identified contracts. No compiler, runtime, safety, or proof completion claim is made.
