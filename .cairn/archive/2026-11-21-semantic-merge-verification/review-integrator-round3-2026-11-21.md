# Integrator review, round 3: semantic-merge-verification Phase 3 (`cairn/semantic-merge`), 2026-11-21

**Reviewer:** the campaign integrator, under owner decision 28 (2026-11-21): reviewer agents are
unavailable, so the integrator reviews branches it did not implement. This is an integrator review,
not a fresh-context one. The owner keeps a veto before any push.

**Reviewed:** branch head `9227f03c`, whose code equals the clean receipt commit `2139fbdc` (the head
adds only the receipt and two document edits). The slice agent wrote it; the integrator did not.
Round 2 (fresh-context, re-review of `f8478169`) returned REVISE on box 3 with R2-F1 (blocker),
R2-F2 (major), R2-F3 and R2-F4 (minor) and notes R2-N1..N3, reopened box 2, and accepted the
GO/NO-GO statement with notes.

| File at `2139fbdc` | sha256 |
|---|---|
| `crates/noble-contracts/tests/models/merge/lean.rs` | `4f3cb9c4a297b4239e5e62427ed95b0a0055287b140a65c0ccd6361db2293b14` |
| `crates/noble-contracts/tests/models/merge/consumer.rs` | `1666689f3c0a9d4be85fbcd09cb4b8dbd3714272cc82c4871d53d3c047d7b45f` |
| `crates/noble-contracts/tests/models/merge/claim.rs` | `20d333d0118f872e1c02b87c23deec8f59fa03ed1bc9a7ffe6cd711714a7af95` |
| `crates/noble-contracts/tests/models/merge/verification.rs` | `fdc1eb9210cf2790ba67f347710f78222d28070b9f6b0c584aaad25b62cee338` |
| `proofs/mc1/NobleContracts/Merge.lean` | `e806bf2d1fdb9898cb3e9563354a7a698a3e5d7c23d2b97012938054306f4f77` |
| `verification/semantic-merge-verification/run.mjs` | `d1afa4e759133b2a4e40c0d9089fd57d3f1b13c64e3d6906c7f2d845140e508a` |
| `verification/semantic-merge-verification/receipt-2139fbdc8bea.json` | `5936bdda2a89ae417d634f0291534d0e1b9f816bbcfad299c97d6247aa68a7d5` |

## Round-2 findings

**R2-F1 (blocker): resolved.** Acceptance no longer elaborates producer text in the accepting
process. The submission is compiled once in a producer process (fresh directory, cleared
environment, bubblewrap); only its `.olean` crosses. A fresh checker process reads that file with
`Lean.readModuleData` (data only: no import, no initializers), refuses any producer axiom, computes
`merge_claim`'s declaration closure inside the producer module, replays it in dependency order
through `Environment.addDeclCore` with the default checks on, refuses any declaration kind other
than theorems and definitions, requires the replayed type to equal (or be defeq to) the checker's
own elaboration of the regenerated claim, and walks `collectAxioms` against `propext`,
`Classical.choice` and `Quot.sound`. The acceptance marker exists only in the checker source and
binds the exact claim and proof bytes. A producer that disabled checking in its own process
(`debug.skipKernelTC`, `addDeclCore … (doCheck := false)`) leaves an `.olean` whose declarations
fail the checker's kernel replay. [INFERENCE, not tested here] Compiled-code evaluation inside the
replay (`Lean.reduceBool`) cannot reach producer definitions, which are replayed but not compiled,
and `Lean.ofReduceBool` would fail the axiom allowlist. The clean receipt `receipt-2139fbdc8bea.json`
(`dirty: false`, source revision `2139fbdc`, isolation `bubblewrap`) records the controls:

| Control | Observed |
|---|---|
| `by_elab` reads the marker from the source view and aborts | `unknown` (the checker finds no `merge_claim`) |
| `by_elab` with `debug.skipKernelTC` (CASE-03 and CASE-07) | `unknown`, not accepted |
| `by_elab` with `addDeclCore … doCheck=false` | `unknown` |
| `by_elab` IO write through a string-assembled identifier | not accepted; `side_effect_outside_workspace: false` |
| wrong proof | `unknown` |
| exhausted / internal failure | observed with real controls |

**R2-F2 (major): resolved.** The overclaims ("producer-as-data", "the submission is data, not Lean
source", "acceptance is never taken from producer-printable output") are gone from `design.md`; the
new text says the producer's elaborator code runs in the producer process and only the replay
checker decides.

**R2-F3 (minor): resolved.** Plan step 3's pass criterion (`design.md:623-624`) names the marker read,
`debug.skipKernelTC`, unchecked `addDeclCore` and an IO write.

**R2-F4 (minor): resolved.** `run.mjs` joins `proofs/mc1` with `path.join`.

**R2-N1: resolved** (`Merge.lean:1068` says `MAX + 2` wraps to `1`). R2-N2 and R2-N3 were optional.

## Remaining limits (recorded, not defects)

- The lane is a test-only model with process-level isolation, not the production consumer; the
  production integration stays unselected and the recorded decision stays NO-GO.
- The replay trusts the pinned library environment and the Lean kernel. It refuses, rather than
  replays, producer inductives, opaques and quotients.
- Merging into the batch tree raises the MC1 gate's required-theorem count to 155 (90 + 24 NamedV3 +
  10 refutation + 31 Merge); the MC1 lane must pass with that count before any push.

## Verdict

- **Box 2 (generator, consumer regeneration, isolated check): ACCEPT (integrator review).** The
  re-tick on the replay receipt stands.
- **Box 3 (SM-3–SM-6 dispositions and integration plan, step 7a): ACCEPT (integrator review).**
  SM-1/SM-2 proved, SM-3/SM-4 narrowed with reasons, SM-5/SM-6 tested only, SM-7/SM-8 documented.
- **GO/NO-GO: NO-GO stands.**
