# Signature-checked recursion (K-CHECK-02, #99)

> "Recursive definitions require an explicit signature and check recursive calls against that signature without polymorphic recursion."

Two lanes give this requirement a model theorem and executed scenarios. They build on what the `ma-language-prerequisites` change implemented, under the opt-in `--signed-self-recursion` profile:
- the frontend binds a self reference to its enclosing signed owner;
- the kernel's `check_signed_structure` types each `SignedSelf` site as an invocation of the owner's scheme and requires every site's witness to be the owner witness.

## Reference model: `check.mjs`

`proofs/m3/NobleM2/SignedSelf.lean` adds the signed check to the M2 reference model.

**Representation.** A signed candidate marks its self sites by index. Each marked node is the owner invocation the kernel types it as (`crates/noble-kernel/src/acceptance/nodes.rs`), so the existing `Derives` judgment and `check_soundness` apply to the body unchanged.

**The checker.** `checkSigned` mirrors the kernel's signed preflight (`preflight.rs`). Each check below refuses with `unsupported nodeForm`, then the reference checker runs:
- the owner is a named definition;
- its declared signature, at the owner witness, is the expected interface;
- every marked site invokes the owner at that witness.

| Theorem | Claim |
|---|---|
| `checkSigned_soundness` | acceptance implies the reference checker's well-formedness and `SignedDerives`: the body derives the owner's declared signature at the owner witness, and every self site uses that witness |
| `checkSigned_refuses_polymorphic_recursion` | a self site at any other witness is refused |
| `checkSigned_refuses_a_foreign_self_site` | a self site invoking another definition (mutual recursion) is refused |
| `checkSigned_requires_a_named_owner` | an owner with no declared signature is refused |
| `forever_accepted`, `forever_signed_derives` | `def forever [ forever ]` is accepted at `[] -- [] ! pure` and satisfies `SignedDerives` |
| `polymorphic_ordinary_check_accepts`, `polymorphic_signed_refused`, `monomorphic_accepted` | the ordinary checker types a self call that instantiates `S -- S` at a different stack; the signed check refuses it and accepts the same body at the owner witness |
| `unsigned_owner_refused`, `foreign_self_site_refused` | a bootstrap word cannot own a self call, and a site invoking another named definition is refused |

The retained receipt is `receipt.json`, from the clean commit `8b476c63`. The lane reads the module's import closure from the sources and requires it to stay inside the reference model: no extraction, no Aeneas, no Mathlib. It builds that closure in an isolated project with the pinned Lean 4.35.0 and audits a closed theorem inventory (`proofs/m3/signed-self-inventory.json`).

Every theorem uses only `propext`, `Classical.choice` and `Quot.sound`, with one exception: three acceptance fixtures, and `forever_signed_derives` through one of them. An acceptance runs the whole reference checker, and its effect union `EffSet.union` is well-founded recursion the kernel cannot reduce. Those fixtures are therefore closed by `native_decide`, as the model's own acceptance fixtures (`Regression.ce_accepted`, `Termination.accepted`) are. The lane allows that axiom only on an inventoried evaluation fixture, and the refusals are decided by the kernel itself.

## Executed scenarios: `scenarios.mjs`

The lane builds the CLI offline from the clean commit and runs three production framed sessions (`noble session --framed --declared-modules --text-byte-cursor`). Every report has zero guest requests and protected operations.

| Case | Session | Observation |
|---|---|---|
| REC-01 | signed | `def down [ dup 0 = [ ] [ 1 - down ] if ]` with signature `S I64 -- S I64 ! pure`: `5 count@1.down` is `normal` with `I64 0` |
| REC-02 | signed | `def recur [ recur ]` without a signature: refused at resolve, "cyclic local module definitions" |
| REC-03 | signed | `def recur [ 1 recur ]` against `S -- S`, a polymorphic self call: refused at check |
| REC-04 | signed | mutual recursion `even`/`odd`: refused at resolve |
| REC-05 | signed | `def forever [ forever ]`, a tail runaway: `runtime-exhausted` on the `execution work` quota, never `normal`, and the session ends with exit 5 |
| REC-06 | signed | `def grow [ grow 1 drop ]`, a non-tail runaway: `runtime-exhausted` on `continuation bytes`, exit 5 |
| REC-07 | ordinary | the same signed countdown under `Declared-Text-v1`: refused at resolve |

The retained receipt is `scenarios-8b476c63.json`, from the clean commit `8b476c63`. The conformance cases are `specs/conformance/recursion-cases.json`. `crates/noble-cli/tests/smoke.rs` (`signed_self_recursion_runs_at_its_signature_and_a_runaway_ends_in_a_quota`) pins the same observations in the regular test suite.

## Limits

- **No correspondence.** The model theorem is about the reference model. The retained kernel extraction predates `SignedSelf`, so no correspondence with the Rust kernel is claimed.
- **What the model leaves out.**
  - module ownership;
  - the public checker's refusal of `SignedSelf` nodes;
  - self edges through ordinary invocations, which the whole-submission dependency audit refuses (B-CHECK-02).
- **Opt-in only.** Signed recursion is available only in the opt-in profile; the ordinary profiles still refuse every self reference.
- **No tail calls.** Calls run through the runtime's trampoline. A tail runaway therefore ends on the step quota rather than the frame quota, and no `return_call` is emitted.
