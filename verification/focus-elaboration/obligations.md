# Separate obligations (issue #79)

The issue requires five formally separate obligations rather than one word "verified". Each row
states what it would mean and what is asserted **today**, which is nothing: this lane contains
measurements and a decision, not a proof.

| Obligation | What it would mean | Asserted today |
|---|---|---|
| surface typing preservation | the checked ordered context after elaboration is exactly the shape the surface declared, with no implicit `drop`, `dup` or capture | **none.** The two compiled library forms check because the existing checker admits them; that is not a theorem about an elaborator |
| observational simulation | a program written with the surface and its explicit expansion agree on values, stack order, resource obligations and effect traces | **none.** The compiled forms agree on the value, the ordered typed stack, the reported stack height and the (empty) effect trace in the measurement, for three examples, on one build; agreement between two hand-authored forms is not a theorem about an elaborator |
| resource accounting | every must-use owner is returned or transitioned exactly once, on normal and ordinary-error paths; no fabricated "closed handle" token | **none** here, and deliberately not restated: #59's oracle is the single authority for this axis |
| source-to-core implementation correspondence | the elaboration the source says happened is what the checker accepted, at the exact source spans | **none.** No elaborator exists |
| backend/host correspondence | the emitted program's behaviour on the backend equals the checked semantics, including trap, quota and cleanup ownership remaining host-side | **none here.** This is the subject of #82 and the refinement lanes, not of this lane |

## Consequences that follow from keeping them separate

- A measurement is not evidence for any of these rows: `receipts/2026-10-31/measure.json` and
  `receipts/2026-11-13/measure.json` record what one checked CLI build did for each form, and the
  lane README says so. The newer receipt also records which axis the CLI does **not** expose: the
  resource-obligation axis has no must-use word in this profile to observe, so its silence there is
  recorded as silence, not as a pass.
- Reusing #59's fragment for resource accounting is a requirement, not a convenience. A second
  ownership oracle would be a second authority on whether an owner was preserved.
- Because the elaborator does not exist, rows 1, 4 and 5 cannot be advanced by authoring more
  documents. Honest status is "none asserted" until an implementation exists.
