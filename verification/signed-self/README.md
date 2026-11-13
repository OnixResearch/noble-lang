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

The lane builds the CLI offline from the clean commit and runs five production framed sessions (`noble session --framed --declared-modules --text-byte-cursor`). Every report has zero guest requests and protected operations.

| Case | Session | Observation |
|---|---|---|
| REC-01 | signed | `def down [ dup 0 = [ ] [ 1 - down ] if ]` with signature `S I64 -- S I64 ! pure`: `5 count@1.down` is `normal` with `I64 0` |
| REC-02 | signed | `def recur [ recur ]` without a signature: refused at resolve, "cyclic local module definitions" |
| REC-03 | signed | `def recur [ 1 recur ]` against `S -- S`, a polymorphic self call: refused at check |
| REC-04 | signed | mutual recursion `even`/`odd`: refused at resolve |
| REC-05 | signed | `def forever [ forever ]`, a tail runaway: `runtime-exhausted` on the `execution work` quota, never `normal`, and the session ends with exit 5 |
| REC-06 | signed-non-tail | `def grow [ grow 1 drop ]`, a non-tail runaway: `runtime-exhausted` on `continuation bytes`, exit 5 |
| REC-07 | ordinary | the same signed countdown under `Declared-Text-v1`: refused at resolve |
| REC-08 | signed-deep | the positivity-guarded tail countdown `def down [ dup 0 swap < [ 1 - down ] [ ] if ]`, run at 5000 (about 9.8 times the 512-slot budget): `normal` with `I64 0`, peak continuation bytes 8 of 4096, 50034 of the 100000 execution steps |
| REC-09 | signed-deep-control | the same countdown made non-tail by `0 +` after the call, run at 5000: `runtime-exhausted` on `continuation bytes`, peak 4096, exit 5, never `normal` |
| REC-10 | signed-deep | the non-tail countdown at 511, the deepest start of that shape that fits the budget: `normal` with `I64 0`, peak continuation bytes 4096 |
| REC-11 | signed-deep | the non-tail countdown at 512: `runtime-exhausted` on `continuation bytes`, peak 4096, exit 5 |

The retained receipts are `scenarios-8b476c63.json` (REC-01…REC-07, from the clean commit `8b476c63`) and `scenarios-ebf3a877.json` (REC-01…REC-11, from the clean commit `ebf3a877`; each conformance case binds the receipt it was measured in). The conformance cases are `specs/conformance/recursion-cases.json`. `crates/noble-cli/tests/smoke.rs` (`signed_self_recursion_runs_at_its_signature_and_a_runaway_ends_in_a_quota`) pins the REC-01…REC-06 observations in the regular test suite.

Run the lane with the selected Node, from a clean worktree, into a directory outside it:

```
SELECTED_NODE verification/signed-self/scenarios.mjs NEW_EXTERNAL_DIRECTORY
```

## Decision: opt-in signed recursion, and why no `return_call` is emitted

Recorded for #99 (owner addendum of 2026-11-08). Three calls, each with its evidence and its reopen condition.

**Signed recursion stays opt-in.** The issue body never asks for a default change, and a default change is a language-profile decision with visible identity consequences. DX-TEXT-01 requires a session that selects both `--declared-modules` and `--text-byte-cursor` to report itself as `Declared-Text-v1` (`.cairn/specs/developer-experience/spec.md`), and that name comes from `declared_profile` (`crates/noble-cli/src/core/output/declared.rs:152`). Enabling signed recursion by default would rename every such report to `Signed-Declared-Text-v1` and make REC-07 — the ordinary profile refusing the same self reference — unreachable under the same flags. The flag is already gated: it requires `--declared-modules`, and for a session `--framed` (`crates/noble-cli/src/core/arguments.rs:118-124`). Reopen if the owner selects the signed profile as the language default; that change needs a new profile revision and a re-authored REC-07.

**`return_call` is not applicable, and the guarantee it was asked for is measured instead.** A named call is not emitted as a Wasm `call`: it lowers to `Action::Call` → `(call $enqueue_program …)` (`crates/noble-wasm/src/source/emit/functions.rs:122`), a hand-off to the runtime's continuation stack. Only the bootstrap words emit direct `$op_*` calls, and no bootstrap word recurses. There is therefore no Wasm call in tail position for `return_call` to rewrite. The property the addendum asks of a tail call — "a deep-recursion fixture that completes with tail calls, and ends as the fuel or trap failure without them" — is a property of the continuation stack, and REC-08…REC-11 measure it:

- a call that is the final operation of its program enqueues no continuation (`functions.rs:53-67`), and an `if` branch is itself enqueued and entered through the trampoline (`crates/noble-wasm/runtime/operations.wat:68-75`), so a final call inside a branch keeps the depth constant too;
- `$enqueue` refuses only when the next slot would exceed `continuation_limit` (`crates/noble-wasm/runtime/stack.wat:61-72`; 4096 bytes at 8 bytes per slot, `crates/noble-wasm/runtime/common.wat:15`), so the frame budget is 512 slots;
- a 5000-deep tail countdown completes at a peak of 8 continuation bytes, about ten times the 512-slot budget; the same countdown with one operation after the call peaks at 4096 and ends in the quota at 512.

Reopen if the lowering starts emitting a Wasm call for a named definition, or if a `return_call` path is added.

**Tool tail-call support is irrelevant while no `return_call` is emitted, and it is recorded anyway.** The M3 backend lane probes `return_call` in V8 and wasm-tools (`tools/m3-wasm.mjs:295`, the `tail_call` probe), and the runtime configuration records `"tail_call": "probed; not-required-or-used-by-workload"` (`crates/noble-cli/src/core/runtime/config.json:99`). Pinned binaryen is not probed. The addendum's third ask — gate the lowering on that support — has nothing to gate while no `return_call` is emitted; a future lowering that emits one MUST first re-check those probes and add the binaryen probe.

## Limits

- **No correspondence.** The model theorem is about the reference model. The retained kernel extraction predates `SignedSelf`, so no correspondence with the Rust kernel is claimed.
- **What the model leaves out.**
  - module ownership;
  - the public checker's refusal of `SignedSelf` nodes;
  - self edges through ordinary invocations, which the whole-submission dependency audit refuses (B-CHECK-02).
- **Opt-in only.** Signed recursion is available only in the opt-in profile; the ordinary profiles still refuse every self reference.
- **Tail calls are trampolined, not Wasm tail calls.** No `return_call` is emitted. A call in final position leaves the continuation depth constant (REC-08, REC-10), so a tail runaway ends on the step quota (REC-05) rather than the frame quota, while the same recursion with an operation after the call ends on `continuation bytes` (REC-06, REC-09, REC-11).
