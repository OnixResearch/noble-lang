module
public import Aeneas
public import NobleKernel.Types
public import NobleContractImpl.Types
@[expose] public section

open Aeneas Aeneas.Std

namespace noble_wasm

/-!
External types of the noble-wasm extraction. Every declaration below is a
definition or a safe data type; none is an axiom or an opaque constant.

* Types the inherited extractions already model are aliases of those models
  (the kernel's `EffSet` and array iterator, and the frontend's types), so a
  value crosses the crate boundary without any conversion.
* The standard-library containers are value-only models in the style of the
  inherited Aeneas `Vec`: they keep the elements and their order, and erase
  allocation, capacity and node layout. The functions over them are in
  `StdModels.lean`.
-/

/-- [core::mem::maybe_uninit::MaybeUninit]: as in MC1, retain initialization
state without inventing a value of `T`: `none` is uninitialized storage and
`some value` is initialized storage. The extraction exposes no raw-byte or
partial-initialization operations. -/
@[rust_type "core::mem::maybe_uninit::MaybeUninit"]
abbrev core.mem.maybe_uninit.MaybeUninit (T : Type) := Option T

/-- [noble_kernel::types::EffSet]: effects use the inherited kernel's complete
ordered representation. -/
@[rust_type "noble_kernel::types::EffSet"]
abbrev noble_kernel.types.EffSet := _root_.noble_kernel.types.EffSet

/-- [core::array::iter::IntoIter]: the inherited kernel's model, which keeps
the iterator's alive window front first (see `NobleKernel/TypesExternal.lean`).
The array iterator functions delegate to the kernel's models over the same
values. -/
@[rust_type "core::array::iter::IntoIter"]
abbrev core.array.iter.IntoIter (T : Type) (N : Usize) :=
  _root_.noble_kernel.core.array.iter.IntoIter T N

/-- [core::str::error::Utf8Error]: the backend shares the frontend's lossless
UTF-8 diagnostic representation. -/
@[rust_type "core::str::error::Utf8Error"]
abbrev core.str.error.Utf8Error := _root_.noble_contracts.core.str.error.Utf8Error

/-- [alloc::rc::Rc]: a value-only carrier of the shared payload, of the same
shape as the frontend's `Rc` model. The backend only shares immutable `Origin*`
trees through `Rc`, so the payload is the whole observable value; reference
counts and allocation identity are outside the value model (`Rc::ptr_eq` is
modelled in `StdModels.lean`). It is declared here rather than aliased to the
frontend's carrier because the generated `Origin*` types nest it recursively,
and the kernel accepts a nested occurrence only of an inductive type itself,
not through an alias. -/
@[rust_type "alloc::rc::Rc"]
structure alloc.rc.Rc (T : Type) where
  value : T

/-- [alloc::collections::btree::map::BTreeMap]: the map's entries in ascending
key order, which is the order every `BTreeMap` traversal (`keys`, `values`,
`clone`, `eq`, `fmt`, `hash`) visits. The node structure, the allocator value
and spare node capacity are not represented: only `BTreeMap::<K, V, Global>::new`
constructs a map in this subject, and the `Global` allocator carries no state.
Like the inherited `Vec`, the entry count is bounded by `usize::MAX`. -/
@[rust_type "alloc::collections::btree::map::BTreeMap"]
structure alloc.collections.btree.map.BTreeMap (K V A : Type) where
  entries : alloc.vec.Vec (K × V)

/-- [alloc::collections::btree::map::Keys]: the entries still to be visited,
in ascending key order; `next` yields their keys. -/
@[rust_type "alloc::collections::btree::map::Keys"]
structure alloc.collections.btree.map.Keys (K V : Type) where
  remaining : List (K × V)

/-- [alloc::collections::btree::map::Values]: the entries still to be visited,
in ascending key order; `next` yields their values. -/
@[rust_type "alloc::collections::btree::map::Values"]
structure alloc.collections.btree.map.Values (K V : Type) where
  remaining : List (K × V)

/-- [alloc::collections::btree::map::entry::VacantEntry]: Rust keeps the key, a
handle to the leaf edge where it belongs and a dormant borrow of the map. The
model keeps the key and the map split at that edge: `before` holds the entries
with smaller keys and `after` the entries with larger keys, so the map is
`before ++ after` until a value is inserted. -/
@[rust_type "alloc::collections::btree::map::entry::VacantEntry"
  (mutRegions := #[0])]
structure alloc.collections.btree.map.entry.VacantEntry (K V A : Type) where
  key : K
  before : List (K × V)
  after : List (K × V)
  bound : before.length + after.length ≤ Usize.max

/-- [alloc::collections::btree::map::entry::OccupiedEntry]: Rust keeps a handle
to the found key-value slot and a dormant borrow of the map. The model keeps the
map split at that slot: the stored key and its current value, the entries with
smaller keys (`before`) and those with larger keys (`after`). -/
@[rust_type "alloc::collections::btree::map::entry::OccupiedEntry"
  (mutRegions := #[0])]
structure alloc.collections.btree.map.entry.OccupiedEntry (K V A : Type) where
  before : List (K × V)
  key : K
  value : V
  after : List (K × V)
  bound : before.length + after.length + 1 ≤ Usize.max

/-! Foreign frontend values are the actual extracted frontend values, not a
second model of world identity, independent admission, retained source or
checked source-proof views. -/

@[rust_type "noble_contracts::component::World"]
abbrev noble_contracts.component.World := _root_.noble_contracts.component.World

@[rust_type "noble_contracts::Diagnostic"]
abbrev noble_contracts.Diagnostic := _root_.noble_contracts.Diagnostic

@[rust_type "noble_contracts::component::CheckedExport"]
abbrev noble_contracts.component.CheckedExport :=
  _root_.noble_contracts.component.CheckedExport

@[rust_type "noble_contracts::source::declared::state::SignedSourceOwner"]
abbrev noble_contracts.source.declared.state.SignedSourceOwner :=
  _root_.noble_contracts.source.declared.state.SignedSourceOwner

@[rust_type "noble_contracts::source::declared::state::DeclaredTextSourceOwner"]
abbrev noble_contracts.source.declared.state.DeclaredTextSourceOwner :=
  _root_.noble_contracts.source.declared.state.DeclaredTextSourceOwner

@[rust_type "noble_contracts::source::Error"]
abbrev noble_contracts.source.Error := _root_.noble_contracts.source.Error

@[rust_type "noble_contracts::source::proof::CheckedDefinitionId"]
abbrev noble_contracts.source.proof.CheckedDefinitionId :=
  _root_.noble_contracts.source.proof.CheckedDefinitionId

@[rust_type "noble_contracts::source::proof::CheckedSelectedTarget"]
abbrev noble_contracts.source.proof.CheckedSelectedTarget :=
  _root_.noble_contracts.source.proof.CheckedSelectedTarget

@[rust_type "noble_contracts::source::proof::CheckedGeneratedWitness"]
abbrev noble_contracts.source.proof.CheckedGeneratedWitness :=
  _root_.noble_contracts.source.proof.CheckedGeneratedWitness

@[rust_type "noble_contracts::source::proof::CheckedQuoteSite"]
abbrev noble_contracts.source.proof.CheckedQuoteSite :=
  _root_.noble_contracts.source.proof.CheckedQuoteSite

end noble_wasm
