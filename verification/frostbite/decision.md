# Frostbite correspondence decision (#140)

**Decision: DECIDED as a written correspondence; the hardware target is NO-GO-until-artifact.**
No public ISA document, RTL, official simulator or FPGA playground for Frostbite could be pinned by
content identity on 2026-11-19, so no lowering, no simulator run and no measurement was attempted.
This decision does not amend #37's NO-GO for default adoption of the net backend, the opt-in status of
`noble ngraph eval --net`, the labelled DUP/SUP model gate, or any Wasm or acceptance path.

The facts, sources checked, correspondence table and frozen comparison protocol are in
[README.md](README.md); this file records the decision per acceptance item and the conditions that
reopen it.

## Frozen criteria

These are the criteria the decision is judged by, committed before any artifact existed and before any
measurement governed by them:

1. A hardware target is admissible only if its artifact can be named by content identity — a public
   repository commit or tag with the exact bytes at a path, or an archived document/release asset with
   a SHA-256 and a retrieval record — and if it defines claim atomicity, memory visibility and the
   arithmetic contract the lowered nets use.
2. Agreement is checked against the existing software reducer on the same net and inputs, at the same
   fuel: ordered outputs, total interaction count, and the peak-live-agent bound. Timing is not an
   admission criterion.
3. A construct the artifact cannot express is reported as `unsupported`; it is never a silent pass.
4. A disagreement with #124's equality results, or with inets' claim-count/thread-count confluence, is
   a finding and must carry a replay bundle that reproduces on the software arm alone.
5. The authors' 4.59×, 8.77×, 6.02× and 3.83× figures, the TCPU identity, performance-per-watt,
   performance-per-area, GDS frequency and area are not criteria here.
6. Noble's linearity and single-ownership rule stays in force: a peek or copy that would alias a value
   without an explicit Noble-legal duplication rule is a rejected or unsupported lowering.

## Per acceptance item

| #140 acceptance item | Decision | Where it is carried | Verification |
|---|---|---|---|
| The campaign is selected only by an explicit request; it does not change default Noble acceptance, Wasm execution, or the existing I64 net backend | **DONE / DECIDED — default-off.** This slice adds two documents and nothing else; no backend, default, contract or acceptance path changed | [README](README.md) header and "Pinned references → Noble side"; this file's header | `git diff --stat 725245b9..HEAD` lists only `verification/frostbite/`; `git status --short` clean after commit |
| Until the ISA, RTL or FPGA playground is publicly pinned, record the hardware target as unavailable and limit work to a written correspondence — active pairs, linearity, fanout, erase, labelled DUP/SUP against take/peek/copy and work-stealing frames — and state that the write-up is not a pass for a hardware run | **DONE / DECIDED.** Hardware recorded unavailable on dated checks; the correspondence covers every construct named in the item, including the work-stealing frames | [README](README.md) "Sources checked for a pinnable hardware artifact", "Hardware status", "Correspondence" (eleven rows: active pairs, linearity, DUP, ERA, labelled DUP/SUP, I64 arithmetic nodes, plan/worklist, take, peek, copy, frames and stealing) | checks tabulated with dates 2026-11-18/19: blog read (states RTL/ISA unreleased), X post HTTP 404, six `architectlabs.com` paths HTTP 404, both `*.architectlabs.com` hosts unreachable, GitHub account with zero public repositories, repository search 0 results, arXiv none |
| **If** a public ISA, official simulator or FPGA playground can be pinned, lower a frozen subset of already admitted Noble I64 nets (or an equivalent inets fixture) and check results against the existing software reducer on the same inputs; a missing construct is unsupported, not a silent pass | **DECIDED — NO-GO-until-artifact; the run itself is NOT DONE because no artifact exists.** The protocol and the first fixture set are frozen now, so the first admissible artifact cannot move them | [README](README.md) "Frozen comparison protocol" steps 1–6, with the fixture set named explicitly | no run is claimed; the condition is instrumented, not satisfied. This is the honest state of a conditional acceptance item |
| Pin the inets reference revision used for the comparison; a disagreement with #124's schedule independence or equal interaction counts is a finding with a replay bundle | **DONE.** Pinned `98bbb01070664e4462a35efa5d62de6f27143572` (SHA-256 `c7a35dad…`, vendored, digest-enforced), with the issue's `01bd554a…` and current HEAD `eaddcc14…` recorded and the reason for the choice stated. The replay-bundle rule is frozen | [README](README.md) "Pinned references" (three revisions compared by blob and ancestry; constructs present/absent tabulated) and "Frozen comparison protocol" step 5 | `git ls-remote` and the repository's commit/compare/contents endpoints on 2026-11-19; theorem names located in the vendored file by `grep`. No disagreement is claimed or tested: nothing ran |
| Do not import the authors' 4.59×, 8.77×, 6.02× or 3.83× figures, or their TCPU identity, as a pass or fail criterion; perf-per-watt and area are out of scope | **DONE / DECIDED.** Recorded as non-claims; no figure appears as evidence anywhere in the two documents except as a quoted claim being excluded | [README](README.md) "Non-claims"; criterion 5 above | the documents contain no measurement and no threshold derived from the claims |
| Linearity and single ownership stay in force: a peek or copy that would alias a value without an explicit Noble-legal duplication rule is a rejected or unsupported lowering, not an optimization | **DONE / DECIDED.** Each aliasing mechanism is classified: `copy` of a live value is **rejected** as described; cross-net `take` and speculative cross-net `peek` are **unsupported**; duplication is legal only through the net's own `dup`/`dupL`–`sup` rules | [README](README.md) "Correspondence" rows *Linearity*, *Fanout/DUP*, *take*, *peek*, *copy*, *frames and stealing*, each with the construct it would break | the classification is derived from the pinned machine's action set and `Writes`, and from Noble's rule table; the exact declarations are named in "Constructs verified present in the pinned bytes" |

Summary: five items DONE/DECIDED, one item (the conditional hardware run) DECIDED as blocked by a
missing artifact with its protocol frozen. Nothing is claimed as executed that was not executed.

## Why NO-GO now, and not "await"

The acceptance item is conditional on an artifact, so the conservative call is to decide the condition
rather than to leave the item open-ended:

- The post states the ISA and RTL are not released and names releasing them, and an FPGA playground, as
  future work. There is no artifact whose bytes could be named today, so a run has no input identity.
- A correspondence written without an artifact is not a substitute: the blog defines no claim
  atomicity, memory-visibility rule, instruction encoding, frame layout, queue arbitration or
  arithmetic contract, so the rows that matter — `take` beyond the claim, `peek` across a net, `copy` of
  a live value, stealing a half-claimed pair — cannot be tested even in principle against it.
- Deciding NO-GO-until-artifact keeps the item falsifiable: the protocol, the fixture set and the
  disagreement rule are fixed before any artifact appears, which is the only way a later run can be
  honest about whether it moved a goalpost.

## Reopen conditions

The hardware item reopens — and a GO requires all of the following — when:

1. **An artifact exists and is pinned.** Any one of: the ISA document, official simulator, or FPGA
   playground/bitstream source, published at a public repository commit or tag with the exact bytes at
   a path, or as an archived document/release asset with a SHA-256 and a retrieval record, with a
   version string and build recipe. Recorded in a receipt *before* any run (protocol step 1).
2. **The artifact states the three contracts the correspondence needs.** Claim atomicity (can another
   tile observe a half-claimed pair?), memory visibility for the read/peek path, and the arithmetic
   contract for 64-bit wrapping add/sub/mul and equality with exact bit patterns and operand order. If
   any is missing, the corresponding rows stay `unsupported` and the item cannot be recorded as passed.
3. **The first fixture set is the one frozen in the README** — the five plan families of
   `verification/net-plans/criteria.md`, the four rule images of the plan corpus in both orientations
   and both storage policies with their extreme-payload extras, and the labelled fixtures only if the
   artifact claims a labelled/alternative primitive. No additions and no substitutions at first
   lowering; an addition is a new frozen revision with its own record.
4. **The software oracle is unchanged** on those inputs, and agreement is claimed per construct with
   `unsupported` and `refused` reported separately from `agrees` (protocol steps 3–4).
5. **Any disagreement carries the replay bundle of protocol step 5**, reproducible on the software arm
   alone.

If a public artifact appears but states none of the three contracts, the outcome is recorded as
**unsupported for the hardware path** — not as a GO and not as a failure of this lane. If a pinned
artifact contradicts #124's equality results on the frozen fixtures, that is a finding about the
software path, handled by #124's own replay rule, and it does not authorize any change to defaults here.

## What this lane does not decide

- It does not decide that Frostbite is expressible as Noble's I64 interaction system, nor the converse;
  the correspondence table records requirements and verdicts, not a translation.
- It does not decide anything about Vine-to-ISA lowering or baseline measurement, which belong to
  [compiler-lab #266](https://github.com/brittonr/compiler-lab/issues/266), nor about the upstream
  reducer-level question (claims, `Copies`, worklist machines, starvation), which belongs to
  [inets #12](https://github.com/OnixResearch/inets/issues/12).
- It does not re-pin `proofs/inets`. The revision analysis in the README explains why a later upstream
  revision (`01bd554a…` or HEAD `eaddcc14…`, which restructures the library into modules) is a
  vendoring change plus a fresh foundation receipt, and therefore an owner/integrator decision rather
  than part of this slice.
- It does not change `.cairn/specs/`, the labelled DUP/SUP gate, the #110 plan comparison, #124's
  results, or #37's NO-GO and frozen 1.15 threshold.

## Checks for this change

The lane is documentation only; the checks are the repository's documents and Cairn gates, run from a
clean committed tree of this branch:

```sh
bun tools/cairn-specs.mjs --self-test
NOBLE_GIT="$(command -v git)" bun test tools/cairn-specs.test.mjs tools/check-specs.test.mjs \
  tools/calculator-headroom.test.mjs tools/source-inventory-renew.test.mjs
bun tools/check-specs.mjs --self-test
cairn validate --root "$PWD" --policy <cairn>/cairn-policy/generated/cairn-policy.json
```

No Rust, Lean, Nix or policy file changed, so the Octet, clippy, test and Lean lanes are unaffected by
this slice; the integrator runs the full ladder on the merged tree.
