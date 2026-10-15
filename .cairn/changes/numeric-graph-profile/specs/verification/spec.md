# Numeric graph derivative policies and assurance boundary

## ADDED Requirements

### Requirement: NGRAPH-DERIV-01
r[NGRAPH-DERIV-01]

A numeric graph training contract MUST name a versioned derivative policy that is separate from the forward semantics. `ngraph-deriv-real/0` MUST differentiate the intended real-valued arithmetic, ignoring FP32 rounding, and MUST give the ReLU and ReGLU gate a zero derivative at a zero gate, including `-0`. It MUST NOT be described as the derivative of the floating-point function. By default, admission MUST statically refuse a training contract in which any declared parameter reaches the loss through a hard boolean node or through the index operand of an index read. Straight-through, smooth-surrogate, zero-index and discrete-difference index rules MUST each be explicit per-node opt-ins and MUST be recorded in training provenance. Gradients with respect to selected candidates MUST be kept distinct from gradients with respect to the index. Reverse evaluation MUST return gradients only for declared parameters and MUST NOT mutate the checked Program, the graph or any parameter snapshot.

### Requirement: NGRAPH-PROOF-01
r[NGRAPH-PROOF-01]

Numeric graph assurance claims MUST separately state reference agreement, finite differential or test coverage, discharged theorem obligations and trusted compiler, runtime and toolchain boundaries. Extraction preservation, graph-checker soundness, compiled-evaluator refinement, FP32 reference-model correspondence, fact-transfer soundness and derivative-policy conformance MUST remain open obligations until separately proved or evidenced. Matching hashes, agreement between evaluators, finite differential tests and producer-supplied metadata MUST NOT be reported as discharging any of them.
