# Decision: no new focus syntax now; library-form composition is the selected answer (issue #79)

Recorded 2026-10-31 by noble-maintainers from the measurements in `receipts/2026-10-31/measure.json`.
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
| authoring complexity | nested-product: 9 explicit tokens vs 18 library tokens vs 6 sugar tokens; type-changing-step: 3 vs 9 vs 4. The library form pays more tokens than the explicit form for a single use, and sugar pays the fewest. | sugar wins on this axis alone, and only against the library form |
| generated code size | library forms emit 68 127 B / 61 043 B against 61 094 B / 56 472 B explicit (+11.7 % / +8.1 %) | composition is not free at these sizes |
| checking cost | compile medians 1.829 / 1.510 ms library vs 1.567 / 1.499 ms explicit | no measurable frontend saving to fund an elaborator |
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
