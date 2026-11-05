# Construction-checked invariants on opaque nominal types (#89): no-go for now

**Question.** Would attaching a predicate to an `opaque` nominal type (`T.new` discharges it, consumers
of `T.into` assume it) remove repeated `requires` clauses from real Noble contracts? The issue asks
this before choosing syntax.

**Decision: no-go, deferred.** The repository has no contract that could use such an invariant. The
mechanism has no consumer until a contract surface can name a nominal type, and that prerequisite is
larger than the invariant itself. Syntax is not chosen.

## Evidence

`verification/opaque-invariants/survey.mjs` measures the authored contract corpus and probes the
production CLI. Receipt: `verification/opaque-invariants/receipt.json`.

**Corpus.** The corpus is every tracked contract fixture, `.contract` and `.noble-contract` file,
inline contract in Rust and lane scripts, and conformance case input. It excludes receipts, locks and
specification examples. It holds 37 distinct contracts in 29 files. Two format templates
(`{requires}`) are excluded.

| `requires` | Contracts |
|---|---|
| `true` | 31 |
| none (refusal fixtures) | 2 |
| a bound on one I64 input | 2: `nonnegative-increment` `(le 0 (in x))`, `conditional-monotonicity` `(lt (in x) 9223372036854775807)` |
| other | 2: `structural-roundtrip` `(def identity)`, the refusal fixture `hidden-output` `(eq (def result) 0)` |

No non-trivial `requires` clause repeats across contracts, and no contract names a nominal type in
its inputs or outputs. The two I64 bounds are the only clauses a wrapper such as `NonNegative` could
replace. They would move only if those contracts were rewritten over a nominal input, which neither
surface admits:

| Probe | Surface | Result |
|---|---|---|
| `(input (x Positive))` | MC1 `explain-proof` | refused, exit 2, `contract-invalid`: `unknown type` at the type name |
| the same contract over `(input (x I64))` | MC1 `explain-proof` | admitted: the Lean statement is generated |
| `(Pi (x Positive) (Eq Positive x x))` after `opaque Positive I64 public` | intrinsic `verify-module` | refused at acceptance, exit 2: `not a bound Type0 code` |
| the same proof over `I64` | intrinsic `verify-module` | admitted: it passes acceptance and reaches the independent recheck |

The source agrees with the probes. The MC1 type grammar admits only `Unit`, `Bool`, `I64`, `Text`,
`Syntax`, `Contract`, `Evidence` and `Certified` atoms with the `Pair`/`Sum`/`List`/`Program`
constructors (`crates/noble-contracts/src/syntax/typing.rs`). Its predicate operations refuse
`Ty::Nominal` operands. Intrinsic codes are `Unit`, `Bool`, `I64`, `Text` or bound code variables
(`crates/noble-contracts/src/intrinsic.rs`).

## Answers to the issue's open questions

1. **Contract field or `opaque` declaration.** Not chosen, as the issue asks, until the benefit is
   shown. One observation: the reserved `invariant` field sits on the contract form, which cannot
   name the type. The `opaque` declaration owns the payload type and the `new`/`into` visibility.
2. **Can the Lean model carry the predicate on `NominalTypeId` without changing
   `MC1Obligation.claim`?** No. The MC1 model's `Ty`
   (`proofs/mc1/NobleContracts/Model.lean`) has no nominal constructor, and generated obligations
   state `inputTypes : List Ty` over that structural `Ty`. A nominal input therefore changes the
   model that every claim is stated over, before any invariant is added.
3. **Does a current fixture repeat such `requires` clauses?** No; see the table above.

## What would reopen it

- A contract surface that names declared nominal types: the MC1 type grammar, a nominal `Ty` in the
  Lean model, and predicate access to the payload through `into`.
- Contracts over those types that repeat a payload guard. The `opaque AttemptId I64`-style worker
  schemas are a plausible source, but none carries a contract today.

Once both exist, rerun the survey. The predicate-language, self-reference and `assumptions`-block
conditions in the issue remain the acceptance plan.

## Non-claims

The survey measures the tracked corpus only. The probes show what the current surfaces admit; no
invariant mechanism is built, checked or proved.
