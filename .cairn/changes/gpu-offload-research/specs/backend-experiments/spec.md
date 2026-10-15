# GPU offload study for pure numeric Programs

## ADDED Requirements

### Requirement: GPU-SCOPE-01
r[GPU-SCOPE-01]

A GPU offload experiment MUST be an explicitly requested, nonselected profile. It MUST be entered only through an explicit typed batch operation whose reference semantics is the sequential, order-preserving application of an `I64 -> I64` Program to each element. Automatic inference of parallel regions MUST NOT be part of the initial profile. A site is GPU-eligible only if an independent checker establishes all of the following over the kernel-accepted candidate:

- the Program's code is statically known at preparation;
- its latent effect bound is empty, and it uses only `I64` literals, wrapping `+ - *`, `=`, `dup`, `drop`, `swap`, and `dip`/`if` with eligible quotation literals, plus calls to eligible nonrecursive resolved definitions;
- its captures are immutable `I64` or `Bool` scalars;
- its symbolic expansion stays within declared node, depth and register bounds.

Host effects, resources, aggregates, reflection, runtime-selected or runtime-composed Program invocation and profile-specific words MUST be ineligible. An empty effect bound alone MUST NOT establish eligibility.

### Requirement: GPU-DISPATCH-01
r[GPU-DISPATCH-01]

Eligibility checking and device-kernel generation MUST occur at preparation, and runtime `quote`, `compose` and `run` MUST NOT invoke a source or kernel compiler. Eligibility MUST NOT imply device selection. Runtime selection MUST choose only among the CPU path and already prepared kernels, under an explicit host policy, device capabilities, batch size and a declared cost model. The batch operation's work and allocation charge MUST be computed and precharged before device selection. The charge MUST be identical whichever device executes. Device selection, fallback and retry MUST NOT change result values, element order, quota outcomes or report classes; only diagnostics and timing may differ.

### Requirement: GPU-NUM-01
r[GPU-NUM-01]

Device kernels MUST implement K-NUM-01 exactly. `+`, `-` and `*` on `I64` MUST produce the low 64 bits of the mathematical result with signed interpretation, deeper operand before top operand, independent of optimization, workgroup shape or lowering variant. A device lacking native 64-bit integer support MUST use a separately tested exact multiword lowering, or refuse. Signed arithmetic with undefined overflow in a kernel source language MUST NOT be used. Division, floating-point, fused, saturating or approximate arithmetic MUST NOT be introduced.

### Requirement: GPU-FALLBACK-01
r[GPU-FALLBACK-01]

An empty batch MUST return an empty list without a device launch. Under a device-optional policy, an ineligible site, an absent or incapable device, a pipeline-compilation, launch or device-loss failure MUST discard all device results and produce the CPU result exactly once, with a recorded fallback reason. Under a device-required policy, an ineligible site MUST refuse at preparation with zero guest requests or launches. A runtime device failure MUST report an explicit device failure with no output published. Insufficient precharged budget MUST refuse before launch, with the same quota class on every device. A deadline or cancellation MUST be reported distinctly from quota exhaustion and traps, with no partial output. Device buffers and in-flight submissions MUST remain owned until device completion is observed and then be retired.

### Requirement: GPU-GATE-01
r[GPU-GATE-01]

No GPU backend MAY be selected without executed tests that compare CPU and every device lowering variant bit-for-bit against independently derived mathematical expectations. Those tests MUST cover overflow and 32-bit carry/borrow boundaries, operand order, scalar captures supplied after compilation and reused across launches, nested `dip`/`if`, empty, single-element, below-threshold and at-bound/over-bound batches, the eligibility-rejection matrix, every fallback and device-failure path, interruption, element order and repeated-launch determinism. Each run MUST record the pinned API, driver, device, kernel-generator and lowering variant. Existing Wasm arithmetic comparisons and proofs MUST NOT be inherited as GPU evidence.
