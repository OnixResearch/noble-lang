## Context

The kernel already checks complete ordered interfaces. MC1 prepares explicit monomorphic program quotations, but no Noble Wasm backend exists. M3 is the resource-free feasibility comparison, not the similarly named checker proof directory. Both candidates must run the same workload or retain an evidenced blocker; at least one must pass every M3 execution gate.

## Decisions

### Decision: Shared checked lowering and compiled dispatch

**Choice:** Add a `no_std` + `alloc` `noble-wasm` crate depending inward on `noble-kernel`. Its public compiler validates raw candidate/request inputs before lowering or publishing a module. The CLI uses existing MC1 preparation for the fixed experimental vocabulary; the backend independently checks that raw boundary rather than trusting forgeable `Checked` fields. Pure lowering/emission follows existing explicit Result/worklist patterns and enters the actual Charon/Aeneas lane from the outset.

**Rationale:** Reusing the current frontend avoids a second parser. A narrow experimental command does not imply a general M4 source compiler or require ordinary applications to prove contracts. Backend admission compares complete type sequences, not only arity; unsupported words/types and invalid candidates produce no artifact.

**Choice:** Emit native Wasm operations and statically compiled continuation functions. A fixed function table dispatches `(environment_handle) -> ()` entries. A bounded explicit continuation stack schedules dynamic `run`, selected list branches, and left/right composition children. Quotes retain captures; compositions retain already compiled program entries. Semantic recipes are immutable data used only for reflection.

**Rationale:** Static continuations preserve execution after nested `run` without native recursion proportional to composition depth. Runtime trees neither create Wasm types nor select opcodes from recipes. Typed program interface IDs intern complete monomorphic stacks; runtime composition requires exact compatibility.

### Decision: Comparable bounded graph storage

**Choice:** Use a common private-handle ABI, unboxed tagged operand slots, and immutable 48-logical-byte graph cells. WasmGC uses typed structs with actual child references and a bounded root array. Managed-linear-memory uses 48-byte records. Both use bounded region lifetimes: an immutable compiled-vocabulary prefix plus a transient experiment region. Cleanup clears transient roots, frames and observed handles; it does not promise immediate physical GC reclamation or shrinking linear-memory pages.

**Rationale:** Identical graph algorithms isolate the storage choice. This is a comparison of these layouts, not a claim about every possible WasmGC or linear-memory implementation. GC physical cell size remains unknown; the common logical charge is a policy unit, not a physical measurement.

**Choice:** Bound transient allocation, retained recipe program leaves, composition depth, operand bytes, continuation bytes, and dispatch steps. The standard tree matrix uses 256 program leaves and depth 256. An increment/double leaf has two observable recipe atoms; 256 leaves therefore retain 512 normalized atoms. Construction quotas and invocation quotas are separate non-success outcomes; all paths account for transient cleanup.

**Rationale:** Left/right/balanced trees retain the same ordered leaves but have different pending-continuation requirements. Region rollback makes partial allocation failures observable and recoverable without pretending GC reachability retires resources; resources are unsupported.

### Decision: Pinned execution and honest measurements

**Choice:** Use Node 24.13.0 / V8 13.6.233.17-node.37 as a verification-only persistent-instance host, wasm-tools 1.245.1 as assembler/validator, and Binaryen 125 for `-O2 --strip-debug`, all selected through the locked Nix input. Record exact store/source identities, emitted module hashes, target, flags, input workload and limits. Node uses eager optimizing Wasm compilation with lazy compilation and tier-up disabled. GC, reference types, typed function references, multi-value, tail calls and components are listed separately; unused engine capabilities are not claimed as used module features.

**Rationale:** Scalar exports and imported-function absence make post-compilation inputs and the missing compiler service directly observable. JavaScript is only the experimental driver, not production lowering. Component interoperation, resources and ABI costs remain unmeasured M5 work.

**Choice:** Execute all specified cases and negative boundaries under optimization off/on. Compare complete recipes and wrapping results with independent host oracles, check imports, account logical allocations and cleanup, and record section sizes and repeated timing samples separately for preparation, engine compilation/instantiation, construction and invocation. Report process memory separately from guest logical allocation and linear memory. Unavailable per-object GC size or per-case engine peaks are explicit unknowns.

**Rationale:** Performance never outweighs failed correctness. ADAPT-12 exercises the selection gate with zero working candidates, fabricated measurements, missing pins and a fast incorrect candidate. Results remain experimental and do not discharge universal lowering/emission, generated Wasm, optimizer or engine correctness obligations.

### Decision: Managed linear memory for the M4 resource-free direction

**Choice:** Select `managed-linear-memory` after both candidates passed 52 scenarios under each optimization mode. The bounded arena makes its record storage observable and avoids requiring WasmGC. Preserve both experimental implementations and the full comparison artifacts.

**Rationale:** This is an accounting and engine-feature tradeoff, not a speed or physical-memory victory. The selected candidate retains 131072 linear-memory bytes after cleanup versus 65536 for GC; optimized modules are 6984 versus 6879 bytes. GC's physical cells, root table, collection cost and reclaimed bytes remain unknown. The linear representation carries manual record/tag/bounds invariants. Selection does not close M4, M5, PO-17/18 or SO-07.

## Risks / Trade-offs

- Owned WAT runtime fragments, assembler, optimizer, Node/V8 and driver/oracles are explicit non-Rust trust boundaries, not covered by Aeneas extraction.
- The GC root-array region policy intentionally retains graphs until cleanup; measured memory/performance applies only to that policy.
- Process high-water memory can include driver/oracle costs. Record its exact scope and do not call it a per-program GC measurement.
- Interface specialization is finite and declared. Unsupported programs fail explicitly; no hidden owner-slot erasure, general backend completeness or host-effect claim is permitted.
- Existing quality, compiler-unit collection and source-inventory gates must include the new crate and support assets without dropping targets or inventing proof coverage.
