# M3 kernel gate matrix: the extended fragment-v1 kernel (2026-09-18)

Task 2.4 evidence. The kernel extension (tasks 2.1–2.3) adds the `=`
contract (23-entry table), the B-CHECK-02 environment-validation walks
(`acceptance/validate.rs`: recursive definition dependencies and
user-declared recursive schemas), and the B-CHECK-05 witness-resolution
walk (`words/resolve.rs`: cyclic and mutually referential substitution
witnesses), with the candidate format bumped to `noble-candidate/v1`
(reference bindings entered the candidate schema; both revisions
controlled).

Toolchain (pinned): rust
`/nix/store/1yvh3d6y3fj3xk2dgwczrp1dj5svd92c-rust-default-1.96.0-nightly-2026-03-21`,
octet `/nix/store/vj0sg5rn291z78im7gd2cqzqzd0bq3zy-cargo-octet-0.1.0`.

## Gate outputs (exact commands, run from the worktree root)

| Gate | Command | Outcome |
|---|---|---|
| Build | `cargo build --offline --workspace --all-targets` | exit 0 |
| Tests | `cargo test --offline -p noble-kernel` | 9 suites, all ok: acceptance 11, budget 2, docexamples 2, dx01 3, fragment 6, property 2, recursion-schema-cycle 11 (+ lib/doctests 0) |
| Workspace tests | `cargo test --offline --workspace` | all suites ok (kernel suites above; noble-cli and lib suites green) |
| Clippy | `cargo clippy --offline --workspace --all-targets --all-features -- -D warnings` | exit 0, 0 errors, 0 warnings |
| rustfmt | `cargo fmt --check` | clean |
| Octet architecture | `rm -rf .octet/compiler-architecture/shards && cargo-octet check --workspace -- --all-targets --all-features` | exit 0; **Architecture findings: 0** (receipt `74f86a7c581e48fc87b24d7bcf74e1e6ad3f5030212f1c59c0f17204700ff16a`); lint phase warning-only — pre-existing production findings and DX test-file style warnings only, unchanged in kind from the M2 record |

## New kernel surfaces

- `contracts::bootstrap::equals` — `= : S I64 I64 -- S Bool ! {}`
  (K-NUM-01); `Behavior::Equals`; table entry 7 of 23 in `Definition`
  order (`dup drop swap dip + - * = quote compose run reflect unit pair
  unpair inl inr case if nil cons list.case test.emit`).
- `contracts::{SchemaId, SchemaDecl}` and `Env::{deps, schemas}` — the
  external-environment-data surface; `environment()` builds the fixed
  table with no dependencies and no schemas.
- `acceptance::validate::{dependencies, schemas}` — the B-CHECK-02
  bounded, work-charged rejection walks (cycle detection by iterative
  three-color DFS naming the identity; per-edge and per-entry charging
  against the declared work limit; the bootstrap table charges nothing).
- `words::Binding::Ref` and `words::resolve::resolve` — reference
  bindings (type equations) with the B-CHECK-05 resolution walk
  (charge-before-hop, pigeonhole cycle detection, kind checking at the
  terminal binding); `acceptance::parts::instantiate::apply` resolves
  every witness before substitution and returns the resolved witness.
- `untrusted::CANDIDATE_FORMAT = 1`; `UnsupportedKind::{RecursiveDependency,
  RecursiveSchema}`; `Constraint::CyclicWitness`;
  `InstError::{CyclicWitness, WalkExhausted}`.

## New control surfaces (task 2.2/2.3)

- `tests/recursion-schema-cycle.rs` (11 controls): self and mutual
  recursive dependencies (naming the definition), the acyclic boundary
  companion and its exhausted companion, preflight-before-body-check,
  recursive schema naming its declaration, non-recursive schema boundary
  and exhaustion, self and mutual cyclic witnesses (invalid), resolvable
  chains (accepted), the witness-walk boundary and exhausted companions,
  and kind-crossed references.
- `tests/acceptance.rs` (+5 controls): the per-word table (23 entries,
  each with one accepting and one rejecting fixture naming its violated
  constraint), the duplication-shares-one-interface negative, the
  `Sum<Resource<R>, I64>` payload-exposure positive with its
  non-`Data`-sum negative, the explicit-union (`if` over an emitting and
  a quiet branch against an empty allowance) negative, and the
  advertised-refinement (`Sum<I64,Bool>` read as `Sum<Bool,I64>`)
  negative.
- `tests/fragment.rs` (+2 controls): `=` substitution
  (`S I64 I64 -- S Bool` exactly) and the words-level resolution walk
  (chains resolve with charged hops; cycles and kind crossings reject;
  fuel below the hop count fails closed).
- `tests/property/*`: the pool is all 23 entries plus the resource-maker
  fixture (`fit::WORDS`, 24 ids) with `=`, `case`, and `list.case` fit
  arms; the oracle gains independent `=`, `case`, and `list.case` arms
  written from the documented contracts; the generator emits complete
  eliminator exercises (discriminator + two branches sharing a claimed
  interface + the word), so the eliminators genuinely fire —
  `case=171 if=173 list.case=161` over the 1000-candidate agreement lane
  (0 disagreements); the malformed lane adds the `cyclic-witness`
  corruption (23 rejections in 200; 0 accepted, 0 panics).
- `tests/docexamples/decode.rs`: the word table with `=` and the
  renumbered ids; `noble-candidate/v1` decodes to the supported revision
  and every other format string (including `v0`) decodes to a foreign
  revision — both revisions controlled.

## Regeneration re-binding (the kernel-change duty, 2026-09-18)

The loop recorded in `proofs/m2/NobleKernel.lean`'s header (pinned
charon `b104e24f` → aeneas `505b6ca3`, `-split-files -gen-lib-entry
-all-computable`) re-run after the extension: **charon exit 0, aeneas
exit 0** (two extraction-subset repairs were forced on the way, both
matching the M2 probe-ladder refusal classes:

1. `words/resolve.rs` — an option-combinator closure over the borrowed
   arena (`and_then(|slot| inst.bindings.get(slot))`) hit
   `InterpBorrowsCore` "Can't end abstraction 5 as it is set as
   non-endable"; replaced by two explicit branches (the M2
   "option-combinator closures became branches" discipline).
2. `acceptance/validate.rs` / `words/resolve.rs` — loop conditions of the
   shape `while … && failure.is_none()` where `failure` is written but
   never read in the body made Aeneas emit a bare `none` whose implicit
   type Lean could not synthesize (`don't know how to synthesize implicit
   argument T`); the loops now exit by `break`, keeping the condition
   numeric/state-only.

After re-extraction: `proofs/m2` regenerated (Funs.lean 9066 lines,
Types.lean 490), one new external routed to the split-file template —
`core::slice::Slice::last` — implemented in `FunsExternal.lean` as
`ok s.val.getLast?` inside the `noble_kernel` namespace (modeled in the
coverage record with the other std mirrors). Reference-model bindings
updated for the v1 shapes without changing any theorem statement:
`embedEnv` embeds empty `deps`/`schemas`; `embedBehavior` gains
`.equals`; the projections gain `CyclicWitness`/`RecursiveDependency`/
`RecursiveSchema` arms over new reference constructors; `embedCandidate`
maps the reference format marker through `+1` (the supported revision
advanced to 1 with the schema change — documented at the definition);
the reference table gains `= : S I64 I64 -- S Bool ! {}` after the three
arith entries so the definition order matches the kernel again.

Gate re-runs on the regenerated tree:

```console
$ lake build NobleM2 NobleM2.Refinement NobleKernel   # proofs/m2, lean 4.31.0
info: ... build succeeded
$ verification/m2-coverage-gate.sh
[m2-coverage] PASS COVERAGE-JOIN (total=515 classified=515 extracted=491
  proved=1 modeled=18 excepted=5 open=0) … PASS (all checks)
$ verification/m2-proof-gate.sh
[m2-gate] PASS TOOLCHAIN … PASS REQUIRED-THEOREMS (inventory, axiom
  policy, subject binding) … PASS (all checks)
```

The coverage record `verification/m2-coverage.json` was re-derived from
the regenerated inventory (474 → 515 functions; the 41 new generated
functions — the `words::resolve` walk, `acceptance::validate` walks,
`bootstrap::equals`, and the `SchemaId`/`SchemaDecl`/`Env`-field
machinery — are classified `extracted`, `core.slice.Slice.last` is
`modeled` with the other std mirrors; the `acceptance.check` `proved`
classification and its thirteen citations survive verbatim). The twelve
M2 refinement theorems still prove against the regenerated checker:
the extension is behavior-preserving on the whole v0 fixture matrix.
