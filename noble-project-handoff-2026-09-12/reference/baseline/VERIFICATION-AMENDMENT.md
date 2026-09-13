# Core Specification Amendment — Verification

Document: AMEND-V001  
Revision: 0.1.0-draft.1  
Date: 2026-09-09  
Applies to: SPEC-0001, 0.1.0-draft.1  
Status: Proposed additive integration; no Project source or repository has been overwritten

## 1. Integration scope

This amendment supplements the supplied core specification with the verification commitment, checker boundary, optional evidence rules, selected implementation tools and stronger release gates. It preserves existing evaluation, typing, resource, recipe, identity and Wasm contracts. It introduces no source punctuation and does not resolve or revert parallel comment/token-efficiency or interactive-tooling work.

The [exact patch](amendments/SPEC-0001-verification.patch) is generated against the supplied baseline, whose fingerprints are in [baseline.json](amendments/baseline.json). The [integration preview](amendments/SPEC-0001-integration-preview.md) shows the resulting text; its unchanged lexical section is baseline context, not a new decision about comments. For a newer branch, merge only the identified verification changes rather than replacing the entire specification with the preview.

Copy the new specifications beside the baseline when applying the patch; preview-relative links are interpreted at that eventual root location. The existing baseline's original conformance fixture package was not supplied and is not recreated or represented as tested here. This package adds only its own verification scenarios.

Conformance must cite both `SPEC-0001@0.1.0-draft.1` and `verification-extension@0.1.0-draft.1`. The base revision field is preserved; an extension row is added so the amended text cannot be mistaken for the untouched baseline. The original RFC and decision register remain unchanged; [DECISIONS.md](DECISIONS.md) supplies the verification addendum.

## 2. Header and release-gate changes

Add the header row `Verification extension: 0.1.0-draft.1; SPEC-V001, SPEC-V002, IMPL-V001; no completed proofs claimed`.

Replace G-01 with the full algorithm plus explicit candidate/acceptance boundary. Add G-08 requiring actual reviewed Lean metatheory and checker soundness/termination for the complete stable subset. Add G-09 requiring scoped claim/evidence reporting, pinned tools and disclosed trust boundaries for advertised assurance. A plan alone no longer satisfies the stable-core proof gate.

The remaining inserted text follows. It is the same requirement text used in the patch, not a separate competing specification.

## 3. Add §2.1 before §3

**K-VERIFY-01.** Formal verification SHALL be a core development commitment under [SPEC-V001](VERIFICATION.md). Ordinary programs MUST retain the existing source forms and `Program<S,T,e>` interface. Handwritten behavioral proofs, a runtime prover, dependent types, and new proof punctuation are not required by this commitment.

**K-VERIFY-02.** Reports MUST distinguish mechanized language metatheory, verification of actual Rust implementation functions, optional behavioral proofs about Noble programs, and correspondence to executed Wasm. A proof for one target MUST NOT be reported as establishing the others.

The selected implementation profile uses Lean 4 for the reference semantics and metatheory, Charon/Aeneas to relate selected ordinary Rust functions to Lean models, and Verus for selected imperative components. These are proof/build-time tools, not a second production Noble evaluator. See [IMPL-V001](VERIFICATION-TOOLCHAIN.md).

## 4. Add §8.3 before §9

### 8.3 Verification boundary

**K-CHECK-04.** Inference and elaboration SHALL produce an explicit resolved candidate for an acceptance checker. The checker MUST validate the relevant interfaces, references, witnesses, eligibility and effect constraints without trusting a candidate's claimed signature. The complete algorithm and witness encoding remain required follow-up specifications.

**K-CHECK-05.** Optional behavioral contracts MUST NOT be premises used to bypass the core stack, resource or effect checks. A checker timeout, unsupported construct or internal failure MUST NOT produce acceptance.

The reference checking relation SHALL be formalized in Lean 4. Soundness of the mathematical checker, refinement of an actual Rust implementation, and trust in extraction/build tools are separate obligations under SPEC-V001 and IMPL-V001.

## 5. Add §11.7 before §12

### 11.7 Optional behavioral verification

**P-VERIFY-01.** Successful `prepare` establishes the required executable interface, resource eligibility and effect bound; it MUST NOT be treated as a proof of an arbitrary behavioral postcondition. Editing and rechecking syntax MUST NOT automatically transfer evidence about the original program.

**P-VERIFY-02.** Ordinary `quote`, `compose`, `call` and `reify` MUST NOT invoke proof search. Reusable builder theorems may describe runtime-built programs, with their assumptions and instantiations checked separately by an explicit verification service when policy requires it.

Optional contracts and proof evidence are specified in [SPEC-V002](PROGRAM-CONTRACTS.md). Partial correctness, prefix safety and total correctness are distinct claims. Verifying untrusted proof source is not an undocumented part of preparation.

## 6. Add §12.4 before Part C

### 12.4 Verification evidence and identity

**P-ID-06.** Optional behavioral contracts and proof evidence SHALL be associated separately from canonical program identity. Adding a proof of unchanged code MUST NOT alter its recipe or definition/program-value identity. Evidence MUST nevertheless bind the exact subject or quantified family, captures/instantiation, semantic context, claim and assumptions to which it applies.

**P-ID-07.** A proof about a recipe MUST NOT be treated as proof about independently supplied executable bytes. The source/recipe-to-artifact correspondence and runtime authorization remain separate acceptance conditions. Canonical evidence transport is not standardized by this extension.

## 7. Add §13.5 before §14

### 13.5 Implementation verification profile

**W-VERIFY-01.** The selected reference verification implementation SHALL follow IMPL-V001: Lean 4 for the model, Charon/Aeneas for supported ordinary Rust refinement, and Verus for selected imperative components. Results MUST retain each toolchain's assumptions and explicitly status cross-tool bridges. No automatic Verus-to-Lean proof interchange is assumed.

**W-VERIFY-02.** Verification of Rust implementation components MUST NOT be reported as source-to-Wasm semantic preservation without evidence for the relevant compiler, artifact-loading and execution boundaries. Public/FFI/Wasm entry wrappers MUST validate preconditions not established by a verified caller.

Selected tools and compatible pinned versions are different decisions. This extension selects roles but supplies no tested version matrix or verified tool binaries.

## 8. Add §16.4 before Appendix A

### 16.4 Verification conformance

**C-VERIFY-01.** A verification report MUST identify the baseline specification revision and this verification extension revision. It MUST separate expected fixtures, executed tests, proof obligations and accepted proofs. The included verification package reports no completed language or implementation proofs.

**C-VERIFY-02.** Verification coverage MUST include runtime builder operands, reflection-sensitive observations, wrapping arithmetic, effect prefixes, recursive eligibility, hidden resource state, ordinary error results and applicable abnormal-cleanup boundaries. Excluded constructs MUST remain visible; a theorem for a smaller subset does not establish stable full-core coverage.

**C-VERIFY-03.** A claim of a verified Rust acceptance checker additionally requires correspondence for the actual implementation/configuration. A claim reaching Wasm additionally requires the relevant backend/loading correspondence. Assumptions, solver failures, timeouts and unsupported cases MUST NOT be reported as proved obligations.

See [SPEC-V001](VERIFICATION.md), [SPEC-V002](PROGRAM-CONTRACTS.md), [IMPL-V001](VERIFICATION-TOOLCHAIN.md) and [DEC-V001](DECISIONS.md) for the selected obligations, tool roles, policy and open work.

