# VerifiedHAMT evaluation (issue #60, compatibility unit)

Bounded evaluation of VerifiedHAMT as a **candidate** executable index for one
existing Lean reference environment. No dependency is adopted, the production
representation is unchanged, and nothing here is a performance or correctness
claim about an adapter.

## Decision

**Stop before adoption.** The library compiles under Noble's pinned Lean 4.31.0
without any repository-wide toolchain upgrade and has no external packages, but
three of its lookup theorems have **different transitive axiom dependencies**
under 4.31.0 than the upstream receipt records, and the upstream bundled
constructor is **last-write-wins** where Noble's environment lookup is
first-match. Both are addressable, but neither is free, so this unit records
compatibility and stops rather than adopting anything.

## Pinned subjects

| Item | Value |
|---|---|
| VerifiedHAMT revision | `24bbc61c6ca37ef38f984566399461c9afa2b0d3` (`refs/heads/main`) |
| License | Apache-2.0 |
| Upstream declared toolchain | `leanprover/lean4:v4.32.0` (`lean-toolchain`) |
| Upstream external packages | **none** (`lake-manifest.json` `packages: []`) |
| Toolchain actually used | Noble's pinned Lean **4.31.0** |
| Noble source | `063bf1f84` |
| Noble lookup under test | `Environment.lookup env slot := env.entries.find? (fun entry => entry.slot == slot)` (`proofs/mc1/NobleContracts/NamedSubject.lean`) |

Receipts: `receipts/2026-10-26/` (upstream `lean-toolchain`, `lakefile.toml`,
`lake-manifest.json`, the build log under 4.31, the duplicate-slot evaluation,
the axiom audit, and `pins.json`).

## Measured compatibility

**The library builds under 4.31.0.** With the package's `lean-toolchain`
overridden to `leanprover/lean4:v4.31.0`, `lake build VerifiedHAMT` completes
successfully — 24 targets, no errors, no warnings. The declared 4.32.0 target is
therefore a *declared* pin, not a build requirement for the library, so the
issue's option "explicitly validated compatible revision, not a repository-wide
toolchain upgrade" is satisfiable for the library itself.

**Upstream tests do not all build under 4.31.0.** `lake build VerifiedHAMTTests`
fails in `VerifiedHAMTTests.Find` with three `#guard_msgs` mismatches; the
modules before it build. Recorded (`receipts/2026-10-26/upstream-tests-under-4.31.log`):

| Theorem | Upstream records | Measured under 4.31.0 |
|---|---|---|
| `VerifiedHAMT.findNode_sound` | `[propext, Quot.sound]` | `[propext, Classical.choice, Quot.sound]` |
| `VerifiedHAMT.find?_eq_some_iff` | `[propext, Quot.sound]` | `[propext, Classical.choice, Quot.sound]` |
| `VerifiedHAMT.find?_eq_none_iff` | `[propext, Quot.sound]` | `[propext, Classical.choice, Quot.sound]` |

Independently confirmed through Noble's own audit path rather than by trusting
the upstream guard (`receipts/2026-10-26/axiom-audit.lean`, `.out`), which also
records `VerifiedHAMT.Map.find?_eq_some_iff` at the same three axioms.

Consequence: the three axioms are inside Noble's allow-list, so this is **not** a
policy violation, but the upstream axiom receipt cannot be reused as evidence
under Noble's toolchain. Any adoption needs a version-aware per-theorem audit
bound to the pinned toolchain, exactly as Noble's own foundation receipts do.

## Measured duplicate-slot semantics

Noble's lookup is first-match; the upstream bundled constructor is
last-write-wins. Measured (`receipts/2026-10-26/duplicate-slot-eval.lean`):

```
#eval (Map.ofList [(7, "old"), (3, "three"), (7, "new")]).find? 7   -- some "new"
#eval (Map.ofList [(7, "old"), (3, "three"), (7, "new")]).size      -- 2
#eval (Map.empty.insert 7 "old").insert 7 "new" |>.find? 7          -- some "new"
```

So an ordinary insert-based conversion of a list with duplicate slots
satisfies the upstream library's own README behaviour and **violates** Noble's
first-match rule. The issue's required decision is therefore live, and one half
of it is already partly answered by Noble's own source:

- `ExactClosure` already obliges `(env.entries.map (fun entry => entry.slot)).Nodup`
  — "no table slot is duplicated" is an **existing** closure obligation, not a
  premise silently added by this experiment;
- but `Environment.lookup` itself is total over an arbitrary `Environment`, and
  the type carries no uniqueness proof, so a map adapter MUST NOT rely on that
  obligation implicitly.

Any adapter must therefore either be **first-wins for arbitrary lists** or
refuse lists that do not carry the checked unique-slot premise. Nothing in this
unit chooses silently.

## What this unit does not claim

- No adapter exists, no relation between list entries and map contents is
  stated or proved, and no preservation theorem (empty/build/lookup, absent key,
  unaffected insertion) is claimed.
- No differential, upstream regression or compiler-IR result is claimed beyond
  the two build outcomes above.
- No benchmark, allocation or retained-memory measurement was taken, so there is
  no performance evidence either way — the issue's cold/build-inclusive versus
  repeated-lookup split is untouched.
- No adoption, no runtime or compiler substitution, no deletion, general resource
  table or cache redesign. Apache-2.0 licensing is recorded, not adjudicated.
- The three axiom sets are allow-list-compatible; that is not a claim that the
  theorems are axiom-free or that they mean what Noble's environment means.

## Adapter unit (2026-10-27): the decision, the relation and the measured costs

**Decision: stop before adoption.** The duplicate policy is **first-wins for arbitrary lists**, not a
unique-slot premise, because `NamedV1.Environment.lookup` is total over an arbitrary environment and
the type carries no uniqueness proof: an adapter that relied on `ExactClosure`'s existing
`Nodup` obligation implicitly would refuse inputs the reference rule accepts.

`adapter/Environment.lean` builds the index with `foldr`, so the first occurrence of a slot is
inserted last and wins. `lookup_first_match` proves, on **every** list, duplicate-free or not, that
the index's lookup is the reference first-match lookup and returns the **complete** first entry.
`index_keeps_the_order` keeps the ordered list as the source of truth, `absent_slot_remains_absent`
makes `none` a real answer, and `different_slot_insertion_is_unaffected` shows inserting an entry
under another slot at either end changes nothing (lawful equality/hashing remains the premise).

`adapter/corpus.lean` is an **oracle**, not a second adapter: every expected value is written
literally from the reference rule, and the corpus covers empty, singleton, absent key, sparse slots,
collision-heavy slots, repeated identical duplicates and repeated duplicates with **different**
provenance and body. The different-duplicate fixture is the control that detects accidentally
choosing the last entry: `lastWins repeatedDifferent` returns a different entry, measured. Substituted
provenance and substituted interfaces are different entries, and a dropped entry reads as absent.
Map lookups are not reducible by `decide`, so each adapter assertion goes through the proved relation
to the reference rule and only that rule is decided.

**Measured costs** (`receipts/2026-10-27/benchmark.out`, 2000 repeats, this host only):

| Size | build list | build index | lookup hit-first list/index | hit-last list/index | absent list/index |
|---|---|---|---|---|
| 1 | 326 ns | 1121 ns | 330 / 573 ns | 327 / 566 ns | 284 / 481 ns |
| 8 | 721 ns | 5386 ns | 345 / 601 ns | 702 / 598 ns | 661 / 505 ns |
| 64 | 4139 ns | 42796 ns | 389 / 1131 ns | 3956 / 918 ns | 3882 / 1097 ns |
| 512 | 33982 ns | 699749 ns | 360 / 875 ns | 26336 / 817 ns | 25533 / 768 ns |

So the index is 3.4x-20x more expensive to build at the sizes a named environment actually has, is
**slower** for a first-slot hit at every measured size, and only becomes clearly faster for
last-slot and absent lookups from about 64 entries upward. No allocation or retained-memory
measurement was taken; the numbers above are wall-clock per operation over an already built
representation for the lookup rows and build-inclusive for the build rows.

**Why the decision is stop rather than continue.** Nothing in this corpus or these numbers shows a
benefit at Noble's sizes, the upstream axiom receipts cannot be reused under Noble's toolchain, and
adopting a dependency would put an unverified-for-Noble library in the lookup path of accepted table
slots. The adapter stays an isolated evaluation artifact: the production representation, proof
authority and `crates/noble-contracts/src/rendering` are untouched.

**Receipts** (`receipts/2026-10-27/`): `pins.json` (library commit `24bbc61c`, toolchain, adapter
sources, theorem list, decision and basis), `adapter-build.out` (both adapter files transcribed with
the pinned Lean, empty output = no errors or warnings), `benchmark.out`, `sha256sums.txt` binding the
exact sources the numbers belong to, `run.md` for the method.

## Next unit when this is taken up

1. Map-backed adapter that keeps the ordered entry list as the source of truth
   for traversal, dependency order and identity, with an explicit list↔map
   relation and the duplicate policy chosen above.
2. `Entry.matches` and every field (provenance, ordered interfaces, body,
   dependencies, rank) preserved and compared in the theorem, not just slots or
   lengths.
3. Independent expected values that do not reuse the adapter's conversion or
   hashing, plus the collision-heavy and repeated-lookup corpus.
4. Only then the declared-size cost comparison.
