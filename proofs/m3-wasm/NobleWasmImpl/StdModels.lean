module
public import Aeneas
public import NobleWasmImpl.Types
public import NobleKernel.FunsExternal
public import NobleContractImpl.StdModels
@[expose] public section

open Aeneas Aeneas.Std Result
open Aeneas.Data.ListN

-- The generated names below repeat a namespace component (for example
-- `core.borrow.Borrow.Blanket.borrow`); the generated `Funs.lean` that calls
-- them disables the same linter.
set_option linter.dupNamespace false

namespace noble_wasm

/-!
Standard-library boundaries of the noble-wasm extraction. Each definition names
the Rust item it models; the rules below hold for all of them.

* Scalars use Aeneas's fixed-width mathematical values, with the pinned
  extraction targeting little-endian x86_64 (64-bit `usize`). Fallible casts
  retain Rust's success/error distinction; saturating multiplication clamps the
  exact mathematical product.
* Every supplied trait implementation (`Clone`, `PartialEq`, `PartialOrd`,
  `Ord`, `Debug`, `Hash`, `Default`, closures) is invoked in Rust's order, the
  same number of times, and its failures propagate; Rust's short-circuiting is
  kept. Closure state is threaded through `FnMut` calls.
* Slice, vector and map values retain every element and its order; no pointer
  or allocation identity is used. As in the inherited Aeneas vector model,
  allocator exhaustion, spare capacity and type-layout-dependent allocation
  limits are not observable; an element count beyond `usize::MAX` fails with
  the capacity-overflow panic.
* Formatting follows the inherited abstract `Formatter`: writes return it
  unchanged and payload `Debug` implementations still run and may fail. No
  string literal is written here, so these models add no native evaluation.
* Where these models reuse an identical inherited model, they call the
  kernel's or the frontend's definition of the same Rust item.

Three boundaries return `fail .undef`, exactly like the inherited
`core::mem::size_of`: their Rust value depends on state outside the value
model, so no faithful definition with the generated signature exists, and
claiming any `ok` value would be false for some execution.
* `Rc::ptr_eq` compares allocation identity. In this value-only model two
  `Rc` values are equal exactly when their payloads are, which Rust's separate
  allocations do not imply. It is reached only by
  `source::origin::bounded_graph_cells`, which counts distinct `Rc` cells.
* `<Vec<T> as Hash>::hash` hashes the length prefix and then calls
  `T::hash_slice`, which std overrides for integer `T` to feed the hasher one
  `write` of the packed bytes rather than one `write` per element. The pinned
  `core.hash.Hash` record carries no `hash_slice`, so the element type cannot
  be told apart, and a hasher may observe how its input is split. No extracted
  function calls a `Hash` implementation.
* The raw-pointer `get_unchecked` projections of `str`, following the pinned
  library's slice `get_unchecked` models: raw pointers carry no provenance
  in the value model, and Rust leaves out-of-range projections undefined.

`BTreeMap` (see `TypesExternal.lean`) keeps its entries in ascending key order.
Rust searches its B-tree by comparing the sought key with stored keys
(`key.cmp(k)`) node by node, left to right, stopping at the first key that is
not smaller; the model compares with the stored keys in ascending order under
the same rule. For an `Ord` whose `cmp` is a total order and returns on every
pair (Rust's `Ord` contract), both find the same entry or insertion point and
so return the same values and leave the same map; they differ only in how many
comparisons run, so a `cmp` that fails or diverges on some pair can fail here
where the B-tree never compares that pair. The subject's key types are `u128`
and `Vec<u8>` (looked up as `[u8]`), whose comparisons are total and never
fail in this model.
-/

/-! ## core::array -/

/-- [core::array::equality::{impl PartialEq<[U; N]> for &'_0 [T]}::eq]: the
kernel's model (length, then the supplied element comparison in order). -/
@[rust_fun "core::array::equality::{core::cmp::PartialEq<&'0 [@T], [@U; @N]>}::eq"]
def Shared0Slice.Insts.CoreCmpPartialEqArray.eq {T U : Type} {N : Usize}
    (inst : core.cmp.PartialEq T U) (left : Slice T) (right : Array U N) : Result Bool :=
  _root_.noble_kernel.Shared0Slice.Insts.CoreCmpPartialEqArray.eq inst left right

/-- [core::array::iter::{impl IntoIterator for [T; N]}::into_iter]: the
kernel's model; the iterator type is the kernel's (`TypesExternal.lean`). -/
@[rust_fun
  "core::array::iter::{core::iter::traits::collect::IntoIterator<[@T; @N], @T, core::array::iter::IntoIter<@T, @N>>}::into_iter"]
def Array.Insts.CoreIterTraitsCollectIntoIteratorTIntoIter.into_iter {T : Type} {N : Usize}
    (array : Array T N) : Result (core.array.iter.IntoIter T N) :=
  _root_.noble_kernel.Array.Insts.CoreIterTraitsCollectIntoIteratorTIntoIter.into_iter array

/-- [core::array::iter::{impl Iterator for IntoIter<T, N>}::next]: the kernel's
model (the front of the alive window). -/
@[rust_fun
  "core::array::iter::{core::iter::traits::iterator::Iterator<core::array::iter::IntoIter<@T, @N>, @T>}::next"]
def core.array.iter.IntoIter.Insts.CoreIterTraitsIteratorIterator.next {T : Type} {N : Usize}
    (iter : core.array.iter.IntoIter T N) :
    Result (Option T × core.array.iter.IntoIter T N) :=
  _root_.noble_kernel.core.array.iter.IntoIter.Insts.CoreIterTraitsIteratorIterator.next iter

/-- The items of `core::array::from_fn` still to generate: `remaining` items,
starting at index `N - remaining`, threading the generator's state. -/
def arrayFromFnItems {T F : Type} (inst : core.ops.function.FnMut F Usize T) (N : Usize) :
    (remaining : Nat) → F → Result (ListN T remaining)
  | 0, _ => ok .nil
  | remaining + 1, generator => do
    let index : Usize :=
      UScalar.ofNatCore (N.val - (remaining + 1)) (by have := N.hBounds; omega)
    let (item, generator) ← inst.call_mut generator index
    let rest ← arrayFromFnItems inst N remaining generator
    ok (.cons item rest)

/-- [core::array::from_fn]: std's `try_from_fn_erased` calls the generator on
`0, 1, …, N - 1` in order, through `FnMut`, and collects every item; the closure
is dropped afterwards. -/
@[rust_fun "core::array::from_fn"]
def core.array.from_fn {T F : Type} (N : Usize) (inst : core.ops.function.FnMut F Usize T)
    (generator : F) : Result (Array T N) := do
  let items ← arrayFromFnItems inst N N.val generator
  ok ⟨items⟩

/-! ## core::bool and core::borrow -/

/-- [core::bool::{bool}::then_some]: the kernel's model. -/
@[rust_fun "core::bool::{bool}::then_some"]
def core.bool.Bool.then_some {T : Type} (condition : Bool) (value : T) : Result (Option T) :=
  _root_.noble_kernel.core.bool.Bool.then_some condition value

/-- [core::borrow::{impl Borrow<T> for T}::borrow]: a shared borrow of the value
itself, which is the value in the functional model. -/
@[rust_fun "core::borrow::{core::borrow::Borrow<@T, @T>}::borrow"]
def core.borrow.Borrow.Blanket.borrow {T : Type} (value : T) : Result T :=
  ok value

/-! ## core::convert -/

@[rust_fun "core::convert::num::{core::convert::From<i64, u32>}::from"]
def I64.Insts.CoreConvertFromU32.from (value : U32) : Result I64 :=
  ok (UScalar.hcast .I64 value)

@[rust_fun
  "core::convert::num::ptr_try_from_impls::{core::convert::TryFrom<usize, u32, core::num::error::TryFromIntError>}::try_from"]
def Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from
    (value : U32) : Result (core.result.Result Usize core.num.error.TryFromIntError) :=
  _root_.noble_kernel.Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from value

@[rust_fun
  "core::convert::num::ptr_try_from_impls::{core::convert::TryFrom<u64, usize, core::num::error::TryFromIntError>}::try_from"]
def U64.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from
    (value : Usize) : Result (core.result.Result U64 core.num.error.TryFromIntError) :=
  _root_.noble_kernel.U64.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from value

@[rust_fun
  "core::convert::num::{core::convert::TryFrom<u8, u64, core::num::error::TryFromIntError>}::try_from"]
def U8.Insts.CoreConvertTryFromU64TryFromIntError.try_from
    (value : U64) : Result (core.result.Result U8 core.num.error.TryFromIntError) :=
  core.num.tryFromUScalar .U8 value

@[rust_fun
  "core::convert::num::{core::convert::TryFrom<u32, u64, core::num::error::TryFromIntError>}::try_from"]
def U32.Insts.CoreConvertTryFromU64TryFromIntError.try_from
    (value : U64) : Result (core.result.Result U32 core.num.error.TryFromIntError) :=
  core.num.tryFromUScalar .U32 value

/-! ## core::fmt -/

/-- [core::fmt::{impl Debug for (U, T)}::fmt]: std's `debug_tuple("")` formats
the first field and then the second, skipping the second once the first fails.
The punctuation writes are the abstract formatter's unchanged pass-through. -/
@[rust_fun "core::fmt::{core::fmt::Debug<(@U, @T)>}::fmt"]
def Pair.Insts.CoreFmtDebug.fmt {U T : Type} (first : core.fmt.Debug U)
    (second : core.fmt.Debug T) (pair : U × T) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) := do
  let (result, formatter) ← first.fmt pair.1 formatter
  match result with
  | .Err error => ok (.Err error, formatter)
  | .Ok () => second.fmt pair.2 formatter

/-! ## core::hash

The pinned library's `Hasher` record carries only `write` and `finish`, so a
hasher is represented by them: the integer writes below are the `Hasher`
trait's default `write(&value.to_ne_bytes())`, little-endian on the target. -/

/-- Byte `index` of `value`, least significant first. -/
def littleEndianByte (value index : Nat) : U8 :=
  ⟨BitVec.ofNat _ (value / 256 ^ index)⟩

/-- [core::hash::impls::{impl Hash for u32}::hash]: `state.write_u32(*self)`. -/
@[rust_fun "core::hash::impls::{core::hash::Hash<u32>}::hash"]
def U32.Insts.CoreHashHash.hash {H : Type} (hasher : core.hash.Hasher H) (value : U32)
    (state : H) : Result H :=
  hasher.write state (.from [littleEndianByte value.val 0, littleEndianByte value.val 1,
    littleEndianByte value.val 2, littleEndianByte value.val 3] (by scalar_tac))

/-- [core::hash::impls::{impl Hash for u8}::hash]: `state.write_u8(*self)`. -/
@[rust_fun "core::hash::impls::{core::hash::Hash<u8>}::hash"]
def U8.Insts.CoreHashHash.hash {H : Type} (hasher : core.hash.Hasher H) (value : U8)
    (state : H) : Result H :=
  hasher.write state (.from [value] (by scalar_tac))

/-- [core::hash::impls::{impl Hash for bool}::hash]: `state.write_u8(*self as u8)`. -/
@[rust_fun "core::hash::impls::{core::hash::Hash<bool>}::hash"]
def Bool.Insts.CoreHashHash.hash {H : Type} (hasher : core.hash.Hasher H) (value : Bool)
    (state : H) : Result H :=
  hasher.write state (.from [⟨BitVec.ofNat _ (if value then 1 else 0)⟩] (by scalar_tac))

/-- `Hasher::write_length_prefix(len)`: `write_usize(len)`, the eight
little-endian bytes of the 64-bit `usize`. -/
def writeLengthPrefix {H : Type} (hasher : core.hash.Hasher H) (state : H) (length : Nat) :
    Result H :=
  hasher.write state (.from [littleEndianByte length 0, littleEndianByte length 1,
    littleEndianByte length 2, littleEndianByte length 3, littleEndianByte length 4,
    littleEndianByte length 5, littleEndianByte length 6, littleEndianByte length 7]
    (by scalar_tac))

/-! ## core::mem -/

/-- [core::mem::size_of]: the kernel's model, `fail .undef` (the target layout
size is outside the value model; see `NobleKernel/FunsExternal.lean`). -/
@[rust_fun "core::mem::size_of"]
def core.mem.size_of (T : Type _) : Result Usize :=
  _root_.noble_kernel.core.mem.size_of T

/-- [core::mem::take]: `replace(dest, T::default())`: the default is built
first; the result is the old value and the place then holds the default. -/
@[rust_fun "core::mem::take"]
def core.mem.take {T : Type} (inst : core.default.Default T) (value : T) : Result (T × T) := do
  let fresh ← inst.default
  ok (value, fresh)

/-! ## core::num -/

/-- Taking the magnitude in `Int` handles `i64::MIN` without signed overflow. -/
@[rust_fun "core::num::{i64}::unsigned_abs"]
def core.num.I64.unsigned_abs (value : I64) : Result U64 :=
  UScalar.tryMk .U64 value.val.natAbs

/-- [core::num::{usize}::checked_ilog2]: `None` for zero, otherwise
`Some(BITS - 1 - leading_zeros)`, the floor of the base-2 logarithm. It is
below 64, so the 32-bit result is exact. -/
@[rust_fun "core::num::{usize}::checked_ilog2"]
def core.num.Usize.checked_ilog2 (value : Usize) : Result (Option U32) :=
  if value.val = 0 then ok none else ok (some ⟨BitVec.ofNat _ (Nat.log2 value.val)⟩)

@[rust_fun "core::num::{u64}::saturating_mul"]
def core.num.U64.saturating_mul (left right : U64) : Result U64 :=
  UScalar.tryMk .U64 (min U64.max (left.val * right.val))

@[rust_fun "core::num::{usize}::saturating_mul"]
def core.num.Usize.saturating_mul (left right : Usize) : Result Usize :=
  _root_.noble_kernel.core.num.Usize.saturating_mul left right

/-! ## core::ops::range -/

/-- [core::ops::range::{RangeInclusive<Idx>}::contains]: `RangeBounds::contains`
with `RangeInclusive`'s bounds: `start <= item`, and then `item <= end`, or
`item < start` once the range is exhausted (its end bound becomes
`Excluded(start)`); the second comparison runs only when the first holds. -/
@[rust_fun "core::ops::range::{core::ops::range::RangeInclusive<@Idx>}::contains"]
def core.ops.range.RangeInclusive.contains {Idx U : Type}
    (_inst : core.cmp.PartialOrd Idx Idx) (startInst : core.cmp.PartialOrd Idx U)
    (endInst : core.cmp.PartialOrd U Idx) (range : core.ops.range.RangeInclusive Idx)
    (item : U) : Result Bool := do
  let afterStart ← startInst.le range.start item
  if afterStart then
    if range.exhausted then endInst.lt item range.start else endInst.le item range.end
  else ok false

/-! ## core::option -/

@[rust_fun "core::option::{core::clone::Clone<core::option::Option<@T>>}::clone"]
def core.option.Option.Insts.CoreCloneClone.clone
    {T : Type} (inst : core.clone.Clone T) (value : Option T) : Result (Option T) :=
  _root_.noble_kernel.core.option.Option.Insts.CoreCloneClone.clone inst value

@[rust_fun "core::option::{core::option::Option<&'0 @T>}::copied"]
def core.option.OptionShared0T.copied
    {T : Type} (inst : core.marker.Copy T) (value : Option T) : Result (Option T) :=
  _root_.noble_contracts.core.option.OptionShared0T.copied inst value

@[rust_fun
  "core::option::{core::cmp::PartialEq<core::option::Option<@T>, core::option::Option<@T>>}::eq"]
def core.option.Option.Insts.CoreCmpPartialEqOption.eq
    {T : Type _} (inst : core.cmp.PartialEq T T) (left right : Option T) : Result Bool :=
  _root_.noble_contracts.core.option.Option.Insts.CoreCmpPartialEqOption.eq inst left right

/-- [core::option::{impl Debug for Option<T>}::fmt]: the kernel's model. -/
@[rust_fun "core::option::{core::fmt::Debug<core::option::Option<@T>>}::fmt"]
def core.option.Option.Insts.CoreFmtDebug.fmt {T : Type} (inst : core.fmt.Debug T)
    (value : Option T) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  _root_.noble_kernel.core.option.Option.Insts.CoreFmtDebug.fmt inst value formatter

/-- [core::option::{Option<T>}::is_none_or]: the kernel's model. -/
@[rust_fun "core::option::{core::option::Option<@T>}::is_none_or"]
def core.option.Option.is_none_or {T F : Type} (inst : core.ops.function.FnOnce F T Bool)
    (value : Option T) (predicate : F) : Result Bool :=
  _root_.noble_kernel.core.option.Option.is_none_or inst value predicate

/-- [core::option::{Option<T>}::as_ref]: the kernel's model. -/
@[rust_fun "core::option::{core::option::Option<@T>}::as_ref"]
def core.option.Option.as_ref {T : Type} (value : Option T) : Result (Option T) :=
  _root_.noble_kernel.core.option.Option.as_ref value

/-- [core::option::{Option<T>}::unwrap_or_else]: the payload of `Some`;
for `None` the closure's result through `FnOnce::call_once`. -/
@[rust_fun "core::option::{core::option::Option<@T>}::unwrap_or_else"]
def core.option.Option.unwrap_or_else {T F : Type} (inst : core.ops.function.FnOnce F Unit T)
    (value : Option T) (fallback : F) : Result T :=
  match value with
  | some payload => ok payload
  | none => inst.call_once fallback ()

/-- [core::option::{Option<T>}::map_or]: the default for `None`; for `Some(v)`
the function's result on `v`. -/
@[rust_fun "core::option::{core::option::Option<@T>}::map_or"]
def core.option.Option.map_or {T U F : Type} (inst : core.ops.function.FnOnce F T U)
    (value : Option T) (fallback : U) (function : F) : Result U :=
  match value with
  | none => ok fallback
  | some payload => inst.call_once function payload

/-- [core::option::{Option<T>}::and_then]: the kernel's model. -/
@[rust_fun "core::option::{core::option::Option<@T>}::and_then"]
def core.option.Option.and_then {T U F : Type} (inst : core.ops.function.FnOnce F T (Option U))
    (value : Option T) (function : F) : Result (Option U) :=
  _root_.noble_kernel.core.option.Option.and_then inst value function

/-- [core::option::{Option<T>}::filter]: `Some(x)` survives exactly when the
predicate, called once on it, returns `true`. -/
@[rust_fun "core::option::{core::option::Option<@T>}::filter"]
def core.option.Option.filter {T P : Type} (inst : core.ops.function.FnOnce P T Bool)
    (value : Option T) (predicate : P) : Result (Option T) :=
  match value with
  | none => ok none
  | some payload => do
    let keep ← inst.call_once predicate payload
    ok (if keep then some payload else none)

/-- [core::option::{Option<T>}::get_or_insert_with]: for `None` the closure
runs once and its value is stored; the result is the stored value, and the
place afterwards holds `Some` of whatever was written through the reference. -/
@[rust_fun "core::option::{core::option::Option<@T>}::get_or_insert_with"]
def core.option.Option.get_or_insert_with {T F : Type}
    (inst : core.ops.function.FnOnce F Unit T) (value : Option T) (make : F) :
    Result (T × (T → Option T)) :=
  match value with
  | some payload => ok (payload, fun written => some written)
  | none => do
    let payload ← inst.call_once make ()
    ok (payload, fun written => some written)

/-- [core::option::{Option<T>}::replace]: `mem::replace(self, Some(value))`:
the old option is returned and the place holds `Some(value)`. -/
@[rust_fun "core::option::{core::option::Option<@T>}::replace"]
def core.option.Option.replace {T : Type} (slot : Option T) (value : T) :
    Result (Option T × Option T) :=
  ok (slot, some value)

/-- [core::option::{Option<T>}::zip]: the kernel's model. -/
@[rust_fun "core::option::{core::option::Option<@T>}::zip"]
def core.option.Option.zip {T U : Type} (left : Option T) (right : Option U) :
    Result (Option (T × U)) :=
  _root_.noble_kernel.core.option.Option.zip left right

/-- [core::option::{impl Try for Option<T>}::branch]: the kernel's model. -/
@[rust_fun "core::option::{core::ops::try_trait::Try<core::option::Option<@T>>}::branch"]
def core.option.Option.Insts.CoreOpsTry_traitTry.branch {T : Type} (value : Option T) :
    Result (core.ops.control_flow.ControlFlow (Option Never) T) :=
  _root_.noble_kernel.core.option.Option.Insts.CoreOpsTry_traitTry.branch value

/-- [core::option::{impl FromResidual<Option<!>> for Option<T>}::from_residual]:
the kernel's model. -/
@[rust_fun
  "core::option::{core::ops::try_trait::FromResidual<core::option::Option<@T>, core::option::Option<!>>}::from_residual"]
def core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionNever.from_residual
    (T : Type) (residual : Option Never) : Result (Option T) :=
  _root_.noble_kernel.core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionNever.from_residual
    T residual

/-- [core::option::{Option<Option<T>>}::flatten]: the inner option of `Some`,
`None` otherwise. -/
@[rust_fun "core::option::{core::option::Option<core::option::Option<@T>>}::flatten"]
def core.option.OptionOption.flatten {T : Type} (value : Option (Option T)) :
    Result (Option T) :=
  match value with
  | some inner => ok inner
  | none => ok none

/-! ## core::result -/

@[rust_fun "core::result::{core::result::Result<@T, @E>}::is_err"]
def core.result.Result.is_err
    {T E : Type} (value : core.result.Result T E) : Result Bool :=
  _root_.noble_contracts.core.result.Result.is_err value

/-- [core::result::{Result<T, E>}::ok]: the kernel's model. -/
@[rust_fun "core::result::{core::result::Result<@T, @E>}::ok"]
def core.result.Result.ok {T E : Type} (value : core.result.Result T E) : Result (Option T) :=
  _root_.noble_kernel.core.result.Result.ok value

/-! ## core::slice -/

/-- `SlicePartialOrd::partial_compare` over the elements in order: the first
pair whose `partial_cmp` is not `Some(Equal)` decides (an incomparable pair
gives `None`); a common prefix then compares lengths. -/
def slicePartialCompare {T : Type _} (inst : core.cmp.PartialOrd T T) :
    List T → List T → Result (Option Ordering)
  | [], [] => ok (some .eq)
  | [], _ :: _ => ok (some .lt)
  | _ :: _, [] => ok (some .gt)
  | left :: lefts, right :: rights => do
    let order ← inst.partial_cmp left right
    match order with
    | some .eq => slicePartialCompare inst lefts rights
    | other => ok other

/-- `SliceOrd::compare`: the same chaining with `Ord::cmp`. -/
def sliceCompare {T : Type _} (inst : core.cmp.Ord T) : List T → List T → Result Ordering
  | [], [] => ok .eq
  | [], _ :: _ => ok .lt
  | _ :: _, [] => ok .gt
  | left :: lefts, right :: rights => do
    let order ← inst.cmp left right
    match order with
    | .eq => sliceCompare inst lefts rights
    | other => ok other

/-- [core::slice::cmp::{impl PartialOrd for [T]}::partial_cmp]: std's default
`partial_compare`. For integer elements std compares with `Ord` (or `memcmp`)
instead; the pinned integer instances are total and side-effect free, so the
result is the same. -/
@[rust_fun "core::slice::cmp::{core::cmp::PartialOrd<[@T], [@T]>}::partial_cmp"]
def Slice.Insts.CoreCmpPartialOrdSlice.partial_cmp {T : Type _} (inst : core.cmp.PartialOrd T T)
    (left right : Slice T) : Result (Option Ordering) :=
  slicePartialCompare inst left.val right.val

/-- [core::slice::cmp::{impl Ord for [T]}::cmp]: std's default `compare`. -/
@[rust_fun "core::slice::cmp::{core::cmp::Ord<[@T]>}::cmp"]
def Slice.Insts.CoreCmpOrd.cmp {T : Type _} (inst : core.cmp.Ord T) (left right : Slice T) :
    Result Ordering :=
  sliceCompare inst left.val right.val

/-- [core::slice::{[T]}::starts_with]: `self.len() >= n && needle == &self[..n]`,
where slice equality checks the lengths and then calls the element `ne` on
`(needle[i], self[i])` in order, stopping at the first difference. -/
@[rust_fun "core::slice::{[@T]}::starts_with"]
def core.slice.Slice.starts_with {T : Type} (inst : core.cmp.PartialEq T T)
    (slice needle : Slice T) : Result Bool :=
  if needle.length ≤ slice.length then
    core.slice.cmp.PartialEqSlice.eq inst needle
      (.from (slice.val.take needle.length) (by
        have := slice.property
        simp only [List.length_take]
        omega))
  else ok false

/-- The probes of std's `binary_search_by` (the pinned branch-free form) with
the comparator `if pred(x) { Less } else { Greater }`: while more than one
candidate remains, probe `base + size / 2` and move `base` there unless the
probe is `Greater`; then probe `base` once more. The predicate's state is
threaded through every probe. -/
def partitionPointProbe {T P : Type} (inst : core.ops.function.FnMut P T Bool)
    (elements : List T) (size base : Nat) (inRange : base + size ≤ elements.length)
    (nonempty : 0 < size) (predicate : P) : Result Nat :=
  if more : 1 < size then do
    let (less, predicate) ← inst.call_mut predicate (elements[base + size / 2]'(by omega))
    partitionPointProbe inst elements (size - size / 2)
      (if less then base + size / 2 else base)
      (by split <;> omega) (by omega) predicate
  else do
    let (less, _) ← inst.call_mut predicate (elements[base]'(by omega))
    ok (base + if less then 1 else 0)
termination_by size
decreasing_by omega

/-- [core::slice::{[T]}::partition_point]: `binary_search_by(|x| if pred(x)
{ Less } else { Greater }).unwrap_or_else(|i| i)`. The comparator is never
`Equal`, so the search returns `Err(base + (cmp == Less))`; an empty slice
returns 0 without calling the predicate. The index is at most the length. -/
@[rust_fun "core::slice::{[@T]}::partition_point"]
def core.slice.Slice.partition_point {T P : Type} (inst : core.ops.function.FnMut P T Bool)
    (slice : Slice T) (predicate : P) : Result Usize :=
  if empty : slice.val.length = 0 then ok 0#usize
  else do
    let index ← partitionPointProbe inst slice.val slice.val.length 0 (by omega) (by omega)
      predicate
    UScalar.tryMk .Usize index

@[rust_fun "core::slice::{[@T]}::first"]
def core.slice.Slice.first {T : Type} (slice : Slice T) : Result (Option T) :=
  _root_.noble_contracts.core.slice.Slice.first slice

@[rust_fun "core::slice::{[@T]}::last"]
def core.slice.Slice.last {T : Type} (slice : Slice T) : Result (Option T) :=
  ok slice.val.getLast?

@[rust_fun "core::slice::raw::from_ref"]
def core.slice.raw.from_ref {T : Type} (value : T) : Result (Slice T) :=
  ok (.from [value] (by scalar_tac))

/-! ## core::str -/

/-- Both crates share the frontend's byte-preserving UTF-8 validation and its
exact invalid-prefix/error-length representation. -/
@[rust_fun "core::str::converts::from_utf8"]
def core.str.converts.from_utf8 (bytes : Slice U8) :
    Result (core.result.Result Str core.str.error.Utf8Error) :=
  _root_.noble_contracts.core.str.converts.from_utf8 bytes

/-- `Str` is the inherited exact UTF-8 byte slice; Rust's view does not copy it. -/
@[rust_fun "core::str::{str}::as_bytes"]
def core.str.Str.as_bytes (text : Str) : Result (Slice U8) :=
  ok text

/-- [core::str::{str}::get]: `i.get(self)` through the supplied `SliceIndex`. -/
@[rust_fun "core::str::{str}::get"]
def core.str.Str.get {I O : Type} (inst : core.slice.index.SliceIndex I Str O) (text : Str)
    (index : I) : Result (Option O) :=
  inst.get index text

/-- `str::is_char_boundary`: index 0 and the length are boundaries, an index
past the end is not, and an inner index is one when its byte is not a UTF-8
continuation byte (`(b as i8) >= -0x40`, that is `b < 0x80 || b >= 0xC0`). -/
def strIsCharBoundary (text : Str) (index : Nat) : Bool :=
  if index = 0 then true
  else if inside : index < text.val.length then
    decide ((text.val[index]'inside).val < 128 ∨ 192 ≤ (text.val[index]'inside).val)
  else index = text.val.length

/-- [core::str::traits::{impl SliceIndex<str> for RangeTo<usize>}::get]: the
byte prefix when `end` is a char boundary, `None` otherwise. -/
@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeTo<usize>, str, str>}::get"]
def core.ops.range.RangeToUsize.Insts.CoreSliceIndexSliceIndexStrStr.get
    (range : core.ops.range.RangeTo Usize) (text : Str) : Result (Option Str) :=
  if strIsCharBoundary text range.end.val then
    core.slice.index.SliceIndexRangeToUsizeSlice.get range text
  else ok none

/-- [...::get_mut]: as `get`; writing back through the prefix replaces the same
bytes of the string (the library's slice prefix model). -/
@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeTo<usize>, str, str>}::get_mut"]
def core.ops.range.RangeToUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_mut
    (range : core.ops.range.RangeTo Usize) (text : Str) :
    Result (Option Str × (Option Str → Str)) :=
  if strIsCharBoundary text range.end.val then
    core.slice.index.SliceIndexRangeToUsizeSlice.get_mut range text
  else ok (none, fun _ => text)

/-- [...::get_unchecked]: `(0..end).get_unchecked(slice)` on a raw pointer;
`fail .undef` (see the header). -/
@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeTo<usize>, str, str>}::get_unchecked"]
def core.ops.range.RangeToUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_unchecked
    (_range : core.ops.range.RangeTo Usize) (_text : ConstRawPtr Str) :
    Result (ConstRawPtr Str) :=
  fail .undef

/-- [...::get_unchecked_mut]: as `get_unchecked`. -/
@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeTo<usize>, str, str>}::get_unchecked_mut"]
def core.ops.range.RangeToUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_unchecked_mut
    (_range : core.ops.range.RangeTo Usize) (_text : MutRawPtr Str) :
    Result (MutRawPtr Str) :=
  fail .undef

/-- [...::index]: the prefix, or `slice_error_fail`'s panic when `end` is not a
char boundary. -/
@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeTo<usize>, str, str>}::index"]
def core.ops.range.RangeToUsize.Insts.CoreSliceIndexSliceIndexStrStr.index
    (range : core.ops.range.RangeTo Usize) (text : Str) : Result Str :=
  if strIsCharBoundary text range.end.val then
    core.slice.index.SliceIndexRangeToUsizeSlice.index range text
  else fail .panic

/-- [...::index_mut]: as `index`, with the library's prefix write-back. -/
@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeTo<usize>, str, str>}::index_mut"]
def core.ops.range.RangeToUsize.Insts.CoreSliceIndexSliceIndexStrStr.index_mut
    (range : core.ops.range.RangeTo Usize) (text : Str) : Result (Str × (Str → Str)) :=
  if strIsCharBoundary text range.end.val then
    core.slice.index.SliceIndexRangeToUsizeSlice.index_mut range text
  else fail .panic

/-! ## core::tuple and alloc::boxed -/

/-- Rust's tuple equality does not evaluate the second comparison when the
first is false; failures or divergence in either invoked comparison propagate. -/
@[rust_fun "core::tuple::{core::cmp::PartialEq<(@U, @T), (@U, @T)>}::eq"]
def Pair.Insts.CoreCmpPartialEqPair.eq
    {U T : Type} (first : core.cmp.PartialEq U U) (second : core.cmp.PartialEq T T)
    (left right : U × T) : Result Bool := do
  let equal ← first.eq left.1 right.1
  if equal then second.eq left.2 right.2 else ok false

@[rust_fun "alloc::boxed::{core::convert::AsRef<Box<@T>, @T>}::as_ref"]
def Box.Insts.CoreConvertAsRef.as_ref {T : Type} (_A : Type)
    (value : T) : Result T :=
  ok value

/-! ## alloc::collections::btree::map -/

/-- Where a sought key sits among `n` entries in ascending key order: at a
stored entry, or at the gap before entry `index` (`index = n` is the end). -/
inductive BTreeLocation (n : Nat) where
  | found (index : Nat) (inBounds : index < n)
  | vacant (index : Nat) (atMost : index ≤ n)

/-- Compare the sought key with each stored key in ascending order
(`compare stored` is `key.cmp(stored)`): skip while `Greater`, stop at `Equal`
(found) or `Less` (vacant here); past the last entry the key is vacant at the
end. -/
def btreeLocate {K V : Type} (compare : K → Result Ordering) :
    (entries : List (K × V)) → Result (BTreeLocation entries.length)
  | [] => ok (.vacant 0 (Nat.le_refl 0))
  | (stored, _) :: rest => do
    let order ← compare stored
    match order with
    | .lt => ok (.vacant 0 (Nat.zero_le _))
    | .eq => ok (.found 0 (Nat.succ_pos _))
    | .gt => do
      let location ← btreeLocate compare rest
      match location with
      | .found index inBounds => ok (.found (index + 1) (Nat.succ_lt_succ inBounds))
      | .vacant index atMost => ok (.vacant (index + 1) (Nat.succ_le_succ atMost))

/-- [alloc::collections::btree::map::{BTreeMap<K, V, Global>}::new]: the empty
map. -/
@[rust_fun
  "alloc::collections::btree::map::{alloc::collections::btree::map::BTreeMap<@K, @V, alloc::alloc::Global>}::new"]
def alloc.collections.btree.map.BTreeMapKVGlobal.new (K V : Type) :
    Result (alloc.collections.btree.map.BTreeMap K V Global) :=
  ok ⟨alloc.vec.Vec.new (K × V)⟩

/-- [alloc::collections::btree::map::{BTreeMap<K, V, A>}::len]: the entry
count. -/
@[rust_fun
  "alloc::collections::btree::map::{alloc::collections::btree::map::BTreeMap<@K, @V, @A>}::len"]
def alloc.collections.btree.map.BTreeMap.len {K V A : Type}
    (_allocInst : core.alloc.AllocatorClone A) (map : alloc.collections.btree.map.BTreeMap K V A) :
    Result Usize :=
  ok map.entries.len

/-- [alloc::collections::btree::map::{BTreeMap<K, V, A>}::is_empty]:
`self.len() == 0`. -/
@[rust_fun
  "alloc::collections::btree::map::{alloc::collections::btree::map::BTreeMap<@K, @V, @A>}::is_empty"]
def alloc.collections.btree.map.BTreeMap.is_empty {K V A : Type}
    (_allocInst : core.alloc.AllocatorClone A) (map : alloc.collections.btree.map.BTreeMap K V A) :
    Result Bool :=
  ok map.entries.val.isEmpty

/-- [alloc::collections::btree::map::{BTreeMap<K, V, A>}::get]: search with
`key.cmp(stored.borrow())` (see the header); the value of an equal key, or
`None`. -/
@[rust_fun
  "alloc::collections::btree::map::{alloc::collections::btree::map::BTreeMap<@K, @V, @A>}::get"]
def alloc.collections.btree.map.BTreeMap.get {K V A Q : Type}
    (_allocInst : core.alloc.AllocatorClone A) (borrowInst : core.borrow.Borrow K Q)
    (_ordInst : core.cmp.Ord K) (keyOrd : core.cmp.Ord Q)
    (map : alloc.collections.btree.map.BTreeMap K V A) (key : Q) : Result (Option V) := do
  let location ← btreeLocate (fun stored => do
    let view ← borrowInst.borrow stored
    keyOrd.cmp key view) map.entries.val
  match location with
  | .found index inBounds => ok (some (map.entries.val[index]'inBounds).2)
  | .vacant _ _ => ok none

/-- The entry for a located key: the map split at the found entry or at the
gap where the key belongs. -/
def btreeEntryAt {K V A : Type} (map : alloc.collections.btree.map.BTreeMap K V A) (key : K) :
    BTreeLocation map.entries.val.length → alloc.collections.btree.map.entry.Entry K V A
  | .found index inBounds =>
    .Occupied {
      before := map.entries.val.take index
      key := (map.entries.val[index]'inBounds).1
      value := (map.entries.val[index]'inBounds).2
      after := map.entries.val.drop (index + 1)
      bound := by
        have := map.entries.property
        simp only [List.length_take, List.length_drop]
        omega }
  | .vacant index atMost =>
    .Vacant {
      key
      before := map.entries.val.take index
      after := map.entries.val.drop index
      bound := by
        have := map.entries.property
        simp only [List.length_take, List.length_drop]
        omega }

/-- The map once the entry's borrow ends: a vacant entry leaves the entries as
they were; an occupied one puts its key and current value back in place. -/
def btreeEntryBack {K V A : Type} :
    alloc.collections.btree.map.entry.Entry K V A → alloc.collections.btree.map.BTreeMap K V A
  | .Vacant vacant =>
    ⟨alloc.vec.Vec.from (vacant.before ++ vacant.after) (by
      have := vacant.bound
      simp only [List.length_append]
      omega)⟩
  | .Occupied occupied =>
    ⟨alloc.vec.Vec.from (occupied.before ++ (occupied.key, occupied.value) :: occupied.after) (by
      have := occupied.bound
      simp only [List.length_append, List.length_cons]
      omega)⟩

/-- [alloc::collections::btree::map::{BTreeMap<K, V, A>}::entry]: search with
`key.cmp(stored)`; an equal key gives the occupied entry, which keeps the stored
key (the given key is dropped), otherwise the vacant entry holds the given key
at its insertion point. The backward function rebuilds the map from the
entry's final state. -/
@[rust_fun
  "alloc::collections::btree::map::{alloc::collections::btree::map::BTreeMap<@K, @V, @A>}::entry"]
def alloc.collections.btree.map.BTreeMap.entry {K V A : Type}
    (_allocInst : core.alloc.AllocatorClone A) (ordInst : core.cmp.Ord K)
    (map : alloc.collections.btree.map.BTreeMap K V A) (key : K) :
    Result (alloc.collections.btree.map.entry.Entry K V A ×
      (alloc.collections.btree.map.entry.Entry K V A →
        alloc.collections.btree.map.BTreeMap K V A)) := do
  let location ← btreeLocate (fun stored => ordInst.cmp key stored) map.entries.val
  ok (btreeEntryAt map key location, btreeEntryBack)

/-- `VacantEntry::insert(value)`: store the key and value at the vacant
position; a map already holding `usize::MAX` entries cannot count another one
and fails like the inherited vector's capacity overflow. The value written
through the returned reference becomes the stored value. -/
def btreeInsertVacant {K V A : Type} (vacant : alloc.collections.btree.map.entry.VacantEntry K V A)
    (value : V) : Result (V × (V → alloc.collections.btree.map.entry.Entry K V A)) :=
  if bound : vacant.before.length + vacant.after.length + 1 ≤ Usize.max then
    ok (value, fun written => .Occupied {
      before := vacant.before, key := vacant.key, value := written,
      after := vacant.after, bound })
  else fail .panic

/-- [alloc::collections::btree::map::entry::{Entry<'a, K, V, A>}::or_default]:
an occupied entry yields its value; a vacant one first builds `V::default()`
and then inserts it (`VacantEntry::insert`). The backward function turns the
value written through the reference into the entry's final state. -/
@[rust_fun
  "alloc::collections::btree::map::entry::{alloc::collections::btree::map::entry::Entry<'a, @K, @V, @A>}::or_default"]
def alloc.collections.btree.map.entry.Entry.or_default {K V A : Type}
    (_ordInst : core.cmp.Ord K) (defaultInst : core.default.Default V)
    (_allocInst : core.alloc.AllocatorClone A) :
    alloc.collections.btree.map.entry.Entry K V A →
      Result (V × (V → alloc.collections.btree.map.entry.Entry K V A))
  | .Occupied occupied => ok (occupied.value, fun written => .Occupied { occupied with value := written })
  | .Vacant vacant => do
    let value ← defaultInst.default
    btreeInsertVacant vacant value

/-- The map after `insert` at a located position: a found key keeps its stored
key and takes the new value; otherwise the pair is inserted at its position. -/
def btreeInsertAt {K V A : Type} (map : alloc.collections.btree.map.BTreeMap K V A) (key : K)
    (value : V) : BTreeLocation map.entries.val.length →
      Result (Option V × alloc.collections.btree.map.BTreeMap K V A)
  | .found index inBounds =>
    ok (some (map.entries.val[index]'inBounds).2,
      ⟨alloc.vec.Vec.from
        (map.entries.val.set index ((map.entries.val[index]'inBounds).1, value)) (by
          have := map.entries.property
          simp only [List.length_set]
          omega)⟩)
  | .vacant index _ =>
    if bound : map.entries.val.length + 1 ≤ Usize.max then
      ok (none, ⟨alloc.vec.Vec.from
        (map.entries.val.take index ++ (key, value) :: map.entries.val.drop index) (by
          simp only [List.length_append, List.length_take, List.length_cons, List.length_drop]
          omega)⟩)
    else fail .panic

/-- [alloc::collections::btree::map::{BTreeMap<K, V, A>}::insert]:
`match self.entry(key) { Occupied(e) => Some(e.insert(value)), Vacant(e) =>
{ e.insert(value); None } }`: the old value of an equal key is returned and
replaced (the stored key is kept); a new key is inserted in order. -/
@[rust_fun
  "alloc::collections::btree::map::{alloc::collections::btree::map::BTreeMap<@K, @V, @A>}::insert"]
def alloc.collections.btree.map.BTreeMap.insert {K V A : Type}
    (_allocInst : core.alloc.AllocatorClone A) (ordInst : core.cmp.Ord K)
    (map : alloc.collections.btree.map.BTreeMap K V A) (key : K) (value : V) :
    Result (Option V × alloc.collections.btree.map.BTreeMap K V A) := do
  let location ← btreeLocate (fun stored => ordInst.cmp key stored) map.entries.val
  btreeInsertAt map key value location

/-- Clone every key and then its value, in ascending key order: std's
`clone_subtree` walks the tree in order and clones each `k` before its `v`. -/
def btreeCloneEntries {K V : Type} (keyInst : core.clone.Clone K)
    (valueInst : core.clone.Clone V) : {n : Nat} → ListN (K × V) n → Result (ListN (K × V) n)
  | _, .nil => ok .nil
  | _, .cons (key, value) rest => do
    let key ← keyInst.clone key
    let value ← valueInst.clone value
    let rest ← btreeCloneEntries keyInst valueInst rest
    ok (.cons (key, value) rest)

/-- [alloc::collections::btree::map::{impl Clone for BTreeMap<K, V, A>}::clone]:
a map with the cloned entries in the same order. -/
@[rust_fun
  "alloc::collections::btree::map::{core::clone::Clone<alloc::collections::btree::map::BTreeMap<@K, @V, @A>>}::clone"]
def alloc.collections.btree.map.BTreeMap.Insts.CoreCloneClone.clone {K V A : Type}
    (keyInst : core.clone.Clone K) (valueInst : core.clone.Clone V)
    (_allocInst : core.alloc.AllocatorClone A) (map : alloc.collections.btree.map.BTreeMap K V A) :
    Result (alloc.collections.btree.map.BTreeMap K V A) := do
  let entries ← btreeCloneEntries keyInst valueInst map.entries.slice.list
  ok ⟨⟨⟨map.entries.slice.leng, entries, map.entries.slice.bound⟩⟩⟩

/-- [alloc::collections::btree::map::{BTreeMap<K, V, A>}::keys]: an iterator
over the entries in ascending key order. -/
@[rust_fun
  "alloc::collections::btree::map::{alloc::collections::btree::map::BTreeMap<@K, @V, @A>}::keys"]
def alloc.collections.btree.map.BTreeMap.keys {K V A : Type}
    (_allocInst : core.alloc.AllocatorClone A) (map : alloc.collections.btree.map.BTreeMap K V A) :
    Result (alloc.collections.btree.map.Keys K V) :=
  ok ⟨map.entries.val⟩

/-- [alloc::collections::btree::map::{BTreeMap<K, V, A>}::values]: as `keys`. -/
@[rust_fun
  "alloc::collections::btree::map::{alloc::collections::btree::map::BTreeMap<@K, @V, @A>}::values"]
def alloc.collections.btree.map.BTreeMap.values {K V A : Type}
    (_allocInst : core.alloc.AllocatorClone A) (map : alloc.collections.btree.map.BTreeMap K V A) :
    Result (alloc.collections.btree.map.Values K V) :=
  ok ⟨map.entries.val⟩

/-- [alloc::collections::btree::map::{impl Iterator for Keys<'a, K, V>}::next]:
the next key in ascending order; an exhausted iterator yields `None` and stays
exhausted. -/
@[rust_fun
  "alloc::collections::btree::map::{core::iter::traits::iterator::Iterator<alloc::collections::btree::map::Keys<'a, @K, @V>, &'a @K>}::next"]
def alloc.collections.btree.map.Keys.Insts.CoreIterTraitsIteratorIteratorSharedAK.next
    {K V : Type} (iter : alloc.collections.btree.map.Keys K V) :
    Result (Option K × alloc.collections.btree.map.Keys K V) :=
  match iter.remaining with
  | [] => ok (none, iter)
  | (key, _) :: rest => ok (some key, ⟨rest⟩)

/-- [alloc::collections::btree::map::{impl Iterator for Values<'a, K, V>}::next]:
the next value in ascending key order. -/
@[rust_fun
  "alloc::collections::btree::map::{core::iter::traits::iterator::Iterator<alloc::collections::btree::map::Values<'a, @K, @V>, &'a @V>}::next"]
def alloc.collections.btree.map.Values.Insts.CoreIterTraitsIteratorIteratorSharedAV.next
    {K V : Type} (iter : alloc.collections.btree.map.Values K V) :
    Result (Option V × alloc.collections.btree.map.Values K V) :=
  match iter.remaining with
  | [] => ok (none, iter)
  | (_, value) :: rest => ok (some value, ⟨rest⟩)

/-- Pairwise `(&K, &V)` equality in order, stopping at the first difference:
the key comparison, then the value comparison only when the keys are equal. -/
def btreeEntriesEqual {K V : Type} (keyInst : core.cmp.PartialEq K K)
    (valueInst : core.cmp.PartialEq V V) : List (K × V) → List (K × V) → Result Bool
  | (leftKey, leftValue) :: lefts, (rightKey, rightValue) :: rights => do
    let sameKey ← keyInst.eq leftKey rightKey
    if sameKey then
      let sameValue ← valueInst.eq leftValue rightValue
      if sameValue then btreeEntriesEqual keyInst valueInst lefts rights else ok false
    else ok false
  | _, _ => ok true

/-- [alloc::collections::btree::map::{impl PartialEq for BTreeMap<K, V, A>}::eq]:
`self.len() == other.len() && self.iter().zip(other).all(|(a, b)| a == b)`. -/
@[rust_fun
  "alloc::collections::btree::map::{core::cmp::PartialEq<alloc::collections::btree::map::BTreeMap<@K, @V, @A>, alloc::collections::btree::map::BTreeMap<@K, @V, @A>>}::eq"]
def alloc.collections.btree.map.BTreeMap.Insts.CoreCmpPartialEqBTreeMap.eq {K V A : Type}
    (keyInst : core.cmp.PartialEq K K) (valueInst : core.cmp.PartialEq V V)
    (_allocInst : core.alloc.AllocatorClone A)
    (left right : alloc.collections.btree.map.BTreeMap K V A) : Result Bool :=
  if left.entries.val.length = right.entries.val.length then
    btreeEntriesEqual keyInst valueInst left.entries.val right.entries.val
  else ok false

/-- Format each key and then its value, in order, stopping at the first error. -/
def btreeFormatEntries {K V : Type} (keyInst : core.fmt.Debug K) (valueInst : core.fmt.Debug V) :
    List (K × V) → core.fmt.Formatter →
      Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter)
  | [], formatter => ok (.Ok (), formatter)
  | (key, value) :: rest, formatter => do
    let (result, formatter) ← keyInst.fmt key formatter
    match result with
    | .Err error => ok (.Err error, formatter)
    | .Ok () => do
      let (result, formatter) ← valueInst.fmt value formatter
      match result with
      | .Err error => ok (.Err error, formatter)
      | .Ok () => btreeFormatEntries keyInst valueInst rest formatter

/-- [alloc::collections::btree::map::{impl Debug for BTreeMap<K, V, A>}::fmt]:
`f.debug_map().entries(self.iter()).finish()`: each entry formats its key and
then its value, and a failure skips the remaining entries. -/
@[rust_fun
  "alloc::collections::btree::map::{core::fmt::Debug<alloc::collections::btree::map::BTreeMap<@K, @V, @A>>}::fmt"]
def alloc.collections.btree.map.BTreeMap.Insts.CoreFmtDebug.fmt {K V A : Type}
    (keyInst : core.fmt.Debug K) (valueInst : core.fmt.Debug V)
    (_allocInst : core.alloc.AllocatorClone A) (map : alloc.collections.btree.map.BTreeMap K V A)
    (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  btreeFormatEntries keyInst valueInst map.entries.val formatter

/-- Hash each key and then its value (`(&K, &V)::hash`), in order. -/
def btreeHashEntries {K V H : Type} (keyInst : core.hash.Hash K) (valueInst : core.hash.Hash V)
    (hasher : core.hash.Hasher H) : List (K × V) → H → Result H
  | [], state => ok state
  | (key, value) :: rest, state => do
    let state ← keyInst.hash hasher key state
    let state ← valueInst.hash hasher value state
    btreeHashEntries keyInst valueInst hasher rest state

/-- [alloc::collections::btree::map::{impl Hash for BTreeMap<K, V, A>}::hash]:
`state.write_length_prefix(self.len())`, then every entry in order. -/
@[rust_fun
  "alloc::collections::btree::map::{core::hash::Hash<alloc::collections::btree::map::BTreeMap<@K, @V, @A>>}::hash"]
def alloc.collections.btree.map.BTreeMap.Insts.CoreHashHash.hash {K V A H : Type}
    (keyInst : core.hash.Hash K) (valueInst : core.hash.Hash V)
    (_allocInst : core.alloc.AllocatorClone A) (hasher : core.hash.Hasher H)
    (map : alloc.collections.btree.map.BTreeMap K V A) (state : H) : Result H := do
  let state ← writeLengthPrefix hasher state map.entries.val.length
  btreeHashEntries keyInst valueInst hasher map.entries.val state

/-! ## alloc::rc

`Rc` is a value-only carrier of an immutable payload (`TypesExternal.lean`):
dereferencing reads the payload, cloning shares it (reference counts are not
represented), and comparisons and formatting delegate to the payload. -/

/-- [alloc::rc::{Rc<T>}::new]: a new carrier of the value. -/
@[rust_fun "alloc::rc::{alloc::rc::Rc<@T>}::new"]
def alloc.rc.Rc.new {T : Type} (value : T) : Result (alloc.rc.Rc T) :=
  ok ⟨value⟩

/-- [alloc::rc::{Rc<T>}::ptr_eq]: allocation identity, `fail .undef` (see the
header). -/
@[rust_fun "alloc::rc::{alloc::rc::Rc<@T>}::ptr_eq"]
def alloc.rc.Rc.ptr_eq {T : Type} (_A : Type) (_left _right : alloc.rc.Rc T) : Result Bool :=
  fail .undef

/-- [alloc::rc::{impl Deref for Rc<T>}::deref]: the shared payload. -/
@[rust_fun "alloc::rc::{core::ops::deref::Deref<alloc::rc::Rc<@T>, @T>}::deref"]
def alloc.rc.Rc.Insts.CoreOpsDerefDeref.deref {T : Type} (_A : Type) (rc : alloc.rc.Rc T) :
    Result T :=
  ok rc.value

/-- [alloc::rc::{impl AsRef<T> for Rc<T>}::as_ref]: the shared payload. -/
@[rust_fun "alloc::rc::{core::convert::AsRef<alloc::rc::Rc<@T>, @T>}::as_ref"]
def alloc.rc.Rc.Insts.CoreConvertAsRef.as_ref {T : Type} (_A : Type) (rc : alloc.rc.Rc T) :
    Result T :=
  ok rc.value

/-- [alloc::rc::{impl Clone for Rc<T>}::clone]: another pointer to the same
payload; the strong-count increment is not represented. -/
@[rust_fun "alloc::rc::{core::clone::Clone<alloc::rc::Rc<@T>>}::clone"]
def alloc.rc.Rc.Insts.CoreCloneClone.clone {T A : Type}
    (_allocInst : core.alloc.AllocatorClone A) (rc : alloc.rc.Rc T) : Result (alloc.rc.Rc T) :=
  ok rc

/-- [alloc::rc::{impl PartialEq for Rc<T>}::eq]: `**self == **other`. For an
`Eq` payload std first compares allocation identity; identical allocations hold
identical payloads, on which a reflexive `Eq` returns `true`, so the result is
the payload comparison that this model always runs. -/
@[rust_fun "alloc::rc::{core::cmp::PartialEq<alloc::rc::Rc<@T>, alloc::rc::Rc<@T>>}::eq"]
def alloc.rc.Rc.Insts.CoreCmpPartialEqRc.eq {T : Type} (_A : Type)
    (inst : core.cmp.PartialEq T T) (left right : alloc.rc.Rc T) : Result Bool :=
  inst.eq left.value right.value

/-- [alloc::rc::{impl PartialEq for Rc<T>}::ne]: `**self != **other`, with the
same allocation-identity shortcut for `Eq` payloads as `eq`. -/
@[rust_fun "alloc::rc::{core::cmp::PartialEq<alloc::rc::Rc<@T>, alloc::rc::Rc<@T>>}::ne"]
def alloc.rc.Rc.Insts.CoreCmpPartialEqRc.ne {T : Type} (_A : Type)
    (inst : core.cmp.PartialEq T T) (left right : alloc.rc.Rc T) : Result Bool :=
  inst.ne left.value right.value

/-- [alloc::rc::{impl Debug for Rc<T>}::fmt]: `fmt::Debug::fmt(&**self, f)`. -/
@[rust_fun "alloc::rc::{core::fmt::Debug<alloc::rc::Rc<@T>>}::fmt"]
def alloc.rc.Rc.Insts.CoreFmtDebug.fmt {T : Type} (_A : Type) (inst : core.fmt.Debug T)
    (rc : alloc.rc.Rc T) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  inst.fmt rc.value formatter

/-! ## alloc::string -/

@[rust_fun "alloc::string::{alloc::string::String}::as_bytes"]
def alloc.string.String.as_bytes (text : String) : Result (Slice U8) :=
  _root_.noble_contracts.alloc.string.String.as_bytes text

-- Rust's `String::len` counts UTF-8 bytes, not Unicode scalar values.
@[rust_fun "alloc::string::{alloc::string::String}::len"]
def alloc.string.String.len (value : String) : Result Usize :=
  _root_.noble_contracts.alloc.string.String.len value

@[rust_fun "alloc::string::{core::clone::Clone<alloc::string::String>}::clone"]
def alloc.string.String.Insts.CoreCloneClone.clone (text : String) : Result String :=
  _root_.noble_contracts.alloc.string.String.Insts.CoreCloneClone.clone text

/-! ## alloc::vec -/

/-- Reservation changes capacity, not elements. The inherited layout-free Vec
model erases allocation, but an overflowing element count still panics. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::reserve"]
def alloc.vec.Vec.reserve {T : Type} (_A : Type) (vector : alloc.vec.Vec T)
    (additional : Usize) : Result (alloc.vec.Vec T) :=
  if vector.val.length + additional.val ≤ Usize.max then ok vector else fail .panic

/-- Exact reservation has the same element-count overflow boundary; only
spare capacity differs, which the inherited vector representation erases. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::reserve_exact"]
def alloc.vec.Vec.reserve_exact {T : Type} (A : Type) (vector : alloc.vec.Vec T)
    (additional : Usize) : Result (alloc.vec.Vec T) :=
  alloc.vec.Vec.reserve A vector additional

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::as_slice"]
def alloc.vec.Vec.as_slice {T : Type} (A : Type) (vector : alloc.vec.Vec T) :
    Result (Slice T) :=
  _root_.noble_kernel.alloc.vec.Vec.as_slice A vector

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::pop"]
def alloc.vec.Vec.pop {T : Type} (A : Type) (vector : alloc.vec.Vec T) :
    Result (Option T × alloc.vec.Vec T) :=
  _root_.noble_kernel.alloc.vec.Vec.pop A vector

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::is_empty"]
def alloc.vec.Vec.is_empty {T : Type} (A : Type) (vector : alloc.vec.Vec T) :
    Result Bool :=
  _root_.noble_kernel.alloc.vec.Vec.is_empty A vector

/-- [alloc::vec::{Vec<T>}::truncate]: the kernel's model. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::truncate"]
def alloc.vec.Vec.truncate {T : Type} (A : Type) (vector : alloc.vec.Vec T) (length : Usize) :
    Result (alloc.vec.Vec T) :=
  _root_.noble_kernel.alloc.vec.Vec.truncate A vector length

/-- [alloc::vec::{Vec<T>}::remove]: the kernel's model (order-preserving;
an invalid index fails out of bounds). -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::remove"]
def alloc.vec.Vec.remove {T : Type} (A : Type) (vector : alloc.vec.Vec T) (index : Usize) :
    Result (T × alloc.vec.Vec T) :=
  _root_.noble_kernel.alloc.vec.Vec.remove A vector index

/-- [alloc::vec::{Vec<T>}::clear]: the kernel's model. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::clear"]
def alloc.vec.Vec.clear {T : Type} (A : Type) (vector : alloc.vec.Vec T) :
    Result (alloc.vec.Vec T) :=
  _root_.noble_kernel.alloc.vec.Vec.clear A vector

/-- [alloc::vec::{impl Default for Vec<T>}::default]: the kernel's model. -/
@[rust_fun "alloc::vec::{core::default::Default<alloc::vec::Vec<@T>>}::default"]
def alloc.vec.Vec.Insts.CoreDefaultDefault.default (T : Type) : Result (alloc.vec.Vec T) :=
  _root_.noble_kernel.alloc.vec.Vec.Insts.CoreDefaultDefault.default T

/-- Call the predicate once on each element, in order, threading its state, and
keep exactly the elements it accepts, in their order. -/
def vecRetainElements {T F : Type} (inst : core.ops.function.FnMut F T Bool) :
    (elements : List T) → F → Result ({kept : List T // kept.length ≤ elements.length} × F)
  | [], predicate => ok (⟨[], Nat.le_refl 0⟩, predicate)
  | element :: rest, predicate => do
    let (keep, predicate) ← inst.call_mut predicate element
    let (kept, predicate) ← vecRetainElements inst rest predicate
    if keep then ok (⟨element :: kept.val, Nat.succ_le_succ kept.property⟩, predicate)
    else ok (⟨kept.val, Nat.le_succ_of_le kept.property⟩, predicate)

/-- [alloc::vec::{Vec<T>}::retain]: `retain_mut` calls the predicate exactly
once per element, in order, and keeps the accepted elements in order; the
predicate is dropped afterwards. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::retain"]
def alloc.vec.Vec.retain {T : Type} (_A : Type) {F : Type}
    (inst : core.ops.function.FnMut F T Bool) (vector : alloc.vec.Vec T) (predicate : F) :
    Result (alloc.vec.Vec T) := do
  let (kept, _) ← vecRetainElements inst vector.val predicate
  ok (alloc.vec.Vec.from kept.val (Nat.le_trans kept.property vector.property))

/-- [alloc::vec::{impl Hash for Vec<T>}::hash]: `fail .undef` (see the
header). -/
@[rust_fun "alloc::vec::{core::hash::Hash<alloc::vec::Vec<@T>>}::hash"]
def alloc.vec.Vec.Insts.CoreHashHash.hash {T : Type} (_A : Type) {H : Type}
    (_inst : core.hash.Hash T) (_hasher : core.hash.Hasher H) (_vector : alloc.vec.Vec T)
    (_state : H) : Result H :=
  fail .undef

/-- [alloc::slice::{impl Borrow<[T]> for Vec<T>}::borrow]: `&self[..]`, the
vector's elements as a slice. -/
@[rust_fun "alloc::slice::{core::borrow::Borrow<alloc::vec::Vec<@T>, [@T]>}::borrow"]
def alloc.vec.Vec.Insts.CoreBorrowBorrowSlice.borrow {T : Type} (_A : Type)
    (vector : alloc.vec.Vec T) : Result (Slice T) :=
  ok vector.slice

/-- [alloc::vec::{impl IntoIterator for &'a Vec<T>}::into_iter]: `self.iter()`,
the slice iterator over the elements. -/
@[rust_fun
  "alloc::vec::{core::iter::traits::collect::IntoIterator<&'a alloc::vec::Vec<@T>, &'a @T, core::slice::iter::Iter<'a, @T>>}::into_iter"]
def SharedAVec.Insts.CoreIterTraitsCollectIntoIteratorSharedATIter.into_iter {T : Type}
    (_A : Type) (vector : alloc.vec.Vec T) : Result (core.slice.iter.Iter T) :=
  core.slice.Slice.iter vector.slice

/-- [alloc::vec::{impl PartialOrd for Vec<T>}::partial_cmp]: the slices'
`partial_cmp`. -/
@[rust_fun
  "alloc::vec::{core::cmp::PartialOrd<alloc::vec::Vec<@T>, alloc::vec::Vec<@T>>}::partial_cmp"]
def alloc.vec.Vec.Insts.CoreCmpPartialOrdVec.partial_cmp {T : Type _} (_A1 _A2 : Type _)
    (inst : core.cmp.PartialOrd T T) (left right : alloc.vec.Vec T) : Result (Option Ordering) :=
  Slice.Insts.CoreCmpPartialOrdSlice.partial_cmp inst left.slice right.slice

/-- [alloc::vec::{impl Ord for Vec<T>}::cmp]: the slices' `cmp`. -/
@[rust_fun "alloc::vec::{core::cmp::Ord<alloc::vec::Vec<@T>>}::cmp"]
def alloc.vec.Vec.Insts.CoreCmpOrd.cmp {T : Type _} (_A : Type _) (inst : core.cmp.Ord T)
    (left right : alloc.vec.Vec T) : Result Ordering :=
  Slice.Insts.CoreCmpOrd.cmp inst left.slice right.slice

-- Slice equality checks length first, then invokes the supplied element
-- comparison in order; its failures propagate without visiting later items.
@[rust_fun
  "alloc::vec::partial_eq::{core::cmp::PartialEq<alloc::vec::Vec<@T>, [@U; @N]>}::eq"]
def alloc.vec.Vec.Insts.CoreCmpPartialEqArray.eq
    {T U : Type} (_A : Type) {N : Usize} (inst : core.cmp.PartialEq T U)
    (left : alloc.vec.Vec T) (right : Array U N) : Result Bool :=
  core.slice.cmp.PartialEqSlice.eq inst left.slice right.to_slice

@[rust_fun
  "alloc::vec::partial_eq::{core::cmp::PartialEq<alloc::vec::Vec<@T>, [@U; @N]>}::ne"]
def alloc.vec.Vec.Insts.CoreCmpPartialEqArray.ne
    {T U : Type} (_A : Type) {N : Usize} (inst : core.cmp.PartialEq T U)
    (left : alloc.vec.Vec T) (right : Array U N) : Result Bool :=
  core.slice.cmp.PartialEqSlice.ne inst left.slice right.to_slice

end noble_wasm
