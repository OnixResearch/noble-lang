import NobleM2.Check

/-!
# Dependency-walk order independence: finite examples (#134)

Kernel-checked finite examples, not a general theorem: each statement fixes
one small dependency graph, one numbering of its definitions and one order of
every dependency list, and evaluates the reference model's dependency walk
(`validateDeps`, B-CHECK-02) on it. Together they pin the behaviour issue #134
requires of the walk on the shapes its inventory found order-dependent:

- an acyclic graph validates whatever its edge order, with the same remaining
  work (one unit per edge): an edge to a finished definition is neither a
  cycle nor walked again;
- a cyclic graph is rejected, and the rejection names a definition that lies
  on a cycle: the one the first closing edge reaches, after which the walk
  stops;
- under a tight work budget the first failure is the one reported.

Before the #134 fix, `depWalk` kept finished definitions on its path, so the
forward diamond below was rejected as a cycle closing on `2` while the
reversed diamond validated, and the four-definition diamond was rejected in
both orders as a cycle closing on `3`.

Each proof is `decide +kernel`: `depWalk` is well-founded recursion, which the
elaborator's `decide` does not unfold, so the kernel evaluates the walk. These
are model theorems only. The kernel's matching regression is a Rust test
(`crates/noble-kernel/tests/recursion_schema_cycle/orders.rs`), and no
correspondence between the two is claimed here.
-/

namespace NobleM2.DependencyOrder

/-- An environment whose definitions `0, 1, ...` carry exactly the given
dependency lists; their schemes play no part in the dependency walk. -/
def depsEnv (deps : List (List Nat)) : Env :=
  { (default : Env) with defs := deps.map (fun _ => default), deps := deps }

/-- The acyclic diamond `0 → 1, 0 → 2, 1 → 2` validates, charging one unit
per edge. Walking `0`'s list forward, the shared `2` finishes before `1`
reaches it; that edge is not a cycle. -/
theorem diamond_validates :
    validateDeps (depsEnv [[1, 2], [2], []]) 100 = .ok 97 := by
  decide +kernel

/-- The same diamond with `0`'s list reversed validates with the same
remaining work. -/
theorem diamond_reversed_validates :
    validateDeps (depsEnv [[2, 1], [2], []]) 100 = .ok 97 := by
  decide +kernel

/-- The diamond's verdict and remaining work do not depend on the order of
`0`'s dependency list. -/
theorem diamond_edge_order_agrees :
    validateDeps (depsEnv [[1, 2], [2], []]) 100
      = validateDeps (depsEnv [[2, 1], [2], []]) 100 :=
  diamond_validates.trans diamond_reversed_validates.symm

/-- The four-definition diamond `0 → 1, 0 → 2, 1 → 3, 2 → 3` validates with
four units charged in the forward order. -/
theorem diamond_four_validates :
    validateDeps (depsEnv [[1, 2], [3], [3], []]) 100 = .ok 96 := by
  decide +kernel

/-- The four-definition diamond validates with the same remaining work when
`0`'s list is reversed. -/
theorem diamond_four_reversed_validates :
    validateDeps (depsEnv [[2, 1], [3], [3], []]) 100 = .ok 96 := by
  decide +kernel

/-- A self-loop behind a chain, `0 → 1, 1 → 1, 2 → 0`: the walk from `0`
closes the self-loop first and names `1`, the only definition on a cycle. -/
theorem selfLoop_behind_chain_names_the_loop :
    validateDeps (depsEnv [[1], [1], [0]]) 100 = .depCycle 1 := by
  decide +kernel

/-- The same graph renumbered so the self-loop comes last (old `0, 1, 2`
become `0, 2, 1`): `0 → 2, 1 → 0, 2 → 2`. The rejection still names the
self-loop, now `2`. -/
theorem selfLoop_last_names_the_loop :
    validateDeps (depsEnv [[2], [0], [2]]) 100 = .depCycle 2 := by
  decide +kernel

/-- Two cycles sharing a definition, `0 → 1, 1 → 2, 1 → 0, 2 → 1`: `1`
enqueues `2`, then its edge back to the on-path `0` closes the first cycle,
and the rejection names `0`, which lies on `0 → 1 → 0`. -/
theorem overlapping_cycles_name_the_first_closing_edge :
    validateDeps (depsEnv [[1], [2, 0], [1]]) 100 = .depCycle 0 := by
  decide +kernel

/-- Two disjoint cycles, `0 ↔ 1` and `2 ↔ 3`: the rejection names `0`, a
member of the first cycle the walk reaches. -/
theorem disjoint_cycles_name_the_first_reached :
    validateDeps (depsEnv [[1], [0], [3], [2]]) 100 = .depCycle 0 := by
  decide +kernel

/-- A definition shared by two roots and entering a self-loop,
`0 → 2, 1 → 2, 2 → 3, 3 → 3`: the rejection names the self-loop `3`, not the
shared `2`, which lies on no cycle. -/
theorem shared_entry_names_the_loop :
    validateDeps (depsEnv [[2], [2], [3], [3]]) 100 = .depCycle 3 := by
  decide +kernel

/-- With one unit of work, the self-loop `0 → 0` charges it and rejects
naming `0`; the later root `1`, whose edge would find the work spent, is
never walked. -/
theorem selfLoop_rejects_within_one_unit :
    validateDeps (depsEnv [[0], [2], []]) 1 = .depCycle 0 := by
  decide +kernel

/-- With no work, the self-loop edge cannot be charged, so exhaustion is the
first failure. -/
theorem selfLoop_exhausts_without_work :
    validateDeps (depsEnv [[0], [2], []]) 0 = .exhausted := by
  decide +kernel

end NobleM2.DependencyOrder
