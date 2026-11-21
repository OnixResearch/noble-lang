# Integrator review: `cairn/acorr-gate` (ACORR gate fixes), 2026-11-21

**Reviewer:** the campaign integrator, under owner decision 28 (2026-11-21): reviewer agents are
unavailable, so the integrator reviews branches it did not implement. This is an integrator review,
not a fresh-context one. The owner keeps a veto before any push.

**Reviewed commit:** `4fbf1a53e6023d14fea9cd9b9f182a678e44872c` (two commits on `6e5edf4c`:
`d710e251`, `4fbf1a53`). Neither commit was written by the integrator.

| File | sha256 at `4fbf1a53` |
|---|---|
| `verification/acorr/gate.mjs` | `0e352bbe57591525ea22b59033729273226d352c58c871cbd5aa26e901756da0` |
| `verification/acorr/prepare.mjs` (unchanged) | `c197378ac37dda45d47f107345579ce0578bf9e4bb9f972eac120b128e865f74` |
| `specs/conformance/artifact-correspondence-cases.json` | `c137686182311f62e15c1cd2f5f57295b39e843cfdbd975f2d4d96a04bb5865a` |

## What was checked

Every hunk was read against the change's design table
(`.cairn/changes/artifact-correspondence-extension/design.md`, "ACORR-CASE" rows) and the PAA-1
report shape. `node --check` passes on the reviewed gate. No consumer (Rust) code changes.

| Hunk | Finding | Disposition |
|---|---|---|
| Admitted counters read from `envelope.invocation` | The design keeps host-selected invocation observations outside the envelope identity; a refusal report keeps the top-level pair. The old reads of an absent top-level field made every admitted case fail. Every admitted variant now asserts both counters. | Correct |
| ACORR-CASE-03 split | Design line 149: candidate *and* SOURCE `[ 2 + ]` refuse `subject-mismatch`; candidate `[ 2 + ]` bytes with SOURCE `[ 1 + ]` refuse `correspondence-reject`. The gate now runs both with those sources. The registration's `input.source` changes from `[ 1 + ]` to `[ 2 + ]` and records the second control under `also`, matching the design. The case is `not-run`, so no promoted state is rewritten. | Correct |
| Prepared pin documents named per edit | Fixes the EEXIST on the second altered pin document (CASE-04/08/12). | Correct |
| CASE-06 mutation anchor | Now S-CASE-07's targeted push mutation (`(call $push_i64 (i64.const 1))` to `const 2`), which the design names; the old first-`(i64.add` anchor mutated heap accounting. `replaceUnique` refuses unless the anchor occurs exactly once. | Correct |
| CASE-08 helper mutation | Alters `$op_add`'s body in the candidate's emitted WAT. The design says "a candidate built by an independently rebuilt CLI whose `operations.wat` add helper is altered"; a WAT-level edit of the same helper yields the same candidate-side divergence without trusting a second build. Recorded as a note, not a defect. | Accept with note N-1 |
| CASE-09 copies | The certified-call candidate now holds the call artifact, not the subject module; before, the call comparison could never admit. | Correct |
| CASE-11 loop order | One producer candidate per mode, offered under both proof artifacts, so the erasure control compares the consumer's rebuilds rather than two producer builds. | Correct |
| `--only` | Rehearsal subsets never yield `passed`; unselected cases stay `not-executed` and the result is `incomplete`. | Correct |

**Note N-1.** CASE-08's helper control mutates the emitted helper in the candidate rather than
rebuilding a CLI with an altered `operations.wat`. The receipt must describe it that way; promotion
text must not claim an independently rebuilt CLI.

## Verdict

**ACCEPT (integrator review).** The fixes align the gate with the design and change no admission
code. Promotion of ACORR-CASE-01..12 still requires a complete gate run on a clean commit with the
barrier CLI, recorded in an immutable receipt; this review promotes nothing.
