import Aeneas
import NobleWasmImpl.Types
import NobleKernel.FunsExternal
import NobleContractImpl.StdModels

open Aeneas Aeneas.Std Result

namespace noble_wasm

/-!
Standard-library boundaries of the expanded managed-memory compiler extraction.
Scalars use Aeneas's fixed-width mathematical values, with the pinned extraction
targeting x86_64. Fallible casts retain Rust's success/error distinction;
saturating multiplication clamps the exact mathematical product. Clone and
comparison boundaries invoke the supplied trait implementations, preserving
their failures and Rust's left-to-right short-circuiting.

Slice and vector endpoints retain every element and its order; no pointer or
allocation identity is used. As in the inherited Aeneas vector model, allocator
exhaustion, spare capacity, and type-layout-dependent allocation limits are not
observable. Reserve nevertheless retains the capacity-overflow panic when the
requested element count exceeds `usize::MAX`, rather than wrapping that count.

These definitions add no native-evaluation proofs. The generated emitter's
byte-array construction may transitively use native evaluation through the
pinned Aeneas library; the implementation audit reports that separate boundary.
-/

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

/-- Taking the magnitude in `Int` handles `i64::MIN` without signed overflow. -/
@[rust_fun "core::num::{i64}::unsigned_abs"]
def core.num.I64.unsigned_abs (value : I64) : Result U64 :=
  UScalar.tryMk .U64 value.val.natAbs

/-- The pinned signed builtin incorrectly takes `I8` bytes. Decode unsigned
bytes with its exact `U64` counterpart, then reinterpret the same 64 bits as
two's complement: values at least `2^63` become `value - 2^64`. -/
@[rust_fun "core::num::{i64}::from_le_bytes" -canFail]
def core.num.I64.from_le_bytes (bytes : Array U8 8#usize) : I64 :=
  UScalar.hcast .I64 (_root_.Aeneas.Std.core.num.U64.from_le_bytes bytes)

@[rust_fun "core::num::{u64}::saturating_mul"]
def core.num.U64.saturating_mul (left right : U64) : Result U64 :=
  UScalar.tryMk .U64 (min U64.max (left.val * right.val))

@[rust_fun "core::num::{usize}::saturating_mul"]
def core.num.Usize.saturating_mul (left right : Usize) : Result Usize :=
  _root_.noble_kernel.core.num.Usize.saturating_mul left right

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
    {T : Type} (inst : core.cmp.PartialEq T T) (left right : Option T) : Result Bool :=
  _root_.noble_contracts.core.option.Option.Insts.CoreCmpPartialEqOption.eq inst left right

@[rust_fun "core::result::{core::result::Result<@T, @E>}::is_err"]
def core.result.Result.is_err
    {T E : Type} (value : core.result.Result T E) : Result Bool :=
  _root_.noble_contracts.core.result.Result.is_err value

@[rust_fun "core::result::{core::result::Result<@T, @E>}::unwrap_or"]
def core.result.Result.unwrap_or
    {T E : Type} (value : core.result.Result T E) (fallback : T) : Result T :=
  _root_.noble_kernel.core.result.Result.unwrap_or value fallback

/-- Reuse the frontend's state-threaded traversal: the returned iterator has
consumed the matching item and the index is relative to its initial cursor. -/
@[rust_fun
  "core::slice::iter::{core::iter::traits::iterator::Iterator<core::slice::iter::Iter<'a, @T>, &'a @T>}::position"]
def core.slice.iter.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.position
    {T P : Type} (inst : core.ops.function.FnMut P T Bool)
    (iterator : core.slice.iter.Iter T) (predicate : P) :
    Result (Option Usize × core.slice.iter.Iter T) :=
  _root_.noble_contracts.core.slice.iter.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.position
    inst iterator predicate

@[rust_fun "core::slice::{[@T]}::first"]
def core.slice.Slice.first {T : Type} (slice : Slice T) : Result (Option T) :=
  _root_.noble_contracts.core.slice.Slice.first slice

@[rust_fun "core::slice::{[@T]}::last"]
def core.slice.Slice.last {T : Type} (slice : Slice T) : Result (Option T) :=
  ok slice.val.getLast?

@[rust_fun "core::slice::raw::from_ref"]
def core.slice.raw.from_ref {T : Type} (value : T) : Result (Slice T) :=
  ok (.from [value] (by scalar_tac))

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

/-- Rust's tuple equality does not evaluate the second comparison when the
first is false; failures or divergence in either invoked comparison propagate. -/
@[rust_fun "core::tuple::{core::cmp::PartialEq<(@U, @T), (@U, @T)>}::eq"]
def Pair.Insts.CoreCmpPartialEqPair.eq
    {U T : Type} (first : core.cmp.PartialEq U U) (second : core.cmp.PartialEq T T)
    (left right : U × T) : Result Bool := do
  let equal ← first.eq left.1 right.1
  if equal then second.eq left.2 right.2 else ok false

@[rust_fun "alloc::string::{alloc::string::String}::as_bytes"]
def alloc.string.String.as_bytes (text : String) : Result (Slice U8) :=
  _root_.noble_contracts.alloc.string.String.as_bytes text

@[rust_fun "alloc::string::{core::clone::Clone<alloc::string::String>}::clone"]
def alloc.string.String.Insts.CoreCloneClone.clone (text : String) : Result String :=
  _root_.noble_contracts.alloc.string.String.Insts.CoreCloneClone.clone text

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

end noble_wasm
