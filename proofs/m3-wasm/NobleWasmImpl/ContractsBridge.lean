module
public import Aeneas
public import NobleContractImpl.Funs
public import NobleContractImpl.KernelBridge
public import NobleWasmImpl.KernelBridge
@[expose] public section

open Aeneas Aeneas.Std Result

-- The generated boundary names below repeat a namespace component (for example
-- `noble_contracts.source.proof.CheckedSelectedTarget.source`); the generated
-- `Funs.lean` that calls them disables the same linter.
set_option linter.dupNamespace false

namespace noble_wasm
namespace ContractsBridge

/-! Only nominal dependency representations change at this boundary. World,
CheckedExport, Diagnostic, the declared source owners, the source error, and the
checked source-proof handles (definition identity, selected target, generated
witness, quote site) are the actual frontend types. ResourceKind, EffId, NodeId
and Definition are the same reducible U32 representation in both extractions,
so their values and optional values pass through unchanged. Kernel values the
frontend returns cross through the frontend's and this crate's kernel bridges,
both total and lossless. -/

def toType : noble_contracts.component.Type → _root_.noble_contracts.component.Type
  | .Boolean => .Boolean
  | .S64 => .S64
  | .CheckedU64 => .CheckedU64
  | .String => .String
  | .Bytes => .Bytes
  | .ResultS64String => .ResultS64String
  | .ResultBytesString => .ResultBytesString
  | .StreamU8 => .StreamU8
  | .FutureS64 => .FutureS64
  | .FutureResultS64String => .FutureResultS64String
  | .Own kind => .Own kind
  | .Borrow kind => .Borrow kind

def fromType : _root_.noble_contracts.component.Type → noble_contracts.component.Type
  | .Boolean => .Boolean
  | .S64 => .S64
  | .CheckedU64 => .CheckedU64
  | .String => .String
  | .Bytes => .Bytes
  | .ResultS64String => .ResultS64String
  | .ResultBytesString => .ResultBytesString
  | .StreamU8 => .StreamU8
  | .FutureS64 => .FutureS64
  | .FutureResultS64String => .FutureResultS64String
  | .Own kind => .Own kind
  | .Borrow kind => .Borrow kind

@[simp] theorem fromType_toType (x : noble_contracts.component.Type) :
    fromType (toType x) = x := by
  cases x <;> rfl

@[simp] theorem toType_fromType (x : _root_.noble_contracts.component.Type) :
    toType (fromType x) = x := by
  cases x <;> rfl

def toOperation (x : noble_contracts.component.Operation) :
    _root_.noble_contracts.component.Operation :=
  { identity := x.identity
    word := x.word
    core_module := x.core_module
    core_name := x.core_name
    export_name := x.export_name
    parameters := KernelBridge.mapVec toType x.parameters
    results := KernelBridge.mapVec toType x.results
    asynchronous := x.asynchronous
    effect := x.effect
    definition := x.definition }

def fromOperation (x : _root_.noble_contracts.component.Operation) :
    noble_contracts.component.Operation :=
  { identity := x.identity
    word := x.word
    core_module := x.core_module
    core_name := x.core_name
    export_name := x.export_name
    parameters := KernelBridge.mapVec fromType x.parameters
    results := KernelBridge.mapVec fromType x.results
    asynchronous := x.asynchronous
    effect := x.effect
    definition := x.definition }

@[simp] theorem fromOperation_toOperation (x : noble_contracts.component.Operation) :
    fromOperation (toOperation x) = x := by
  cases x
  simp [fromOperation, toOperation]

@[simp] theorem toOperation_fromOperation (x : _root_.noble_contracts.component.Operation) :
    toOperation (fromOperation x) = x := by
  cases x
  simp [toOperation, fromOperation]

def toStage : noble_contracts.component.Stage → _root_.noble_contracts.component.Stage
  | .Wit => .Wit
  | .Binding => .Binding
  | .Export => .Export
  | .Acceptance => .Acceptance

def fromStage : _root_.noble_contracts.component.Stage → noble_contracts.component.Stage
  | .Wit => .Wit
  | .Binding => .Binding
  | .Export => .Export
  | .Acceptance => .Acceptance

@[simp] theorem fromStage_toStage (x : noble_contracts.component.Stage) :
    fromStage (toStage x) = x := by
  cases x <;> rfl

@[simp] theorem toStage_fromStage (x : _root_.noble_contracts.component.Stage) :
    toStage (fromStage x) = x := by
  cases x <;> rfl

def toError (x : noble_contracts.component.Error) : _root_.noble_contracts.component.Error :=
  { stage := toStage x.stage
    diagnostic := x.diagnostic }

def fromError (x : _root_.noble_contracts.component.Error) : noble_contracts.component.Error :=
  { stage := fromStage x.stage
    diagnostic := x.diagnostic }

@[simp] theorem fromError_toError (x : noble_contracts.component.Error) :
    fromError (toError x) = x := by
  cases x
  simp [fromError, toError]

@[simp] theorem toError_fromError (x : _root_.noble_contracts.component.Error) :
    toError (fromError x) = x := by
  cases x
  simp [toError, fromError]

def toSpan (x : noble_contracts.Span) : _root_.noble_contracts.Span :=
  { start := x.start
    «end» := x.«end» }

def fromSpan (x : _root_.noble_contracts.Span) : noble_contracts.Span :=
  { start := x.start
    «end» := x.«end» }

@[simp] theorem fromSpan_toSpan (x : noble_contracts.Span) : fromSpan (toSpan x) = x := by
  cases x
  rfl

@[simp] theorem toSpan_fromSpan (x : _root_.noble_contracts.Span) : toSpan (fromSpan x) = x := by
  cases x
  rfl

def fromSourceProofRefusal :
    _root_.noble_contracts.source.proof.SourceProofRefusal →
      noble_contracts.source.proof.SourceProofRefusal
  | .UnsupportedProfile => .UnsupportedProfile
  | .StaleContext => .StaleContext
  | .MissingTarget => .MissingTarget
  | .MissingEvidence => .MissingEvidence
  | .MismatchedSource => .MismatchedSource
  | .MismatchedDefinition => .MismatchedDefinition
  | .MismatchedInterface => .MismatchedInterface
  | .MismatchedClaim => .MismatchedClaim
  | .Assumptions => .Assumptions
  | .IndependentCheckFailed => .IndependentCheckFailed

/-- The quotation body is the same borrowed `u32` slice on both sides. -/
def fromCheckedLexicalKind :
    _root_.noble_contracts.source.proof.CheckedLexicalKind →
      noble_contracts.source.proof.CheckedLexicalKind
  | .I64 value => .I64 value
  | .Builtin word => .Builtin word
  | .Quotation body => .Quotation body

def fromCheckedLexicalNode (x : _root_.noble_contracts.source.proof.CheckedLexicalNode) :
    noble_contracts.source.proof.CheckedLexicalNode :=
  { kind := fromCheckedLexicalKind x.kind
    authored_span := x.authored_span.map fromSpan
    generated_witness := x.generated_witness }

def fromCheckedOperationSite (x : _root_.noble_contracts.source.proof.CheckedOperationSite) :
    noble_contracts.source.proof.CheckedOperationSite :=
  { lexical_node := x.lexical_node
    authored_span := x.authored_span.map fromSpan
    generated_witness := x.generated_witness }

end ContractsBridge

open ContractsBridge

/-! Every frontend boundary calls the actual extracted Rust function or global.
Result binds preserve failure and divergence; successful values change only
their representation, and both Rust Result constructors are mapped without
replacing frontend diagnostics, source errors or source-proof refusals. -/

@[rust_const "noble_contracts::component::MAX_OPERATIONS"]
def noble_contracts.component.MAX_OPERATIONS : Result Std.Usize :=
  ok _root_.noble_contracts.component.MAX_OPERATIONS

@[rust_fun "noble_contracts::component::{core::clone::Clone<noble_contracts::component::Type>}::clone"]
def noble_contracts.component.Type.Insts.CoreCloneClone.clone
    (self : noble_contracts.component.Type) : Result noble_contracts.component.Type := do
  let value ← _root_.noble_contracts.component.Type.Insts.CoreCloneClone.clone (toType self)
  ok (fromType value)

@[rust_fun "noble_contracts::component::{core::cmp::PartialEq<noble_contracts::component::Type, noble_contracts::component::Type>}::eq"]
def noble_contracts.component.Type.Insts.CoreCmpPartialEqType.eq
    (self other : noble_contracts.component.Type) : Result Bool :=
  _root_.noble_contracts.component.Type.Insts.CoreCmpPartialEqType.eq
    (toType self) (toType other)

@[rust_fun "noble_contracts::component::{noble_contracts::component::Operation}::input_types"]
def noble_contracts.component.Operation.input_types
    (self : noble_contracts.component.Operation) :
    Result (alloc.vec.Vec noble_kernel.types.Ty) := do
  let value ← _root_.noble_contracts.component.Operation.input_types (toOperation self)
  ok (KernelBridge.mapVec
    (fun ty => KernelBridge.fromTy (_root_.noble_contracts.KernelBridge.toTy ty)) value)

@[rust_fun "noble_contracts::component::{noble_contracts::component::Operation}::output_types"]
def noble_contracts.component.Operation.output_types
    (self : noble_contracts.component.Operation) :
    Result (alloc.vec.Vec noble_kernel.types.Ty) := do
  let value ← _root_.noble_contracts.component.Operation.output_types (toOperation self)
  ok (KernelBridge.mapVec
    (fun ty => KernelBridge.fromTy (_root_.noble_contracts.KernelBridge.toTy ty)) value)

@[rust_fun "noble_contracts::component::{noble_contracts::component::World}::imports"]
def noble_contracts.component.World.imports
    (self : noble_contracts.component.World) :
    Result (Slice noble_contracts.component.Operation) := do
  let value ← _root_.noble_contracts.component.World.impl.imports self
  ok (KernelBridge.mapSlice fromOperation value)

@[rust_fun "noble_contracts::component::{noble_contracts::component::World}::exports"]
def noble_contracts.component.World.exports
    (self : noble_contracts.component.World) :
    Result (Slice noble_contracts.component.Operation) := do
  let value ← _root_.noble_contracts.component.World.impl.exports self
  ok (KernelBridge.mapSlice fromOperation value)

@[rust_fun "noble_contracts::component::{noble_contracts::component::World}::is_async"]
def noble_contracts.component.World.is_async
    (self : noble_contracts.component.World) : Result Bool :=
  _root_.noble_contracts.component.World.is_async self

@[rust_fun "noble_contracts::component::{noble_contracts::component::World}::worker_program_kind"]
def noble_contracts.component.World.worker_program_kind
    (self : noble_contracts.component.World) :
    Result (Option noble_kernel.types.ResourceKind) :=
  _root_.noble_contracts.component.World.impl.worker_program_kind self

/-- The world's own type mapping; the candidate type and the returned kernel type
cross the frontend and kernel bridges unchanged in value. -/
@[rust_fun "noble_contracts::component::{noble_contracts::component::World}::noble_type"]
def noble_contracts.component.World.noble_type
    (self : noble_contracts.component.World) (ty : noble_contracts.component.Type) :
    Result (Option noble_kernel.types.Ty) := do
  let value ← _root_.noble_contracts.component.World.noble_type self (toType ty)
  ok (value.map (fun ty =>
    KernelBridge.fromTy (_root_.noble_contracts.KernelBridge.toTy ty)))

@[rust_fun "noble_contracts::component::context::{noble_contracts::component::World}::build_context"]
def noble_contracts.component.context.World.build_context
    (self : noble_contracts.component.World) : Result (alloc.vec.Vec Std.U8) :=
  _root_.noble_contracts.component.context.World.build_context self

@[rust_fun "noble_contracts::component::{noble_contracts::component::World}::environment"]
def noble_contracts.component.World.environment
    (self : noble_contracts.component.World) :
    Result (core.result.Result noble_kernel.contracts.Env noble_contracts.component.Error) := do
  let value ← _root_.noble_contracts.component.World.environment self
  match value with
  | .Ok environment =>
    ok (.Ok (KernelBridge.fromEnv (_root_.noble_contracts.KernelBridge.toEnv environment)))
  | .Err error => ok (.Err (fromError error))

@[rust_fun "noble_contracts::component::{noble_contracts::component::CheckedExport}::world_context"]
def noble_contracts.component.CheckedExport.world_context
    (self : noble_contracts.component.CheckedExport) : Result (Slice Std.U8) :=
  _root_.noble_contracts.component.CheckedExport.world_context self

@[rust_fun "noble_contracts::component::{noble_contracts::component::CheckedExport}::index"]
def noble_contracts.component.CheckedExport.index
    (self : noble_contracts.component.CheckedExport) : Result Std.Usize :=
  _root_.noble_contracts.component.CheckedExport.impl.index self

@[rust_fun "noble_contracts::component::{noble_contracts::component::CheckedExport}::source"]
def noble_contracts.component.CheckedExport.source
    (self : noble_contracts.component.CheckedExport) : Result (Slice Std.U8) :=
  _root_.noble_contracts.component.CheckedExport.source self

@[rust_fun "noble_contracts::component::{noble_contracts::component::CheckedExport}::submission"]
def noble_contracts.component.CheckedExport.submission
    (self : noble_contracts.component.CheckedExport) :
    Result (Option noble_kernel.execution.Submission) := do
  let value ← _root_.noble_contracts.component.CheckedExport.submission self
  ok (value.map (fun submission =>
    KernelBridge.fromSubmission (_root_.noble_contracts.KernelBridge.toSubmission submission)))

@[rust_fun "noble_contracts::{core::clone::Clone<noble_contracts::Span>}::clone"]
def noble_contracts.Span.Insts.CoreCloneClone.clone (self : noble_contracts.Span) :
    Result noble_contracts.Span := do
  let value ← _root_.noble_contracts.Span.Insts.CoreCloneClone.clone (toSpan self)
  ok (fromSpan value)

@[rust_fun "noble_contracts::{core::fmt::Debug<noble_contracts::Span>}::fmt"]
def noble_contracts.Span.Insts.CoreFmtDebug.fmt (self : noble_contracts.Span)
    (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  _root_.noble_contracts.Span.Insts.CoreFmtDebug.fmt (toSpan self) formatter

/-- Re-derivation re-runs the frontend's own declared-text check; the derived
submission crosses the kernel bridges and a source error is the frontend's. -/
@[rust_fun "noble_contracts::source::declared::state::{noble_contracts::source::declared::state::DeclaredTextSourceOwner<'0>}::rederive"]
def noble_contracts.source.declared.state.DeclaredTextSourceOwner.rederive
    (self : noble_contracts.source.declared.state.DeclaredTextSourceOwner) :
    Result (core.result.Result noble_kernel.execution.Submission noble_contracts.source.Error) := do
  let value ← _root_.noble_contracts.source.declared.state.DeclaredTextSourceOwner.rederive self
  match value with
  | .Ok submission =>
    ok (.Ok (KernelBridge.fromSubmission
      (_root_.noble_contracts.KernelBridge.toSubmission submission)))
  | .Err error => ok (.Err error)

@[rust_fun "noble_contracts::source::declared::state::{noble_contracts::source::declared::state::SignedSourceOwner<'0>}::rederive"]
def noble_contracts.source.declared.state.SignedSourceOwner.rederive
    (self : noble_contracts.source.declared.state.SignedSourceOwner) :
    Result (core.result.Result noble_kernel.execution.Submission noble_contracts.source.Error) := do
  let value ← _root_.noble_contracts.source.declared.state.SignedSourceOwner.rederive self
  match value with
  | .Ok submission =>
    ok (.Ok (KernelBridge.fromSubmission
      (_root_.noble_contracts.KernelBridge.toSubmission submission)))
  | .Err error => ok (.Err error)

@[rust_fun "noble_contracts::source::proof::{core::clone::Clone<noble_contracts::source::proof::CheckedDefinitionId>}::clone"]
def noble_contracts.source.proof.CheckedDefinitionId.Insts.CoreCloneClone.clone
    (self : noble_contracts.source.proof.CheckedDefinitionId) :
    Result noble_contracts.source.proof.CheckedDefinitionId :=
  _root_.noble_contracts.source.proof.CheckedDefinitionId.Insts.CoreCloneClone.clone self

@[rust_fun "noble_contracts::source::proof::{core::fmt::Debug<noble_contracts::source::proof::CheckedDefinitionId>}::fmt"]
def noble_contracts.source.proof.CheckedDefinitionId.Insts.CoreFmtDebug.fmt
    (self : noble_contracts.source.proof.CheckedDefinitionId) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  _root_.noble_contracts.source.proof.CheckedDefinitionId.Insts.CoreFmtDebug.fmt self formatter

@[rust_fun "noble_contracts::source::proof::{core::cmp::PartialEq<noble_contracts::source::proof::CheckedDefinitionId, noble_contracts::source::proof::CheckedDefinitionId>}::eq"]
def noble_contracts.source.proof.CheckedDefinitionId.Insts.CoreCmpPartialEqCheckedDefinitionId.eq
    (self other : noble_contracts.source.proof.CheckedDefinitionId) : Result Bool :=
  _root_.noble_contracts.source.proof.CheckedDefinitionId.Insts.CoreCmpPartialEqCheckedDefinitionId.eq
    self other

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedDefinitionId}::identity"]
def noble_contracts.source.proof.CheckedDefinitionId.identity
    (self : noble_contracts.source.proof.CheckedDefinitionId) : Result Std.U64 :=
  _root_.noble_contracts.source.proof.CheckedDefinitionId.impl.identity self

@[rust_fun "noble_contracts::source::proof::{core::clone::Clone<noble_contracts::source::proof::CheckedGeneratedWitness>}::clone"]
def noble_contracts.source.proof.CheckedGeneratedWitness.Insts.CoreCloneClone.clone
    (self : noble_contracts.source.proof.CheckedGeneratedWitness) :
    Result noble_contracts.source.proof.CheckedGeneratedWitness :=
  _root_.noble_contracts.source.proof.CheckedGeneratedWitness.Insts.CoreCloneClone.clone self

@[rust_fun "noble_contracts::source::proof::{core::fmt::Debug<noble_contracts::source::proof::CheckedGeneratedWitness>}::fmt"]
def noble_contracts.source.proof.CheckedGeneratedWitness.Insts.CoreFmtDebug.fmt
    (self : noble_contracts.source.proof.CheckedGeneratedWitness)
    (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  _root_.noble_contracts.source.proof.CheckedGeneratedWitness.Insts.CoreFmtDebug.fmt self formatter

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::definition_id"]
def noble_contracts.source.proof.CheckedSelectedTarget.definition_id
    (self : noble_contracts.source.proof.CheckedSelectedTarget) :
    Result noble_contracts.source.proof.CheckedDefinitionId :=
  _root_.noble_contracts.source.proof.CheckedSelectedTarget.definition_id self

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::source_generation"]
def noble_contracts.source.proof.CheckedSelectedTarget.source_generation
    (self : noble_contracts.source.proof.CheckedSelectedTarget) : Result Std.U64 :=
  _root_.noble_contracts.source.proof.CheckedSelectedTarget.source_generation self

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::source"]
def noble_contracts.source.proof.CheckedSelectedTarget.source
    (self : noble_contracts.source.proof.CheckedSelectedTarget) : Result (Slice Std.U8) :=
  _root_.noble_contracts.source.proof.CheckedSelectedTarget.source self

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::submission"]
def noble_contracts.source.proof.CheckedSelectedTarget.submission
    (self : noble_contracts.source.proof.CheckedSelectedTarget) :
    Result noble_kernel.execution.Submission := do
  let value ← _root_.noble_contracts.source.proof.CheckedSelectedTarget.impl.submission self
  ok (KernelBridge.fromSubmission (_root_.noble_contracts.KernelBridge.toSubmission value))

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::definition"]
def noble_contracts.source.proof.CheckedSelectedTarget.definition
    (self : noble_contracts.source.proof.CheckedSelectedTarget) :
    Result noble_kernel.execution.Definition := do
  let value ← _root_.noble_contracts.source.proof.CheckedSelectedTarget.impl.definition self
  ok (KernelBridge.fromExecutionDefinition
    (_root_.noble_contracts.KernelBridge.toExecutionDefinition value))

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::root"]
def noble_contracts.source.proof.CheckedSelectedTarget.root
    (self : noble_contracts.source.proof.CheckedSelectedTarget) :
    Result noble_kernel.untrusted.Checked := do
  let value ← _root_.noble_contracts.source.proof.CheckedSelectedTarget.impl.root self
  ok (KernelBridge.fromChecked (_root_.noble_contracts.KernelBridge.toChecked value))

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::body"]
def noble_contracts.source.proof.CheckedSelectedTarget.body
    (self : noble_contracts.source.proof.CheckedSelectedTarget) :
    Result noble_kernel.untrusted.Checked := do
  let value ← _root_.noble_contracts.source.proof.CheckedSelectedTarget.impl.body self
  ok (KernelBridge.fromChecked (_root_.noble_contracts.KernelBridge.toChecked value))

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::lexical_body"]
def noble_contracts.source.proof.CheckedSelectedTarget.lexical_body
    (self : noble_contracts.source.proof.CheckedSelectedTarget) : Result (Slice Std.U32) :=
  _root_.noble_contracts.source.proof.CheckedSelectedTarget.lexical_body self

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::lexical_nodes_len"]
def noble_contracts.source.proof.CheckedSelectedTarget.lexical_nodes_len
    (self : noble_contracts.source.proof.CheckedSelectedTarget) : Result Std.Usize :=
  _root_.noble_contracts.source.proof.CheckedSelectedTarget.lexical_nodes_len self

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::checked_lexical_node"]
def noble_contracts.source.proof.CheckedSelectedTarget.checked_lexical_node
    (self : noble_contracts.source.proof.CheckedSelectedTarget) (node : Std.U32) :
    Result (core.result.Result noble_contracts.source.proof.CheckedLexicalNode
      noble_contracts.source.proof.SourceProofRefusal) := do
  let value ← _root_.noble_contracts.source.proof.CheckedSelectedTarget.checked_lexical_node
    self node
  match value with
  | .Ok checked => ok (.Ok (fromCheckedLexicalNode checked))
  | .Err refusal => ok (.Err (fromSourceProofRefusal refusal))

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::is_generated_operation"]
def noble_contracts.source.proof.CheckedSelectedTarget.is_generated_operation
    (self : noble_contracts.source.proof.CheckedSelectedTarget)
    (node : noble_kernel.untrusted.NodeId) : Result Bool :=
  _root_.noble_contracts.source.proof.CheckedSelectedTarget.is_generated_operation self node

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::checked_operation_site"]
def noble_contracts.source.proof.CheckedSelectedTarget.checked_operation_site
    (self : noble_contracts.source.proof.CheckedSelectedTarget)
    (node : noble_kernel.untrusted.NodeId) :
    Result (core.result.Result noble_contracts.source.proof.CheckedOperationSite
      noble_contracts.source.proof.SourceProofRefusal) := do
  let value ← _root_.noble_contracts.source.proof.CheckedSelectedTarget.checked_operation_site
    self node
  match value with
  | .Ok site => ok (.Ok (fromCheckedOperationSite site))
  | .Err refusal => ok (.Err (fromSourceProofRefusal refusal))

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedSelectedTarget<'0>}::checked_quote_site"]
def noble_contracts.source.proof.CheckedSelectedTarget.checked_quote_site
    (self : noble_contracts.source.proof.CheckedSelectedTarget)
    (node : noble_kernel.untrusted.NodeId) :
    Result (core.result.Result noble_contracts.source.proof.CheckedQuoteSite
      noble_contracts.source.proof.SourceProofRefusal) := do
  let value ← _root_.noble_contracts.source.proof.CheckedSelectedTarget.checked_quote_site
    self node
  match value with
  | .Ok site => ok (.Ok site)
  | .Err refusal => ok (.Err (fromSourceProofRefusal refusal))

@[rust_fun "noble_contracts::source::proof::{noble_contracts::source::proof::CheckedQuoteSite<'0>}::interface"]
def noble_contracts.source.proof.CheckedQuoteSite.interface
    (self : noble_contracts.source.proof.CheckedQuoteSite) :
    Result noble_kernel.untrusted.Interface := do
  let value ← _root_.noble_contracts.source.proof.CheckedQuoteSite.impl.interface self
  ok (KernelBridge.fromInterface (_root_.noble_contracts.KernelBridge.toInterface value))

end noble_wasm
