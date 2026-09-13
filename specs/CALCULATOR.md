<!-- Generated compatibility view. Edit .cairn/specs/calculator/spec.md instead. -->
# Exact calculator and AI-authoring benchmark

Document: SPEC-CALC001  
Revision: 0.1.0-draft.5  
Status: Selected application contract. Implementation, execution, and proofs remain open.

## 1. Scope

**CALC-SCOPE-01.** Noble MUST provide an exact programmable calculator as its first AI-authoring reference application. The calculator MUST use Noble library programs for parsing, validation, and evaluation. A thin host shell owns terminal input/output and session storage.

This is an application contract, not a new language conformance profile. The calculator expression grammar does not change Noble source grammar. An interpreter for calculator expressions does not satisfy Noble's compiled-program backend obligations.

**CALC-SCOPE-02.** Calculator arithmetic MUST NOT change Noble's wrapping `I64` operators or expand `Core-Bootstrap`. Arbitrary-precision arithmetic belongs in a separately specified library. Unsupported library features MUST remain explicit rather than fall back to machine arithmetic.

[`CORE-BOOTSTRAP.md`](CORE-BOOTSTRAP.md) remains the first compiler target. [`PROGRAM-CONTRACTS.md`](PROGRAM-CONTRACTS.md) governs optional application proofs. This amendment does not require dependent types, implicit coercions, or a new kernel mechanism.

## 2. Exact numeric library

`BigInt` and `Rat` below name conceptual library types. They are not new primitive Noble types or frozen declaration syntax.

**CALC-NUM-01.** `BigInt` MUST represent signed mathematical integers without a fixed numerical width. Operations MUST return exact results within the declared resource budget. Exhaustion MUST produce `ResourceLimit`, not a wrapped value, approximate value, or ordinary success.

An implementation has finite memory. Arbitrary precision does not promise unlimited input size or successful completion under every budget.

**CALC-NUM-02.** Each accepted `Rat` MUST contain an integer numerator `n` and a positive integer denominator `d`. Its invariant is `gcd(abs(n), d) = 1`. Zero MUST have the unique representation `0/1`. Construction MUST reject a zero denominator. Constructors and decoders MUST establish this invariant before they expose an accepted value.

Normalization moves the sign into the numerator and removes common factors. Rational equality compares mathematical values, not input spelling. For example, `2/4` and `1/2` denote the same value.

**CALC-NUM-03.** Calculator literals MUST denote exact rational values. A digit sequence denotes an integer with denominator one. A finite decimal with `k` fractional digits denotes its combined integer digits divided by `10^k`, followed by normalization. Parsing MUST NOT pass through binary floating point or a fixed-width integer conversion.

Thus `0.1` denotes `1/10`, and `0.1 + 0.2` denotes `3/10`. This decimal policy belongs to the calculator. It does not select decimal literals for Noble source.

**CALC-NUM-04.** Calculator `+`, `-`, `*`, and `/` MUST use exact rational arithmetic. Division by zero MUST return `DivisionByZero`. It MUST NOT return zero, infinity, `NaN`, or a truncated quotient.

**CALC-NUM-05.** Calculator exponentiation MUST accept only integral exponent values. A nonintegral exponent MUST return `NonIntegerExponent`. Negative exponents MUST use exact reciprocals, with `DivisionByZero` for a zero base. Exponent zero MUST return one, including `0^0`.

The `0^0 = 1` convention belongs to this integer-power operation. It does not state a theorem about limits or arbitrary real exponentiation. A rational exponent value with denominator one is integral, regardless of its original spelling.

**CALC-NUM-06.** Conversions between exact numbers and Noble `I64` MUST be explicit. Conversion to `I64` MUST reject fractions and out-of-range integers. Conversion from `I64` MUST preserve its signed value. Calculator integration MUST NOT reinterpret a core wrapping result as an unbounded-integer result.

The library interface gate must define ordinary `Result` schemas for numeric errors. Representation, library word names, and checked conversions remain open deliverables.

## 3. Expression grammar

**CALC-PARSE-01.** The first calculator expression parser MUST use this grammar and consume the complete input:

```text
expression := sum
sum        := product { ("+" | "-") product }
product    := unary { ("*" | "/") unary }
unary      := ("+" | "-") unary | power
power      := primary [ "^" unary ]
primary    := number | identifier | identifier "(" [ arguments ] ")"
            | "(" expression ")"
arguments  := expression { "," expression }
number     := digits [ "." digits ]
digits     := digit { digit }
digit      := "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
identifier := (ASCII-letter | "_") { ASCII-letter | digit | "_" }
```

Identifiers and numbers use maximal token matching. Spaces, tabs, carriage returns, and newlines separate tokens. Whitespace cannot split a number or identifier. No comments, implicit multiplication, scientific notation, or other literal forms belong to this expression version.

Sum and product operators associate to the left. Exponentiation associates to the right and binds more tightly than a leading sign. Signed exponents remain valid. These rules give `-2^2 = -4`, `2^3^2 = 512`, and `2^-3 = 1/8`.

**CALC-PARSE-02.** Invalid syntax, trailing input, unknown names, and wrong call arity MUST produce distinct diagnostics. Each diagnostic MUST name its category and the relevant expression span. Unavailable provenance MUST remain explicit. The parser MUST NOT guess a corrected expression and execute it silently.

Expression spans use zero-based, half-open UTF-8 byte offsets. Invalid UTF-8 is a syntax error. Builtin scientific functions are not implicitly available names.

The calculator validates the complete expression before evaluation. It evaluates binary operands and call arguments from left to right. An error prevents evaluation of the remaining operands or arguments.

## 4. Definitions and session state

**CALC-STATE-01.** Variables and function parameters MUST use lexical resolution. Function definitions MUST retain immutable references to the exact global values and earlier function definitions they resolve. Rebinding a display name MUST NOT change an existing function. Parameters shadow global values within their function body.

The first version permits nonrecursive functions with exact rational arguments and results. Repeated parameter names, self-recursion, and cyclic definition dependencies are definition errors. Function definitions cannot capture host capabilities. A later recursion extension requires a separate contract.

**CALC-STATE-02.** A submission MUST produce either an accepted value and new session state or an explicit failure. Syntax errors, definition errors, numeric errors, and resource exhaustion MUST leave the prior calculator state unchanged. A function definition MUST resolve and validate its entire body before installation, without evaluating that body.

The shell serializes session submissions. It publishes a new state only after evaluation or definition admission succeeds and bounded formatting succeeds. Earlier successful submissions remain intact. Terminal output failure does not imply rollback of an already committed calculator state.

Concrete variable-assignment and function-definition commands remain a CLI design gate. Structured fixtures describe definitions without selecting their user-facing spelling. Calculator validation is not a substitute for Noble's program acceptance checker.

## 5. Effects, limits, and display

**CALC-BOUND-01.** The calculator core MUST receive the expression, immutable environment, and deterministic budget as explicit inputs. It MUST return values, diagnostics, and proposed state without host effects. The shell MUST NOT supply ambient filesystem, network, clock, or process authority to calculator expressions.

**CALC-LIMIT-01.** The implementation MUST bound input bytes, numeric digits, intermediate integer bits, syntax depth, call depth, evaluation work, and output size. Bounds MUST apply before unsafe allocation or unbounded work, including parsing, normalization, exponentiation, and formatting. Each exceeded bound MUST identify its category through `ResourceLimit`.

The gate must define budget units and accounting. A host timeout or internal failure remains distinct from a deterministic `ResourceLimit`. No failure can count as a successful calculation. Tests must cover each limit independently.

**CALC-DISPLAY-01.** The default exact display MUST use an integer for denominator one and `n/d` otherwise. A user-selected finite-decimal display MUST preserve the same exact value. Nonterminating decimal expansions MUST remain fractions unless the user explicitly requests approximation. Formatting MUST obey the output budget.

For example, `3/10` can display as exact `0.3`. A truncated decimal for `1/3` cannot carry an exact-result label.

**CALC-APPROX-01.** The first version MUST NOT silently convert exact values to approximate values. Approximation, symbolic expressions, trigonometry, units, and real or complex arithmetic remain later extensions. Each extension requires explicit types, conversions, domain errors, and evidence contracts.

A future approximation operation must declare its precision, rounding, and error claims. A requested digit count alone does not establish accuracy. A mathematical `Real` model is not automatically an executable representation of every real number.

## 6. AI-authoring evaluation

**CALC-AI-01.** Benchmark tasks MUST bind an immutable starting revision, a requested change, and independently owned acceptance criteria. The producer MUST NOT change those criteria, strengthen preconditions, remove tests, or grant additional authority to obtain acceptance. Authorized task changes require a new task revision.

The initial portfolio covers five change families:

1. Implement the exact expression evaluator for a declared operator subset.
2. Add integer powers without changing the selected precedence rules.
3. Add lexical variables and nonrecursive reusable functions.
4. Repair a supplied parser defect without breaking existing expressions.
5. Extend numeric capacity without introducing narrowing, wrapping, or silent approximation.

**CALC-AI-02.** A benchmark run MUST record the model/version, task revision, prompts and supplied context, tool interfaces, budgets, attempts, patches, compiler diagnostics, and outcomes. Records MUST identify the compiler/backend and exact numeric-library revisions. Sensitive inputs require authorized retention or explicit replay limitations.

Reports must distinguish first-attempt acceptance, final acceptance, repair attempts, total tokens, elapsed time, regressions, and incorrect success claims. An incorrect success claim means that a producer reports success while independent acceptance rejects its result.

**CALC-AI-03.** Comparison runs MUST use declared comparable tasks, context, tools, and budgets. Reports MUST retain failures, unsupported tasks, timeouts, and harness errors. Held-out acceptance cases MUST remain outside producer control. A small example suite or one model MUST NOT establish universal AI suitability.

The comparison candidates are stack-only source, source with local names, and structured syntax edits. They are not presumed equivalent in cost. Unsupported authoring routes remain labeled unsupported until their language and tool gates close.

**CALC-AI-04.** Compiler assistance MUST expose versioned structured diagnostics, relevant interfaces, stack/effect constraints, and available value origins. Text and structured edits MUST pass the same independent program acceptance boundary. Edits against a stale base revision MUST be rejected before publication. Incomplete editor holes MUST remain nonexecutable.

[`DEVELOPER-EXPERIENCE.md`](DEVELOPER-EXPERIENCE.md) owns the underlying diagnostics, local-name, hole, and identity contracts. This benchmark adds no alternative trusted checker. Concrete tool schemas remain an implementation-entry gate.

## 7. Evidence and delivery

**CALC-EVIDENCE-01.** Application acceptance MUST use the actual Noble compiler and applicable Wasm backend. It MUST include exact results, error paths, unchanged state after failure, and explicit limits. An independent exact-arithmetic oracle and seeded incorrect implementations MUST exercise the harness. Shared arithmetic or parser code MUST NOT serve as the sole independent oracle.

The AI-authoring report must distinguish functional tests from optional behavioral proofs. Passing tests does not establish parser correctness or a universal arithmetic theorem.

**CALC-EVIDENCE-02.** Proof claims MUST bind the actual calculator implementation and the correct exact-number model. Normalization, arithmetic, parsing, and session transitions require their own claims. Proofs about wrapping `I64`, floating point, or a rewritten reference example MUST NOT automatically certify the exact calculator.

Zero-divisor preconditions and numeric error branches must remain explicit in the model. External arithmetic libraries require a reviewed implementation and correspondence boundary. Native arithmetic acceleration cannot silently replace the Noble reference application or inherit its proof claims.

**CALC-GATE-01.** Calculator acceptance MUST follow M4 and explicit library, declaration/module, iteration or recursion, text-processing, error-schema, and budget gates. The complete application MUST NOT block M1 through M4. Bootstrap arithmetic demonstrations MUST NOT count as an implemented exact calculator.

| Slice | Entry gate | Required evidence |
|---|---|---|
| Bootstrap precursor | M2/M4 | Existing checked `I64` arithmetic and runtime composition, labeled as a narrower slice |
| Exact library | After M4 and library/type gates | Big integers, normalized rationals, exact decimals, checked conversions, and independent limits |
| Calculator v1 | Exact library and expression/session gates | Actual Noble/Wasm parser, lexical functions, exact results, and unchanged state after failure |
| AI-authoring report | Calculator harness and declared authoring tools | Recorded change tasks, held-out acceptance, negative controls, costs, and visible failures |
| Optional proof report | Applicable MC1/MC2 support | Exact subject/model correspondence and accepted evidence under SPEC-V002 |

The [calculator scenarios](conformance/calculator-cases.json) are unexecuted harness designs. `Calculator-Design` and `AI-Authoring-Design` label application/test lanes, not Noble language conformance profiles. JSON exact-number fields use decimal strings. They do not define portable Noble serialization.

[Source provenance](SOURCES.md) records the mathematical-computing references and their limits. No calculator, benchmark, or proof execution is claimed by this amendment.
