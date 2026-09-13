# Flat specifications to Cairn

## Scope

The migration converts the twelve normative documents in `specs/spec-family.json` to `.cairn/specs/<id>/spec.md`.
All 475 requirement IDs remain unchanged.
All 149 scenario records and all 30 proof obligations retain their original status.
The 189 requirements without linked scenarios retain an explicit open test-design notice.

The first mechanical conversion passed an exact text round trip for each source document before any file write.
This check included explanatory paragraphs, examples, requirement labels, section headings, and rebased local links.
The original handoff directory, ZIP, and review evidence remain unchanged.

The native Cairn files now own the normative prose.
The old normative document paths contain generated compatibility views.
The requirement ledger points to the native files through paths relative to `specs/`.
Supporting decisions, roadmap, status, scenario JSON, and proof records remain in `specs/`.

## Normative wording

Cairn identified eighteen existing requirement blocks without an explicit normative word.
The conversion makes their existing rules explicit with `MUST` or `MUST NOT`.
It does not select new syntax, dependencies, proof results, or backend representations.

| Document | Requirement IDs |
|---|---|
| Language | K-SYN-04, K-EVAL-01, K-EVAL-03, P-ID-08, W-WASI-03 |
| Safety | S-LANG-03, S-TYPE-01, S-CONC-06, S-VERIFY-01, S-GATE-02 |
| WIT/WASI | WI-ARCH-04, WI-WIT-03, WI-SYN-02, WI-SAFE-03 |
| Resource adapters | RA-SCOPE-01 |
| Verification | V-SAFE-01, V-SAFE-03 |
| Toolchain | VT-ROLE-03 |

Requirement strength elsewhere remains unchanged, including recommendations and permitted choices.
The revision remains `0.1.0-draft.5` because this is a format conversion with explicit wording for existing obligations.

## Scenario mapping

Each requirement with linked cases has Cairn `GIVEN`, `WHEN`, and `THEN` clauses.
The clauses identify the case, profile, procedure kind, input fields, and expected fields in the unchanged scenario ledger.
A case can appear under multiple requirements. Repeated links are not independent test executions.
Requirements without cases receive no invented behavioral scenario.

## Validation boundaries

The conversion adapter detects stale compatibility views, mismatched IDs, duplicate requirements, missing scenario targets, and malformed generated regions.
The existing document validator reads native files and retains its negative controls for false execution and proof claims.
The Cairn runner binds its binary and policy to one immutable upstream revision.

No active implementation change or archived completion is created by this conversion.
No Git commit or push occurs because the Noble directory is not a Git repository.
