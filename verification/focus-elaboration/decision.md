# Decision: no new focus syntax now; library-form composition is the selected answer (issue #79)

Recorded 2026-10-31 by noble-maintainers from the measurements in `receipts/2026-10-31/measure.json`;
amended 2026-11-13 with mechanically recomputed authored sizes and the missing nested pair (see the
addendum at the end). The decision itself is unchanged.
Choosing library-only or no new syntax is an explicit valid outcome of the issue; the owner can
override this decision, and the numbers below are the input to that choice.

## Decision

**No new focus syntax is selected.** Focused updates are authored with the existing checked stack
operations, factored as composed quotations where the operation is reused (`compose`, `run`).
The proposal is not rejected forever; it is rejected *on the evidence available*: no example class
in this repo yet shows reuse at a scale where the sugar's authored-size gain survives its
unproven semantics.

## Why, in the issue's own terms

| Criterion | What was measured | Consequence |
|---|---|---|
| authoring complexity | nested-product: 9 explicit tokens vs 18 library tokens vs 7 sugar tokens; type-changing-step: 3 vs 9 vs 4; type-changing-nested: 9 vs 18 vs 7 (recomputed mechanically on 2026-11-13 — the 2026-10-31 row recorded 6 for the nested snippet, which its own spacing does not support). The library form pays more tokens than the explicit form for a single use, and sugar pays the fewest. | sugar wins on this axis, but only against the library form: against the issue's preferred statically inlined form it saves 2 tokens on the product examples and costs 1 on the one-step example (addendum) |
| generated code size | library forms emit 68 127 B / 61 043 B against 61 094 B / 56 472 B explicit (+11.5 % / +8.1 %) | composition is not free at these sizes |
| checking cost | compile medians 1.829 / 1.510 ms library vs 1.567 / 1.499 ms explicit, in the 2026-10-31 receipt. The 2026-11-13 receipt's own medians do not reproduce an ordering (its min/max spreads overlap between forms), so no checking-cost claim rests on them | no frontend saving was measured that could fund an elaborator |
| execution cost | 46 vs 25 steps and 26 vs 12 steps, 1920 B vs 1152 B and 768 B vs 288 B allocated | each composition boundary pays a captured-quotation dispatch |

So the only axis on which sugar could improve on the library form is authored size, while the
library form's *own* advantage over explicit code is unproven at the scale that exists today. Adding
a surface whose sole demonstrated benefit is fewer authored tokens — while its obligation story is
open, see below — would be a decision on ergonomics intuition, not on evidence.

## What would reopen this decision

1. An example class where the same factored, checked operation is used several times per program,
   so the library form's dispatch cost is amortised and the authored-size win is not the only win.
2. A residual/reconstruction operation expressible in the *present* core: issue item 3 forbids
   encoding a resource-bearing residual as a normal first-class Program capture, so the sugar must
   be static elaboration/inlining plus explicit stack framing. Until a worked elaboration exists,
   the proposal is a sketch rather than a design.
3. The move-threaded half of the motivating example becoming executable in the selected profile.
   Today no must-use resource word exists in source, so `focus` cannot be evaluated where it
   matters most.

## What this decision does NOT claim

- It does not claim the flow-typing paper's results certify Noble; the paper's pure-core semantics
  are not a mechanized proof of this profile, and shared references are outside its language.
- It does not claim composable library code is cheaper than explicit code in general; only that at
  the measured scale it is not.
- It does not claim the sugar is unexpressible. It claims that the expressible version requires
  static elaboration, and that the current examples do not justify building it.

## Addendum 2026-11-13: authored sizes recomputed, and the missing pair added

The 2026-10-31 revision costed the sugar by hand (6 tokens for the nested snippet, 4 for the
one-step snippet) and never compared it against the form the issue's step 3 actually prefers: static
inlining. `measure.mjs` now costs the `forms/proposed-sugar.md` snippets mechanically by the same
whitespace-token rule it applies to the compiled forms, and refuses a run in which an expansion
block in that document no longer equals the form file it restates. Two things follow.

**One of the hand counts does not reproduce.** `7 focus (0 =)` is 4 whitespace tokens, as recorded.
`1 5 pair focus .0 (+ 1)` is 7 whitespace tokens, not 6: the earlier row counted `(+ 1)` as a single
token, which the snippet's own spacing does not support. The grouping parentheses cost nothing extra
either — stripping `(` and `)` before tokenizing leaves both counts unchanged, because in these
snippets the parentheses are glued to the adjacent word.

**Against static inlining the sugar's win is small, and it is not uniform.** The issue's step 3
prefers a statically inlined elaboration, so the honest comparison is sugar against the inlined
form, not sugar against the composed library form:

| Example | sugar | inlined explicit (`swap` framing) | inlined `dip` | composed library |
|---|---|---|---|---|
| nested-product | 7 | 9 | 10 | 18 |
| type-changing-nested | 7 | 9 | 10 | 18 |
| type-changing-step | 4 | 3 | — (no cell under the top to reach) | 9 |

Sugar minus the inlined explicit form: −2, −2, **+1** tokens.

So on the two product-path examples the sugar saves two tokens over the already-checked inlined
form, and on the one-step example it *costs* one token, because there is no framing left to elide
when the focus is the whole stack. The large win stays where the 2026-10-31 decision put it: against
the composed library form (7 vs 18, 7 vs 18, 4 vs 9). But the library form is not the recommended
target — the measurements in `README.md` show it paying a dispatch per boundary (46 steps against
25 for `nested-product`, 68 127 B against 61 094 B) — so a decision that rested on "sugar beats the
library form" would be comparing against the wrong baseline.

The measured `dip` forms sharpen the same point. `dip` is the present profile's own way to reach the
cell under the top one without capturing a residual in a first-class Program, and it is *not* an
authored-size saving either: `1 5 pair unpair [ 1 + ] dip pair` is 10 tokens against 9 for the
swap-based explicit form, and it executes 27 steps against 25. Its value is structural (no swap
pair to get wrong, no residual capture), not lexical.

**The decision is unchanged.** No new focus syntax is selected; focused updates stay authored with
the existing checked stack operations, factored as composed quotations where an operation is reused.
The addendum narrows the rationale rather than the conclusion. Measured against the baseline the
decision relies on — the composed library form — the sugar's authored-size advantage is real and
large (7 vs 18, 7 vs 18, 4 vs 9), but that baseline is itself the more expensive target on every
program-property axis, so beating it does not show the sugar is cheaper than what the issue's step 3
would actually emit. Measured against that statically inlined baseline the advantage is 0–2 tokens
and negative where there is no path to elide. With its obligation story (`obligations.md` rows 1, 4
and 5) still open, 0–2 tokens is not evidence for a new surface. `README.md` carries the 2026-11-13
numbers and states which of them are comparable with the 2026-10-31 receipt (module bytes and step
counts, not wall times).
