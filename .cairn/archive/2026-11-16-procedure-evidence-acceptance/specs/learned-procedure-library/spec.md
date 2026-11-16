# Learned procedure evidence acceptance

## MODIFIED Requirements

### Requirement: LPL-03
r[LPL-03]

**LPL-03.**

A promotion state MUST be exactly one of `candidate`, `tested`, `promoted` or
`superseded`. `tested` MUST require at least one accepted `test`-class
evidence record whose verified execution outcome is `passed`, bound to the
exact subject and the selected fixture set, and it MUST NOT imply any proof.
`promoted` MUST require that the consumer-selected promotion policy names
every obligation it demands, and each demanded obligation MUST cite an
accepted evidence record of an allowed class whose verified outcome is
`proved`, with its exact subject, toolchain identity, command and input
binding, and policy revision. A `test`, `review` or `assumption` record MUST
NOT establish a `proved` outcome; a record asserting `tested` or `promoted`
without the matching accepted evidence MUST be refused as `forged-status`, and
an `assumption` record MUST remain visible and MUST NOT be counted as proved.

Evidence supplied with a record, by its producer, its submitter or any other
caller, is a claim. A claim MUST be retained as visible, unverified provenance
and MUST NOT by itself establish `tested` or `promoted`, whatever its class,
outcome, toolchain string, claim text, subject binding or policy revision.
Evidence is accepted only at admission, by a verifier that the library owner
supplied when constructing or loading the library; no record field,
submitter, digest, name, file path or persisted byte can select, replace or
bypass that verifier. Each accepted claim MUST be recorded as a separate
verification receipt minted by the library. The receipt MUST bind the exact
subject identity; the obligation, class and policy revision of the claim it
answers and a digest of that exact claim; the verifier and toolchain identity
that ran; digests of the command and inputs it ran; and the outcome the
verifier observed. The verified outcome, not the claimed outcome, decides
status. A caller MUST NOT be able to construct, edit or re-target a receipt,
and receipts carried by an incoming record MUST NOT be trusted: the admitting
library discards them and decides from its own verifier and its own retained
decisions. A verified proof outcome for a `test`, `validation`, `review` or
`assumption` class, or a receipt without its verifier, toolchain, command or
input binding, MUST be refused. Without an owner verifier every claim remains
unverified, so neither `tested` nor `promoted` can be reached.

A persisted receipt MUST NOT be trusted from bytes. Load MUST re-establish
every persisted receipt by asking the loading owner's verifier about the same
subject and claim, and MUST refuse `invalid`, naming the receipt, when no
verifier is supplied or its answer differs. Load MUST NOT mint a receipt,
upgrade a claim or repair a state, so a successful load reproduces the
identical identities, receipts, statuses, state digests and retrieval order
required by LPL-07. Rollback MUST refuse `invalid` when a receipt's toolchain
is unavailable, as LPL-06 requires for evidence toolchains. An export MUST keep
claims and receipts in separate fields.

A promotion MUST NOT transfer to a different body, capture list,
instantiation, dependency set, source revision, semantic revision, toolchain or
policy revision. Demotion or invalidation MUST be recorded as a new record
state or a new library generation; a retained record MUST NOT be edited, and
deleting a cited evidence reference MUST invalidate the promotion rather than
silently preserve it. Inspection of a promoted record MUST NOT start a prover,
fetch a proof file, invoke the owner's verifier or acquire authority
implicitly.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

#### Scenario: tested and proved stay separately visible

- GIVEN two records with the same interface and authority, one carrying only
  accepted passing `test` evidence and one carrying an accepted proof-class
  record for its exact obligation
- WHEN both are exported or retrieved
- THEN the test-only record MUST report `tested` rather than `promoted` under a
  policy that demands proof, both evidence classes MUST remain visible, and no
  export MAY collapse them into one status

#### Scenario: a forged promotion is refused

- GIVEN a record that sets `promoted` while its evidence records are a test
  outcome, a review note or an assumption, or whose toolchain identity is
  unavailable
- WHEN the promotion is admitted or verified
- THEN it MUST be refused as `forged-status` or `stale-context` for the exact
  obligation, and the previous state MUST remain authoritative

#### Scenario: promotion does not travel with a name

- GIVEN a promoted record and a new record with a changed body, capture or
  dependency but the same interface and name
- WHEN the new record is admitted
- THEN it MUST start at `candidate` and MUST NOT inherit any evidence,
  promotion or certification from the name, interface or template identity

#### Scenario: a claim is provenance, not accepted evidence

- GIVEN a record whose evidence claims `lean`/`proved` or `test`/`passed` with
  a nonempty toolchain string, the exact subject binding and the selected
  policy revision, and no receipt from the library owner's verifier for that
  claim
- WHEN it is admitted as `promoted` or `tested`
- THEN admission MUST refuse `forged-status` for the exact obligation or
  status, the library MUST remain unchanged, and the same claim admitted with
  `candidate` status MUST remain visible as unverified provenance

#### Scenario: a receipt is not trusted from a record or from bytes

- GIVEN a record cloned from a library whose verifier accepted its claims, or
  a persisted library carrying receipts
- WHEN the record is admitted to a library whose owner's verifier does not
  accept those claims, or the bytes are loaded without the verifier that
  re-derives each identical receipt
- THEN admission MUST decide status from its own verifier and refuse
  `forged-status`, load MUST refuse `invalid` at the first unconfirmed receipt,
  and retrieval, export and encoding MUST NOT invoke a verifier
