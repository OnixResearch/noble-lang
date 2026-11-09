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

`receipt-28f40c2c.json` is the lane's receipt from the clean commit `28f40c2c`. `receipt-a1b2b39c.json`, from the clean commit `a1b2b39c`, retains this report unchanged together with the signed report below (receipt schema v2).

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

## Signature-checked recursion (#99)

`principality/signed.rs` extends the differential to signature-checked recursive definitions. The frontend does not infer a recursive scheme there. Under signed self recursion it checks the body against the declared signature, with every self call at the owner's own instance. Two consequences of principality remain that a frontend could get wrong while every current gate passes:
- **Uses.** A call of an accepted definition must be accepted at exactly the concrete input stacks that instantiate its declared signature, with the instantiated output stack. A frontend that fixed a variable of the declared signature while checking the body would refuse some instance.
- **Instances.** A body accepted at a declared signature must be accepted at every declared instance of it, as the substitution lemma requires for monomorphic recursion.

Each body is declared as `module m@1 [ signature p … export p def p [ BODY ] ]` in a fresh signed session and called as `m@1.p`. The expected verdicts come from the declared signatures alone, by matching a concrete stack against `S inputs -- S outputs`; they are never taken from the frontend. A finding is shrunk by removing words while the body keeps a self word and still shows the same disagreement.

| Bound | Value |
|---|---|
| words | the 13 words above, plus the self call `p` and the quoted self call `[ p ]` |
| bodies | 1 to 3 words with at least one self word: 1236 bodies |
| signatures | `forall<S:stack> [ S inputs -- S outputs ! pure ]` for 8 pairs: `--`, `-- I64`, `I64 --`, `I64 -- I64`, `Bool -- Bool`, `I64 -- I64 I64`, `I64 I64 -- I64`, `Bool I64 -- Bool I64` |
| declared instances | 6 pairs: `--` has the instances `I64 -- I64`, `Bool -- Bool` and `Bool I64 -- Bool I64`; `-- I64` has `I64 -- I64 I64`; `I64 --` has `I64 I64 -- I64`; `I64 -- I64` has `Bool I64 -- Bool I64` |
| input stacks | depth 0 to 3 over the same four value types: 85 stacks |

| Measure | Count |
|---|---|
| body/signature declarations | 9888 |
| accepted declarations | 229 |
| call comparisons | 19 465 |
| calls the declared signatures accept | 6857 |
| calls the frontend and kernel accept | 6857 |
| declared instances of an accepted signature | 150, all accepted |
| refusals at signatures that are not instances | 347 |
| discrepancies | 0 |

These counts are from `receipt-a1b2b39c.json`.

**Control.** `a_narrowed_signature_is_caught` declares `dup [ p ] drop` at `S I64 I64 -- S I64 I64 I64`. That is an instance of `S I64 -- S I64 I64`, where the body is also accepted. The control judges the narrow definition's calls against the general signature. It catches the `Bool I64` stack, which only the general signature accepts, and agrees wherever the top two values are `I64`. A frontend that fixed the row's top value would produce exactly this. The 347 refusals show that the instance closure is not vacuous.

## Limits

This is bounded evidence over the universe above, not a proof.
- **Constructibility.** The body route is the frontend's own inference at concrete types. An interface the kernel would accept, but that the frontend cannot construct even at concrete inputs, is outside the comparison. An independent enumerator of node instantiations would be needed to reach it.
- **Context bound.** Contexts are single words and cover programs of at most two words.
- **Signed recursion.** Declared signatures are row-polymorphic over `I64` and `Bool`, with no value-type variables. Self calls appear only as `p` and `[ p ]`. Unsigned recursion is refused by the ordinary profiles and is not compared.
- **Excluded features.** Effects are #28's oracle.
