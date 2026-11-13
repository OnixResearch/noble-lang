# Authoring forms for focused, move-threaded updates (issue #79)

This lane answers issue #79: compare the **same bounded examples** in the authoring forms today's
selected source profile actually offers — explicit stack code, the `dip` form, and a derived
wiring-library form — against a proposed focus sugar, with numbers, and say which examples run in
the profile and which are model-only.

## Forms

| Form | What it is | Status here |
|---|---|---|
| explicit | the word sequence written out by hand at the use site (`pair`, `unpair`, `swap`, `+`) | compiled, timed and executed |
| `dip` | the same update using the profile's own word for reaching the cell *under* the top one (`x y [q] dip` applies `q` to `x`), so the update needs no swap pair and captures no residual | compiled, timed and executed |
| library | the same checked operations factored into quotations and composed (`compose`, `run`), which is what a derived wiring library would supply instead of the idiom at each use | compiled, timed and executed |
| sugar | a proposed finite focus surface (`focus` over a product path, exactly one active focus) that elaborates into a compiled form | **model-only**: no such syntax exists in the selected profile, so its numbers are authored-source sizes, never a compile time or a step count |

Timing a form that cannot be compiled would be inventing data, so sugar has no compile or execution
row. `forms/proposed-sugar.md` states what each snippet would be, which compiled form it expands
to, and carries the machine-readable markers `measure.mjs` costs it from.

## Examples and results (2026-11-13, `receipts/2026-11-13/measure.json`)

| Example | Form | authored tokens | of which stack words | quotations | compile median (min/max) | module bytes / lines | execution steps | allocated bytes | value | reported height / `stack_length` |
|---|---|---|---|---|---|---|---|---|---|---|
| nested-product | explicit | 9 | 6 | 0 | 7.601 ms (4.823–9.761) | 61 094 B / 1057 | 25 | 1152 B | `Pair(2, 5)` | 1/1 |
| nested-product | `dip` | 10 | 5 | 1 | 6.972 ms (5.333–11.388) | 61 653 B / 1063 | 27 | 1200 B | `Pair(2, 5)` | 1/1 |
| nested-product | `dip`-library | 19 | 8 | 4 | 6.886 ms (4.031–9.428) | 68 686 B / 1173 | 48 | 1968 B | `Pair(2, 5)` | 1/1 |
| nested-product | library | 18 | 9 | 3 | 8.159 ms (5.765–12.211) | 68 127 B / 1167 | 46 | 1920 B | `Pair(2, 5)` | 1/1 |
| type-changing-nested | explicit | 9 | 6 | 0 | 4.087 ms (3.299–6.957) | 62 328 B / 1077 | 28 | 1152 B | `Pair(Bool false, 5)` | 1/1 |
| type-changing-nested | `dip` | 10 | 5 | 1 | 4.93 ms (3.434–5.905) | 62 876 B / 1083 | 30 | 1200 B | `Pair(Bool false, 5)` | 1/1 |
| type-changing-nested | `dip`-library | 19 | 8 | 4 | 6.441 ms (5.205–6.984) | 70 386 B / 1198 | 52 | 1968 B | `Pair(Bool false, 5)` | 1/1 |
| type-changing-nested | library | 18 | 9 | 3 | 5.931 ms (5.503–7.158) | 69 839 B / 1192 | 50 | 1920 B | `Pair(Bool false, 5)` | 1/1 |
| type-changing-step | explicit | 3 | 1 | 0 | 8.332 ms (7.054–9.889) | 56 472 B / 970 | 12 | 288 B | `Bool false` | 1/1 |
| type-changing-step | library | 9 | 3 | 2 | 6.288 ms (5.022–8.619) | 61 043 B / 1045 | 26 | 768 B | `Bool false` | 1/1 |

Seven runs after a warm-up per form on one `noble` binary — the receipt records its SHA-256
(`4c432d62…`), its cargo profile (`release`) and the source commit it was produced from
(`adffa9d`, clean tree) — with `compile --opt off`, and the executed shape from `noble live repl`
on the same source. `type-changing-nested` in every form is the pair the first revision of this
lane was missing: a nested update whose inner operation turns an `I64` into a `Bool`, so the
reconstruction type differs from the input type.

**Read the timing column per receipt, not across receipts.** Wall time belongs to the binary and the
host. The 2026-10-31 receipt measured a packaged release CLI (medians near 1.5 ms); this one
measured the release build of this worktree, and its own min/max spreads (for example `nested-product` explicit
4.823–9.761 ms, and 8.159 ms for the library form against 7.601 ms for the explicit one) overlap and
reverse between forms. At this scale the medians establish no ordering of checking
cost, so this lane makes no checking-cost claim from them. Module bytes, line counts and execution
steps are properties of the emitted programs and of the checked work, and those are stable across
both receipts: the explicit and library forms reproduce the 2026-10-31 values byte for byte in their
module sizes and step counts.

**What the numbers do say.** Every form of every example family produces the same value, so the
comparison is on identical checked work. The composed library form is more expensive on every
program-property axis measured — more authored tokens, more execution steps (46 vs 25, 50 vs 28,
26 vs 12), a larger generated module (+11.5 %, +12.1 %, +8.1 %) and more allocation — because each
composition boundary pays one captured-quotation dispatch. The `dip` form removes the swap pair from
the source (5 stack-manipulation words against 6) but is *not* cheaper overall at this size: it adds
one quotation and one captured dispatch per update (27 steps against 25, 30 against 28; 61 653 B
against 61 094 B; 1200 B allocated against 1152 B). Its case is structural — no swap pair to get
wrong, and no residual captured as a first-class Program — not lexical or runtime cost.

## What the receipt observes, and what the profile does not expose

Per form the receipt records the observations issue item 3 asks for, and names the one the profile
cannot give:

| Axis | What is recorded | How it is obtained |
|---|---|---|
| value | the typed top-of-stack cell, unmodified as the CLI reports it (`topOfStack`, `value`) | `noble live repl` report `stack` |
| stack order | the whole ordered typed stack; `stackOrder` is its type sequence, so a differential sees a swap of two same-typed cells | `readStack` enumerates the bottom of the stack first, and the receipt establishes that direction with its own `1 5` probe (`stackOrderProbe.bottomToTop: true`) instead of asserting it |
| stack height | `metrics.stack_length` cross-checked against the number of reported cells (`stackHeightConsistent`); 1/1 for every form here | the CLI's own metric, not a count derived from the array |
| effect trace | `guest_requests` and `request_trace` (`effectTrace`); `0` and empty for every pure form here | `noble live repl` report |
| resource obligations | **not observable in this profile**, recorded as `{"observable": false}` with the reason | no must-use resource word exists in source, so no form here owns an obligation to observe; that axis is #59's fragment (`proofs/foundation/NobleFoundation/Ownership.lean`) |

The `1 5` probe is a measurement of the reporting surface, not a form: it is excluded from the form
rows and recorded once at the top of the receipt.

## Proposed sugar

| Sugar | authored tokens | tokens with parentheses stripped | expands to (tokens) | inlined explicit form (tokens) | composed library form (tokens) |
|---|---|---|---|---|---|
| nested-product | 7 | 7 | `nested-product.dip` (10) | `nested-product.explicit` (9) | `nested-product.library` (18) |
| type-changing-nested | 7 | 7 | `type-changing-nested.dip` (10) | `type-changing-nested.explicit` (9) | `type-changing-nested.library` (18) |
| type-changing-step | 4 | 4 | `type-changing-step.library` (9) | `type-changing-step.explicit` (3) | `type-changing-step.library` (9) |

`measure.mjs` costs each sugar snippet by the same whitespace-token rule it applies to the `.noble`
files, marks it `"compilable": false` and records the compiled form it expands to. It also refuses a
run in which a `role=expansion` block in `forms/proposed-sugar.md` no longer equals the form file it
restates, so those two documents cannot drift apart. The sugar never gets a compile time, a module
size or a step count here.

## The third example is model-only, with the reason named

| Example | Shape | Status |
|---|---|---|
| move-threaded-capability | sequential non-destructive operations on a must-use capability | **model-only**: the selected source profile has no must-use resource word, so no source form of this example exists to compare. The ownership fragment is the Lean model (`proofs/foundation/NobleFoundation/Ownership.lean`), and a focus elaboration must not borrow the immutable-local lowering for it. |

## Reproducing

```sh
node verification/focus-elaboration/measure.mjs \
  --noble <noble-cli> --output <scratch>/measure.json
```

The script refuses a dirty working tree with exit 3 and the offending paths named, because a receipt
that cites a commit must have been produced from that commit; `--allow-dirty` downgrades that to a
recorded `"cleanTreeEvidence": false` for a deliberate exploratory run. It exits non-zero if a form
fails to compile or execute, if a sugar expansion block stops matching its form, if a comparison
names a form that was not measured, or if the stack-order probe does not establish the report
direction — a receipt is never written past those silently. Run it with a release build for numbers
comparable to the packaged CLI; a debug build's wall times are not.

## Where the rest of the issue lives

| Document | Answers |
|---|---|
| `decision.md` | the written go/no-go: no new focus syntax now, library-form composition selected, the recomputed sugar-against-inlining comparison, and what would reopen it |
| `differential-plan.md` | the oracle, vectors and rules for later differential tests of values, stack order, resource obligations and effect traces; every item 4–5 control mapped to a #59 theorem or to "not expressible"; every unexecuted vector marked `not-run` |
| `obligations.md` | the five obligations kept formally separate, each asserted as *none* today |
| `forms/proposed-sugar.md` | the sugar snippets, the compiled form each expands to, and what a selectable design would still need |

## Non-claims

- Compile wall time is a frontend measurement on one host and one binary, not a proof-cost claim.
  Do not compare medians across receipts; compare module sizes and step counts, or re-measure.
- No form is claimed to produce identical recipe or hash identities. Different elaboration may
  produce different Programs; only the checked value, the ordered typed stack, the reported height
  and the effect trace are compared.
- The sugar is neither implemented nor proposed as selected syntax here. Whether to select it is the
  owner's decision, and these numbers are the input to that decision.
- Agreement between hand-authored forms on one build is not a theorem about an elaborator; the five
  obligations in `obligations.md` remain unasserted.
- Nothing here proves the move-threaded example in Lean beyond the ownership fragment that already
  exists, and no correspondence between the model and the selected profile is implied.
