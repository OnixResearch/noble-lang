# Historical inputs

`corpus.mjs`, `export.mjs` and the frozen `profiles/noble.json` read
`.cairn/changes/live-reference-lifetimes/design.md` as a live input. That change was archived on
2026-11-23 as `.cairn/archive/2026-11-23-live-reference-lifetimes/`, so re-running this tree against
the current source fails by construction. Its corpora, exports and receipts keep the paths and
digests they ran with; they are not regenerated or edited.

Owner decision 4 (2026-11-20) makes a verification tree that lists change documents as live inputs
historical-only once the change is archived; decision 20 applies the same rule to frozen scripts
that assert earlier case states.
