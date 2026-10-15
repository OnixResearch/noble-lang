# Calculator-v1 committed session: replay vs restore (issue #17)

A measurement record, not an acceptance claim. It compares the per-line
definition replay at the base with the committed session and byte-exact
restore of `feat/issue-17-calculator-snapshot`. `summary.json` holds every
per-line figure, exit status, output and hash.

## Method

- **Builds.** Both are release-profile `noble` binaries with the same
  temporary instrumentation, `instrumentation.patch` (sha256
  `36be86c3…44e4`), which is not part of the committed source. It counts
  `Session::execute` calls, one guest module each, and prints one
  `noble-measure` stderr line per submission.
  - before: base `ffcce53` (origin/main `44329dc` plus formatting and a vendor
    re-pin, with no calculator change) plus the patch. sha256 `f1488f98…7fec`.
  - after: the source of `5d2a963` plus the patch. sha256 `5bc66ffa…b1f2`.
- **Runs.** `run.sh NAME BINARY STDIN` runs `noble calculator` with `env -i`
  and its own `TMPDIR`. The four runs were concurrent, on a shared 32-core
  host with a load average around 58. Wall times are therefore noisy. Module
  counts do not depend on load.
- **Counting.** A line's count includes every module that the submission
  executed. In the before build this includes the fresh candidate's
  `calc_env@1.initial` and the replay of every earlier `let`/`fn`. The
  totals also include the one startup module.

## Per-line cost of the same expression

Input is `perline.stdin`: `1 + 2`, `let a = 1`, `1 + 2`, `fn h(y)=y + 1`,
`1 + 2`, `let b = 2`, `fn k(z)=z * 2`, `1 + 2`.

| `1 + 2` after | before: modules | before: wall | after: modules | after: wall |
|---|---|---|---|---|
| 0 definitions | 459 | 333 s | 458 | 508 s |
| 1 definition | 724 | 672 s | 458 | 552 s |
| 2 definitions | 986 | 1,123 s | 458 | 467 s |
| 4 definitions | 1,513 | 1,491 s | 458 | 468 s |

Definition lines show the same pattern. Before, they cost 266, 528, 793 and
1,055 modules. After, they cost 265, 262, 265 and 262. Both runs printed
`3 1 3 3 2 3` and exited 0. Over the whole session, before took 6,325
modules and 6,741 s, and after took 2,887 modules and 3,209 s.

## CALC-06 full session

Input is `calc06.stdin`, the CALC-06 operations in one process.

| | modules | wall | exit | stdout | stderr |
|---|---|---|---|---|---|
| before | 10,036 | 9,340 s (2.6 h) | 1 | `10 20 11 3 20` | `DivisionByZero [10,11)`, `WrongArity [0,1)` |
| after | 2,938 | 3,383 s (0.94 h) | 1 | `10 20 11 3 20` | `DivisionByZero [10,11)`, `WrongArity [0,1)` |

Per line, before needed 290, 575, 864, 1,308, 1,126, 1,642, 1,506, 1,239 and
1,485 modules. After needed 289, 285, 289, 444, 262, 516, 380, 113 and 359.
After therefore runs 3.4× fewer modules.

- **Exit status 1** is the CLI contract: the exit status is 1 if any line
  failed. CALC-06 deliberately contains two failing operations,
  `let x = 1 / 0` and `f(1, 2)`. Every value line printed its expected result.
- **`DivisionByZero [10,11)`.** The CALC-06 case records only the category
  `DivisionByZero`, with no span. This stdin spells the operation
  `let x = 1 / 0`. Its bytes 0–9 are `let x = 1 `, so byte 10 is `/` and
  `[10,11)` is the division operator. Both builds report that same span. The
  `[9,10)` quoted in issue #17 came from the pre-port tree, whose stdin is not
  available here. That span would point at `/` only if the line had no space
  before the operator, as in `let x = 1/0`.

## CALC-STATE-02 on real definition lines

The ignored test
`calculator::tests::calc_state_02_failed_lines_leave_definitions_byte_identical`
was run from a frozen copy of the release test executable (sha256
`5e29de3c…f6a5`) with the assembler `0c9c00ad…0d88`.

It passed in 2,414 s. The real failures were
`DivisionByZero [10,11)` and `ResourceLimit IntermediateIntegerBits [2,3)`.
Injected native, host and infrastructure failures followed. After each one,
the namespace, compiler, stack and arena fingerprints equalled those of the
reference session. The later lines `f(1)` and `x` printed `11` and `20`, the
same values and fingerprints as the reference session.
