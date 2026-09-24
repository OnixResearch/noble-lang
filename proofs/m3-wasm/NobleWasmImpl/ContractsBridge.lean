import NobleContractImpl.Funs
import NobleContractImpl.KernelBridge
import NobleWasmImpl.KernelBridge

open Aeneas Aeneas.Std Result

namespace noble_wasm
namespace ContractsBridge

/-! Only nominal dependency representations change at this boundary. World,
CheckedExport and Diagnostic are the actual frontend types. ResourceKind,
EffId and Definition are the same reducible U32 representation in both
extractions, so their values and optional values pass through unchanged. -/

def toType : noble_contracts.component.Type → _root_.noble_contracts.component.Type
  | .Boolean => .Boolean
  | .S64 => .S64
  | .String => .String
  | .Bytes => .Bytes
  | .ResultS64String => .ResultS64String
  | .Own kind => .Own kind
  | .Borrow kind => .Borrow kind

def fromType : _root_.noble_contracts.component.Type → noble_contracts.component.Type
  | .Boolean => .Boolean
  | .S64 => .S64
  | .String => .String
  | .Bytes => .Bytes
  | .ResultS64String => .ResultS64String
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

end ContractsBridge

open ContractsBridge

/-! Every frontend boundary calls the actual extracted Rust function or global.
Result binds preserve failure and divergence, and the environment bridge maps
both Rust Result constructors without replacing frontend diagnostics. -/

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

@[rust_fun "noble_contracts::component::{noble_contracts::component::Type}::noble"]
def noble_contracts.component.Type.noble
    (self : noble_contracts.component.Type) : Result noble_kernel.types.Ty := do
  let value ← _root_.noble_contracts.component.Type.noble (toType self)
  ok (KernelBridge.fromTy (_root_.noble_contracts.KernelBridge.toTy value))

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

@[rust_fun "noble_contracts::component::{noble_contracts::component::World}::build_context"]
def noble_contracts.component.World.build_context
    (self : noble_contracts.component.World) : Result (alloc.vec.Vec Std.U8) :=
  _root_.noble_contracts.component.World.build_context self

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

end noble_wasm
