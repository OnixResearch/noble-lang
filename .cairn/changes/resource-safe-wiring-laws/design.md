# Arrow-inspired, resource-safe wiring library and law suite: design

## Context

Issue #11 asks for a bounded library proposal and an evidence plan for wiring `Program<S,T,e>` values. It also asks for a law suite, a test and proof plan, an optional known-graph analysis, and packaging through #7's law bundles (per the issue comment). This change is design only. It changes no code, executes no scenario and promotes no proof. r[WIRE-EVID-01]

The issue links are pinned to `0ea10d3`. This design was verified against `ffcce53`, which is origin/main `44329dc` plus the format and pin commit.

### Citation drift

| Cited (0ea10d3) | Current (ffcce53) | Drift |
|---|---|---|
| `bootstrap.rs#L95-117` (`dip`) | `fn dip` L95-115; L117 is `fn arith` | cite overruns by 2 lines |
| `bootstrap.rs#L158-213` | `compose` L158-191, `run` L193-211 | none |
| `bootstrap/data.rs#L101-155` | `case` L101-131, `if_word` L133-158 | cite ends 3 lines early |
| `programs.wat#L22-76` | `$compose` L22-48, `$compose_entry` L55-58, `$op_run` L72-75 | none |
| `Composition.lean#L25-37` | docstring L24-27, `TypedPC.compose` L28-37 | starts inside the docstring |
| `Rules.lean#L396-434` | `pc_dip` L397, `pc_run` L408, `pc_if_join` L416, `pc_case_join` L424-432 | none |
| `language/spec.md#L1582-1601` | P-RECIPE-03 L1582, P-RECIPE-04 L1594-1604 | none |

### Baseline facts used here

| Fact | Source |
|---|---|
| Word schemes, bottom-to-top: `dup`/`drop`: `S A -- S A A` and `S A -- S` (where `Data(A)`); `swap: S A B -- S B A`; `dip: S A Program<S,T,e> -- T A ! e`; `quote: S A -- S Program<R,R A,{}>` (where `Capture(A)`); `compose: S Program<A,B,e> Program<B,C,f> -- S Program<A,C,e∪f> ! {}`; `run: S Program<S,T,e> -- T ! e` | `bootstrap.rs` L58-211; language spec §6.3-7.1 L786-1017 |
| `pair: S A B -- S Pair<A,B>`; `unpair` is its inverse; `inl`/`inr`; `case: S Sum<A,B> Program<S A,T,e> Program<S B,T,f> -- T ! e∪f`; `if: S Bool Program<S,T,e> Program<S,T,f> -- T ! e∪f`. These take no `Data` constraint, so they are move-only. | `bootstrap/data.rs` L17-158; spec §7.2-7.3 |
| The kernel has one eligibility predicate, `is_data`, checked only for `dup`, `drop` and `quote`. It rejects any value that contains `Resource` or `LiveRef`. `Program` values are `Data`. | `acceptance/parts/instantiate.rs` L49-95, L284-291; `types/data.rs` L10-62 |
| Source `signature` binders are only `stack`, `value` and `effect`. A signature names exactly one effect word: `pure`, `test.emit`, `test.clock`, or one variable. It cannot state `Data(A)` or `e∪f`. Eligibility is replayed at each instantiation. | `source/declared/signatures.rs` L118-130, L208-232; `source/inference/validation.rs` L1-3 |
| The language states composition laws only "where both executions terminate normally under the same host responses". They "do not equate elapsed time, allocation strategy, or behavior under different resource quotas". If `p` aborts, `q` in `compose(p,q)` does not start. | K-EVAL-04, spec L665-678 |
| Effect union is associative, commutative and idempotent only as a summary. "Execution order remains sequential and noncommutative." | spec §8.2 L1214-1216; VC-COMPOSE-02 |
| At runtime, `compose` checks complete signature IDs (status 3). It sets depth to `max(w)+1` and leaves to `n(l)+n(r)`, and applies the recipe-leaf quota (2) and depth quota (3). It schedules left before right using LIFO. Failure is sticky. `dip` restores its value after the program runs. | `programs.wat` L22-58; `data.wat` L26-31; `accounting.wat` L2-7; `common.wat` L12-13 (limits 256) |
| The backend admits no resource types. | `crates/noble-cli/src/core/runtime/abi.json` L99 |
| MC1 Lean is pure and covers normal returns only: no host operations (`host_unsupported`, `Rules.lean` L178), no resource types, no quotas. `dup` is typed without `Data`. No frame lemma exists. | `Model.lean` L3-21, L98-153; `Rules.lean` |
| There is no Noble prelude. `rot`/`unrot` exist only in calculator fixtures, and `result@1` is the only reserved library. | `fixtures/calculator-bits.noble` L55-63; `fixtures/result.noble` |

### Design-time observations (not evidence records)

These were run once with the prebuilt `noble-44329dc-release` binary (SHA-256 `e1fb70ba…fdc708127`) on throwaway files outside the repository. `wire.noble` is the module in Decision 1, with SHA-256 `96a5a269a5d227ac16f2e71b0d2aa7846576c4a11b2ff1bb2eefb49ceed9db28`. The resource fixture `ledger.noble` (`module ledger@1 [ opaque CounterOwner Resource<test.counter> private export CounterOwner ]`) has SHA-256 `37ce9e09…381125f0`.

These observations are not scenario executions or conformance evidence. Every WIRE-CASE remains `absent/not-run/open/unassessed`.

**O11-1. Identity and associativity, via `noble run`.**

| Program | Result | Steps | Allocated bytes |
|---|---|---|---|
| `5 [ 1 + ] run` | `6` | 17 | 480 |
| `5 [ ] [ 1 + ] compose run` | `6` | 25 | 768 |
| `5 [ 1 + ] [ ] compose run` | `6` | 25 | 768 |
| `5 [ 1 + ] [ 2 * ] compose [ 3 - ] compose run` | `9` | 38 | — |
| `5 [ 1 + ] [ 2 * ] [ 3 - ] compose compose run` | `9` | 39 | — |

`reflect` of both associativity groupings gives the identical six-node recipe. `[ 1 + ] [ ] compose` reflects the same recipe as `[ 1 + ]`.

**O11-2. Grouping changes accounting and failure class.** Chains of `k` compositions of `[ 1 + ]`, then `run`:

| `k` | Left-nested | Balanced |
|---|---|---|
| 16 | normal `17`, 178 steps | normal `17`, 182 steps |
| 32 | normal, 338 steps | normal, 343 steps |
| 40 | normal, 418 steps | normal, 423 steps |
| 48 | normal, 498 steps | normal, 503 steps |
| 56 | normal, 578 steps | normal, 583 steps |
| 63 | `runtime-exhausted` (`recipe leaves`) | static `exhausted` ("independent source compilation refused") |

**O11-3. Equal results, different recipes.** `[ 1 1 + ] reflect` gives a three-node recipe; `[ 2 ] reflect` gives a one-node recipe.

**O11-4. The `wire@1` module links, and its combinators run.** The module linked in `session --framed --declared-modules`, so the kernel accepted every exported body against its signature.

| Program | Result | Steps |
|---|---|---|
| `10 true pair [ 1 + ] w.first unpair` | `11 true` | — |
| `10 20 pair [ 2 * ] w.second unpair` | `10 40` | — |
| `10 20 pair [ 1 + ] [ 2 * ] w.split unpair` | `11 40` | 101 |
| `10 20 pair [ 1 + ] w.first [ 2 * ] w.second unpair` | `11 40` | 90 |
| `7 [ 1 + ] [ 2 * ] w.fanout unpair` | `8 14` | — |
| `5 inl [ 1 + ] w.left` | `Sum 6` | — |
| `5 inr [ 1 + ] [ 2 * ] w.choose` | `Sum 10` | — |
| `5 inl [ 1 + ] [ drop 0 ] w.join` | `6` | — |
| `5 inr [ 1 + ] w.right` | `Sum 6` | — |
| `5 inl [ 1 + ] w.right` | `Sum 5` | — |
| `1 2 pair 3 pair w.assoc_pair unpair unpair` | `1 2 3` | — |
| `1 2 pair w.project_first` | `1` | — |
| `5 w.identity [ 1 + ] w.then w.apply` | `6` | 34 |
| `5 [ 1 + ] w.identity w.then w.apply` | `6` | 34 |

In one long multi-submission session, `split` and `fanout` were refused with "independent source compilation refused" (`exhausted`). In fresh sessions they ran. [INFERENCE] A cumulative preparation limit causes this; no type error was involved.

**O11-5. Effect order with an identical effect bound.** `fx@1` is `require emit Text -- ! test.emit`, `def say [ emit unit ]`, bound `allow`.

| Program | Trace |
|---|---|
| `"L" "R" pair [ f.say ] w.first [ f.say ] w.second` | `L, R` |
| `"L" "R" pair [ f.say ] w.second [ f.say ] w.first` | `R, L` |
| `[ w.left ] [ w.right ] compose run`, where `w` aliases a separate emitter module `wires@1` (`def left [ "L" emit ]`, `def right [ "R" emit ]`) | `L, R` |
| the same, reversed | `R, L` |

Both product orders return `Pair<Unit,Unit>`. With effectful operands, `w.split` was refused at preparation ("independent source compilation refused", `exhausted`). [INFERENCE] This is a preparation limit, not a type error; it is listed under Risks.

**O11-6. Denied request.** With the binding set to `deny`, `[ "L" f.say drop ] [ "R" f.say drop ] compose run` traps with status 5. The trace is `["L:deny"]`. `R` never starts. The session state is `terminated-after-runtime-failure`.

**O11-7. Resource eligibility,** via `compile --declared-modules --input-type ledger@1.CounterOwner`.

- `dup`, `drop`, `quote`, `[ ] [ ] wire@1.fanout` and `unit swap pair wire@1.project_first` are each rejected at kernel acceptance: "kernel rejected a non-capturable value; word dup|drop|quote; required S Data".
- `[ ] wire@1.under`, `unit pair [ ] wire@1.first`, `unit pair wire@1.swap_pair`, `unit swap pair [ ] [ ] wire@1.split` and even the empty program produced no eligibility rejection. They were then refused: "unsupported … independent source compilation refused". This is consistent with `abi.json` L99.

**O11-8. Static rejections.**

| Program | Rejected by |
|---|---|
| `5 [ 1 + ] [ [ 1 ] [ 2 ] if ] compose run` | `compose` (intermediate `I64` versus `Bool`) |
| `4 true [ 1 + ] [ drop true ] if` | `if` (branch joins `I64` versus `Bool`) |
| `5 inl [ 1 + ] [ drop true ] case` | `case` |

Each is rejected as `type-reject` "stack/program join".

## Decisions

### Decision 1: `wire@1` library vocabulary

**Choice:** Specify a small library module whose every word is derived from existing builtins. The sketch below is the exact text that linked in O11-4. Its signatures use the source syntax at `44329dc`, with one shared effect variable per word. r[WIRE-API-01]

```noble
module wire@1 [
    signature rot forall<S:stack,A:value,B:value,C:value>
        [ S A B C -- S B C A ! pure ]
    def rot [ [ swap ] dip swap ]

    signature identity forall<S:stack,R:stack> [ S -- S Program<R,R,pure> ! pure ]
    export identity
    def identity [ [ ] ]

    signature then forall<S:stack,A:stack,B:stack,C:stack,eps:effect>
        [ S Program<A,B,eps> Program<B,C,eps> -- S Program<A,C,eps> ! pure ]
    export then
    def then [ compose ]

    signature apply forall<S:stack,T:stack,eps:effect> [ S Program<S,T,eps> -- T ! eps ]
    export apply
    def apply [ run ]

    signature under forall<S:stack,T:stack,A:value,eps:effect> [ S A Program<S,T,eps> -- T A ! eps ]
    export under
    def under [ dip ]

    signature first forall<S:stack,A:value,B:value,C:value,eps:effect>
        [ S Pair<A,B> Program<S+A,S+C,eps> -- S Pair<C,B> ! eps ]
    export first
    def first [ swap unpair rot dip pair ]

    signature second forall<S:stack,A:value,B:value,D:value,eps:effect>
        [ S Pair<A,B> Program<S+B,S+D,eps> -- S Pair<A,D> ! eps ]
    export second
    def second [ swap unpair swap rot dip swap pair ]

    signature split forall<S:stack,A:value,B:value,C:value,D:value,eps:effect>
        [ S Pair<A,B> Program<S+A,S+C,eps> Program<S+B,S+D,eps> -- S Pair<C,D> ! eps ]
    export split
    def split [ [ first ] dip second ]

    signature fanout forall<S:stack,A:value,C:value,D:value,eps:effect>
        [ S A Program<S+A,S+C,eps> Program<S+A,S+D,eps> -- S Pair<C,D> ! eps ]
    export fanout
    def fanout [ [ [ dup pair ] dip ] dip split ]

    signature left forall<S:stack,A:value,B:value,C:value,eps:effect>
        [ S Sum<A,B> Program<S+A,S+C,eps> -- S Sum<C,B> ! eps ]
    export left
    def left [ [ inl ] compose [ inr ] case ]

    signature right forall<S:stack,A:value,B:value,D:value,eps:effect>
        [ S Sum<A,B> Program<S+B,S+D,eps> -- S Sum<A,D> ! eps ]
    export right
    def right [ [ inr ] compose [ inl ] swap case ]

    signature choose forall<S:stack,A:value,B:value,C:value,D:value,eps:effect>
        [ S Sum<A,B> Program<S+A,S+C,eps> Program<S+B,S+D,eps> -- S Sum<C,D> ! eps ]
    export choose
    def choose [ [ inr ] compose [ [ inl ] compose ] dip case ]

    signature join forall<S:stack,A:value,B:value,T:stack,eps:effect>
        [ S Sum<A,B> Program<S+A,T,eps> Program<S+B,T,eps> -- T ! eps ]
    export join
    def join [ case ]

    signature swap_pair forall<S:stack,A:value,B:value> [ S Pair<A,B> -- S Pair<B,A> ! pure ]
    export swap_pair
    def swap_pair [ unpair swap pair ]

    signature assoc_pair forall<S:stack,A:value,B:value,C:value>
        [ S Pair<Pair<A,B>,C> -- S Pair<A,Pair<B,C>> ! pure ]
    export assoc_pair
    def assoc_pair [ unpair [ unpair ] dip pair pair ]

    signature project_first forall<S:stack,A:value,B:value> [ S Pair<A,B> -- S A ! pure ]
    export project_first
    def project_first [ unpair drop ]
]
```

The table below gives each word's role, its *intended* signature, its eligibility and its order. In the intended signatures, `e∪f` is the general bound. Source signatures cannot express `e∪f` today ("unknown signature effect variable"), so the checked sketch instantiates both program bounds at one `eps`. Literal quotations widen to `eps` conservatively (K-EFFECT-02). A runtime-selected Program with a different fixed bound needs an explicit K-EFFECT-05 adaptation first. The builtins `compose`, `case` and `if` keep the full union. Effect-union signature syntax is a future implementation gate.

| Word | Arrow analog | Intended signature (bottom → top) | Derivation | Eligibility | Evaluation order |
|---|---|---|---|---|---|
| `identity` | `id` | `S -- S Program<R,R,{}> ! {}` | `[ ]` | none | none (construction is pure) |
| `then` | `>>>` | `S Program<A,B,e> Program<B,C,f> -- S Program<A,C,e∪f> ! {}` | `compose` | none. The intermediate stack must unify completely (K-PROG-05). | at run time, left then right; the right does not start if the left aborts (K-EVAL-04) |
| `apply` | `app` (ArrowApply) | `S Program<S,T,e> -- T ! e` | `run` | none | invokes the program now |
| `under` | frame | `S A Program<S,T,e> -- T A ! e` | `dip` | none. `A` is moved, inaccessible to the program, and restored exactly once on normal return (K-STACK-01). | the program, then the restore |
| `first` | `first` | `S Pair<A,B> Program<S A,S C,e> -- S Pair<C,B> ! e` | `swap unpair rot dip pair` | none (`B` is moved) | the program, then the repair |
| `second` | `second` | `S Pair<A,B> Program<S B,S D,f> -- S Pair<A,D> ! f` | `swap unpair swap rot dip swap pair` | none | the program, then the repair |
| `split` | `***` (two owned inputs) | `S Pair<A,B> Program<S A,S C,e> Program<S B,S D,f> -- S Pair<C,D> ! e∪f` | `[ first ] dip second` | none; two independent owners | all of `p`, then all of `q`. Never interleaved or parallel. |
| `fanout` | `&&&` (one shared input) | `S A Program<S A,S C,e> Program<S A,S D,f> -- S Pair<C,D> ! e∪f` | `[ [ dup pair ] dip ] dip split` | **`Data(A)`**, checked at each specialization; a resource is refused (O11-7) | `dup`, then `p`, then `q` |
| `left` | `left` | `S Sum<A,B> Program<S A,S C,e> -- S Sum<C,B> ! e` | `[ inl ] compose [ inr ] case` | none; the payload is moved | the program runs only on `inl` |
| `right` | `right` | `S Sum<A,B> Program<S B,S D,f> -- S Sum<A,D> ! f` | `[ inr ] compose [ inl ] swap case` | none | the program runs only on `inr` |
| `choose` | `+++` | `S Sum<A,B> Program<S A,S C,e> Program<S B,S D,f> -- S Sum<C,D> ! e∪f` | `[ inr ] compose [ [ inl ] compose ] dip case` | none. The unselected program is discarded unrun; this is safe because program environments are `Capture` (§7.2). | exactly one branch |
| `join` | `\|\|\|` | `S Sum<A,B> Program<S A,T,e> Program<S B,T,f> -- T ! e∪f` | `case` | none | exactly one branch |
| `swap_pair`, `assoc_pair` | product routing | `S Pair<A,B> -- S Pair<B,A>`; `S Pair<Pair<A,B>,C> -- S Pair<A,Pair<B,C>>` | `unpair swap pair`; `unpair [ unpair ] dip pair pair` | none (moves only) | — |
| `project_first` | `arr fst` | `S Pair<A,B> -- S A ! {}` | `unpair drop` | **`Data(B)`**; a resource is refused (O11-7) | — |
| `branch` | (ArrowChoice via `Bool`) | the builtin `if` | no wrapper | none | exactly one branch |
| constant entry | `arr (const v)` | `S A -- S Program<R,R A,{}>` | `quote` | **`Capture(A)`**; a resource is refused (O11-7) | — |

How pure checked Programs enter the library: there is **no** `arr : function → Program`. A Program enters as one of the following:

- a quotation literal, whose whole body is checked (K-PROG-03);
- a named definition inside a quotation, such as `[ m.f ]`, whose recipe is a reference and is not inlined;
- `quote` of a `Capture` value;
- a value produced by `compose`, `case` or `if`.

No host function, closure over the live stack (K-PROG-02) or captured resource (K-PROG-08) can become a Program.

Routing two independently owned inputs (`split`) is resource-safe. Fanning out one input (`fanout`) is not, because it requires `Data` on everything duplicated. A projection that discards needs `Data` on the discarded value. Constant lifting needs `Capture`. Moving or framing a resource through `under`, `first`, `second`, `split`, `swap_pair`, `assoc_pair` or `choose` keeps its identity and single owner. These words never copy, discard, release or capture it (K-RES-01/02, K-DATA-01).

Because a signature cannot state `Data(A)`, eligibility is enforced at each specialization. The kernel still enforces it: O11-7 shows `fanout` instantiated at a resource is rejected at its internal `dup`. Diagnostics should name the library word as well as `dup`; that is an implementation gate.

**Rationale:** Every combinator is a checked composition of existing words. Its signature, effects and eligibility therefore follow from the kernel, not from library claims. This avoids a second trusted vocabulary.

### Decision 2: Observation relations and the law matrix

**Choice:** Every law names its relation, its premises and its supported profile. r[WIRE-LAW-01]

The observation relations are:

| Id | Relation |
|---|---|
| **N** | Normal-return result equivalence: equal final stacks on normal return, for every input; partial correctness, as in MC1 `Exec`/`PC` (`Rules.lean` L366) |
| **R** | Equality of the specified recipe observation (`reflect`, P-RECIPE-01..04) |
| **T** | Ordered host-request traces over every finite prefix, including denied and failed requests (V-EFFECT-01, V-MODEL-04) |
| **O** | Resource ownership transitions and multiplicity (V-RESOURCE-01) |
| **F** | Failure class and cleanup responsibility: trap, cancel, quota; guest versus host (K-EVAL-04, K-RES-04, V-RESOURCE-03) |
| **Q** | Quota and accounting: steps, allocation, recipe leaves and depth |
| **ID** | Definition and program-value identity, and proof-subject identity (P-ID-06, VC-ID-01/02) |

Laws are stated **modulo quota**, following K-EVAL-04. A claim covers the case where both sides terminate normally under the same host responses. If either side exhausts a quota, no equality is claimed.

Legend:

- ✓spec: specified.
- ✓mc1: proved in the MC1 model with existing rules.
- ✓obs: observed once (design-time only).
- ✗: fails, with the witness given.
- ?: open, because the model is missing.
- n/a: not applicable.

| # | Law (stack form) | Premises | N | R | T | O | F | Q | ID / subject |
|---|---|---|---|---|---|---|---|---|---|
| L1 | left identity: `[ ] p compose` ≈ `p` | typed `p` | ✓mc1 (`exec_nil_iff`, `exec_append_iff`) | ✓spec (P-RECIPE-03), ✓obs (O11-1) | ✓spec (K-EVAL-04) | n/a (pure); ? otherwise | ✓spec | ✗ (17 vs 25 steps; extra cell; depth +1) | Same modeled code list (`List.nil_append`). The executed cell differs, so #10 is needed. |
| L2 | right identity: `p [ ] compose` ≈ `p` | as L1 | ✓mc1 | ✓spec, ✓obs | ✓spec | as L1 | ✓spec | ✗ (O11-1) | as L1 |
| L3 | associativity | `B`, `C` unify | ✓mc1 (`List.append_assoc`) | ✓spec, ✓obs | ✓spec: `p,q,r` in order both ways | ? | ✓spec: abort of `p` stops `q,r` | ✗ (38 vs 39; O11-2 changes the failure class at k=63) | as L1 |
| L4 | apply: `v quote run` ≈ (push `v`); `p q compose run` ≈ `p run` then `q run` | `Capture(v)` | ✓mc1 (`exec_quote_run_iff` L361, `exec_compose_run_iff` L353) | n/a (consumed) | ✓spec | ? | ✓spec | ✗ | — |
| L5 | frame fusion: `[ p ] under [ q ] under` ≈ `[ p q ] under` | the held `A` is untouched | ✓mc1 (`exec_dip_iff` L308 + append) | ✗ (different recipes) | ✓spec | ✓spec (K-STACK-01; restored once); model ? | ✓spec: on abort, `A` is retired by the host, not resurrected | ✗ | different subjects |
| L6 | pair routing: `pair unpair` ≈ `[ ]`; `unpair pair` ≈ `[ ]`; `swap_pair swap_pair` ≈ `[ ]`; `assoc_pair` has an inverse | — | ✓mc1 (`pair_unpair_exec_iff`, Examples L138; `swap_iff`) | ✗ | ✓ (no effects) | ✓spec (K-DATA-01 moves); model ? | ✓ | ✗ | different subjects |
| L7 | first fusion: `[p] w.first [q] w.first` ≈ `[p] [q] compose w.first` | — | derivable from L3 and L5 (new lemma) | ✗ | ✓spec | ? | ✓spec | ✗ | different |
| L8 | split definition: `[p] [q] w.split` ≈ `[p] w.first [q] w.second` | — | ✓ by unfolding (`split`'s body) | ✗ | ✓spec (`p` then `q`) | ? | ✓spec | ✗ (101 vs 90 steps, O11-4) | different definition identity |
| L9 | **exchange (not adopted)**: `[p] w.first [q] w.second` ≈ `[q] w.second [p] w.first` | — | ✓ only if `e = f = {}` | ✗ | **✗** when both are effectful (O11-5: `L,R` vs `R,L`) | ? | **✗** if either side can fail, including pure quota (K-ERROR-03) | ✗ | unsupported under T, F and Q |
| L10 | fanout: `x [p] [q] w.fanout` ≈ `x dup pair [p] [q] w.split` | **`Data(A)`** | ✓ by unfolding | ✗ | ✓spec (`p` then `q`) | for a resource `A`: refused (O11-7) | ✓spec | ✗ | — |
| L11 | choice β: `v inl [p] w.left` ≈ `v p inl`; `v inr [p] w.left` ≈ `v inr` | — | ✓mc1 (`exec_case_left_iff`/`right_iff` L325/330) | ✗ | ✓spec (only the selected branch runs) | ? | ✓spec | ✗ | — |
| L12 | join naturality: `s [p] [q] w.join r` ≈ `s [p r] [q r] w.join` | the shared join type `T` | ✓mc1 (`pc_case_join` L424 form) | ✗ | ✓spec (`r` after either branch) | ? | ✓spec | ✗ | — |
| L13 | branch β: `true [p] [q] if` ≈ `p`; `false [p] [q] if` ≈ `q` | — | ✓mc1 (`exec_if_true_iff`/`false` L335/339; `pc_if_join` L416) | ✗ | ✓spec | ? | ✓spec | ✗ | — |
| L14 | **behavioral ≠ identity**: `[ 1 1 + ]` vs `[ 2 ]` | — | ✓ (same normal result) | **✗** (O11-3) | ✓ | n/a | ✓ | ✗ | **different** recipes and definitions (P-RECIPE-04). This is no contextual equivalence when `reflect` is present (V-OBS-01). |

Three Haskell Arrow laws are classified, not imported:

- `arr id = id` and `arr (f >>> g) = arr f >>> arr g`: n/a, because there is no `arr`.
- `first f >>> arr fst = arr fst >>> f`, i.e. `[p] w.first w.project_first` ≈ `w.project_first p`: N ✓ only with `Data(B)`. Q ✗. Refused for a resource `B`.
- `first (first f) >>> arr assoc = arr assoc >>> first f`, using `assoc_pair`: N ✓ (moves only). R ✗.

Strength and scope:

- Every N entry is normal-return partial correctness. Termination is never claimed.
- T and F claims are about finite prefixes. They are only ✓spec until a trace model exists (Decision 5).
- An effect bound such as `{test.emit}` is the same for both sides of L9. That equality of bounds is not evidence of commutation (§8.2, VC-COMPOSE-02).
- Product and fanout evaluation order is fixed as in Decision 1. No law licenses swapping, duplicating or parallelizing effects.

Behavioral equivalence is separate from identity:

- Only L1-L3 preserve the recipe observation (R), by the specified P-RECIPE-03 normalization.
- No law makes two different definitions or proof subjects identical.
- Any reuse of evidence across an N-law requires the #7 checked derivation bound to the new subject. The only exception is where the modeled subject is literally equal (L1-L3 in MC1).

### Decision 3: Worked examples

1. **Pure pipeline (O11-1, O11-4).** `5 [ 1 + ] [ 2 * ] compose [ 3 - ] compose run` gives `9` under both groupings, with the same recipe and 38 vs 39 steps. The library form `5 w.identity [ 1 + ] w.then w.apply` gives `6`.

   The checked claim for `[ 1 + ] [ 2 * ] compose [ 3 - ] compose` is that for every tail and every `x`, a normal return gives `wrap64(wrap64((x+1)*2)-3)`. It comes from `TypedPC.compose` twice, with exact bridges. The proof is planned, not run.

2. **Resource-preserving frame (design-only; not executable at `44329dc`).** `n r [ 1 + ] w.under` with `r : ledger@1.CounterOwner` instantiates `under` at:

   `S I64 CounterOwner Program<S I64,S I64,pure> -- S I64 CounterOwner ! pure`

   The handle is moved out of the program's view and restored exactly once after normal return. The program cannot capture it (K-PROG-08). On abort, the host retires it idempotently and it is not resurrected (K-STACK-01, K-RES-04). No `Data` or `Capture` is required.

   Today the kernel reports no eligibility rejection, but the backend refuses resource inputs (O11-7; `abi.json` L99). The example therefore stays separate from the first bundle (Decision 7), until the ownership model (PO-05, V-RESOURCE-01..03) and resource-admitting execution (m6-style cases) exist.

3. **Sum branch (O11-4).**
   - `5 inl [ 1 + ] w.left` gives `inl 6`.
   - `5 inl [ 1 + ] w.right` gives `inl 5` unchanged.
   - `5 inr [ 1 + ] [ 2 * ] w.choose` gives `inr 10`.
   - `5 inl [ 1 + ] [ drop 0 ] w.join` gives `6`.
   - The raw form `true inr [ 1 + ] [ drop 0 ] case` gives `0`.

   Only the selected branch's effects can occur. Both branches contribute to the bound (K-CONTROL-01).

4. **Ordered effects (O11-5, O11-6).** `"L" "R" pair [ f.say ] w.first [ f.say ] w.second` emits `L, R`. The exchanged order emits `R, L`. Both have effect bound `{test.emit}` and return `Pair<Unit,Unit>`, which refutes L9 under T. With `deny`, the first request traps (status 5), the right program never starts, and earlier effects are not rolled back (K-EVAL-04).

5. **Rejected resource fanout (O11-7).** `[ ] [ ] wire@1.fanout` with input `ledger@1.CounterOwner` is rejected at kernel acceptance: "word dup; required S Data". The same happens for `project_first` discarding a resource (`drop`) and for `quote` of a resource (`Capture`).

### Decision 4: Executable-law test plan

**Choice:** Specify a corpus of unexecuted designs, WIRE-CASE-01 to WIRE-CASE-24. Each case is a separate submission per law side and records:

- the profile: `Core-Bootstrap` through `noble run`, or `Declared-Modules-v1` through `session --framed`/`compile`;
- the exact source units, the bindings file and the inputs;
- the expected observations, stated **independently from the specification by a reviewer** before any run, never captured from an implementation run;
- the comparator relation, and the observation fields compared versus ignored.

For example, N-laws compare `stack` and ignore `metrics.execution_steps`. Q-cases assert only a classification, never step equality. r[WIRE-EVID-01]

Reproducibility comes from:

- pinned flake and toolchain, plus a binary digest;
- runtime pins (`crates/noble-cli/src/core/runtime/config.json`: Node, wasm-tools, binaryen);
- fixed inputs, including `I64` boundaries `-9223372036854775808`, `-1`, `0` and `9223372036854775807`.

The optional property lane follows DX-PROPERTY-01/02: it records the seed, generator and shrinker revisions and bounds. Well-typed candidates go through acceptance, and malformed fuzzing is a separate lane. Every case keeps independent implementation, execution, proof and trust fields (EV-STATE-01).

| Case | Kind | Input (sketch) | Independent expectation |
|---|---|---|---|
| WIRE-CASE-01 | L1/L2 | `x [ ] [ 1 + ] compose run` and variants, for each boundary `x` | stack `wrap64(x+1)` on both sides; equal recipe after `reflect`; steps ignored |
| WIRE-CASE-02 | L3 | three groupings of `[1 +] [2 *] [3 -]` | `wrap64(wrap64((x+1)*2)-3)`; identical recipe |
| WIRE-CASE-03 | L3/Q | left-nested versus balanced chains at the reviewed `k` | equal N where both are normal. Classification mismatches near the limits are *allowed and recorded*, not counted as law failures. |
| WIRE-CASE-04 | L4 | `v quote run`; `p q compose run` vs `p run q run` | equal stacks |
| WIRE-CASE-05 | L5 | `x y [p] w.under [q] w.under` vs `x y [p q] w.under` | equal stacks; `y` restored once |
| WIRE-CASE-06 | L6 | `pair unpair`, `swap_pair` twice, `assoc_pair` with its inverse | the original stack |
| WIRE-CASE-07 | L7/L8 | `first` fusion; `split` vs `first`/`second` | equal pairs; ordered traces when effectful |
| WIRE-CASE-08 | L10 | `x [p] [q] w.fanout` vs `x dup pair [p] [q] w.split` (I64 `x`) | equal pairs |
| WIRE-CASE-09 | L11/L13 | both choice branches for `left`, `right`, `choose`, `if` | exact selected results; zero requests from the unselected branch |
| WIRE-CASE-10 | L12 | join naturality, both branches | equal stacks |
| WIRE-CASE-11 | ordered effects | Decision 3 example 4, both orders | traces `L,R` and `R,L`; the T relation is reported as **not equal** |
| WIRE-CASE-12 | L14 | `[ 1 1 + ]` vs `[ 2 ]` | N equal; R and ID different |
| WIRE-CASE-13 | negative: resource fanout | `fanout` at `CounterOwner` | kernel reject at `dup`; zero guest requests |
| WIRE-CASE-14 | negative: resource discard | `project_first` with resource `B`; `drop` | kernel reject at `drop` |
| WIRE-CASE-15 | negative: resource capture | `quote` of `CounterOwner`; a quotation literal that mentions a live value | kernel reject |
| WIRE-CASE-16 | negative: incompatible intermediate | O11-8 `compose` | `type-reject` before execution |
| WIRE-CASE-17 | negative: wrong branch join | O11-8 `if`/`case` | `type-reject` |
| WIRE-CASE-18 | negative: stale or wrong-subject evidence | a bundle law proved for builtin `compose` used for `def compose2 [ compose ]`; a `[ 1 1 + ]` claim reused for `[ 2 ]` | refused by #7 bundle and subject checks |
| WIRE-CASE-19 | ordinary error | `result@1.map_ok` through `w.then` on `Err` | normal return carrying `Err`; not a trap |
| WIRE-CASE-20 | trap | Decision 3 example 4 with `deny`; `test.abort` (status 6) mid-`split` | the trap status; trace prefix only; the right side never starts; the session is terminated |
| WIRE-CASE-21 | quota exhaustion | O11-2 at `k=63`; step quota mid-`fanout` | the failure classification and stage; no false normal result |
| WIRE-CASE-22 | cancellation | the m6 `future-cancel` pattern wrapped in `w.under` (component profile) | host cancel; guest not resumed; the held handle retired by the host |
| WIRE-CASE-23 | resource frame | the m6 `trapped-resource` pattern (`counters.transfer 1 counters.transfer`) with `under` | on trap, the host retires invocation-owned handles idempotently; no stale handle is exposed |
| WIRE-CASE-24 | exchange refusal | L9 with one pure-but-quota-failing side | different trace prefixes; the law is reported unsupported |

Cleanup responsibilities:

| Party | Responsibility |
|---|---|
| Guest | On every normal path, move, consume or release each handle (K-RES-01). The library cannot catch traps and adds no cleanup. |
| Host | On abort, retire invocation-owned handles idempotently without resuming the guest (K-RES-04). Report the actual effect prefix. Earlier effects are not rolled back (K-EVAL-04). |
| Library | No cleanup role. |

Passing any subset of this corpus is finite regression evidence. It is never reported as a law proof (V-STATUS-01, EV-GATE-02).

### Decision 5: Proof plan

**Choice:** Reuse existing reviewed rules where they exist. Every gap is listed as a new obligation. r[WIRE-EVID-01]

| Law | Existing rules reused | New theorem obligations | Model, implementation and correspondence obligations |
|---|---|---|---|
| L1-L3 | `exec_nil_iff` L214, `exec_append_iff` L219, `List.append_assoc` (L5 attribute), `TypedPC.compose` and `CodeTyped.append` (`Composition.lean` L6-37), `exec_compose_run_iff` L353 | `Wire.then_left_id`, `then_right_id` (need `List.nil_append`/`append_nil` whitelisted), `then_assoc` as `PC`/`TypedPC` equivalences | R needs PO-08 (recipe normalization) and refinement of `$concat_recipe` (`programs.wat` L5-10, PO-12/17). T and F need a trace model. |
| L4 | `exec_quote_run_iff` L361, `exec_run_iff` L273, `pc_run` L408 | — | `Capture` premise: PO-06 |
| L5 | `exec_dip_iff` L308, `pc_dip` L397 | `Wire.under_fuse`; a general frame lemma `Run p s t → Run p (s++r) (t++r)`. [INFERENCE] It is provable by induction; it is absent today. | O: ownership model PO-05, V-RESOURCE-01..03, SO-04; K-STACK-01 restore-once |
| L6 | `pair_iff`/`unpair_iff` L99/105, `exec_pair_iff`/`exec_unpair_iff` L284/288, `pair_unpair_exec_iff` (Examples L138), `swap_iff` L33 | `Wire.swap_pair_invol`, `assoc_pair_iso` | O: K-DATA-01 move preservation in the model |
| L7, L8, L10 | the above, applied to the library definitions | statements over named `wire@1` definitions. Two routes: (a) inlined code in MC1; (b) #7 revision-3-style contract calls once Program-typed slots are supported. Route (b) is outside the first #7 profile. | the kernel `is_data` replay at specialization (`instantiate.rs`) has no refinement proof; PO-11 covers only the bootstrap checker v1 |
| L9 | — | a counterexample theorem under T once a trace model exists | trace model: PO-04, SO-03, V-EFFECT-01/02, V-MODEL-04 |
| L11-L13 | `exec_case_left_iff`/`right_iff` L325/330, `pc_case_join` L424, `exec_if_true_iff`/`false` L335/339, `pc_if_join` L416 | `Wire.left_beta`, `join_natural` | — |
| L14 | `reflect_iff` L88 (recipe observed) | `Wire.recipe_distinct` (`[1 1 +]` ≠ `[2]` as recipes) | V-OBS-01 contextual relation with `reflect` |
| All | — | — | kernel schemes vs Lean `OpTyped` (PO-11 bootstrap v1 only); runtime `compose`/`run`/`dip` vs model (PO-17, PO-18); executed artifact (#10); quota semantics (V-MODEL-03, `safety-runtime-quotas`) |

The first bounded proof fragment is pure MC1 with N only, covering L1-L6 and L11-L13. Effects, resources, failure and quota extensions remain explicit obligations. They are not extended by analogy.

### Decision 6: Optional known-graph profile and one candidate optimization

**Choice:** `Known-Graph-Draft` is an optional analysis profile. It never restricts ordinary execution. r[WIRE-GRAPH-01]

**Eligibility.** A Program qualifies when all of the following hold:

- its finite recipe (P-RECIPE-01) has a fully resolved closure of immutable named definitions;
- every `run`, `dip`, `case`, `if` or `list.case` operand is a statically known recipe node: a literal quotation, a known-graph definition, or a `compose`, `case` or `if` of such nodes;
- no control flow depends on `reflect`. `Syntax` cannot become callable without validation (SO-05);
- every capture is `Capture` data;
- the recipe leaf count and depth are within the declared limits.

**When the check runs.**

- **Statically,** after kernel acceptance of a source definition.
- **For a runtime-constructed Program,** only by an *explicit* host analysis over its reflected recipe after construction, once its closure is known. The result is bound to the exact recipe and value identity. The check never runs inside ordinary `quote`, `compose` or `run` (V-CORE-01, VC-BUILD-02).

An unsupported Program stays an ordinary Program and runs normally.

**Analyses.** Stack shape per node, the sequential effect-order graph, resource move paths, and recipe size and depth against `recipe_limit`/`depth_limit`. A known graph does **not** establish termination, GPU eligibility (#2), numeric-graph eligibility (#14) or profitable parallelism.

**Candidate optimization: compose-tree flattening.** This means executing a known-graph left-nested `compose` chain as one flat sequence instead of the runtime tree (`programs.wat` L22-58).

| Concern | Analysis |
|---|---|
| Reflection | The recipe is already normalized by grouping (P-RECIPE-03; O11-1). Flattening must leave `reflect` byte-identical and must not change any intermediate Program value that escapes (for example through `dup` or storage). Escaping intermediates disqualify the chain. |
| Proof identity | The *logical* subject is unchanged: the MC1 code list is equal by `List.append_assoc`. Existing evidence stays bound to the same subject, but the *executed artifact* changes. Admission therefore needs semantic and artifact correspondence or translation validation (#10, PO-17), not a new proof. By contrast, a rewrite that changes the source recipe creates a new subject. An example is fusing `pair unpair` into `[ ]` (L6). Such a rewrite needs fresh evidence, or a checked transformation rule that names its relation (VC-REWRITE-01/02). Equal results or hashes transfer nothing. |
| Ordering | Left-to-right order and "the right side does not start after the left aborts" (K-EVAL-04) must be preserved. Flattening crosses no effect boundary, but abort propagation must be validated per artifact. |
| Quota | Accounting changes: steps differ (O11-1, O11-2), allocation differs, and depth is `max+1` per node. The *failure class can change* (O11-2 at `k=63`: runtime versus static exhaustion). The first profile therefore admits flattening only when static analysis shows both forms within the recipe and depth limits. Step counts are declared not preserved (Q ✗). The optimization is refused under any policy whose claim includes step quotas (H-LIMIT-01). |
| Rejection cases | unknown leaves; escaping intermediate Programs; effectful leaves without validated abort propagation; quota-bearing claims; any change to `reflect` output |

### Decision 7: Law-bundle packaging

**Choice:** Use the #7 schema (`reusable-contract-calls`, Decision 11) for the first pure bundle. Keep the frame bundle separate. r[WIRE-LAW-01]

**`wire@1.seq-laws`** is the first bundle. It is pure and covers normal return only.

| Field | Value |
|---|---|
| params | `p : Program<A,B,{}>`, `q : Program<B,C,{}>`, `r : Program<C,D,{}>`, with complete ordered interfaces `A→B→C→D` |
| operations | builtin `compose` (word 9; scheme `bootstrap.rs` L158-191); identity `[ ]`. `w.then` and `w.identity` are *distinct* operation identities. A bundle entry for them needs its own checked derivation, for example through the #7 call rule, since `then`'s body is exactly `compose`. |
| laws | `left-id` (`List.nil_append` + `exec_append_iff`); `right-id` (`List.append_nil` + `exec_append_iff`); `assoc` (`List.append_assoc` + `exec_append_iff`); typing by `TypedPC.compose`/`CodeTyped.append` |
| profile | MC1-v1 model revision digest, checker revision, rule-library revision |
| dependency closure | `Rules.exec_nil_iff`, `Rules.exec_append_iff`, `Composition.TypedPC.compose`, `CodeTyped.append`, plus the three `List` lemmas (two pending whitelisting) |
| observation | N; recipe scope "specified (P-RECIPE-03), not proved (PO-08)" |
| scope | normal return established; traces, resources, failure and quota not established |
| assumptions | none |
| resource premises | none (pure carrier) |

**Worked consumer.** The proof syntax is proposed and unchecked, as in #7.

```text
(law w.seq-laws assoc p q r)
  p = [ 1 + ]  ⇒ code [.lit 1, .word 4]
  q = [ 2 * ]  ⇒ code [.lit 2, .word 6]
  r = [ 3 - ]  ⇒ code [.lit 3, .word 5]
displayed instantiated law:
  Exec (([.lit 1, .word 4] ++ [.lit 2, .word 6]) ++ [.lit 3, .word 5]) s t
    ↔ Exec ([.lit 1, .word 4] ++ ([.lit 2, .word 6] ++ [.lit 3, .word 5])) s t
bindings: operation=builtin compose (word 9); theorem=List.append_assoc@pinned Lean core
          via Rules.exec_append_iff; profile=MC1-v1 <model digest>; observation=N;
          assumptions=none; resource premises=none
```

Required refusals (designed, unexecuted; WIRE-CASE-18 and the #7 bundle cases):

| Refusal | Example |
|---|---|
| wrong operation revision | the law applied to `wire@2.then` with a different body, or to `compose` under a different `SEMANTIC_REVISION` |
| different observation relation | a consumer that needs T for effectful `p`/`q`/`r` |
| absent resource premises | a consumer that instantiates a frame law (`under_fuse`) at `CounterOwner` while no frame bundle exists |
| ambiguous selection | `wire@1.seq-laws` and `alt@1.seq-laws` are both visible and unqualified |

A bundle never grants `Data` or `Capture`: a fanout-law instance at a resource is still refused by the kernel. It never commutes ordered effects: L9 is not packaged under T. It never transfers exact-subject evidence on equal results: L14.

**`wire@1.frame-laws`** (L5, L6 with resources) stays **unpackaged**. Its required premises are listed here and are not yet supported:

- move-only use;
- K-STACK-01 restore-once;
- K-RES-04 host retirement;
- an ownership-model theorem (PO-05);
- resource-admitting execution evidence.

## Coordination

- **#2 (GPU) and #14 (numeric graph):** the Decision 6 analysis is only an input to their separate eligibility and cost models.
- **#3 (ownership):** the frame bundle waits on ownership-aware interfaces and lifetimes.
- **#6 `proof-automation-diagnostics`:** reviewed templates for the wiring laws come only from this suite, after review.
- **#7 `reusable-contract-calls`:** bundle schema, envelope and call rule. Library-level contract reuse with Program-typed slots is a future #7 extension.
- **#9 `behavioral-effect-protocol`:** the T and F relations and cleanup.
- **#10 `artifact-correspondence-extension`:** the flattening optimization and every executed-artifact claim.
- **#12 `implicit-proof-arguments`:** bundle arguments are explicit until #12 exposes them.
- **#15 (math foundation):** the frame lemma and L1-L6 are first targets.

## Risks / Trade-offs

- The one-effect-slot signature syntax makes `then`, `split`, `fanout`, `choose` and `join` less general than the builtins. Adding union syntax touches the checker and needs its own acceptance work.
- Effectful `split` was refused at preparation in this build (O11-5), and long sessions refused `split`/`fanout` (O11-4). The cause is unconfirmed. [INFERENCE] It is a preparation budget; it must be investigated before the library is adopted.
- The resource frame example is not executable on the selected backend. Its laws stay spec-only.
- Laws stated modulo quota leave Q-sensitive users without guarantees. That is deliberate (K-EVAL-04), but it limits optimization.

## Acceptance mapping

| Issue #11 checkbox | Section |
|---|---|
| Bounded API sketch with complete stack/effect signatures, derivations from existing words and explicit resource eligibility rules | Decision 1 (checked `wire@1` sketch, signature table, entry of pure Programs, eligibility rules); O11-4, O11-7 |
| Law matrix distinguishing behavioral observations, recipe/identity preservation, premises and unsupported cases | Decision 2 (relations N/R/T/O/F/Q/ID, L1-L14, Haskell-law classification, evaluation order, quota) |
| Worked pure pipeline, resource-preserving frame, sum branch and ordered-effect example, plus rejected resource fanout | Decision 3, examples 1-5 |
| Reproducible executable-law test plan with independent expected observations, negative cases and abnormal-cleanup coverage | Decision 4 (WIRE-CASE-01 to 24, reproducibility, cleanup responsibilities) |
| Proof plan reusing existing composition/frame/choice rules and identifying additional model, implementation and correspondence obligations | Decision 5 |
| Optional known-graph eligibility sketch and one candidate optimization analyzed against reflection, proof identity, ordering and quota | Decision 6 (compose-tree flattening) |
| Comment: package the first pure example as a #7 bundle with theorem references, parameters, ordered interfaces, profile, closure, assumptions and observation; keep the frame bundle separate | Decision 7 (`wire@1.seq-laws`; `wire@1.frame-laws` unpackaged) |
| Comment: worked consumer that selects the bundle and displays the instantiated law; refusals for wrong revision, different observation, absent resource premises and ambiguity; no eligibility, commutation or subject transfer | Decision 7 (worked consumer, refusal table); WIRE-CASE-18 |
