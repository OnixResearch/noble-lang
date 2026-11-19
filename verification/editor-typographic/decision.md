# Decision: the typographic renderer stays default-off (issue #141)

Decision date: 2026-11-19. Frozen inputs: `corpus.json`, `criteria.json`,
`criteria.md` in this directory, committed before the retained measurement.

## Decision

The experiment is a default-off view and edit source over the existing five
editor node kinds. Nothing in the shipped editor path changes:

- `noble editor analyze` and `noble editor admit` keep their JSON ingress,
  exit codes, report schema `noble-editor-report/v1`, spans and `node_path`
  addresses;
- the renderer is not compiled into the `noble` binary and adds no `editor`
  action, so there is no second admission route and no default CLI path that
  reaches it;
- adoption is not implied. The measurement answers only whether the frozen
  corpus survives the round trip.

## Rejected alternatives

1. **Non-default cargo feature.** A `typographic` feature gating a module in
   `crates/noble-cli/src/core/editor/` would be compiled by the required
   `clippy --workspace --all-targets --all-features` run. Nothing in the binary
   would call it, so the all-features configuration would need a dead-code
   suppression, which is a lint silenced rather than a property proven. It would
   also need CLI dispatch changes to be reachable at all, which is a second
   ingress and outside this slice's file ownership.
2. **Opt-in CLI action** (for example `noble editor render`). Every added action
   widens the editor surface that the report contract has to describe and
   re-opens the "one admission route" property. The experiment does not need a
   shipped command to answer its question.
3. **Test-only module in the existing integration target** (chosen). It is the
   strongest available default-off guarantee under the repository's
   compilation-unit cap: no new target, no production file edited, and the
   binary provably cannot reach it. The harness additionally checks that the
   binary does not contain the renderer's marker string and that
   `noble editor typographic` is still a transport refusal.

The cost of this choice is that the renderer is not a library consumers can
call. That is deliberate: the issue asks whether the round trip survives, not
for a shipped API, and a consumer-facing API would need its own review of the
ingress rule.

## What the measurement establishes

Over the frozen corpus, through the real CLI: identity re-encoding, node-tree
equality for each documented widget edit against an independently authored JSON
edit, equal `analyze`/`admit` verdicts on both paths, equal `admitted_source`
bytes for hole-free results, bound and rendered hole facts for every hole state,
and refusals for every negative control without any CLI invocation. The receipt
records the counts, the digests and the verdict fields.

## What it does not establish

- No theorem about all editor trees, and no fuzzing claim: the corpus is
  finite and hand-frozen.
- Nothing about a real GUI: widgets are a data model plus a textual rendering,
  with no assets and no interaction timing.
- Nothing about the referenced posts' editors or their authors' claims.
- Not a claim that every refused gesture is illegal for the JSON path. The
  fourteen-level quotation bound is deliberately conservative (see README), so
  some transport-readable trees are refused.
- Not a performance or usability result.

## Reopen conditions

Revisit only if one of these holds, and then with a new frozen corpus and
criteria committed before a new measurement:

1. a consumer inside this repository needs the view as a shipped library or
   command, with a design that keeps exactly one admission route;
2. the editor JSON reader's nesting or byte bounds change, so the renderer's
   fourteen-level bound must be re-derived;
3. the five node kinds or the `noble-editor-report/v1` contract change;
4. a larger corpus, including generated hostile trees, shows a gesture that
   decodes to an accepted node yet changes verdicts on either path.
