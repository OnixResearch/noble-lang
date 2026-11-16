# Reusable workspace ownership contract (issue #133): bind by compiled identity

Issue #133 asks for one coherent ownership contract for `Compiled::evaluate_in` and
`Compiled::reset`: either bind reusable storage to the exact compiled identity and refuse
mismatches before mutation, or expose an explicit checked rebind or preparation operation.
This file records the selection, the criteria it was judged by, the evidence and the
conditions that would reopen it. The contract text itself lives in the rustdoc of
`crates/noble-ngraph/src/compiled.rs` and in the README section
"Reusable workspace binding".

## Criteria

1. **Exact.** A workspace accepted for graph B must never supply another graph's constants,
   register map, slot map or output map, independently of the injected identity scheme
   (`identity.rs`: a hash identifies bytes and proves nothing).
2. **Refuse before mutation.** A mismatch is refused before any register, caller counter or
   scratch lease is touched, without a panic and without partial output, by both
   `evaluate_in` and `reset`.
3. **Cheap valid path.** Repeated evaluation of one graph in its own workspace adds no
   allocation and no per-node work.
4. **Separate owners (NGRAPH-OWN-01).** Workspace scratch stays owned separately from
   retained graph code.
5. **No unrequested surface.** In-tree callers keep their shape unless the contract needs a
   change.

## Options

| Option | Result |
|---|---|
| Compare profile and register count | Fails criterion 1. As a manual development mutant (not committed) it let the constant-only A/B pair return A's constant, and six of the eleven ownership tests failed. |
| Compare digests (`GraphLogicalId`, `PackedLayoutKey`) | Fails criterion 1 under a colliding identity scheme. The test `a_colliding_identity_scheme_cannot_make_a_foreign_workspace_acceptable` shows the A/B pair with equal graph ids and equal layout keys. |
| Reload constants on every call, check only the size | Exact, but adds one store per constant to every valid call and silently accepts other graphs' storage. Fails criterion 3; the issue allows reinitialization only as an explicit operation. |
| Explicit checked rebind or prepare operation | No in-tree caller reuses storage across graphs. The tick runner replaces its compiled value and workspace together; batch rows, the CLI, the rewrite-cost lane and the experiments evaluate in the workspace of the value that created it. Fails criterion 5. |
| Prepared object owning compiled state and workspace | Changes every caller's ownership structure. Fails criterion 5. |
| **Bind to the compiled identity** (selected) | Meets 1–5, as below. |

## Selected contract

`Compiled::new` creates one dataless shared token (`Rc`), held by the compiled value, its
clones and every workspace it creates, and compared by address. A workspace that holds
another token is refused with `E-INTERFACE` at the graph site. `evaluate_in` checks it first,
before the input preflight, scratch lease or any write. `reset` returns the same refusal and
leaves the workspace unchanged. On its own workspace, `reset` restores the exact
post-construction registers. A workspace never changes owner or size, so no rebind
allocation exists to charge. The token holds no graph or code, and the workspace keeps its
allocation alive, so the address cannot be reused while the workspace exists.

Consequences:
- The valid path adds one pointer comparison per call. This is a structural fact of the
  code; no timing was measured.
- Another `Compiled::new` call is another identity, even for the same checked graph. That
  is a conservative refusal, pinned by `same_graph_reuse_and_clones_keep_their_semantics`.
- `Compiled::reset` now returns `Result<(), Diagnostic>`. Its only in-tree caller was
  `Compiled::workspace`, which now loads constants directly.
- `Workspace` is no longer `Send` or `Sync`. `Compiled` already was not, through the
  checked graph's `Rc`.

## Evidence

- On the unfixed source `62bc0109`, a form of the ownership tests that compiles there failed
  8 of 10 tests. B returned A's constant `Normal([I64(1)])`. A larger F32 workspace returned
  6.5 for 1.5 + 1. Smaller and other-profile workspaces panicked with index out of bounds. A
  foreign `reset` wrote 7 into the owner's constant register.
- After the fix, all 11 ownership tests pass in the `layout_differential` target.
- Manual development mutants, not committed, were each killed by the tests:
  - reset without the binding check: 3 tests fail;
  - reset without clearing scratch: 1 test fails;
  - the check moved after the input preflight: 1 test fails, with a slot-site refusal;
  - the check moved after the scratch lease: 8 tests fail, with lease counters touched.

These are finite executed tests, not a proof that the compiled evaluator refines the
reference (NG-PO-C), and not a layout, schedule or cost claim.

## Reopen conditions

- A caller must move a workspace across threads: replace the `Rc` token after reviewing
  design D2's no-atomics rule.
- A caller must reuse register storage across distinct compiled graphs: add an explicit
  rebind. It must charge the new register bytes against the caller's scratch budget before
  allocating and leave the workspace unchanged on refusal.
- A caller must share one workspace between independently compiled identical graphs: use
  value identity with full-table equality, never digest equality.
- A measured warm-path regression is attributable to the identity check.
