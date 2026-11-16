## Context

Base `605c1d55`. `crates/noble-contracts/src/source/procedures.rs` admitted a
record's status from its own `EvidenceRecord`s. `validate_record` required, for
`tested`, one record with `class == Test && outcome == Passed`, and for
`promoted`, one record per demanded obligation with `outcome == Proved`,
`class.proves()`, the promotion's policy revision and a class the consumer
policy allows. Every field of those records was supplied by the caller. The
only checks were that the subject binding equals the recomputed identity, that
test/validation/review/assumption records do not claim `proved`, and that the
toolchain and claim strings are nonempty. `demote` and reload applied the same
rule. Issue #131's reproducer, and the parallel forged `tested` control, both
returned `Ok` on base (the RED run is recorded in `tasks.md`).

The archived design (`.cairn/archive/2026-11-15-learned-procedure-library/design.md`,
D14) already said that the library "does not independently run a proof checker
on an arbitrary claim string" and that proof validity needs an independently
checked route. LPL-03 nevertheless let such a string satisfy a demanded
obligation, and the test `evidence_classes_policy_and_dependency_edges_are_not_laundered`
asserted that a fabricated `lean`/`proved` record was admitted under a current
policy. LPL-03 itself names the missing pieces: the cited record needs a
"toolchain identity, command and input binding", the scenario speaks of an
"accepted proof-class record", and inspection "MUST NOT start a prover, fetch a
proof file or acquire authority implicitly".

## Decisions

### Decision D1: an evidence record is a claim; acceptance is a separate sealed receipt

**Choice:** `EvidenceRecord` keeps its public shape and becomes the claim type,
retained unchanged as visible, unverified provenance. Acceptance is a separate
`VerifiedEvidence` receipt in the record's mutable state. A receipt's fields are
private, there is no public constructor, and only the library mints one.
`ProcedureRecord::verified_evidence()` exposes receipts read-only.

**Rationale:** Producers, the issue's reproducer and existing fixtures construct
claims with struct literals, so the claim shape must stay stable. A sealed
receipt cannot be built, edited or re-targeted by a caller. Keeping claims and
receipts in separate fields keeps tested, proved and claimed visibly distinct
in every view (LPL-03, LPL-08).

### Decision D2: the owner supplies one verifier at construction or load

**Choice:** `ProcedureLibrary::with_verifier(revision, Box<dyn EvidenceVerifier>)`
and `ProcedureLibrary::load_with_verifier(bytes, namespace, verifier)` bind the
trust root for the library's lifetime. `new` and `load` have none, so every
claim stays unverified and `tested`/`promoted` are unreachable. The trait has
one method, `verify(VerificationSubject, &EvidenceRecord) -> Option<Attestation>`.
`VerificationSubject` exposes only the identity fields (procedure id, chunk
envelope, provenance, library dependencies and identity-subject bytes), never
mutable status, promotion, cost or other claims.

**Rationale:** A verifier argument on each admission would let every
admission path choose its own trust root. Binding it to the library makes the
owner, not a record or a submitter-facing call, decide which checker counts.
An owner may implement the trait over any independently checked route (a
pinned Lean consumer, the MC2 checked-evidence path, an executed fixture
runner). The library ships none and claims none.

**Alternatives rejected:** a per-admission verifier parameter (the trust root
would vary by call site); treating `ConsumerPolicy.allowed_classes` as the
check (it is a whitelist, not verification); a built-in checker (no in-library
checker can validate arbitrary Lean, Verus or test artifacts without the
effects D14 excludes).

### Decision D3: what a receipt binds, and what decides status

**Choice:** The library copies the claim's obligation, class and policy
revision, its exact claim digest (domain-separated digest of the claim's
canonical bytes) and the recomputed subject identity into the receipt. It
copies the verifier identity, toolchain, command digest, input digest and
observed outcome from the attestation. The **verified** outcome decides
status: `tested` needs a receipt `test`/`passed`; each demanded obligation
needs a receipt `proved` of a proof class allowed by the policy, under the
promotion's policy revision. A verified proof outcome for a non-proof class is
refused `forged-status`. A receipt without a verifier identity, a nonempty
bounded toolchain, or nonzero command and input digests is refused `invalid`.

**Rationale:** These are exactly the LPL-03 binding fields. Copying the claim's
fields, rather than taking them from the verifier, means a verifier cannot
re-target its answer to another obligation, class, policy or subject. Letting
the observed outcome win records a failed recheck honestly instead of hiding
it.

### Decision D4: verification runs once per claim, at the admission that introduces it

**Choice:** For each claim of an incoming record, the library reuses the
decision retained for the same subject and identical claim, whether that was
a receipt or no receipt. Otherwise it asks the verifier once. Duplicate claims
within one record are asked once. Status validation then runs on the merged
receipts.

**Rationale:** This keeps LPL-04: re-admitting an identical record mints no
evidence, starts no new generation and does not consult the verifier. It also
makes the verifier's work proportional to new claims.

### Decision D5: receipts never travel with a record

**Choice:** Admission discards every receipt carried by an incoming record and
re-derives receipts from its own retained decisions and verifier.

**Rationale:** A cloned record carries receipts minted by whatever verifier
another library trusted. Discarding them closes the transplant path without
refusing legitimate re-admission into a library whose owner's verifier
re-derives the same receipts.

### Decision D6: persisted receipts are re-established at load, never trusted from bytes

**Choice:** Receipts persist in each record's mutable state, so the state
digest, generation closure and library revision cover them. The store magic
becomes `NPLIB002`; claims-only `NPLIB001` stores are refused `invalid` at
`magic`. Load asks the loading owner's verifier about every persisted receipt
and requires the identical receipt. It reuses an answer for a receipt already
re-established earlier in the same load. It refuses `invalid` at
`verified_evidence` when no verifier is supplied or an answer differs. Load
never mints a receipt, verifies a previously unverified claim or repairs a
status.

**Rationale:** Re-verification preserves LPL-07: a successful load reproduces
identical identities, receipts, statuses, state digests, manifests and
retrieval order. Downgrading receipts to claims at load was rejected. It would
change the state digests bound by every retained generation, so load would
either publish a repaired library, which LPL-07 forbids, or refuse every
library with a receipt anyway, and it would also silently demote states.

### Decision D7: rollback and inspection

**Choice:** Rollback refuses `invalid` at `verified_evidence.toolchain` when a
receipt's toolchain is unavailable, just as it does for claim toolchains
(LPL-06). Rollback and demotion use retained receipts and never consult the
verifier. Records, retrieval, the identity manifest, metrics, encoding and the
canonical chunk order never consult it.

### Decision D8: export

**Choice:** The identity manifest moves to `schema_revision` 1. Each entry keeps
its claims in `evidence` and adds `verified_evidence` (obligation, class,
outcome, subject binding, policy revision, claim digest, verifier, toolchain,
command and input digests). A non-claim states that only `verified_evidence`
supports `tested` or `promoted`.

### Decision D9: relation to D14

D14's **NO-GO** for proof-demanding promotion without checked Rust
implementation-refinement evidence is unchanged. A receipt records that the
owner's verifier accepted a claim. It is the owner's attestation, not a
model-to-implementation refinement, and the trust placed in that verifier is
the owner's explicit assumption. A consumer policy still has to name every
demanded obligation, including any refinement obligation, separately.

### Decision D10: architecture classification of the verifier call

**Choice:** `EvidenceVerifier::verify` is called through a trait object, so
Octet records a `DynamicDispatch` compiler unknown. The trait is listed in
`policy/architecture.ncl` `dynamic_dispatch.local_traits` as a reviewed
extension point, following the `CandidateRanker` precedent. The entry has an
owner, a reason and a reopen condition (an implementation gains a capability
effect or leaves the workspace, the trait grows, or the core calls it during
inspection or takes more of its answer). It is not a waiver and adds no
finding budget.

**Rationale:** A port the owner supplies is dynamic by design. A generic
parameter would also be a trait call on a generic receiver, and it would
thread the verifier type through every library signature without changing
what the core trusts.

## Test plan and evidence

- RED on base `605c1d55` (no other change): the issue's
  `fabricated_lean_proof_must_not_promote_procedure` and the control
  `unexecuted_test_claim_must_not_mark_procedure_tested` both failed.
- GREEN: the same two tests, now in
  `tests/editor/procedures/admission/acceptance/unverified.rs` with one added
  "nothing retained" assertion each, pass. The acceptance suite covers:
  forged-status refusals with no verifier and with an owner who never checked
  the claim; the positive owner-verified path with every receipt binding;
  idempotent re-admission without verifier calls and one call per new claim;
  transplanted receipts refused by other libraries and re-derived by the same
  owner; the verified outcome overriding the claimed one in status and export;
  malformed attestations; status filters and every inspection with zero
  verifier calls; demotion from receipts. The reload suite covers: two
  restarts reproducing manifest, bytes, records and filtered views with one
  call per distinct receipt; refusal without a verifier, with an owner who
  never checked, with a partial owner and with a dissenting owner; a store
  written under a lenient verifier; the revision-1 magic; and rollback
  requiring receipt toolchains.
- The corrected `evidence_classes_policy_and_dependency_edges_are_not_laundered`,
  criteria (now including the unverified and dissenting cases), schema,
  generated-retrieval and reload-parser tests route `tested`/`promoted`
  through an owner fixture ledger. The ledger stands for an owner's
  independently recorded checks and is not a proof checker.

## Risks / Trade-offs

- Trust moves to the owner's verifier. A wrong or permissive owner verifier
  still admits false claims. The receipt records which verifier, toolchain,
  command and inputs vouched, and load re-asks the loading owner, so a store
  cannot carry one owner's trust into another owner's library.
- A nondeterministic verifier makes its libraries unloadable. This is fail
  closed by design.
- `NPLIB001` stores no longer load. The repository retains none (searched for
  the magic); historical receipts retain manifests and byte counts, not stores.
- Historical receipts and the retained `promoted`/`tested` fixtures rested on
  claim-only evidence. They are not rewritten. The erratum in
  `verification/procedure-library/decision.md` records the correction.
