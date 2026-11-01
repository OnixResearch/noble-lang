# Three authoring forms for focused, move-threaded updates (issue #79)

This lane answers issue #79: compare the **same bounded examples** in three authoring forms —
explicit stack code, a derived wiring-library form, and proposed focus sugar — with numbers, and
say which examples run in today's selected source profile and which are model-only.

## Forms

| Form | What it is | Status here |
|---|---|---|
| explicit | the word sequence written out by hand at the use site (`pair`, `unpair`, `swap`, `+`) | compiled, timed and executed |
| library | the same checked operations factored into quotations and composed (`compose`, `run`), which is what a derived wiring library would supply instead of the idiom at each use | compiled, timed and executed |
| sugar | a proposed finite focus surface (`focus` over a product path, exactly one active focus) that elaborates into the library form | **model-only**: no such syntax exists in the selected profile, so its numbers are authored-source sizes of the expansion, never a compile time or a step count |

Timing a form that cannot be compiled would be inventing data, so sugar has no compile or execution
row. `forms/proposed-sugar.md` states what the two snippets would be and which compiled form each
expands to.

## Examples and results (2026-10-31, `receipts/2026-10-31/measure.json`)

Both compiled forms of each example produce the **same value**, so the comparison is on identical
checked work rather than on plausibility:

| Example | Form | authored tokens | of which stack words | quotations | compile median (min/max) | module bytes / lines | execution steps | allocated bytes | value |
|---|---|---|---|---|---|---|---|---|
| nested-product | explicit | 9 | 6 | 0 | 1.567 ms (1.419–2.234) | 61 094 / 1057 | 25 | 1152 | `Pair(2, 5)` |
| nested-product | library | 18 | 9 | 3 | 1.829 ms (1.728–2.134) | 68 127 / 1167 | 46 | 1920 | `Pair(2, 5)` |
| nested-product | sugar | 6 | 2 | 1 | — not compilable — | — | — | — | model-only |
| type-changing-step | explicit | 3 | 1 | 0 | 1.499 ms (1.291–1.768) | 56 472 / 970 | 12 | 288 | `Bool false` |
| type-changing-step | library | 9 | 3 | 2 | 1.510 ms (1.408–1.698) | 61 043 / 1045 | 26 | 768 | `Bool false` |
| type-changing-step | sugar | 4 | 1 | 1 | — not compilable — | — | — | — | model-only |

Seven runs after a warm-up per form, `noble compile --opt off`; the executed shape is
`noble live repl` on the same source. The receipt records the binary's SHA-256
(`980ed0e1…`), so these numbers belong to one checked CLI build rather than to a moving target.

**What the numbers say.** At this scale the composed library form is *more* expensive on every
axis measured — more authored tokens, more execution steps (46 vs 25, 26 vs 12), a larger
generated module and more allocation — because composition pays one captured-quotation dispatch
per boundary. A wiring library earns its keep only when the same factored operation is reused
several times; two examples of one use each cannot show that, and this lane does not pretend
they do. The sugar's authored size is closest to the explicit form while keeping the checked
composition, which is the only direction in which sugar is cheaper than the library form it
expands to.

## The third example is model-only, with the reason named

| Example | Shape | Status |
|---|---|---|
| move-threaded-capability | sequential non-destructive operations on a must-use capability | **model-only**: the selected source profile has no must-use resource word, so no source form of this example exists to compare. The ownership fragment is the Lean model (`proofs/foundation/NobleFoundation/Ownership.lean`), and a focus elaboration must not borrow the immutable-local lowering for it. |

## Reproducing

```sh
node verification/focus-elaboration/measure.mjs \
  --noble <noble-cli> --output verification/focus-elaboration/receipts/<date>/measure.json
```

## Non-claims

- Compile wall time is a frontend measurement on one host, not a proof-cost claim; the checker's
  cost is roughly the same per program, and the module sizes differ because the authored programs
  differ.
- No form is claimed to produce identical recipe or hash identities. Different elaboration may
  produce different Programs; only the checked *value* is compared above.
- The sugar is neither implemented nor proposed as selected syntax here. Whether to select it is
  the owner's decision, and the issue comment records these numbers as the input to that decision.
- Nothing here proves the move-threaded example in Lean beyond the ownership fragment that already
  exists, and no correspondence between the model and the selected profile is implied.
