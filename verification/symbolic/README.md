# Untrusted solver lane for the fixed graph pairs (issue #56, items 4 to 6)

[`solver.mjs`](solver.mjs) runs two SMT solvers on the fixed graph pairs of issue #56 and decides
what their answers may support. The pairs are the ones `noble-ngraph` builds and checks and the
generator writes into `proofs/foundation/NobleFoundation/SymbolicPairCorpus.lean`; the lane parses
that file with a strict grammar and refuses anything else.

## Queries

For each pair the lane writes one QF_BV query for "some listed output differs". Every graph node is
one `define-fun`, so a shared node is defined once and never expanded into a tree. Inputs are
64-bit bit-vectors, arithmetic wraps, `sub a b` is `a - b`, and `eq` yields `Bool`, as in
`PackedSchedule.evalStep`. A sliced query defines only the nodes the listed outputs read; nothing
else is read, so slicing changes neither the query's meaning nor its models.

Bitwuzla and Z3 come from the repository's locked nixpkgs. GNU time measures their peak memory.

## Outcomes

| Outcome | What it needs |
|---|---|
| refuted | a satisfying model with exactly the declared 64-bit slots, which this lane's own evaluator, run on the original graphs and never on the formula, shows to make a listed output differ |
| certified | the pair's `*_certified` theorem, proved within the strict axioms, in a clean-source foundation receipt whose recorded corpus and certified-module bytes are the bytes read here; solver UNSAT never certifies |
| inconclusive | UNSAT without an accepted proof, UNKNOWN, a timeout, a solver error, a model that does not parse or replay, or a query bound to other bytes or revisions |
| unsupported | a node outside the fragment, a read of a later node, arithmetic on `Bool`, or an exhausted node, slot or formula budget |

Each query is bound to the encoder version, the graph IR's semantic revision and I64 profile (read
from `crates/noble-ngraph/src/ir.rs`), and the SHA-256 of the pair's corpus text.

## Encoder checks

Every query also passes two checks that do not trust the formula.
- **Concrete simulation.** At four fixed points and at every replayed witness, the inputs and every
  defined node are pinned to the values this lane's evaluator computes. A faithful encoding stays
  satisfiable; both solvers decide it.
- **Claim check.** The outputs the query's claim compares are read back from its text and must be the
  pair's listed outputs.

## Controls

- **Seeded mutations** of each certified pair's right graph: an operation, an operand order, a
  constant bit and an output each. A mutant is detected when a solver model replays; an undetected
  mutant must have only UNSAT answers, and the receipt lists it.
- **Corrupted encodings**: `sub` with swapped operands, `mul` encoded as `add`, constants with a bit
  flipped, and the last listed output dropped. Every corrupted query must fail an encoder check, and
  none may certify, even when both solvers give the same answer.
- **Bindings**: a stale semantic revision, another I64 profile, another encoder and altered pair
  bytes never refute.
- **Witnesses**: narrow values, a missing, duplicated or extra slot, and unparseable output are
  rejected, against a correct positive control.
- **Certificates**: corpus bytes that differ from the receipt, a dirty receipt, an unproved theorem
  and a disallowed axiom each withhold certification.
- **UNKNOWN and timeouts**: an injected UNKNOWN, an injected solver error and a real watchdog kill are
  inconclusive.
- **Budgets and fragment**: node, slot and formula budgets, a forward read and arithmetic on `Bool`
  are unsupported.

## Measurements

For every pair and solver the receipt reports, separately, the query's size, each cold solver run's
wall time and peak memory, the warm cache hit and its lookup time, and the replay time. No corpus
graph has a node its outputs do not read, so the slicing comparison pads each pair with eight dead
nodes per graph; slicing must drop them and keep every answer.

## Non-claims

The statuses are about the model graphs the corpus states. No source extraction, Wasm lowering or
Rust evaluator is a subject. A refutation here replays through this lane's evaluator; the kernel and
the Rust evaluators replay only the corpus's universe witnesses. Timings and memory are one run on one
host, not a performance claim.

## Evidence

[`receipts/2026-11-09/solver.json`](receipts/2026-11-09/solver.json) is one run from the clean
commit `d761a088`, bound to the foundation receipt `verification/foundation/receipts/cba12a925/`.

| Claim | Status |
|---|---|
| `sharedInput`, `wrapOnlyEquivalent`, `boolOutput` | certified, by their kernel theorems; both solvers report UNSAT, which is recorded and not used |
| `subtractionOrder`, `wrapBoundary`, `repeatedOutputs`, `negation` | refuted: every solver model replays, matching the kernel's universe verdicts |

- **Mutations.** 9 of 11 are detected. The two survivors are equivalent mutants, with only UNSAT
  answers: `wrapOnlyEquivalent` with its constant 1 flipped to 0 (`x · 0 · 0` and `x · 0 · 1` are
  both 0), and `boolOutput` with `x + 0` turned into `x - 0`.
- **Corrupted encodings.** All 18 fail an encoder check: 11 the concrete simulation and 7, every
  dropped output, the claim check. In all 18 the two solvers give the same answer. In the five where
  they agree on SAT for an agreeing pair, replay rejects every model, and no corrupted query
  certifies.
- **Item 5 controls.** Every binding, witness, certificate, UNKNOWN, timeout and budget control has
  the outcome above, against its positive control.
- **Measurements, one run.** Solver runs took 5.1 to 44.7 ms of wall time (median 12.8 ms), with
  peak memory of 4.4 to 5.5 MB for Bitwuzla and 21 to 32 MB for Z3. Replays took 15 to 95 µs, and
  warm cache hits took 1.3 to 32 µs, against 14 retained entries totalling 1674 bytes. On the
  padded pairs, slicing cuts each query from 1165 to 1372 bytes down to 411 to 614, with the same
  answers.
