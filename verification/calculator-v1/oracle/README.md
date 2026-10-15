# CALC-12 independent exact-arithmetic oracle

This directory is the CALC-12 comparison lane (`CALC-EVIDENCE-01`). It judges
what the real `noble calculator` binary printed against an exact reference
written independently of the calculator. It is separate from the
[#19 acceptance runner](../README.md), which checks the cases file's
hand-written expectations, and from any proof. Its results are implementation
observations only. They never change a canonical case `state` or `evidence`.

## Independence

| Component | Shares code with the guest modules, the Rust host or the #19 runner? |
|---|---|
| `rational.mjs`: exact rationals | No. It uses the JavaScript engine's (V8) native arbitrary-precision `BigInt`. The guest uses its own Noble limb lists (`calculator-bigint/-bits/-numeric.noble`); the Rust host computes no calculator value. |
| `calculator.mjs`: lexer, parser, validation, evaluation, session, display | No. It is written from the text of `.cairn/specs/calculator/spec.md`: CALC-PARSE-01 grammar, precedence and associativity; CALC-PARSE-02 diagnostics, whole-expression validation and left-to-right evaluation; CALC-STATE-01/02; CALC-NUM-02..05; CALC-DISPLAY-01. It imports only `rational.mjs`. |
| `generate.mjs`, `faults.mjs`, `compare.mjs` | No. They import only the files above and Node built-ins. |

The oracle has three inputs that are not in the spec:

- **Definition spelling.** The spec leaves the command grammar to an open CLI
  design gate. The oracle uses the README spelling `let NAME = EXPR` /
  `fn NAME(P, ...)=EXPR`, the same spelling the #19 mapping table uses.
- **Definition output.** An accepted `let` prints its value, following
  CALC-STATE-02 ("an accepted value and new session state"). An accepted `fn`
  prints nothing. The spec does not say whether a definition echoes.
- **Diagnostic line format.** The oracle uses the README's stderr line format,
  `Category [start,end)`, followed by `provenance=unavailable`.

The oracle does not model the seven guest budgets. Any `ResourceLimit` is
therefore reported as `budget-divergence`, never as agreement. Generated
inputs keep their magnitudes small, apart from one wide value beyond 64 bits.

**What the oracle judges.** It does not guess where the spec is silent:

- **Values, categories and exit status** are always judged.
- **Spans** are judged only where the CALC-02 examples fix the rule:
  - an unexpected or stray token;
  - the first trailing token;
  - an unknown identifier.
- **Other spans** are recorded as `spansNotJudged`. These are numeric errors,
  wrong arity, definition errors, end of input and malformed definition
  headers.
- **Unresolved readings.** Where two readings of the spec differ, the outcome
  carries an `ambiguous:*` note. Examples are a stray byte after an earlier
  parse error, arity versus argument errors, and redefining a name that a
  function body refers to. The generator never emits such an input, and the
  comparison reports it as `undetermined`.

## Commands

Run every command with the pinned Node (`$NOBLE_NODE/bin/node`). Each one
writes a fresh JSON file outside the repository.

```sh
# Oracle vs the cases file's hand-written expectations (no CLI involved)
node compare.mjs cases --out NEW.json
# Oracle vs every process retained by a #19 runner output directory
node compare.mjs receipts --run RUN_DIR --out NEW.json [--ids-prefix CALC-01/,CALC-02/]
# Seeded generated inputs as a #19 runner vector file (expectations from the oracle)
node compare.mjs generate --seed 2112 --count 12 --sessions 2 --out NEW.json
# Harness-level fault model as the implementation, with bounded shrinking
node compare.mjs harness --vectors VECTORS.json --implementation fault:integer-division --out NEW.json
```

For generated inputs, run `compare.mjs generate`, execute the file with the #19
runner (`acceptance.mjs --vectors FILE`, one process per vector), then run
`compare.mjs receipts` on that output directory. The comparison re-derives
every expectation from each process's retained stdin and argv. It ignores the
runner's own classification.

The generator is mulberry32, seeded and reproducible. It cycles through these
shape families:

- mixed arithmetic;
- right-nested and signed powers;
- division;
- decimals;
- numeric errors;
- the CALC-02 malformed shapes;
- one value wider than 64 bits.

It can also emit short lexical sessions. Every second session includes a
failing `let` followed by a read of that name.

Shrinking is bounded. Each round proposes at most `--shrink-candidates`
strictly smaller inputs, smallest first. The input must stay a disagreement,
and shrinking stops after `--shrink-rounds` rounds. Harness mode shrinks in
process. Against a real binary, each candidate costs one CLI process, so
each round is run as a runner vector file. `compare.mjs shrink --input TEXT
--shrink-candidates N --out NEW.json` emits the input plus at most N
candidates, then `compare.mjs receipts` shows which candidates still
disagree.

## Seeded faults

`faults.mjs` defines the seven CALC-12 incorrect implementations as models.
Each model is the oracle with exactly one defect. `compare.mjs` uses them in
two ways:

1. **Harness-level mutants.** A model stands in for the implementation, which
   exercises the comparison and the shrinker.
2. **Attribution.** For a real disagreement, `matches_fault_models` lists the
   models that predict exactly the observed output. This shows that a real
   mutant binary is rejected for its intended reason.

The models are incorrect by construction and never serve as an oracle. The
real mutant patches live in [`../seeded-faults/`](../seeded-faults/README.md).

## Optional Wolfram provider

The [Wolfram lane](../wolfram/README.md) (#24) is an opt-in second
exact-arithmetic reference. It is limited to the arithmetic subset. It
reuses this directory's generator, shrink candidates and fault models, but
not its arithmetic.

## Non-claims

- Agreement on finitely many inputs does not establish parser correctness or a
  universal arithmetic theorem (`CALC-EVIDENCE-01`).
- No proof is made or checked (`CALC-EVIDENCE-02` proof reports are out of
  scope).
- No canonical case state changes.
