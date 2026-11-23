# Historical gate

This tree is a historical receipt, not a live check.

`gate.mjs` asserts that all nine LSLOT cases are absent with no evidence, which was true at the
source state its receipt records. All nine LSLOT cases are now promoted by
`verification/lslot-cases/gate.mjs` (LSLOT-01, 02, 03, 06 and 09 first, then LSLOT-04, 05, 07 and
08 from the Cairn push-B receipt), so re-running this tree against the current source fails by
construction. Its receipt keeps the source revision and binary digest it ran with; it is not
regenerated, edited or re-run when the change is archived.

OWNER DECISION 2026-11-20 (decision 20, verbatim): "Decision 4 extends to frozen verification
scripts that assert earlier case states: `verification/live-slot-current-source/gate.mjs` and
`verification/numeric-graph-v1/postpromotion.mjs` are historical-only (replay only at their
recorded source state)."
