# Separate duplicate and discard permissions (issue #112)

This lane evaluates a two-bit permission profile beside Noble's existing `Data` / `Resource`
distinction. It does **not** select new source syntax or change the selected Core-Bootstrap
profile. The decision is NO-GO on the evidence available: existing `Data` already permits both
explicit `dup` and `drop`, existing Resources remain exactly-once, and no concrete inert
must-handle data value has been found for which the explicit `drop` in the selected profile is
semantically unacceptable.

## The bounded permission model

The four hypothetical classes are:

| Class | `dup` | `drop` | `Capture` in the model | Interpretation |
|---|---:|---:|---:|---|
| `Data` / unrestricted | yes | yes | yes | Today's unrestricted value |
| Resource / must-use | no | no | no | Today's exactly-once owner; not a resource release operation |
| affine / at-most-once | no | yes | no | Hypothetical inert data; never a LiveRef or host authority |
| must-handle / at-least-once | yes | no | no | Hypothetical inert data whose value-level obligation must be consumed |

For `Pair<a,b>` and `Sum<a,b>`, each permission bit is the conjunction of the corresponding bits
of both payloads; a sum must be safe for either arm. `List<a>` inherits `a`'s bits. An opaque
nominal wrapper inherits its representation's bits, while a variant wrapper combines both arms.
The combination is pointwise AND, so no enclosing constructor launders a restricted member.
Nested Pair/Sum/nominal/List controls are in `cases.txt`; the kernel-checked recursive rules are
in `NobleFoundation.UsePermissions`.

In this proposed algebra, capture requires **both** `dup` and `drop`, so Capture has no additional
cell beyond unrestricted Data. This does not erase the specification's separate semantic
requirement: SPEC-0001 defines `Capture(a)` as Data plus a portable semantic representation. The
selected kernel currently routes `Dup`, `Drop`, and `Quote` through one `Ty::is_data` check. Its
compatibility asymmetry is retained: `is_data` accepts `Contract`, `Evidence`, and `Certified`,
but `valid_generic_argument` rejects them. This lane neither normalizes nor changes that behavior.

Compatibility is deliberately conservative:

* **Data:** keeps today's ability to `dup`, `drop`, and—subject to the selected kernel's current
  shared `is_data` check—`quote` it.
* **Resource and LiveRef:** remain non-Data and non-Capture, and keep their existing ownership and
  host-authorization rules. They cannot be moved into either hypothetical data-only class.
  H-AUTH-04 still prohibits Data, Capture, duplication, discard, serialization, and unchecked
  construction for a live authorization witness.
* **Program:** remains an immutable, unrestricted value even when its input interface names a
  resource, because the interface does not transfer ownership of that resource. Capturing an
  environment still requires every captured payload to be capturable. Closure-use policy and
  captured-payload policy are separate; this model adds no one-shot closure or resource-bearing
  Program capture.

The proof model's exact ordered-stack examples use **top-first** notation (head is the next
constrained position), unlike the source examples below:

```text
dup       [Data(x), S]                 -> [Data(x), Data(x), S]
drop      [Affine(x), S]               -> [S]
quote     [Data(x), S]                 -> [Program(x), S]
quote     [MustHandle(x), S]           -> refused
swap      [Resource(x), Data(y), S]    -> [Data(y), Resource(x), S]
dip       [a, Program(f), S]           -> [a, S]
compose   [Program(f), Program(g), S]  -> [Program(f), S]
run       [Program(f), S]              -> [S]  (accounting projection only)
fail      [a₁, ..., aₙ]                -> [] iff every `aᵢ.drop` is true
branch    two arms from S               -> admitted iff both succeed with identical ordered stacks
```

`drop` above is structural discard, never a destructor or effectful host release. The modeled
`fail` is a normal Result-error stack-unwind rule, not abnormal host cleanup; it requires every
remaining position to be discardable. The model records neither a Result error payload nor an
effect trace, so this is only the local-accounting part of normal error handling. An explicit
operation such as `regions.release` remains an effectful operation in program order; neither
`drop` nor an error edge creates or reorders that effect. Abnormal trap/cancellation/host cleanup
stays on the host side and is not made a guest permission. This rule is intentionally not
identified with #59's stricter `Ownership.fail`, which accepts only an empty operand stack. That
oracle and its retained receipts are untouched.

## Worked source examples

All four source forms are executed with the real selected-profile CLI. They use only current
unrestricted I64 Data; the two-bit alternatives and all Resource/must-use, affine, must-handle,
branch, and error-return cases remain model-only.

| Case | Existing source | Observed value / stack | Compile median / module bytes | Steps / allocated bytes | What it shows |
|---|---|---|---:|---:|---|
| shared configuration | `3 dup 4 + swap -` | `I64 4` | 10.375 ms / 58,218 B | 18 / 576 B | One value must feed two calculations. Today's Data already allows the explicit `dup`; a Resource would correctly refuse it. A split adds no useful permission. |
| discarded intermediate | `1 2 drop 3 +` | `I64 4` | 8.994 ms / 57,324 B | 15 / 480 B | An unused value is explicitly dropped before continuing. Today's Data already allows that structural discard; it does not invoke a host release. |
| harmless duplication control | `7 dup +` | `I64 14` | 10.744 ms / 56,121 B | 11 / 288 B | The value is duplicated and consumed, with no discard or capture policy question. The split does not change admission. |
| must-handle counterfactual | `5 1 + drop` | empty stack | 9.038 ms / 56,354 B | 13 / 384 B | The selected Data profile permits explicit drop. A hypothetical must-handle type would refuse this, but no real source-level value or requirement has been found that should be protected this way; this is not evidence for adoption. |

The explicit Resource comparison is spec-anchored but **not** a Core-Bootstrap `.noble` form:
the bounded-region adapter contract says `region.read` restores the borrowed owner on both normal
Result branches and `regions.release(own<region>)` consumes that owner once
(`specs/RESOURCE-ADAPTERS.md:23`). This is explicit effectful release, not generic `drop`; the
permission model leaves a Resource must-use and cannot turn either `drop` or a permission bit into
that host operation. The resource example is not run by this lane's CLI forms.

The arithmetic examples are worked examples of reuse and explicit discard, not examples where the
new permission categories outperform the existing policy. The resource case shows why structural
drop must remain separate from an explicit effectful release. The harmless-duplication case is a
control that should remain unchanged. The must-handle example keeps the unresolved exception
visible instead of claiming a discovered use-case: it is only a counterfactual because no
selected-profile annotation can mark the result must-handle.

## Reproduction and evidence boundaries

`cases.mjs` is an independent executable expectation generator. Its expected decision rows are
declared in `expectedSingle` and at each multi-position case; they are checked against its own
ordered-stack evaluator before `cases.txt` is emitted. The Lean module has a separate literal
control matrix proved by `decide`; neither side derives its expected outcomes from the Rust
checker. The source measurement runner fails on any compile error, non-normal REPL report, wrong
stack, missing form, or dirty tree. Its clean-source receipt is
[`receipts/2026-11-13/measure.json`](receipts/2026-11-13/measure.json), produced from clean source
commit `74b1748153fb778e53c4dd8830d6912889f00524`. Compile times are the median of seven runs
after one warm-up, `noble compile --opt off`; they price this existing profile only, not a
four-way checker.

Reproduce the local finite artifacts and source-profile measurements with the pinned tools:

```sh
NODE=/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node
"$NODE" verification/use-permissions/cases.mjs
cargo build -p noble-cli --bin noble --offline --locked   # with the pinned toolchain
OUT=<new file outside the repository>
"$NODE" verification/use-permissions/measure.mjs \
  --noble <absolute path of the built noble CLI> --output "$OUT" --runs 7 --opt off
```

The first step creates the committed deterministic corpus. The receipt producer must run only
after the source/docs/model commit is clean; its output is kept outside the repository, then
copied under the dated `receipts/` path and committed as evidence. The receipt's `source_commit`
and `source_tree` identify the exact source used. The measured compile wall time is a selected
profile CLI measurement, not a measurement of an unimplemented two-bit checker.

## Non-claims

* The four-way model is not the Core-Bootstrap type checker, an elaborator, a new profile, or a
  runtime implementation.
* The Node corpus and the four live CLI examples are finite tests. They are not a proof of
  implementation correspondence.
* The Lean theorems establish only the stated permission algebra and finite word/branch/error
  model. They do not prove the Rust walk or establish that the proposed policy is desirable.
* No explicit release operation, host cleanup, effect ordering, loop semantics, or general
  inference is added. H-AUTH-04 and #59's ownership policy remain unchanged.
