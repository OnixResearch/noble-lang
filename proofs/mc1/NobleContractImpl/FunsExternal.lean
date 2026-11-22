module
public import Aeneas
public import NobleContractImpl.Types
public import NobleContractImpl.StdModels
public import NobleContractImpl.KernelBridge
@[expose] public section

/-!
Models of the twenty-four crate-local `#[charon::opaque]` functions of the regenerated
`noble-contracts` subject, the reviewed boundaries that
`verification/mc1/implementation.mjs` admits by exact Rust name. Each names its
Rust item and why the model is faithful.

* `intrinsic::debug_form_kind`, `intrinsic::debug_proof_dependency` and
  `source::editor::debug_node` render a value exactly as `#[derive(Debug)]`
  did; they are opaque only because a transparent body would close a
  trait-impl cycle Aeneas refuses. Their bodies only write through the
  formatter and call the derived `Debug` of the fields. In the Aeneas
  formatter model every write succeeds and returns the formatter unchanged,
  and so does every leaf `Debug` model the derived impls compose (strings,
  scalars, booleans, sequences), so each helper is a successful step that
  preserves the formatter, like the inherited kernel's opaque `debug_*`
  helpers. No formatted text is claimed.
* The seven host plug-in calls go through the handles of
  `TypesExternal.lean`, which carry the host's behaviour; see the handle
  documentation there for why every Rust host is represented.
-/

open Aeneas Aeneas.Std Result

namespace noble_contracts

namespace HostPlugin

/-! ### Injective images of host-call arguments

Every structure is a node of its fields in declaration order, every enum the
index of its variant (with its payload), every option and sequence a tagged
node, and scalars, strings, the kernel's types (through the injective
`KernelBridge.toTy`) and effect sets are leaves. Each position has one Rust
type, so distinct arguments have distinct images. -/

def ofBytes (bytes : List U8) : Value :=
  .node 0 (bytes.map (fun byte => .nat byte.val))

def ofArray {n : Usize} (bytes : Array U8 n) : Value :=
  ofBytes bytes.val

def ofBool (value : Bool) : Value :=
  .nat (if value then 1 else 0)

def ofOption {α : Type} (encode : α → Value) : Option α → Value
  | none => .node 0 []
  | some value => .node 1 [encode value]

def ofList {α : Type} (encode : α → Value) (items : List α) : Value :=
  .node 0 (items.map encode)

def ofTys (tys : alloc.vec.Vec noble_kernel.types.Ty) : Value :=
  ofList (fun ty => .ty (KernelBridge.toTy ty)) tys.val

def ofBudget (budget : source.chunks.Budget) : Value :=
  .node 0 [.nat budget.library_bytes.val, .nat budget.entries.val,
    .nat budget.recipe_nodes.val, .nat budget.stack_height.val,
    .nat budget.dependency_steps.val, .nat budget.instantiation_bindings.val,
    .nat budget.quotation_depth.val, .nat budget.expanded_nodes.val,
    .nat budget.composed_chunks.val, .nat budget.source_bytes.val,
    .nat budget.candidates.val, .nat budget.diagnostics.val,
    .nat budget.checking_work.val, .nat budget.evaluation_steps.val,
    .nat budget.retained_bytes.val]

def ofRankingIdentity (identity : source.chunks.RankingIdentity) : Value :=
  .node 0 [ofArray identity.request, ofArray identity.task_interface,
    ofArray identity.independent_goal, ofArray identity.candidate_set,
    ofArray identity.library_revision, .text identity.feature_revision,
    .nat identity.semantic_revision.val, ofArray identity.policy,
    ofArray identity.editor_request, ofOption ofArray identity.model_identity,
    ofBudget identity.budget]

def ofInterface (interface : source.chunks.Interface) : Value :=
  .node 0 [ofTys interface.stack_in, ofTys interface.stack_out, .effects interface.effects]

def ofScalar : source.chunks.Scalar → Value
  | .I64Scalar value => .node 0 [.int value.val]
  | .BoolScalar value => .node 1 [ofBool value]

def ofCapture (capture : source.chunks.Capture) : Value :=
  .node 0 [.nat capture.index.val, ofScalar capture.value]

def ofDefinitionRef (reference : source.chunks.DefinitionRef) : Value :=
  .node 0 [.nat reference.identity.val, .nat reference.format.val,
    .nat reference.semantic_revision.val, ofArray reference.source_revision]

def ofBinding (binding : source.chunks.Binding) : Value :=
  .node 0 [.nat binding.«variable».val, .nat binding.kind.ctorIdx,
    .ty (KernelBridge.toTy binding.ty)]

def ofInstantiation (instantiation : source.chunks.Instantiation) : Value :=
  ofList ofBinding instantiation.bindings.val

def ofChunkNode : source.chunks.Node → Value
  | .I64Node value => .node 0 [.int value.val]
  | .BoolNode value => .node 1 [ofBool value]
  | .CaptureNode index => .node 2 [.nat index.val]
  | .OperationNode operation width => .node 3 [.nat operation.ctorIdx, .nat width.val]
  | .InvokeNode entry width => .node 4 [.nat entry.val, .nat width.val]
  | .QuotationNode body width =>
    .node 5 [ofList (fun node => .nat node.val) body.val, .nat width.val]

def ofRecipe (recipe : source.chunks.Recipe) : Value :=
  .node 0 [ofList ofChunkNode recipe.nodes.val, ofList (fun node => .nat node.val) recipe.entry.val]

def ofEnvelope (envelope : source.chunks.Envelope) : Value :=
  .node 0 [.nat envelope.format.val, .nat envelope.semantic_revision.val,
    .text envelope.profile, ofArray envelope.chunk_id, ofArray envelope.source_revision,
    ofInterface envelope.interface, ofList ofCapture envelope.captures.val,
    ofList ofDefinitionRef envelope.dependencies.val,
    ofList ofInstantiation envelope.instantiations.val, ofRecipe envelope.body]

def ofEncoder (encoder : source.procedures.Encoder) : Value :=
  .node 0 [.text encoder.id, .nat encoder.encoding_revision.val, ofBool encoder.experimental]

def ofProvenance (provenance : source.procedures.Provenance) : Value :=
  .node 0 [.nat provenance.producer.ctorIdx, .nat provenance.producer_tool_revision.val,
    ofOption ofArray provenance.task_id,
    ofOption (fun revision => .nat revision.val) provenance.grammar_revision]

def ofLibraryDependency (dependency : source.procedures.LibraryDependency) : Value :=
  .node 0 [ofArray dependency.procedure_id, ofArray dependency.chunk_id,
    ofArray dependency.source_revision]

def ofEvidenceRecord (record : source.procedures.EvidenceRecord) : Value :=
  .node 0 [.nat record.obligation.val, .nat record.«class».ctorIdx, .nat record.outcome.ctorIdx,
    ofList .text record.toolchain.val, .text record.claim, ofArray record.subject_binding,
    .nat record.policy_revision.val,
    ofOption (fun marker => .nat marker.ctorIdx) record.capability]

def ofVerifiedEvidence (evidence : source.procedures.acceptance.VerifiedEvidence) : Value :=
  .node 0 [.nat evidence.obligation.val, .nat evidence.«class».ctorIdx,
    .nat evidence.outcome.ctorIdx, ofArray evidence.subject_binding,
    .nat evidence.policy_revision.val, ofArray evidence.claim_digest, .text evidence.verifier,
    ofList .text evidence.toolchain.val, ofArray evidence.command_digest,
    ofArray evidence.input_digest]

def ofPromotion (promotion : source.procedures.Promotion) : Value :=
  .node 0 [.nat promotion.policy_revision.val,
    ofList (fun obligation => .nat obligation.val) promotion.obligations.val]

def ofCost (cost : source.procedures.Cost) : Value :=
  .node 0 [.nat cost.expanded_nodes.val, .nat cost.emitted_source_bytes.val,
    .nat cost.evaluation_steps.val]

def ofRetrievalSignature (signature : source.procedures.RetrievalSignature) : Value :=
  .node 0 [ofTys signature.stack_in, ofTys signature.stack_out, .effects signature.effects,
    .nat signature.status_rank.val, ofList (fun key => .nat key.val) signature.cost_key.val]

def ofMonomial (monomial : source.procedures.alias.Monomial) : Value :=
  .node 0 [.nat monomial.coefficient.val, ofArray monomial.powers]

def ofPolynomial (polynomial : source.procedures.alias.Polynomial) : Value :=
  ofList ofMonomial polynomial.terms.val

def ofCertificate : source.procedures.alias.Certificate → Value
  | .LegacyDigest digest => .node 0 [ofArray digest]
  | .ExactPair pair =>
    .node 1 [ofList ofPolynomial pair.left.val, ofList ofPolynomial pair.right.val,
      ofArray pair.left_closure, ofArray pair.right_closure]

def ofAlias (aliased : source.procedures.Alias) : Value :=
  .node 0 [ofArray aliased.procedure_id, ofCertificate aliased.certificate]

/-- The whole record. The owner's verifier reads it only through
`VerificationSubject`; a function of the whole record covers every function
of that view. -/
def ofProcedureRecord (record : source.procedures.ProcedureRecord) : Value :=
  .node 0 [.nat record.format.val, .nat record.semantic_revision.val,
    ofEncoder record.encoder, ofArray record.procedure_id, ofEnvelope record.chunk,
    ofProvenance record.provenance,
    ofList ofLibraryDependency record.library_dependencies.val,
    ofList ofEvidenceRecord record.evidence.val,
    ofList ofVerifiedEvidence record.verified.val, .nat record.status.ctorIdx,
    ofOption ofPromotion record.promotion, ofCost record.cost,
    ofRetrievalSignature record.retrieval, ofOption ofArray record.supersedes,
    ofOption ofAlias record.alias_of]

/-! ### Host replies as the generated Rust values

Both maps are bijections: the reply types of `TypesExternal.lean` hold
exactly the fields of the Rust values, with the declaration index of each
enum variant. -/

def scorerFailure (kind : Fin 5) : source.chunks.ScorerFailure :=
  if kind.val = 0 then .Unavailable
  else if kind.val = 1 then .StaleProvenance
  else if kind.val = 2 then .InvalidFeature
  else if kind.val = 3 then .InvalidResponse
  else .Evaluation

def scoreResult : ScoreReply →
    core.result.Result source.chunks.CandidateScoreResponse source.chunks.ScorerFailure
  | .response request model ordinal digest score =>
    .Ok {
      request_identity := request
      model_identity := model
      candidate := { ordinal := ordinal, digest := digest }
      score := score }
  | .failure kind => .Err (scorerFailure kind)

def evidenceOutcome (outcome : Fin 9) : source.procedures.EvidenceOutcome :=
  if outcome.val = 0 then .Proved
  else if outcome.val = 1 then .Disproved
  else if outcome.val = 2 then .Unknown
  else if outcome.val = 3 then .Timeout
  else if outcome.val = 4 then .Unsupported
  else if outcome.val = 5 then .Error
  else if outcome.val = 6 then .NotRun
  else if outcome.val = 7 then .Passed
  else .Failed

def attestation (reply : AttestationReply) : source.procedures.acceptance.Attestation :=
  { verifier := reply.verifier, toolchain := reply.toolchain,
    command_digest := reply.command_digest, input_digest := reply.input_digest,
    outcome := evidenceOutcome reply.outcome }

end HostPlugin

/-! ### The three opaque `Debug` helpers -/

/-- [noble_contracts::intrinsic::debug_form_kind]: `Atom(atom)` or
`List(forms)` through `debug_tuple`; see the module documentation. -/
def intrinsic.debug_form_kind (_ : intrinsic.FormKind) (formatter : core.fmt.Formatter) :
    Result ((core.result.Result Unit core.fmt.Error) × core.fmt.Formatter) :=
  ok (.Ok (), formatter)

/-- [noble_contracts::intrinsic::debug_proof_dependency]: the nine fields
through `debug_struct`; see the module documentation. -/
def intrinsic.debug_proof_dependency (_ : intrinsic.ProofDependency)
    (formatter : core.fmt.Formatter) :
    Result ((core.result.Result Unit core.fmt.Error) × core.fmt.Formatter) :=
  ok (.Ok (), formatter)

/-- [noble_contracts::source::editor::debug_node]: each variant through
`debug_tuple`, or `write_str("Hole")`; see the module documentation. -/
def source.editor.debug_node (_ : source.editor.Node) (formatter : core.fmt.Formatter) :
    Result ((core.result.Result Unit core.fmt.Error) × core.fmt.Formatter) :=
  ok (.Ok (), formatter)

/-! ### Host plug-in calls -/

/-- [noble_contracts::source::chunks::ObserverHandle::observe]: calls the
caller's closure. The call's result is the host's next delivery result; the
outcome reaches only the host, which the extracted code never reads back. As
for any `&'a mut` wrapper, the region's final handle is the one given back. -/
def source.chunks.ObserverHandle.observe (observer : source.chunks.ObserverHandle)
    (_ : source.chunks.CandidateOutcome) :
    Result (source.chunks.ObserverHandle ×
      (source.chunks.ObserverHandle → source.chunks.ObserverHandle)) := do
  observer.deliveries observer.delivered
  ok ({ observer with delivered := observer.delivered + 1 }, fun final => final)

/-- [noble_contracts::source::chunks::RankerHandle::model_identity]: the
scorer's answer in its current state. -/
def source.chunks.RankerHandle.model_identity (ranker : source.chunks.RankerHandle) :
    Result (Array U8 32#usize) :=
  ranker.identities ranker.scored

/-- [noble_contracts::source::chunks::RankerHandle::provenance_matches]: the
scorer's answer in its current state to this exact ranking identity. -/
def source.chunks.RankerHandle.provenance_matches (ranker : source.chunks.RankerHandle)
    (identity : source.chunks.RankingIdentity) : Result Bool :=
  ranker.provenance ranker.scored (HostPlugin.ofRankingIdentity identity)

/-- [noble_contracts::source::chunks::RankerHandle::score]: the scorer's next
reply; the request reaches only the host (see `TypesExternal.lean`). The
final handle of the region is the one given back. -/
def source.chunks.RankerHandle.score (ranker : source.chunks.RankerHandle)
    (_ : source.chunks.CandidateScoreRequest) :
    Result ((core.result.Result source.chunks.CandidateScoreResponse
      source.chunks.ScorerFailure) × source.chunks.RankerHandle ×
      (source.chunks.RankerHandle → source.chunks.RankerHandle)) := do
  let reply ← ranker.replies ranker.scored
  ok (HostPlugin.scoreResult reply, { ranker with scored := ranker.scored + 1 },
    fun final => final)

/-- [noble_contracts::source::chunks::MonotonicClock::now_ns]: the host
clock's next reading. -/
def source.chunks.MonotonicClock.now_ns (clock : source.chunks.MonotonicClock) :
    Result (U64 × source.chunks.MonotonicClock) := do
  let reading ← clock.readings clock.taken
  ok (reading, { clock with taken := clock.taken + 1 })

/-- [noble_contracts::source::procedures::acceptance::OwnerVerifier::verify]:
the owner's verifier on this exact record and claim. -/
def source.procedures.acceptance.OwnerVerifier.verify
    (verifier : source.procedures.acceptance.OwnerVerifier)
    (record : source.procedures.ProcedureRecord) (claim : source.procedures.EvidenceRecord) :
    Result (Option source.procedures.acceptance.Attestation) := do
  let reply ← verifier.attest (HostPlugin.ofProcedureRecord record)
    (HostPlugin.ofEvidenceRecord claim)
  ok (reply.map HostPlugin.attestation)

/-- [noble_contracts::source::procedures::acceptance::{impl Debug for OwnerVerifier}::fmt]:
formats exactly as the boxed verifier does: its result under the abstract
formatter, which returns the formatter unchanged. -/
def source.procedures.acceptance.OwnerVerifier.Insts.CoreFmtDebug.fmt
    (verifier : source.procedures.acceptance.OwnerVerifier) (formatter : core.fmt.Formatter) :
    Result ((core.result.Result Unit core.fmt.Error) × core.fmt.Formatter) := do
  let result ← verifier.debug
  ok (result, formatter)

/-! ### The untrusted implicit-argument elaborator

Fourteen bodies of `intrinsic::elaborate` are reviewed opaque boundaries: the
pinned Aeneas refuses them (nested borrows through the elaborator state,
`(&Form, _)` worklists and a borrow-joining `match`). Each model is an `opaque`
constant, which hides an arbitrary inhabitant of its type: no definition can be
unfolded, so every Rust behavior, failure included, is represented and no
theorem can depend on what the elaborator returns. That is faithful because the
elaborator is an untrusted producer: acceptance consumes only the explicit term
it produces, after the extracted checker and the pinned Lean consumer recheck
that term. -/

/-- [noble_contracts::intrinsic::elaborate::{Elaborator}::verify_ground] -/
opaque intrinsic.elaborate.Elaborator.verify_ground :
    intrinsic.elaborate.Elaborator → Result (core.result.Result Unit Diagnostic)

/-- [noble_contracts::intrinsic::elaborate::{Elaborator}::binder_snapshot] -/
opaque intrinsic.elaborate.Elaborator.binder_snapshot :
    intrinsic.elaborate.Elaborator →
      Result ((alloc.vec.Vec String) × (alloc.vec.Vec intrinsic.Sort))

/-- [noble_contracts::intrinsic::elaborate::{Elaborator}::check] -/
opaque intrinsic.elaborate.Elaborator.check :
    intrinsic.elaborate.Elaborator → intrinsic.Form → intrinsic.elaborate.PropTerm → Std.U32 →
      Result ((core.result.Result intrinsic.elaborate.Out Diagnostic) ×
        intrinsic.elaborate.Elaborator)

/-- [noble_contracts::intrinsic::elaborate::{Elaborator}::subst] -/
opaque intrinsic.elaborate.Elaborator.subst :
    intrinsic.elaborate.Elaborator → intrinsic.Form → Slice intrinsic.Form →
      intrinsic.elaborate.PropTerm → Std.U32 →
      Result ((core.result.Result intrinsic.elaborate.Out Diagnostic) ×
        intrinsic.elaborate.Elaborator)

/-- [noble_contracts::intrinsic::elaborate::{Elaborator}::premise_refusal] -/
opaque intrinsic.elaborate.Elaborator.premise_refusal :
    intrinsic.elaborate.Elaborator → Span → intrinsic.elaborate.PropTerm → Result Diagnostic

/-- [noble_contracts::intrinsic::elaborate::{Elaborator}::app] -/
opaque intrinsic.elaborate.Elaborator.app :
    intrinsic.elaborate.Elaborator → intrinsic.Form → Std.U32 →
      Result ((core.result.Result intrinsic.elaborate.Inferred Diagnostic) ×
        intrinsic.elaborate.Elaborator)

/-- [noble_contracts::intrinsic::elaborate::{Elaborator}::finish_out] -/
opaque intrinsic.elaborate.Elaborator.finish_out :
    intrinsic.elaborate.Elaborator → intrinsic.elaborate.Out → Std.U32 →
      Result (core.result.Result intrinsic.Form Diagnostic)

/-- [noble_contracts::intrinsic::elaborate::scan_abbreviation] -/
opaque intrinsic.elaborate.scan_abbreviation :
    intrinsic.Form → Limits →
      Result (core.result.Result intrinsic.elaborate.Abbreviation Diagnostic)

/-- [noble_contracts::intrinsic::elaborate::abbreviation_step] -/
opaque intrinsic.elaborate.abbreviation_step :
    intrinsic.Form → Std.U32 → Limits → intrinsic.elaborate.Abbreviation → Std.U32 →
      alloc.vec.Vec (intrinsic.Form × Std.U32) →
      Result ((core.result.Result Unit Diagnostic) × (alloc.vec.Vec (intrinsic.Form × Std.U32)) ×
        intrinsic.elaborate.Abbreviation × Std.U32)

/-- [noble_contracts::intrinsic::elaborate::find_infer_span] -/
opaque intrinsic.elaborate.find_infer_span :
    intrinsic.Form → Limits → Result (core.result.Result (Option Span) Diagnostic)

/-- [noble_contracts::intrinsic::elaborate::infer_span_step] -/
opaque intrinsic.elaborate.infer_span_step :
    intrinsic.Form → Std.U32 → Limits → Std.U32 → alloc.vec.Vec (intrinsic.Form × Std.U32) →
      Result ((core.result.Result (Option Span) Diagnostic) ×
        (alloc.vec.Vec (intrinsic.Form × Std.U32)) × Std.U32)

/-- [noble_contracts::intrinsic::elaborate::scan_positions] -/
opaque intrinsic.elaborate.scan_positions :
    intrinsic.Form → Limits → Result (core.result.Result Unit Diagnostic)

/-- [noble_contracts::intrinsic::elaborate::push_positions] -/
opaque intrinsic.elaborate.push_positions :
    alloc.vec.Vec (intrinsic.Form × intrinsic.elaborate.Position × Std.U32) →
      Slice intrinsic.Form → intrinsic.elaborate.Position → Std.U32 →
      Result (alloc.vec.Vec (intrinsic.Form × intrinsic.elaborate.Position × Std.U32))

/-- [noble_contracts::intrinsic::elaborate::pi_names] -/
opaque intrinsic.elaborate.pi_names : intrinsic.Form → Result (alloc.vec.Vec String)

end noble_contracts
