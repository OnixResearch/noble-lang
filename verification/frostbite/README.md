# Frostbite correspondence lane (#140)

**Status: DECIDED — written correspondence only.** The Frostbite hardware target is recorded as
**unavailable**: no public ISA document, RTL, official simulator or FPGA playground could be pinned by
content identity on 2026-11-19. The work below is the correspondence between Noble's checked I64
interaction-net path (and the vendored `inets` library it is proved against) and the mechanisms the
authors describe — take, peek, copy, frames, a work queue and work stealing. **This document is not a
pass for a hardware run**, and no hardware or simulator measurement was made or is claimed.

| File | Content |
|---|---|
| [README.md](README.md) | sources checked, pinned references, correspondence table, frozen comparison protocol, non-claims, cross-links |
| [decision.md](decision.md) | per-acceptance-item decisions, the NO-GO-until-artifact decision, reopen conditions |

Nothing in this lane changes default Noble acceptance, Wasm execution, the checked I64 net backend,
the labelled DUP/SUP profile, `.cairn/specs/`, or any Cargo unit. It adds no Rust, Lean, Nix or policy
file; the diff of this slice is these two documents. The #37 decision that `noble ngraph eval --net`
stays opt-in and that the net backend is NO-GO for default adoption is unchanged
([`../ngraph/net-decision.md`](../ngraph/net-decision.md)).

## What the authors claim, and how it is read here

The primary write-up is [Introducing Frostbite](https://architectlabs.com/blog/frostbite) (Architect
Labs, 18 Nov 2026), also summarised in the announcement linked from #140. This lane reads it as a
description of a machine: a program is a graph of nodes that rewrite locally, every value has exactly
one owner, an active pair is claimed and rewritten by one agent, and parallelism is spatial. Everything
the post says about take, peek, copy, frames, the work queue and stealing is treated as an **authors'
claim about an undisclosed machine**, not as a specification: the post defines no instruction encoding,
no memory-visibility rule, no queue arbitration, no frame layout and no stealing protocol. The
performance figures are treated the same way (see [Non-claims](#non-claims)).

The post itself states that the RTL and ISA are not released and lists "Release the RTL and ISA for
Frostbite" and an FPGA-hosted software playground as future work. That is the primary evidence for the
hardware status below; it is consistent with every check in the next section.

## Sources checked for a pinnable hardware artifact

Checks were made on 2026-11-18 and 2026-11-19. "Pinnable" means a document, source tree or bitstream
whose exact bytes can be named by a commit hash or a SHA-256 plus a retrieval record.

| Source | Check | Outcome |
|---|---|---|
| `architectlabs.com/blog/frostbite` | fetched and read in full, 2026-11-19 | the write-up; states RTL/ISA unreleased; names releasing both and an FPGA playground as future work; contains no instruction, frame, queue or visibility contract |
| `x.com/architectlabs/status/2123154026435121255` | fetched, 2026-11-19 | HTTP 404 (the post is not retrievable without the account); third-party mirrors quote the announcement, and a mirror is not a pinnable artifact |
| `architectlabs.com/blog` | fetched, 2026-11-19 | four posts only (Frostbite, Redwood, seed, RTL generation); no ISA, simulator, RTL or playground link |
| `architectlabs.com/about` | fetched, 2026-11-19 | company description; no artifact or repository link |
| `architectlabs.com/docs`, `/playground`, `/research`, `/frostbite`, `/papers`, `/blog/frostbite/paper` | fetched, 2026-11-19 | HTTP 404 for all six |
| `playground.architectlabs.com`, `frostbite.architectlabs.com` | connection attempt, 2026-11-19 | no connection (no HTTP response) |
| `github.com/architectlabs` | fetched, 2026-11-19 | the account exists and has **no public repositories** ("doesn't have any public repositories yet"); the org API endpoint returns 404 |
| GitHub repository search, "frostbite interaction net cpu" | API query, 2026-11-19 | 0 results |
| arXiv API, "frostbite" AND "interaction nets" | query, 2026-11-19 | no entries |
| `vine.dev`, `vine.dev/docs/ivy-ivm/` | fetched, 2026-11-19 | public and content-identifiable (Vine HEAD `cf0c325834d5bf4c8d43aa36f7fb76faf2496988`, unchanged from the revision #37 pinned for its IVM comparison). Documents Ivy's interaction system (annihilate, commute, copy, erase, expand, call, branch) and IVM extrinsics. This is a **software** net runtime, not Frostbite's ISA: it defines no take/peek instruction, frame, work queue or steal |

Conclusion recorded in [decision.md](decision.md): the hardware target is **unavailable**, and the
conditional acceptance item that would lower a frozen fixture onto the target is decided as
NO-GO-until-artifact with the protocol and fixture set frozen in advance below.

## Pinned references

### inets — `98bbb01070664e4462a35efa5d62de6f27143572`

The correspondence uses the vendored revision:

- commit `98bbb01070664e4462a35efa5d62de6f27143572`, file `InteractionNets.lean`, blob
  `e7cb3b776389813355a519fc7244730ab927a993`, 1,926,441 bytes, SHA-256
  `c7a35dad49822583a9ee209b47d8ba5617d5abcb5a7f9f96df05e8502f362f54`, recorded in
  [`../../proofs/inets/UPSTREAM.json`](../../proofs/inets/UPSTREAM.json) and enforced by
  [`verification/foundation/check.mjs`](../foundation/check.mjs); see also
  [`../../proofs/inets/README.md`](../../proofs/inets/README.md).

Why this revision and not another:

- It is the only revision whose bytes are **in this repository**: the foundation check refuses the
  vendored file unless its SHA-256 equals the recorded digest, and every foundation receipt binds that
  digest. Citing a theorem whose bytes are not the audited ones would make the citation unverifiable.
- The revision named in the issue, `01bd554a43504b58dc64c607f46dfe4acc7735cd`, is **three commits
  later** and carries a different file (blob `e6deaaf7c722f4827d3a01fc759298da9d909acc`, 2,020,345
  bytes). Nothing in it can be cited against the vendored bytes without first re-pinning.
- Current upstream `eaddcc1448841ae8803b774e696aabe922408cd7` (2026-11-16, the revision
  [inets #12](https://github.com/OnixResearch/inets/issues/12) is written against) is **141 commits
  ahead** of the pin (110 files changed) and has restructured the library into `InteractionNets/*.lean`
  with a 253,663-byte `InteractionNets.lean` shim. Re-pinning is a vendoring change (the provenance
  record is single-file) plus a new foundation receipt; it is an owner/integrator action, not part of
  a documentation slice.

All revision facts above were taken on 2026-11-19 from `git ls-remote https://github.com/OnixResearch/inets
HEAD` and the repository's commit/compare/contents endpoints.

### Constructs verified present in the pinned bytes

Names below were located in the vendored file on 2026-11-19, so a reader can cite them against the
pinned digest:

| Claim | Declarations in the pin |
|---|---|
| generic interaction system and active pairs | `INet.System`, `INet.Redex`, `INet.Step`, `INet.Steps` |
| strong confluence, unique normal forms, equal step counts, bound | `INet.Step.diamond`, `INet.confluence`, `INet.normal_form_unique`, `INet.reduction_bounded` |
| an agent belongs to at most one active pair | `partner_unique` |
| simultaneous firing of agent-disjoint pairs | `INet.Indep` (a list of redexes, pairwise agent-disjoint), `INet.selectIndep`, `XNet.fireList` |
| concurrent claim and write model | `INet.CState`, `INet.Conc` (a claim costs 1, a write 0), `INet.Run` |
| shared-memory thread machine | `INet.Mach`, thread states `TState.look`/`seek`/`ready`/`half`/`write`, actions `MStep.look`/`first`/`next`/`found`/`quit`/`cas₁`/`undo`/`cas₂`/`agent`/`link`/`done`, `INet.Net.take` |
| the machine computes the net's normal form with one claim per step | `MRun.final`, `MRun.sequential`, `MRun.quiet` |
| any schedule and any number of threads agree | `MRun.confluent` |
| turn-taking and cost bounds | `INet.XMach.solo_bound`, `INet.XMach.roundRobin_bound`, `INet.IMach.cost_le`, `INet.IMach.time_bound`, `INet.fair_window_le` |

### Constructs *not* in the pinned bytes

These are later upstream work and must not be attributed to the pin: the worklist program `WMach`, the
measuring/picker machines `JMach` and `SchedRun`, the window-rest facts `window_rest`, and the numeral
copy rules `Copies.Rules` / `copy_fresh` of `Core/Copy.lean` — all named by inets #12 at `eaddcc14`. A
correspondence that needs them needs a re-pin first. (The pin's own `Copies` lemmas are about net
renamings up to history, not about copy rules for numerals.)

### Noble side

| Reference | What it fixes |
|---|---|
| [`../../docs/i64-net-contract-v3.md`](../../docs/i64-net-contract-v3.md) | `ngraph-net-i64/3`: admitted graphs, the rule table, translation, read-back, interaction count `K = 2·#ops + #dups + #eras`, budgets, trust |
| [`../../docs/labelled-dup-sup-v1.md`](../../docs/labelled-dup-sup-v1.md) | `labelled-dup-sup/1`: `dupL L`, `sup L`, annihilate/commute and OP-SUP rows, declared labels, caps |
| [`../../crates/noble-ngraph/src/net.rs`](../../crates/noble-ngraph/src/net.rs) | `CONTRACT_REVISION`, `LOWERING_REVISION`, `PLAN_REVISION` (`ngraph-net-plan/1`), `PLAN_RULES_REVISION` (`noble-net-i64-rules/3`), `lower`, `reduce`, `Report`, `ReduceRequest` |
| [`../../crates/noble-ngraph/src/net/rewrite.rs`](../../crates/noble-ngraph/src/net/rewrite.rs) | the emitted byte plan: magic `NPLN`, version 1, ≤256 bytes, ≤2 boundaries/results/writes, decoded and re-checked against the live arena |
| [`../../crates/noble-ngraph/src/net/plan/corpus.rs`](../../crates/noble-ngraph/src/net/plan/corpus.rs) | the Rust-emitted plan corpus: `op2`, `op1`, `dup`, `era` in both orientations and both storage policies, plus extreme-payload extras |
| [`../../proofs/foundation/NobleFoundation/NetI64.lean`](../../proofs/foundation/NobleFoundation/NetI64.lean), `NetLabelled.lean` | the Lean models; `i64_normal_form_unique`, `i64_equal_interaction_count`, `parallel_sub_schedules_agree` are #124's selected revision-3 row |
| [`../ngraph/net-decision.md`](../ngraph/net-decision.md) | #37: `--net` opt-in, NO-GO for default adoption, frozen 1.15 threshold |
| [`../net-plans/criteria.md`](../net-plans/criteria.md), [`../net-plans/decision.md`](../net-plans/decision.md) | #110 reuse/allocate plan comparison, its five families and its descriptive-only rule |
| [`../labelled-sup/README.md`](../labelled-sup/README.md), [`../labelled-sup/decision.md`](../labelled-sup/decision.md) | #105: model-only GO, runtime HOLD |
| [`../schedule-independence/README.md`](../schedule-independence/README.md) | where #124's row sits relative to the #134 analysis harness |

`.cairn/specs/` carries no interaction-net requirement text; a case-insensitive search for "interaction
net" over that directory returned no match on 2026-11-19. The net contracts are the two `docs/` files
above and the lane decision records.

## Correspondence

Each row asks one question: given the mechanism the blog describes, **what would Noble's pinned
checker and reducer have to supply for the mapping to be legal, and what does the mapping break if it
is not supplied?** The last column is the verdict this lane records. "Rejected" means the lowering
contradicts Noble's linearity/single-ownership rule; "unsupported" means nothing the authors describe
or publish today defines the construct, so a run must report it as unsupported rather than pass.

| Noble / inets construct | Mechanism the blog describes (authors' claim) | What a legal mapping would require | What an illegal mapping breaks | Verdict |
|---|---|---|---|---|
| **Active pair and rule firing** — `INet.Redex`, `INet.Step`, Noble's rule table (both orientations) | "computation happens by rewriting pairs of nodes that are connected through their ports"; each rewrite touches a small local part; no two active pairs overlap | an atomic claim of exactly two agents by their principal ports, followed by writes of exactly the rule's right-hand-side agents and segments; the claim is the only event that removes agents | splitting a claim into independent takes that other tiles can observe as progress: a half-claimed pair is not an interaction and, if published as one, changes the interaction count against #124 | legal as the pinned claim; requires the artifact to state claim atomicity |
| **Linearity and single ownership** — one producer endpoint per consumer port, `partner_unique`, `Net.WF` and `Paired` well-formedness | "every value has exactly one owner"; no shared mutable state; dependencies are explicit | one writer per agent cell; a value observed by a second consumer only through an explicit duplication agent; a taken agent remains visible as written (`Mach.now` counts held agents) | exposing a value to two readers without a `dup` agent: two consumers of one wire, i.e. a net that is not well formed, or a second observation source that the model does not have | legal only under the pinned held/undo discipline |
| **Fanout / DUP** — `dup ⋈ num v` makes two ordered `num v`; `n ≥ 2` uses become a left-leaning chain of `n-1` `dup`s | no fanout primitive is described; only "copy" | the net's own duplication agent performs the duplication, with the canonical consumer order; duplication is a rewrite, not a memory operation | a hardware copy of a live value where the net has no `dup`: it changes the net and the observation, and removing the `dup` changes `K` | legal if and only if the net's `dup` agent does it; otherwise **rejected** |
| **Erase / ERA** — `era ⋈ num v` drops a value; an unused value gets one `era`; `#eras` is part of `K` | no drop/erase operation is named among take/peek/copy | dropping a value with exactly one owner through the ERA rule only | a value "forgotten" without an interaction: the interaction count then disagrees with `K` and with `i64_equal_interaction_count` | unsupported unless the artifact performs ERA as a rule |
| **Labelled DUP/SUP** — `labelled-dup-sup/1`: `dupL L ⋈ sup L` projection, `dupL L ⋈ sup M` (`L ≠ M`) commute and nest into four agents, OP-SUP distribution, declared duplicate-free labels | nothing: no label, alternative or superposition primitive appears in the post | label-carrying copy and annihilation, plus a nesting bound and a labelled read-back | a label-less copy conflates the equal-label and different-label cases, silently choosing one — exactly the silent pass the acceptance item forbids | **unsupported** by anything published today |
| **I64 arithmetic nodes** — `op2 k ⋈ num a` → `op1 k a`; `op1 k a ⋈ num b` → `num (k a b)`; wrapping modulo 2^64, `sub` is first minus second, `eq` is 1/0; no external callback, no N32 truncation | "data, computation nodes, and control nodes"; the ISA has wide bundles, loop counters and streams | an ISA-level contract for 64-bit wrapping arithmetic and equality with exact bit patterns and operand order, plus the ordered multi-output read-back | an external function call or a narrowed payload silently changes results — the mapping #37 already rejected for Ivy's `ExtFn`/merge path | **unsupported** (no public arithmetic contract); a future lowering must be bit-exact and separately checked |
| **Plan / worklist representation** — `ngraph-net-plan/1` byte plans (`NPLN` v1, ≤256 B) decoded and re-checked against the live arena; slot reuse with generations; LIFO, FIFO and seeded worklists; fuel = `K` | frame manager (parked continuation records and counters), hardware work queue, newest-first pickup, neighbours pull remaining tasks | a queue whose items are claimed redexes, and counters over outstanding writes of a claim | treating a frame as a net agent adds agents no rule's right-hand side produced (`Writes` permits only RHS children); treating a queue item as a plan assumes the hardware consumes emitted plan bytes, which it does not | category-level correspondence only; **not a lowering** |
| **`take`** — `MStep.cas₁` takes the first agent, `cas₂` takes the second and claims, `undo` puts the first back | "operations to take … values from other parts of the net that would be running in parallel" | a take that is part of a claim, with undo, and a claim cost of exactly one interaction | a take from outside the principal-port wire of the agent being claimed has no machine action; a take that hides the agent from readers breaks `Mach.now` and hence `MRun.confluent` | **legal subset**: claim-local take only; cross-net take unsupported |
| **`peek`** — `MStep.look`/`first`/`next`/`found` read cells along the principal wire at cost 0 and are invisible to `Conc` | "peek … values from other parts of the net" | reads that only *find* a redex; the claim then re-reads both agent cells and applies the rule table to the current symbols | a peek whose observed value decides a rewrite without re-reading the cell: the pinned claim re-reads `agent a α` and `agent b β`, so a stale peek cannot change the fired rule. Cross-net speculative peeks have no action at all and no visibility model | **legal subset** (search); speculative cross-net peek **unsupported** |
| **`copy`** — the machine has no copy action: every written agent is a right-hand-side child of the claimed pair, every segment comes from the right-hand side | "copy values from other parts of the net" | duplication expressed by the net's duplication agent (`dup`, or `dupL`/`sup` in the labelled profile) | duplicating a live cell yields two agents on one wire (`partner_unique` fails) or an agent no rule produced; a copy that aliases a value is therefore not an optimization but a different net | **rejected** as described, unless expressed as the net's duplication rule |
| **Frames, work queue, work stealing** — threads have private state `idle`/`look`/`seek`/`ready`/`half`/`write`; work is either searching for an agent or finishing a claimed right-hand side | parked records with counters, a work queue, newest-first pickup, idle neighbours pulling tasks, return packets waking the next section | a transferable claim token, or a queue of unclaimed redexes and a starvation argument; in the pin no `half`/`write` state moves between threads and there is no queue (the worklist program is not in the pin) | if a steal moves a half-claimed pair, two threads can write the same right-hand-side agents; if a steal only moves searching, it is a schedule choice and adds no parallelism beyond `Indep` — and a steal must never become a rewrite rule | **unsupported at the pin**; expressible only as a new turn picker after a re-pin, under inets #12's own criterion |

Reading of the table as a whole: the pinned model already supplies the parts Frostbite's description
maps onto most directly — an atomic claim, a read-only search phase, an undo, and confluence over any
schedule and thread count. It supplies **no** action for cross-net peeking or copying a live value, and
the duplication and erasure Noble relies on are *rules of Noble's table*, so hardware that offers copy
and drop as memory operations would have to be shown to implement those rules exactly — or the lowering
is unsupported. That is the substance of the acceptance item that keeps linearity in force.

## Frozen comparison protocol, for when an artifact can be pinned

Frozen before any measurement, as required. Nothing here has been run: the artifact does not exist.

1. **Artifact admission.** Pin by content identity, recorded in the receipt before any run: either a
   public repository at a tag or commit whose tree holds the ISA document, simulator source or FPGA
   source (commit hash + path + SHA-256 of the exact file bytes), or an immutable archived document or
   release asset (URL + SHA-256 + retrieval date), together with the simulator/bitstream version string
   and its build recipe. A mutable blog page, a login-gated post, a mirror or an aggregator is not an
   artifact. If the artifact defines no claim atomicity, no memory-visibility rule and no
   arithmetic/bit-width contract, the corresponding rows above stay unsupported and the run cannot
   pass them.
2. **Inputs.** A frozen subset of already admitted Noble I64 nets, named exactly and bound into the
   fixture identity tuple: the five families of [`../net-plans/criteria.md`](../net-plans/criteria.md)
   (chain, fanout, erasure, asymmetric-subtraction, wrapping-subtraction) with their graphs and input
   rows; the four rule images `op2`, `op1`, `dup`, `era` in both orientations and both storage policies
   from `crates/noble-ngraph/src/net/plan/corpus.rs`, including its extreme-payload extras; and, only
   if the artifact claims a labelled or alternative primitive, the labelled fixtures of
   `NetLabelled.lean` (equal-label projection, different-label nesting, the OP1-SUP and OP2-SUP
   collapses). Each net is lowered once by the unchanged lowering
   (`ngraph-net-i64-lowering/0`); net bytes, input vector, contract and lowering revision are recorded.
3. **Oracle.** The existing software reducer on the *same* net and inputs: `net::reduce` through
   `noble ngraph eval GRAPH VECTORS --net`, or the independent Lean model `NobleFoundation.NetI64`, at
   the same fuel. Agreement means (a) every ordered output, (b) the total interaction count, (c) the
   peak-live-agent bound (no rule increases it). Timing is not an admission criterion.
4. **Unsupported is a result.** A construct the artifact cannot express — labelled DUP/SUP, ERA of a
   SUP, a boundary-alias case, an I64 payload the ISA cannot carry, ordered multi-output read-back — is
   reported as `unsupported` with the construct and the fixture. It is never counted as agreement, and
   a run that silently drops a construct fails the protocol.
5. **Disagreement with #124 is a finding with a replay bundle.** If the artifact's result disagrees
   with `i64_normal_form_unique`, `i64_equal_interaction_count`, `parallel_sub_schedules_agree`, or
   inets' `MRun.confluent` (same memory and same claim count across schedules and thread counts), the
   bundle must contain: the fixture identity tuple of step 2, the lowering and rule revisions, the
   schedule/order the artifact ran, the artifact's revision and exact command, the observed and the
   expected observation and count, and the software arm's own plan and replay digests from
   `Report.plan_stats`. The disagreement must replay on the software arm alone before any claim is made
   about the artifact; a bundle that only exists on the hardware side is not a finding.
6. **Receipts and scope.** The receipt is produced from a clean committed tree, with the artifact
   bytes archived outside the repository and their digest recorded. Counters and outcomes are
   descriptive: no threshold, no adoption decision and no change to `--net`'s opt-in status follow from
   a comparison run. Adoption of a target is a separate issue with its own frozen criteria.

## Hardware status

**Unavailable**, on the checks tabulated above, all dated 2026-11-18/19. Consequence: acceptance item 3
of #140 ("if a public ISA, official simulator, or FPGA playground can be pinned … lower a frozen subset
… and check results against the existing software reducer") is decided as **NO-GO-until-artifact**; its
fixture set and its disagreement rule are frozen above so that the first admissible artifact cannot
move the goalposts. See [decision.md](decision.md) for the reopen conditions.

## Non-claims

- The authors' 4.59× (all 25 workloads), 8.77× (performance-per-area), 6.02× (search set) and 3.83×
  (held-out set) figures are **not** imported as pass or fail criteria, and no part of this lane
  reproduces, adjusts or disputes them. The TCPU baseline is unnamed in the post ("for legal reasons,
  we cannot disclose the exact CPU"), so it cannot serve as a named comparison machine here.
- Performance-per-watt, performance-per-area, GDS frequency, die area and the 1–5 % ASL accuracy claim
  are out of scope for this issue.
- The blog is quoted as an **authors' claim**. This lane verified only that the page was reachable and
  what it says; it did not verify any measurement, RTL, GDS or Lean contract.
- Nothing here is a proof of anything about Frostbite, and nothing here is a model theorem, an
  implementation test or a correspondence proof about Noble. The three stay separate: Noble's model
  claims are Lean theorems over the pinned inets bytes; Noble's implementation claims are the Rust
  reducer's finite tests and receipts; this lane is a **written correspondence** plus a frozen
  protocol, and it establishes neither.
- The correspondence does not claim that Frostbite is expressible as Noble's I64 system, nor that any
  Noble net is expressible on Frostbite. Unsupported rows are recorded as unsupported.

## Cross-links, and why they do not replace this issue

- [compiler-lab #266](https://github.com/brittonr/compiler-lab/issues/266) — "Evaluate Frostbite's
  Vine-to-ISA compiler and perf-per-watt claims when artifacts are public". It owns Vine-to-ISA
  lowering, the compiler-side artifact discipline, workload identities and any later baseline
  measurement. It explicitly leaves "re-doing Noble net-semantics correspondence" to #140.
- [inets #12](https://github.com/OnixResearch/inets/issues/12) — "Compare Frostbite take, peek, copy,
  and hardware work-stealing with the inets reducer and schedules". It owns the reducer-level question
  (claims, `Indep`/`selectIndep`, `Copies`, `XMach`/`WMach`/`JMach` schedules, starvation) at upstream
  `eaddcc14`. It leaves the language-level net correspondence — Noble's rule table, lowering, plan and
  labelled profile — to #140.
- Neither replaces this issue: #266 would not say which Noble construct a hardware copy breaks, and
  #12 would not say which of Noble's rule-table rows, plan identities or labelled rows a lowering
  must preserve. Conversely this lane does not evaluate Vine-to-ISA lowering and does not extend the
  upstream reducer, so it cannot close either of them.

## Reproducing this lane's checks

The lane adds no executable artifact. Its own gate run is the repository's documents and Cairn checks,
from a clean committed tree:

```sh
bun tools/cairn-specs.mjs --self-test
NOBLE_GIT="$(command -v git)" bun test tools/cairn-specs.test.mjs tools/check-specs.test.mjs \
  tools/calculator-headroom.test.mjs tools/source-inventory-renew.test.mjs
bun tools/check-specs.mjs --self-test
cairn validate --root "$PWD" --policy <cairn>/cairn-policy/generated/cairn-policy.json
```

The reference facts above can be re-derived without running anything:

```sh
git -C <clone of OnixResearch/inets> show 98bbb01070664e4462a35efa5d62de6f27143572:InteractionNets.lean | sha256sum
git ls-remote https://github.com/OnixResearch/inets HEAD
grep -n "MRun.confluent\|MRun.final\|INet.selectIndep\|def Net.take" proofs/inets/InteractionNets.lean
```

The first command must print `c7a35dad49822583a9ee209b47d8ba5617d5abcb5a7f9f96df05e8502f362f54`. Note
that `grep` output is against the repository's own vendored copy, which is what the foundation check
hashes; a later upstream revision will not match it.

## What would change this document

A pinnable artifact (step 1 above) reopens the hardware item; a re-pin of `proofs/inets` reopens the
construct tables; and a change to Noble's rule table, lowering or labelled profile invalidates the
affected rows. Until then this document is the frozen record for #140.
