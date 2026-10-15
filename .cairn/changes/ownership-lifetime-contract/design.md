## Context

Issue #3 asks for a research and design outcome, not an implementation. All source citations were re-read at `44329dc`. The issue's links are pinned to `0ea10d3`; drift between the two is recorded below. Statements marked **[INFERENCE]** were not observed in source or by execution. Text is labeled as one of three kinds: **Design argument** (written, conditional reasoning), **Planned test** (an unexecuted scenario design) or **Proof obligation** (an open claim). None of them is a result.

### Contract preserved

- §4.3 `Data(a)` permits duplication and discard. `Capture(a)` permits embedding in a recipe. Every v0 Program is data and capturable, even when its *input interface* mentions a resource. K-TYPE-02 makes eligibility recursive.
- K-TYPE-01 checks shapes at full arity and order. Untouched prefixes pass through, and stack-tail variables are opaque. K-PROG-05 unifies the whole intermediate stack, including resource positions.
- K-PROG-02/07/08: no implicit stack capture. `quote` takes only `Capture` data, immutably. Resources are never captured, even through containers.
- K-RES-01..04: a resource carries one obligation and must be moved, consumed or released. `dup`/`drop`/`quote`/serialization reject it. Every normal result alternative accounts for it. Abort cleanup is host-owned, idempotent and independent of the guest. K-STACK-01 keeps a `dip`-held value hidden and restores it exactly once. K-CONTROL-01 makes both branches account for every obligation.
- K-DEF-01/02 and P-ID-01/03: dependencies are exact immutable identities, and captured data is part of ProgramValueId. P-PACK-01/02: a portable package carries a finite dependency graph of code and contracts, never host implementations or live resources.
- RA-SCOPE-01, RA-TYPE-02 and WI-RES-02/05: borrows exist only as adapter-local tokens. A source borrow type needs "a separate lifetime contract". BE-RESOURCE-01 and RA-OWN-02: GC or RC reachability never discharges a resource obligation. S-MEM-04: the internal strategy is free if the abstract invariants hold.

### Implementation facts at 44329dc (read, not executed)

| Topic | Source at 44329dc | Drift from the 0ea10d3 links |
|---|---|---|
| Compiled pushes/calls of named Programs read module globals | `crates/noble-wasm/src/source/emit/functions.rs:101` (`Action::Program` → `push_program`), `:117` (`Action::Call` → `enqueue_program`); the helpers at `:189-192` and `:217-223` emit `get_global` (`source/emit.rs:18-22`) | Moved from `:81-90` to `:101-117`; calculator actions were inserted at `:106-110` |
| Module-root retirement releases every module Program global and then collects | `crates/noble-wasm/src/source/emit/live.rs:187-201` (`release_module_roots` → `$storage_unroot_program` → `$storage_collect`) | None |
| Deferred RC: zero-count scan, then child-release cascade | `crates/noble-wasm/runtime/live-linear-storage.wat:241-287` (`$storage_queue`, `$storage_collect`) | None (file unchanged) |
| Code-span owners count only Program cells whose entry field lies in the span | `live-linear-storage.wat:288-313` (`$storage_code_owners`) | None |
| Separate counters: incoming references, saved roots, module roots, ordinary external roots; non-wrapping epoch handles | `live-linear-storage.wat:1-10, 50-67, 78-104, 153-240` | None |
| Live frames pin their selected target with an increment and release it on restore | `crates/noble-wasm/runtime/live-borrows.wat:42-65, 117-147` | None |
| The host's saved-Program graph walk follows only `a/b/c` edges and Program entry spans | `crates/noble-cli/src/core/runtime/slot-engine.mjs:766-791`; module sweep `:663-679`; table clear `:705-733` | Not cited by the issue |
| Ordinary source sessions bump-allocate and never free; failure is terminal | `runtime/source.wat:1-3, 23`; `runtime/accounting.wat:16-28`; README "Persistent sessions" | `accounting.wat` now uses `$cell_limit` instead of the constant 4096 |
| Calculator boundary compaction has the operand stack as its only roots and refuses to move when any reachable Program runs compiled code | `runtime/calculator.wat:136-150, 289-372`; `.cairn/changes/calculator-v1/design.md:60-62` | Added after `0ea10d3` |

The issue's correspondence question stays open. Compiled bodies read named Programs from instance globals. `release_module_roots` unroots every module Program global once the host's sweep runs (`slot-engine.mjs:663-670`). The host only gates that sweep on version and replay owners and on Program entries counted by `code_owners`. Saved-owner attribution follows `a/b/c` child edges and Program entry spans (`:766-791`). **[INFERENCE]** A saved Program whose compiled body reads a module global for a callee that has no other counted owner could therefore outlive that callee's cell. Epoch-qualified handles (`live-linear-storage.wat:1-3, 20-30`) would make such a use fail a liveness check and trap rather than read a reused cell. That outcome is still a lifetime violation. Source reading cannot settle whether the emitted code ever reaches this state, because the global may also be pinned through edges not traced here. It is **not** a confirmed defect. OWN-CASE-06 is the planned end-to-end reproducer.

## Decisions

### Decision: Bounded ownership and borrow model

**Choice:** Recommend a three-class ownership model with no new syntax. Each stack position has exactly one class:

1. **Shared** — `Data(a)`: immutable and freely duplicable or discardable. It includes every v0 Program value.
2. **Linear** — resource-bearing: one obligation, which may be moved, consumed or released (K-RES-01).
3. **Scoped** — a non-owning borrow. It exists only in an opt-in profile. In Core, existing `Data`/`Capture` eligibility plus linear resources suffice, and nothing changes.

Signatures need no new annotations for classes 1 and 2. The class is a function of the instantiated type: `Data` means shared, and resource-bearing means linear. A linear position that persists across a word appears in both its input and output shapes (move-threading, K-RES-03). Stack-tail variables are opaque (K-TYPE-01), so a word that threads `rho` cannot duplicate, drop or capture anything hidden inside it. That is why the obligations compose.

If a future profile adds borrows, it MUST take exactly this bounded form (r[OWN-BORROW-01]):

```text
borrow : S R Program<S Borrow<k,R>, T, e> -- T R ! e
         where k is a fresh rigid scope tag for this site
         and k occurs nowhere in T (stack values or Program interfaces)
```

`borrow` is `dip` with access. The owner `R` is held outside the body's visible stack. It therefore cannot be moved, released or borrowed mutably during the scope, and it is restored exactly once on normal return. `Borrow<k,R>` is neither `Data` nor `Capture`. In the first profile it may not enter `Pair`/`Sum`/`List`/declared aggregates, recipes, continuations, exports, serialization or suspension. Borrowing host operations take it as input and do not return it, for example `fs.read : S Borrow<k,Dir> Text -- S Result<Bytes,FsError> ! {fs.read}`. Repeated use inside one scope goes through a profile-specific `borrow.dup : S Borrow<k,R> -- S Borrow<k,R> Borrow<k,R>`. The body discards a borrow with `borrow.end`. Generic `dup`/`drop` still require `Data`. Named definitions may be rank-1 polymorphic in `k`. A runtime-selected first-class Program has one instantiated interface (K-PROG-09). It therefore cannot be a `borrow` body unless its interface already mentions this site's `k`, which only a value constructed inside the scope can do. That value cannot leave the scope because `k` may not occur in `T`. No higher-rank types are introduced: §4.2's rank-1 restriction stays intact, because `k` is a site-local skolem rather than a user-quantified type.

**Rationale:** The rule is the minimal generalisation of shapes the specifications already accept. RA-TYPE-02 and WI-RES-02 adapter-local borrows are borrows whose scope is one host call. K-LSLOT-02 `LiveRef` is a host-issued borrow whose scope is one root invocation. The skolem escape test makes the "does not outlive its owner" rule a type-level occurs check instead of a dataflow analysis. Rejecting aggregates, suspension and capture keeps the first profile decidable and small. Move-threaded interfaces stay valid: the profile only *adds* checked alternatives.

**Alternatives rejected:** Rust-style lifetime polymorphism and non-lexical lifetimes are excluded by WI-RES-02 and RA-SCOPE-01 and exceed a bounded profile. Affine `Borrow` values that satisfy `Data` would let `List<Borrow>` escape. Borrows of immutable data have no purpose, because `Data` is already shared. Deep-copy semantics for `dup` would make `dup`/`compose` cost proportional to the closure, which the issue rejects.

#### Illustrative transitions (design examples, not tests)

```text
-- shared:  [ 1 + ] dup
   S Program<I64,I64,{}>  --dup-->  S P P      (Program is Data: one storage object, two owner claims; no copy)
-- linear:  dir "a" fs.read               ⇒ S Directory Result<Bytes,FsError>
   S Directory Text -- S Directory Result<Bytes,FsError>   (owner returned exactly once)
-- quote transfers: 5 quote               ⇒ S Program<S',S' I64,{}>  (the I64 owner claim moves into the capture)
-- dip preserves:   dir [ 1 2 + ] dip     ⇒ S I64 Directory  (owner hidden, restored once)
-- scoped (profile only):
   dir [ "a" fs.read.borrowed ] borrow     ⇒ S Result<Bytes,FsError> Directory
```

#### Rejected escape and capture examples (each MUST fail statically)

```text
dir dup                         -- K-RES-02: Linear position is not Data
dir quote                       -- K-PROG-08: resource capture
dir [ ] borrow                  -- body leaves Borrow<k,Directory> in T: k escapes
dir [ borrow.dup pair ] borrow  -- Borrow inside an aggregate (first profile)
dir [ quote ] borrow            -- Borrow is not Capture
dir [ [ fs.read.borrowed ] ] borrow
                                -- T contains Program<S Borrow<k,Dir> Text,…>: k occurs in an interface
dir [ save-program ] borrow     -- a saved Program/continuation would need k beyond the scope
dir [ swap release ] borrow     -- the owner is hidden: release cannot reach it
```

### Decision: Stack operations as ownership transitions

**Choice:** Each stack operation's transition is fixed as follows. Storage reclamation timing is a backend property, defined separately.

| Operation | Eligibility | Ownership transition | Storage obligation (backend-neutral) |
|---|---|---|---|
| `swap` | none | reorders two owner claims; no creation or destruction | none |
| `dup` | `Data` (or `borrow.dup` in the profile) | adds a second owner claim on the same immutable value | sharing; no deep copy is required |
| `drop` | `Data` | ends one owner claim | reclamation MAY be deferred to RC collection, arena reset or engine GC; never host-resource release |
| `quote` | `Capture` | moves the operand's claim into the new Program's environment | the Program owns its capture graph |
| `compose` | none beyond Program types | consumes two Program claims and produces one Program owning both | O(1) node sharing both operands |
| `run` | none beyond Program type | consumes one claim; the activation frame pins the Program package until the frame exits | frame pin counted until exit, normal or abnormal |
| `dip` | none | holds one claim outside the visible stack and restores it exactly once on normal return | held claim counted as a frame root |
| `if`/`list.case` | none | each branch accounts for every incoming linear claim (K-CONTROL-01) | — |

Dynamically built Programs (`quote`, `compose`, runtime selection) carry exactly their checked instantiated interfaces (K-PROG-06/09). Ownership through `run` of a runtime value is therefore the same as through a literal. The checker establishes linear accounting from the interface. The runtime never consults a recipe to recover ownership.

**Rationale:** `swap` and `dip` do not need eligibility because they neither create nor destroy claims (§7.1). Separating the *claim* from the *storage* is what lets `drop` stay a discard of an eligible owner without implying synchronous reclamation or host release. That matches BE-RESOURCE-01 and RA-OWN-02.

### Decision: Owned saved-Program package is the lifetime unit

**Choice:** A saved Program's *executable package* `Pkg(P)` is the smallest set closed under the following members:

1. **Code**: every compiled function or table entry reachable from P's entry, and from every Program reachable in the package. This includes private continuation entries (`functions.rs:41-79`).
2. **Exact named dependencies**: every Program or definition value that compiled code reads through a module global or table entry (`functions.rs:101, 117, 189-223`). It also includes the module instance or global storage that holds those values, and their own packages, transitively.
3. **Captures**: the immutable cell graph reachable through checked edges (`a/b/c`), including captured Programs and Text.
4. **Recipes**: recipe atom, concatenation and Syntax cells needed for P-REFLECT and P-ID observations.
5. **Static data**: static Text segments, nominal schema descriptors, signature and interface entries (for example the calculator signature pool, `calculator-v1/design.md:62`) and invocation witnesses.
6. **Metadata**: interface, effect bound, ProgramValueId/DefinitionId, host-*contract* identities (not implementations or grants), artifact identity (P-ID-07) and accounting revision.

The package excludes host implementations, live resources, `LiveRef`s, authorization grants and registry maps, consistent with P-PACK-01. A saved Program MUST retain all of `Pkg(P)` while any owner of P survives. Retained means each member is reachable through counted owner edges. Creating-invocation exit, `release_module_roots`, definition replacement, Live-Wasm-Draft reload, slot publication or arena reset MUST NOT remove a member reachable from a surviving owner (r[OWN-PKG-01]).

**Rationale:** The calculator design already treats globals as non-relocatable package members. `collect` refuses to move anything when a reachable Program runs compiled code, because "compiled bodies read their quotation and callee Program handles from instance-private globals" (`calculator.wat:144-146`). Copying or retaining only the cell graph is insufficient. H-LIVE-03 states the same rule for Live-Wasm-Draft generations ("old code/instances remain alive only while referenced"). This decision makes it a backend-neutral unit, so #1's slot versions and #5's strategies can refer to it.

### Decision: Scratch storage versus retained packages

**Choice:** Model storage as two disjoint spaces: invocation-local **scratch** and owner-counted **retained** packages. Retained-to-scratch edges are forbidden. Every value that crosses an *escape boundary* is promoted before the boundary completes. The escape boundaries are: root outputs left on the session stack, a saved-owner token, a host park, a live-slot candidate staging, and an export. Promotion takes one of three forms: (a) *copy and relocate* the closure into retained space; (b) *region transfer*, which reclassifies whole scratch pages as retained; (c) *allocate-retained* when the checker predicts escape. Active frames pin their Program package and their `dip`/`borrow`-held owners. Scratch reset is allowed only at a point where no frame, saved owner, module root or map pin references scratch (r[OWN-SCRATCH-01]). On trap, cancellation or failed construction, the host discards scratch without resuming the guest. It retires invocation-owned resources per K-RES-04 and releases frame pins exactly once. Retained packages survive the reset unchanged. A package is retired only when its last counted owner releases it.

| Strategy | Escape cost | Reset/retire cost | Lifetime risk | Profile compatibility at 44329dc |
|---|---|---|---|---|
| Scratch arena + retained packages | promotion O(closure) for copy, O(pages) for transfer | O(1) reset; package retirement O(package) | a missed retained→scratch edge invalidates saved captures | Not implemented. The ordinary source ABI is one session-long bump arena with no reset (`source.wat:23`). The M3 experiment resets to its module baseline (`common.wat:31-37`). |
| Closed copied-and-relocated package | O(closure + code + globals) per escape; code relocation requires table/global rewriting | O(package) on release; no sharing across copies | copies of only cells miss globals/table entries; duplicated code inflates table use | Partially analogous: calculator compaction relocates cells only when no compiled Program is reachable (`calculator.wat:289-372`) |
| Immutable sharing + deferred RC | O(1) `dup`/`compose`; count increment per edge | deferred zero-count scan O(heap cursor) plus cascade (`live-linear-storage.wat:245-287`) | uncounted owner edge (for example code→global) or counter overflow | Implemented only for the opt-in live-slot profile (`live-linear-storage.wat`, `live-borrows.wat`) |

**Rationale:** Issue #3 asks that arena reset never invalidate saved captures or code dependencies. The forbidden retained→scratch edge is the one invariant that makes that true for every promotion form. Abnormal cleanup reuses K-RES-04's host-owned accounting instead of inventing guest-resumed unwinding. That matches today's terminal-failure policy (`source.wat:1-3`; `engine.mjs:1012`; `slot-engine.mjs:1218-1234`).

### Decision: Sharing by default, copying only at boundaries

**Choice:** Semantic `dup` and `compose` share immutable representations and never require a deep copy. Copying is reserved for three cases: cross-instance or process transfer (P-PACK/P-ROUND import and export), promotion out of scratch, and compaction. A copied package MUST be *closed*. It MUST carry or re-link every member of `Pkg(P)`, including globals and table entries. Copying only the Program cell or capture graph is a conformance failure whenever code reads external globals or table entries.

| Operation | Shared representation | Closed copy |
|---|---|---|
| `dup` of Program | O(1) claim (RC +1 or none in arena/GC) | O(package), unacceptable for ordinary `dup` |
| `compose p q` | O(1) node with two owned children | O(package(p)+package(q)) |
| release order of duplicates | any order; storage is freed after the last owner releases | each copy is independent |
| cross-instance export | not allowed (handles are instance-private) | required |

**Rationale:** Immutability makes sharing unobservable (S-MEM-04; P-RECIPE-02 hides allocation identity). The only correctness cost of sharing is accurate owner counting, which the reclamation decision addresses.

### Decision: Reclamation without tracing GC

**Choice:** Non-tracing guest-cell reclamation, whether by ownership, regions or deferred RC, is safe only under all of the following conditions (r[OWN-RECLAIM-01]):

- **C1 complete owner edges.** Every owning edge kind is counted. The kinds are heap child edges, operand and continuation slots, frame pins, `dip`/`borrow`-held claims, saved-owner tokens, module roots, map/root pins (#1), and *code→global/table dependency edges* from Decision 3, item 2. At 44329dc the live-slot runtime counts incoming, saved and module references separately (`live-linear-storage.wat:1-10`). Code→global edges are argued only by module-root lifetime, which is the open OWN-CASE-06 question.
- **C2 logical birth order.** Acyclicity is argued over a logical, never-recycled birth sequence. Recycled numeric handles do not provide it. The ordinary arena enforces strictly backward child handles (`linear-storage.wat:29-35`). The live store reuses physical slots, so numeric order is meaningless there. Its argument is instead immutability plus validate-before-publish (`live-linear-storage.wat:31-35`): a child must already be live when its parent is published, so the child's birth precedes the parent's. Package dependency edges point only to previously admitted exact identities (K-DEF-01), so they form a DAG in admission order. Hashes and immutability alone establish neither acyclicity nor lifetime.
- **C3 recursion.** A self reference is intra-package (K-DEF-03). It MUST NOT be counted as an owner edge, or a self-recursive package can never reach count zero. Mutual recursion is excluded in this revision. If it is admitted later, a strongly connected group must be one package.
- **C4 live indirection.** `LiveRef`s are invocation inputs and registry maps are roots. Neither is a package member, so slot dispatch adds no package edge. If Programs could capture slot IDs or refs, a version could reach itself through its slot and create a cycle. K-LSLOT-02's no-capture rule is therefore load-bearing for C2.
- **C5 bounded counters.** Counters and epochs have bounded width. Overflow refuses or traps and never wraps (`live-linear-storage.wat:50-67, 153-165` on 65535 and 2^24−1; epoch-exhausted slots retire permanently at `:1-3, 105-121`).
- **C6 quiescent collection.** Collection runs only when no frame, continuation or dispatch guard can hold an uncounted reference (`$storage_busy`, `live-linear-storage.wat:45-47`). Any host-held raw handle must be a counted root.
- **C7 separation.** Storage reclamation never discharges a host-resource obligation (BE-RESOURCE-01).

The no-tracing claim is scoped to the guest-cell algorithm only. The host's version-retention bookkeeping walks the guest graph (`slot-engine.mjs:766-791`), and Node/V8 or Wasm GC engine memory is managed independently. Neither is covered.

**Rationale:** These are exactly the premises that the existing zero-count-scan and cascade relies on. The issue warns that DAG arguments over recycled handles or hashes are invalid, and C2 replaces them with a sound order.

### Decision: Proof obligations and correspondence layers

**Choice:** Record the following open obligations. Each remains a **Proof obligation**; none is discharged here.

| ID | Statement | Layer | Related registry |
|---|---|---|---|
| OWN-PO-1 | Normal transitions preserve the whole-configuration owner multiset modulo declared move, consume and release (composition, branches, `dip`, `run`, dynamic Programs) | abstract model | PO-05, SO-04 |
| OWN-PO-2 | Scoped borrows never escape: no reachable value or interface mentions a closed scope tag | abstract model + checker | PO-01, PO-06 |
| OWN-PO-3 | Capture eligibility is recursive and resource-free environments are preserved by `quote`/`compose` | abstract model + checker | PO-06 |
| OWN-PO-4 | `Pkg(P)` closure: every member read during any later execution of P is retained while P has an owner | Rust lowering + Wasm runtime | PO-17, PO-18 |
| OWN-PO-5 | Reclamation safety: a cell is reclaimed only when no counted owner exists (C1–C6) | Wasm runtime | PO-13 |
| OWN-PO-6 | Abnormal cleanup: trap, cancellation or failed construction releases frame pins and retires invocation-owned handles exactly once, without resurrection | host ledger | V-RESOURCE-03, PO-13 |
| OWN-PO-7 | Host ledger correspondence: host owner tokens, version counts and code-span retirement agree with backend counters | host ledger ↔ runtime | PO-14 |

The layers are separated as follows. (i) An *abstract-model* proof (for example in Lean) of OWN-PO-1..3 says nothing about the actual checker, emitted Wasm or host. (ii) *Checker refinement* (PO-11-style) is required for OWN-PO-2/3. (iii) *Rust lowering correspondence* and (iv) *Wasm runtime correspondence* are required for OWN-PO-4/5. (v) *Host-ledger correspondence* is required for OWN-PO-6/7.

Every layer assumes the following:

- **Roots**: the root set is exactly operands, continuations, frames, held claims, saved tokens, module roots and map pins.
- **Reentrancy**: no guest re-entry during collection or retirement; the host serialises `storage_busy`.
- **Counters**: fixed widths with refusal at saturation.
- **Handle reuse**: epoch-qualified handles never wrap, and stale handles fail liveness.
- **Failure behavior**: any nonzero status poisons the instance (`engine.mjs:1012`; `source.wat:1-3`). Cleanup is host-side, and process death reclaims everything without a claim of external exactly-once.

### Decision: Conformance scenario designs (unexecuted)

**Choice:** The following **Planned tests** are registered only in this document. Each has `implementation: absent`, `execution: not-run`, `proof: open`, `trust: unassessed` and `evidence: []`. They are not added to `specs/conformance`.

| ID | Input | Expected observation |
|---|---|---|
| OWN-CASE-01 saved capture after invocation exit | root R1: `def k [ quote ]`, input `5`, save the returned Program; R1 exits; root R2 runs the owner with `20` via `swap [ run ] dip +` | `25`; the capture cell is still live after R1; zero stale-handle traps |
| OWN-CASE-02 shared duplicates released in both orders | `[ 1 + ] dup` saved as two owners A, B; variant 1 releases A then B, variant 2 B then A, with a run between releases | the remaining owner runs successfully after the first release; storage reclaims only after the second; identical final live count |
| OWN-CASE-03 nested composition | left, right and balanced 8-leaf `compose` trees of captured I64 Programs, each saved, then intermediate owners dropped | results equal; every intermediate is reclaimed only when unreferenced; no leaf freed while the root lives |
| OWN-CASE-04 resource rejection | `dup`, `drop`, `quote`, `Pair` then `quote`, `Sum<R,I64>` then `drop`, `List<R>` then `drop`, `borrow` escape variants of Decision 1 | each is statically rejected, with zero emission and zero guest requests |
| OWN-CASE-05 trap during escape | a root that saves a Program, then traps before return | no saved-owner token is issued; frame pins are released exactly once; the instance is poisoned; no cell from the failed root is reachable from a retained owner |
| OWN-CASE-06 module-root retirement with a saved named caller | module M defines `def h [ 1 + ]` and `def g [ h ]`; a returned saved Program from M's compiled code calls `g`; M's version and replay owners are released and `release_module_roots` runs; the saved owner is then invoked | either the invocation returns the correct value, or retirement is refused while the owner lives. A stale-handle trap is a failure. This is the reproducer for the issue's correspondence question. |
| OWN-CASE-07 retained old definition | session `def n [ 1 ]`, `[ n ]` saved, `def n [ 2 ]`, old module retirement requested | saved Program yields `1`; the old code span is not cleared while the saved owner lives; retirement succeeds after release |
| OWN-CASE-08 arena reset with escaping capture | profile with scratch reset: a root returns a Program capturing a scratch-built Text | promotion happens before reset; the returned Program reads identical Text afterward |
| OWN-CASE-09 borrow profile positive | `dir [ "a" fs.read.borrowed borrow.dup "b" fs.read.borrowed ] borrow` | two reads; `Directory` restored exactly once; no borrow outlives the scope |
| OWN-CASE-10 counter saturation | 65,535 incoming references to one cell, then one more | refusal or specified trap; never wrap; existing owners stay valid |
| OWN-CASE-11 self-recursive package reclamation | self-recursive definition saved, then released | package reclaimed; self reference not counted as an owner |
| OWN-CASE-12 data drop never releases a resource | `Pair<I64,Text>` dropped while a resource is held by `dip` | storage claim ends; the resource stays live and is returned exactly once |

## Risks / Trade-offs

- OWN-CASE-06 may reveal that module-root retirement unroots globals that a surviving saved Program's code still reads. Fixing that would need code→global owner edges or module lifetime extended to saved callers. Either option increases retention.
- The borrow profile's no-aggregate rule rejects useful idioms, such as a pair of borrows. Relaxing it needs region inference beyond this bounded model.
- A scratch arena requires escape promotion at every boundary. A missed boundary is a lifetime violation, not a performance bug.
- Deferred RC scans cost O(heap cursor) per collection (`live-linear-storage.wat:245-260`). The cost is bounded by 4,096 cells today but grows with any larger profile.

### Remaining decisions and evidence before any implementation claim

1. Run OWN-CASE-06 and decide whether code→global edges are counted or module lifetime is extended. Accepted evidence needs a source-bound execution receipt.
2. Select or reject the borrow profile, with concrete syntax (out of scope here) and checker refinement evidence for OWN-PO-2.
3. Choose a promotion form for any scratch-arena profile, with measured costs (#5 benchmark plan).
4. Mechanize OWN-PO-1/3 in the abstract model, then obtain correspondence evidence per layer. Abstract proofs never imply runtime correctness.
5. Register OWN-CASE designs in `specs/conformance` only through a separate reviewed change.

## Acceptance mapping

| Issue #3 checkbox | Section |
|---|---|
| Recommend a bounded ownership/borrow model, with illustrative stack transitions and rejected escape/capture examples | Decision: Bounded ownership and borrow model (transitions and rejected examples); Decision: Stack operations as ownership transitions; r[OWN-BORROW-01] |
| Compare scratch arenas plus retained packages, closed-package copying, and immutable sharing/deferred RC, including costs and profile compatibility | Decision: Scratch storage versus retained packages (table); Decision: Sharing by default, copying only at boundaries; Decision: Owned saved-Program package is the lifetime unit |
| State proof obligations for ownership preservation under composition, nonescaping borrows, capture eligibility, complete package dependencies, safe reclamation, and normal/abnormal cleanup | Decision: Proof obligations and correspondence layers (OWN-PO-1..7); Decision: Reclamation without tracing GC (C1–C7) |
| Separate abstract-model proofs from actual checker, Rust lowering, Wasm runtime, and host-ledger correspondence; list assumptions about roots, reentrancy, counters, handle reuse, and failure behavior | Decision: Proof obligations and correspondence layers (layers (i)–(v) and assumptions); r[OWN-EV-01] |
| Define conformance tests for saved captures after invocation exit, shared duplicates released in different orders, nested composition, resource rejection, traps, module-root retirement, and retained old definitions | Decision: Conformance scenario designs (OWN-CASE-01..07, plus 08–12) |
| Document remaining decisions and evidence needed before any implementation claim | Risks / Trade-offs → Remaining decisions and evidence before any implementation claim |
