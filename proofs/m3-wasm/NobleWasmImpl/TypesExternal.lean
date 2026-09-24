import Aeneas
import NobleKernel.Types
import NobleContractImpl.Types

open Aeneas Aeneas.Std

namespace noble_wasm

/-- As in MC1, retain initialization state without inventing a value of `T`:
`none` is uninitialized storage and `some value` is initialized storage.
The extraction exposes no raw-byte or partial-initialization operations. -/
@[rust_type "core::mem::maybe_uninit::MaybeUninit"]
abbrev core.mem.maybe_uninit.MaybeUninit (T : Type) := Option T

/-- Effects use the inherited kernel's complete ordered representation. -/
@[rust_type "noble_kernel::types::EffSet"]
abbrev noble_kernel.types.EffSet := _root_.noble_kernel.types.EffSet

/-- The backend shares the frontend's lossless UTF-8 diagnostic representation. -/
@[rust_type "core::str::error::Utf8Error"]
abbrev core.str.error.Utf8Error := _root_.noble_contracts.core.str.error.Utf8Error

/-- Foreign frontend values are the actual extracted frontend values, not a
second model of world identity, independent admission or retained source. -/
@[rust_type "noble_contracts::component::World"]
abbrev noble_contracts.component.World := _root_.noble_contracts.component.World

@[rust_type "noble_contracts::Diagnostic"]
abbrev noble_contracts.Diagnostic := _root_.noble_contracts.Diagnostic

@[rust_type "noble_contracts::component::CheckedExport"]
abbrev noble_contracts.component.CheckedExport :=
  _root_.noble_contracts.component.CheckedExport

end noble_wasm
