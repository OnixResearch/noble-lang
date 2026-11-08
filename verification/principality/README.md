# Bounded principality differential (#100)

`crates/noble-contracts/tests/source/principality.rs` checks, over a bounded universe, that a definition's inferred scheme is as general as its body. `run.mjs` runs the report from a clean checkout and retains it:

```sh
env -i HOME="$HOME" PATH=/run/current-system/sw/bin \
  "$(node -p 'require("./policy/tool-selection.json").tool_paths.node.output')/bin/node" \
  verification/principality/run.mjs NEW_EXTERNAL_DIRECTORY
```

## Method

Principality says that every interface the kernel accepts for a program is an instance of the frontend's inferred scheme. A frontend that generalised a definition too narrowly would pass every current gate and still refuse compositions that should type-check. Its consequence is that a body and its definition are interchangeable.

For every generated program `P`, every context `C` and every concrete input stack `I`, the differential compares two routes:
- **the body:** `P C`, inferred at `I` and then checked by the kernel;
- **the definition:** `p C` with `def p [ P ]`, where `p`'s scheme is instantiated at `I` and the result is checked by the kernel.

They must agree: both accept with the same output stack, or both refuse. The two kinds of context catch different bugs:
- **The empty context** catches a scheme too narrow in its inputs.
- **A one-word context that consumes `P`'s outputs** catches a variable the frontend fixed when it defined `p`, such as a default to `Unit`. No input stack or output comparison can reveal that, because both routes print the same defaulted type.

A discrepancy is shrunk by removing words while the routes still disagree in the same context at the same input stack.

| Bound | Value |
|---|---|
| words | `dup drop swap 1 true [ ] [ dup ] [ 1 ] [ drop ] compose run dip if` |
| programs | 1 to 3 words: 2379 programs, each with the empty context |
| contexts | every one-word context, for the 182 programs of 1 or 2 words |
| value types | `I64`, `Bool`, `Program<[] -- [I64]>`, `Program<[I64] -- [I64 I64]>` |
| input stacks | depth 0 to 2: 21 stacks |

## Results

| Measure | Empty context | One-word contexts |
|---|---|---|
| program/context pairs | 2379 | 2366 |
| pairs whose definition is accepted | 2076 | 2249 |
| comparisons | 49 959 | 49 686 |
| accepted by the body | 20 939 | 20 750 |
| accepted by the definition | 20 939 | 20 750 |
| discrepancies | 0 | 0 |

No refused definition has an instance the body accepts.

## Controls

A zero result needs controls that can fail. One covers each kind of context:
- **`an_overly_specific_scheme_is_caught`.** It defines `p` as `dup dup 1 + drop`, which matches `dup`'s outputs on `I64` but forces `I64`. This is what a frontend that generalised `dup` too narrowly would infer. The empty context catches it at the `Bool` input, and the routes agree wherever the top is `I64`.
- **`a_definition_time_default_is_caught_only_in_context`.** It defines `p` as `nil dup unit swap cons drop`, which is `nil` with its element type fixed to `Unit`. That is the default a frontend that defaulted at definition time would choose. With the empty context, both routes return `List<Unit>` at all 21 stacks. The context `cons` at an `I64` input accepts the body and refuses `p`.

The control uses `nil` because its element type appears only in its output. A definition can therefore fix that type without involving its own stack row. Fixing the variable of a `[ dup ]` quotation by running one copy over a stack that holds another copy is refused instead: the Program's row then contains itself.

## Limits

This is bounded evidence over the universe above, not a proof.
- **Constructibility.** The body route is the frontend's own inference at concrete types. An interface the kernel would accept, but that the frontend cannot construct even at concrete inputs, is outside the comparison. An independent enumerator of node instantiations would be needed to reach it.
- **Context bound.** Contexts are single words and cover programs of at most two words.
- **Excluded features.** Effects are #28's oracle. Recursive definitions wait for #99.
