# M2 checking fragment v0: definition, schema, and control map

**Scope label:** `M2 fragment v0`. This document fixes the named fragment that the M2 change implements and proves.
It does not claim the whole language, recursion, evaluation, live resources, or Wasm correspondence.
The declarative judgment and the acceptance-theorem shape come from `.cairn/specs/language/spec.md` section 8.1 and V-CHECK-03.

## Types

| Form | JSON | Notes |
|---|---|---|
| Unit | `{"unit":{}}` | `Data` |
| Bool | `{"bool":{}}` | `Data` |
| I64 | `{"i64":{}}` | `Data`; wrapping arithmetic |
| Text | `{"text":{}}` | `Data` |
| Pair | `{"pair":[a,b]}` | `Data` iff both sides `Data`; multi-payload may hold resources for negative fixtures |
| Sum | `{"sum":[a,b]}` | `Data` iff both sides `Data` |
| List | `{"list":a}` | `Data` iff element `Data` |
| Program | `{"program":{"in":[..],"out":[..],"effects":[..]}}` | monomorphic at acceptance; `Data` (captures are `Data`-checked at `quote`) |
| Syntax | `{"syntax":{}}` | inert; `Data` |
| Resource | `{"resource":"<kind>"}` | opaque; never `Data`; no operation consumes a live resource in v0 |

Stacks are ordered lists of types, top on the right. Limits bound stack length and type size.

## Environment contracts

Schemes quantify over stack variables, value-type variables, and effect variables. Every variable set is finite and rank-1.

```text
dup   : S a -- S a a ! {}                 where Data(a)
drop  : S a -- S ! {}                     where Data(a)
swap  : S a b -- S b a ! {}
dip   : S a Program<S,T,e> -- T a ! e
+ - * : S I64 I64 -- S I64 ! {}
quote : R a -- R Program<S, S a, {}> ! {}  where Data(a)
compose : R Program<A,B,e> Program<B,C,f> -- R Program<A,C,union(e,f)> ! {}
run   : S Program<S,T,e> -- T ! e
reflect : R Program<S,T,e> -- R Syntax ! {}
unit  : S -- S Unit ! {}
pair  : S a b -- S Pair<a,b> ! {}
unpair : S Pair<a,b> -- S a b ! {}
inl   : S a -- S Sum<a,b> ! {}
inr   : S b -- S Sum<a,b> ! {}
case  : S Sum<a,b> Program<S a,T,e> Program<S b,T,f> -- T ! union(e,f)
if    : S Bool Program<S,T,e> Program<S,T,f> -- T ! union(e,f)
nil   : S -- S List<a> ! {}
cons  : S a List<a> -- S List<a> ! {}
list.case : S List<a> Program<S,T,e> Program<S a List<a>,T,f> -- T ! union(e,f)
test.emit : S Text -- S Unit ! {test.emit}
```

The environment also supplies named definitions with the same scheme form and the concrete effect-identity list.

## Candidate and request schema (`noble-candidate/v0`)

A request binds the expected interface, the environment, the supported subset, and the limits; the candidate binds its nodes and format, independently of the request (K-CHECK-06).

```json
{
  "request": {
    "expected": {"in": [ {"i64": {}} ], "out": [ {"i64": {}} ], "allowed_effects": [] },
    "environment": {
      "defs": [ {"id": "add", "scheme": {"vars": {"S": "stack"},
                 "stack_in": [ {"var": "S"}, {"i64": {}}, {"i64": {}} ],
                 "stack_out": [ {"var": "S"}, {"i64": {}} ], "effects": [] } } ],
      "effects": ["test.emit"]
    },
    "subset": "core-bootstrap",
    "limits": {"bytes": 65536, "nodes": 256, "depth": 32, "type_size": 256,
               "stack_height": 64, "work": 100000, "diagnostics": 4096}
  },
  "candidate": {
    "format": "noble-candidate/v0",
    "semantic_revision": "0.1.0-draft.5",
    "nodes": [
      {"kind": "literal", "lit": {"i64": 41}},
      {"kind": "invocation", "def": "add", "inst": {"S": []}},
      {"kind": "quotation", "body": [0, 1]}
    ],
    "root": 2
  }
}
```

Node references are exact indices; quotation bodies are finite node lists (indices into the candidate); nesting is bounded by `depth`.
An invocation's `inst` maps every scheme variable to a concrete stack, type, or effect set; a missing, extra, or oversized mapping rejects, and applying it to the scheme must equal the node's derived interface exactly.

## Rules

```text
EMPTY      Gamma |- [] : S -- S ! {}
LITERAL    Gamma |- lit : S -- S a ! {}                       (a the literal's type)
WORD       Gamma |- def[inst] : S -- T ! e                    (apply(scheme(def), inst) = S -- T ! e)
SEQUENCE   Gamma |- p : A -- B ! e   Gamma |- q : B -- C ! f
           --------------------------------------------------
           Gamma |- p q : A -- C ! union(e,f)
QUOTATION  Gamma |- B : A -- C ! e
           ------------------------------------------
           Gamma |- [ B ] : R -- R Program<A,C,e> ! {}
```

Derived constraints: `Data` side conditions on `dup`, `drop`, and `quote`; every derived effect bound must be included in `allowed_effects`; branch rules (`case`, `if`, `list.case`) require both branch programs to check from the same inputs to the same complete output stack and take the union of the latent bounds.

## Limits and outcomes

Limits: `bytes`, `nodes`, `depth`, `type_size`, `stack_height`, `work`, `diagnostics`. Every stage charges work before its next allocation or traversal; a nested stage shares the same budget. Memory is bounded through `nodes`, `type_size`, and `work`.
Outcomes: `accepted` (with the checked interface and derivation record), `invalid` (a named rejection with its diagnostic), `unsupported` (an out-of-fragment form), `exhausted` (the exceeded limit), `internal-failure` (unreachable in v0; reserved and tested as unreachable). Rejection executes no body and issues no candidate-body host request.

Diagnostics carry: failing node index and word or join identity, expected versus actual stack shapes (top-first), the violated constraint (effect inclusion or eligibility), and provenance as explicitly unavailable in v0.

## Control map

Each rule family and limit has a positive control, and rejection families have named negatives; every control keeps the fragment label.

| Family | Positive | Boundary | Exhausted | Rejection |
|---|---|---|---|---|
| FRAG-LIT | literal program accepted | unit/text/bool literals | — | out-of-range `I64` literal |
| FRAG-WORD | instantiated builtin and named def accepted | empty stack variable mapping | — | missing/extra/oversized instantiation, rewrite of the scheme's interface |
| FRAG-SEQ | multi-node joins on matching stacks | empty sequence | — | join mismatch, stack order swap, arity mismatch |
| FRAG-QUOTE | quotation with checked body and construction effects `{}` | body needing inputs | — | unchecked body, latent effect claimed as construction, interface not derived |
| FRAG-BRANCH | `if`, `case`, `list.case` with equal joins | branch order swap | — | unequal output stacks, missing union effect, resource payload join |
| ELIG-DATA | `dup`, `drop`, `quote` on `Data` types | nested `Pair`/`List` of `Data` | — | `Resource<k>`, `Sum<Resource<k>,I64>`, nested resource payloads |
| EFF-INC | bound included in allowed effects | empty sets both sides | — | hidden `test.emit`, narrowed bound, host-denial repair attempt |
| LIM-* | at-limit candidate accepted | exactly at `bytes`, `nodes`, `depth`, `type_size`, `stack_height`, `work`, `diagnostics` | one over each limit | negative sizes and malformed counts |
| OUT-* | accepted outcome distinct | each outcome distinguishable | exhausted marked exhausted | timeout or internal failure never maps to accepted |
| DIAG-* | diagnostic names failing join and shapes | exact-shape message | diagnostic budget exhausted | unavailable provenance reported as unavailable, not invented |

## Omissions

Inference/unification, generalization, recursive definitions, imports, preparation services, live resources, evaluation and recipe semantics, Wasm lowering and execution, concurrency, and contracts.
A result about this fragment MUST NOT be presented as a result for the whole language (V-MODEL-01).
