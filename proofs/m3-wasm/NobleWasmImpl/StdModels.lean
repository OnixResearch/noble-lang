import Aeneas
import NobleWasmImpl.Types
import NobleKernel.FunsExternal

open Aeneas Aeneas.Std Result

namespace noble_wasm

/-!
The nine standard-library boundaries of the emitter extraction. Scalars use
Aeneas's fixed-width mathematical values, with the pinned extraction targeting
x86_64. Fallible casts retain Rust's success/error distinction. Slice and vector
endpoints retain every element and its order; no pointer or allocation identity
is used. As in the inherited Aeneas vector model, allocator exhaustion and spare
capacity are not observable.

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

/-- Taking the magnitude in `Int` handles `i64::MIN` without signed overflow. -/
@[rust_fun "core::num::{i64}::unsigned_abs"]
def core.num.I64.unsigned_abs (value : I64) : Result U64 :=
  UScalar.tryMk .U64 value.val.natAbs

@[rust_fun "core::slice::{[@T]}::last"]
def core.slice.Slice.last {T : Type} (slice : Slice T) : Result (Option T) :=
  ok slice.val.getLast?

/-- `Str` is the inherited exact UTF-8 byte slice; Rust's view does not copy it. -/
@[rust_fun "core::str::{str}::as_bytes"]
def core.str.Str.as_bytes (text : Str) : Result (Slice U8) :=
  ok text

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::pop"]
def alloc.vec.Vec.pop {T : Type} (A : Type) (vector : alloc.vec.Vec T) :
    Result (Option T × alloc.vec.Vec T) :=
  _root_.noble_kernel.alloc.vec.Vec.pop A vector

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::is_empty"]
def alloc.vec.Vec.is_empty {T : Type} (A : Type) (vector : alloc.vec.Vec T) :
    Result Bool :=
  _root_.noble_kernel.alloc.vec.Vec.is_empty A vector

end noble_wasm
