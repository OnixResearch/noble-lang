import Aeneas
import NobleContractImpl.Types
import NobleKernel.FunsExternal

open Aeneas Aeneas.Std Result ControlFlow Error

set_option linter.dupNamespace false
set_option linter.unusedVariables false
set_option maxHeartbeats 1000000
set_option maxRecDepth 2048
set_option Aeneas.customDoElab false

namespace noble_contracts

/-!
Standard-library boundaries for the actual `noble-contracts` extraction.

`Str` is Aeneas's bounded byte slice, whereas owned `String` is Lean's
UTF-8 string. The conversions below preserve bytes, not character counts.
Lean's `String.fromUTF8?` checks `ByteArray.IsValidUTF8`; its successful
branch uses `String.fromUTF8`, whose byte array is definitionally the input.
The separate Rust validator retains Rust's precise invalid-prefix diagnostics.

As in Aeneas's `Vec` model, allocation identity, spare capacity, and allocator
exhaustion are not observable. String allocation requests nevertheless retain
Rust's `isize::MAX` capacity-overflow panic. Checked scalar construction also
rejects mathematical Lean strings too large for `usize`, rather than wrapping
their lengths. Invalid `Str` bytes cannot arise from safe Rust; decoding them
returns Aeneas's `undef`, not a replacement-character or empty-string fallback.

Aeneas's formatter writes return success and the unchanged formatter, erasing
text, escaping, and layout flags. We retain this abstraction, but invoke every
supplied `Debug` implementation in order, short-circuiting both Rust formatting
errors and Aeneas failures and preserving each returned formatter.
-/

/-! ## Slice/array comparison and scalar operations -/

@[rust_fun "core::array::equality::{core::cmp::PartialEq<[@T], [@U; @N]>}::eq"]
def Slice.Insts.CoreCmpPartialEqArray.eq
    {T U : Type} {N : Usize} (inst : core.cmp.PartialEq T U)
    (left : Slice T) (right : Array U N) : Result Bool :=
  core.slice.cmp.PartialEqSlice.eq inst left right.to_slice

@[rust_fun "core::array::equality::{core::cmp::PartialEq<[@T], [@U; @N]>}::ne"]
def Slice.Insts.CoreCmpPartialEqArray.ne
    {T U : Type} {N : Usize} (inst : core.cmp.PartialEq T U)
    (left : Slice T) (right : Array U N) : Result Bool := do
  let equal ← Slice.Insts.CoreCmpPartialEqArray.eq inst left right
  ok (!equal)

@[rust_fun "core::char::convert::{core::convert::From<char, u8>}::from"]
def Char.Insts.CoreConvertFromU8.from (value : U8) : Result Char :=
  ok (_root_.Char.ofUInt8 (UInt8.ofBitVec value.bv))

@[rust_fun "core::convert::num::{core::convert::From<i64, u8>}::from"]
def I64.Insts.CoreConvertFromU8.from (value : U8) : Result I64 :=
  ok (UScalar.hcast .I64 value)

@[rust_fun
  "core::convert::num::ptr_try_from_impls::{core::convert::TryFrom<usize, u32, core::num::error::TryFromIntError>}::try_from"]
def Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from
    (value : U32) : Result (core.result.Result Usize core.num.error.TryFromIntError) :=
  _root_.noble_kernel.Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from value

@[rust_fun
  "core::convert::num::{core::convert::TryFrom<u32, i64, core::num::error::TryFromIntError>}::try_from"]
def U32.Insts.CoreConvertTryFromI64TryFromIntError.try_from
    (value : I64) : Result (core.result.Result U32 core.num.error.TryFromIntError) :=
  if 0 ≤ value.val ∧ value.val ≤ Int.ofNat U32.max then
    ok (.Ok (IScalar.hcast .U32 value))
  else
    ok (.Err ())

/-- Taking the absolute value in `Int` also handles `i64::MIN`, whose unsigned
magnitude is `2^63`. No intermediate signed negation can overflow. -/
@[rust_fun "core::num::{i64}::unsigned_abs"]
def core.num.I64.unsigned_abs (value : I64) : Result U64 :=
  UScalar.tryMk .U64 value.val.natAbs

@[rust_fun "core::num::{u8}::is_ascii_alphabetic"]
def core.num.U8.is_ascii_alphabetic (value : U8) : Result Bool :=
  ok (decide ((65 ≤ value.val ∧ value.val ≤ 90) ∨
    (97 ≤ value.val ∧ value.val ≤ 122)))

@[rust_fun "core::num::{u8}::is_ascii_digit"]
def core.num.U8.is_ascii_digit (value : U8) : Result Bool :=
  ok (decide (48 ≤ value.val ∧ value.val ≤ 57))

@[rust_fun "core::num::{u8}::is_ascii_graphic"]
def core.num.U8.is_ascii_graphic (value : U8) : Result Bool :=
  ok (decide (33 ≤ value.val ∧ value.val ≤ 126))

@[rust_fun "core::num::{u32}::saturating_mul"]
def core.num.U32.saturating_mul (left right : U32) : Result U32 :=
  UScalar.tryMk .U32 (min U32.max (left.val * right.val))

/-! ## Formatting, option, and result -/

@[rust_fun "core::fmt::{core::fmt::Formatter<'a>}::debug_tuple_field2_finish"]
def core.fmt.Formatter.debug_tuple_field2_finish
    (formatter : core.fmt.Formatter) (label : Str)
    (first second : Dyn (fun T => core.fmt.Debug T)) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) := do
  let (result, formatter) ← core.fmt.Formatter.write_str formatter label
  match result with
  | .Err error => ok (.Err error, formatter)
  | .Ok () =>
    let (result, formatter) ← first.inst.fmt first.value formatter
    match result with
    | .Err error => ok (.Err error, formatter)
    | .Ok () => second.inst.fmt second.value formatter

@[rust_fun "core::fmt::{core::fmt::Formatter<'a>}::debug_tuple_field3_finish"]
def core.fmt.Formatter.debug_tuple_field3_finish
    (formatter : core.fmt.Formatter) (label : Str)
    (first second third : Dyn (fun T => core.fmt.Debug T)) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) := do
  let (result, formatter) ←
    core.fmt.Formatter.debug_tuple_field2_finish formatter label first second
  match result with
  | .Err error => ok (.Err error, formatter)
  | .Ok () => third.inst.fmt third.value formatter

@[rust_fun "core::option::{core::fmt::Debug<core::option::Option<@T>>}::fmt"]
def core.option.Option.Insts.CoreFmtDebug.fmt
    {T : Type} (inst : core.fmt.Debug T) (value : Option T)
    (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  _root_.noble_kernel.core.option.Option.Insts.CoreFmtDebug.fmt inst value formatter

@[rust_fun "core::option::{core::option::Option<@T>}::as_deref"]
def core.option.Option.as_deref {T U : Type} (inst : core.ops.deref.Deref T U)
    (value : Option T) : Result (Option U) := do
  match value with
  | none => ok none
  | some value => return some (← inst.deref value)

@[rust_fun "alloc::boxed::{core::fmt::Debug<Box<@T>>}::fmt"]
def Box.Insts.CoreFmtDebug.fmt {T : Type} (_ : Type)
    (inst : core.fmt.Debug T) (value : T) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  inst.fmt value formatter

@[rust_fun "core::option::{core::clone::Clone<core::option::Option<@T>>}::clone"]
def core.option.Option.Insts.CoreCloneClone.clone
    {T : Type} (inst : core.clone.Clone T) (value : Option T) : Result (Option T) :=
  _root_.noble_kernel.core.option.Option.Insts.CoreCloneClone.clone inst value

@[rust_fun "core::result::{core::result::Result<@T, @E>}::is_err"]
def core.result.Result.is_err
    {T E : Type} (value : core.result.Result T E) : Result Bool :=
  _root_.noble_kernel.core.result.Result.is_err value

/-! ## Slice and vector endpoints -/

@[rust_fun "core::slice::{[@T]}::first"]
def core.slice.Slice.first {T : Type} (slice : Slice T) : Result (Option T) :=
  ok slice.val.head?

/-- A returned mutable borrow updates only the last element. As in Aeneas's
`get_mut` boundary, returning `none` through the backward function leaves the
slice unchanged. An empty slice never yields an element to update. -/
@[rust_fun "core::slice::{[@T]}::last_mut"]
def core.slice.Slice.last_mut {T : Type} (slice : Slice T) :
    Result (Option T × (Option T → Slice T)) :=
  ok (slice.val.getLast?, fun replacement =>
    match replacement with
    | none => slice
    | some value => slice.setAtNat (slice.val.length - 1) value)

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::pop"]
def alloc.vec.Vec.pop {T : Type} (A : Type) (vector : alloc.vec.Vec T) :
    Result (Option T × alloc.vec.Vec T) :=
  _root_.noble_kernel.alloc.vec.Vec.pop A vector

/-! ## Rust UTF-8 diagnostics -/

private def utf8Continuation (byte : U8) : Bool :=
  decide (0x80 ≤ byte.val ∧ byte.val ≤ 0xbf)

private def utf8SecondOfThree (first second : U8) : Bool :=
  decide ((first.val = 0xe0 ∧ 0xa0 ≤ second.val ∧ second.val ≤ 0xbf) ∨
    (0xe1 ≤ first.val ∧ first.val ≤ 0xec ∧ 0x80 ≤ second.val ∧ second.val ≤ 0xbf) ∨
    (first.val = 0xed ∧ 0x80 ≤ second.val ∧ second.val ≤ 0x9f) ∨
    (0xee ≤ first.val ∧ first.val ≤ 0xef ∧ 0x80 ≤ second.val ∧ second.val ≤ 0xbf))

private def utf8SecondOfFour (first second : U8) : Bool :=
  decide ((first.val = 0xf0 ∧ 0x90 ≤ second.val ∧ second.val ≤ 0xbf) ∨
    (0xf1 ≤ first.val ∧ first.val ≤ 0xf3 ∧ 0x80 ≤ second.val ∧ second.val ≤ 0xbf) ∨
    (first.val = 0xf4 ∧ 0x80 ≤ second.val ∧ second.val ≤ 0x8f))

/-- The scalar loop in Rust's `core::str::validations::run_utf8_validation`.
`none` means valid input; `some (offset, none)` is an incomplete trailing scalar;
`some (offset, some n)` is the `n`-byte invalid sequence before the first
non-continuation byte. Overlong encodings, surrogates, and scalars above
U+10FFFF fail at their lead byte with length one. The offset is deliberately
unbounded during traversal and checked once on export, so it cannot wrap. -/
private def utf8ErrorAt (offset : Nat) (bytes : List U8) :
    Option (Nat × Option Usize) :=
  match bytes with
  | [] => none
  | first :: rest =>
    if first.val < 0x80 then
      utf8ErrorAt (offset + 1) rest
    else if 0xc2 ≤ first.val ∧ first.val ≤ 0xdf then
      match rest with
      | [] => some (offset, none)
      | second :: rest =>
        if utf8Continuation second then
          utf8ErrorAt (offset + 2) rest
        else
          some (offset, some 1#usize)
    else if 0xe0 ≤ first.val ∧ first.val ≤ 0xef then
      match rest with
      | [] => some (offset, none)
      | second :: rest =>
        if utf8SecondOfThree first second then
          match rest with
          | [] => some (offset, none)
          | third :: rest =>
            if utf8Continuation third then
              utf8ErrorAt (offset + 3) rest
            else
              some (offset, some 2#usize)
        else
          some (offset, some 1#usize)
    else if 0xf0 ≤ first.val ∧ first.val ≤ 0xf4 then
      match rest with
      | [] => some (offset, none)
      | second :: rest =>
        if utf8SecondOfFour first second then
          match rest with
          | [] => some (offset, none)
          | third :: rest =>
            if utf8Continuation third then
              match rest with
              | [] => some (offset, none)
              | fourth :: rest =>
                if utf8Continuation fourth then
                  utf8ErrorAt (offset + 4) rest
                else
                  some (offset, some 3#usize)
            else
              some (offset, some 2#usize)
        else
          some (offset, some 1#usize)
    else
      some (offset, some 1#usize)
termination_by structural bytes

@[rust_fun "core::str::converts::from_utf8"]
def core.str.converts.from_utf8 (bytes : Slice U8) :
    Result (core.result.Result Str core.str.error.Utf8Error) :=
  match utf8ErrorAt 0 bytes.val with
  | none => ok (.Ok bytes)
  | some (offset, errorLength) => do
    let offset ← UScalar.tryMk .Usize offset
    ok (.Err { offset := offset, error_length := errorLength })

@[rust_fun "core::str::error::{core::str::error::Utf8Error}::valid_up_to"]
def core.str.error.Utf8Error.valid_up_to (error : core.str.error.Utf8Error) :
    Result Usize :=
  ok error.offset

@[rust_fun "core::str::error::{core::str::error::Utf8Error}::error_len"]
def core.str.error.Utf8Error.error_len (error : core.str.error.Utf8Error) :
    Result (Option Usize) :=
  ok error.error_length

/-! ## Owned UTF-8 strings -/

@[rust_fun "core::str::{str}::len"]
def core.str.Str.len (value : Str) : Result Usize :=
  ok (Slice.len value)

@[rust_fun "core::str::traits::{core::cmp::PartialEq<str, str>}::eq"]
def Str.Insts.CoreCmpPartialEqStr.eq (left right : Str) : Result Bool :=
  ok (left.val == right.val)

/-- Only safe, validating Lean UTF-8 decoding is used. `Str` itself omits Rust's
valid-UTF-8 invariant, hence the explicit failure on an invalid representation. -/
private def decodeStr (value : Str) : Result String :=
  match _root_.String.fromUTF8?
      (value.val.map (fun byte => UInt8.ofBitVec byte.bv)).toByteArray with
  | some string => ok string
  | none => fail .undef

@[rust_fun
  "alloc::string::{core::cmp::PartialEq<alloc::string::String, alloc::string::String>}::eq"]
def alloc.string.String.Insts.CoreCmpPartialEqString.eq
    (left right : String) : Result Bool :=
  ok (left == right)

@[rust_fun "alloc::string::{alloc::string::String}::with_capacity"]
def alloc.string.String.with_capacity (capacity : Usize) : Result String :=
  if capacity.val ≤ Isize.max.toNat then ok "" else fail .panic

@[rust_fun "alloc::string::{alloc::string::String}::push_str"]
def alloc.string.String.push_str (value : String) (suffix : Str) : Result String := do
  if value.utf8ByteSize + suffix.val.length ≤ Isize.max.toNat then
    let suffix ← decodeStr suffix
    ok (value ++ suffix)
  else
    fail .panic

@[rust_fun "alloc::string::{alloc::string::String}::push"]
def alloc.string.String.push (value : String) (suffix : Char) : Result String :=
  if value.utf8ByteSize + suffix.utf8Size ≤ Isize.max.toNat then
    ok (value.push suffix)
  else
    fail .panic

@[rust_fun "alloc::string::{alloc::string::String}::as_bytes"]
def alloc.string.String.as_bytes (value : String) : Result (Slice U8) :=
  let bytes := value.toUTF8.toList.map (fun byte => (⟨byte.toBitVec⟩ : U8))
  if h : bytes.length ≤ Usize.max then
    ok (.from bytes h)
  else
    fail .integerOverflow

@[rust_fun "alloc::string::{alloc::string::String}::as_str"]
def alloc.string.String.as_str (value : String) : Result Str :=
  alloc.string.String.as_bytes value

@[rust_fun "alloc::string::{alloc::string::String}::len"]
def alloc.string.String.len (value : String) : Result Usize :=
  UScalar.tryMk .Usize value.utf8ByteSize

@[rust_fun "alloc::string::{core::clone::Clone<alloc::string::String>}::clone"]
def alloc.string.String.Insts.CoreCloneClone.clone (value : String) : Result String :=
  ok value

/-- Debug escaping affects only erased formatter text; there is no user-supplied
formatting callback for a string. This uses the same write boundary as Aeneas. -/
@[rust_fun "alloc::string::{core::fmt::Debug<alloc::string::String>}::fmt"]
def alloc.string.String.Insts.CoreFmtDebug.fmt
    (value : String) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) := do
  let bytes ← alloc.string.String.as_bytes value
  core.fmt.Formatter.write_str formatter bytes

@[rust_fun
  "alloc::string::{core::ops::deref::Deref<alloc::string::String, str>}::deref"]
def alloc.string.String.Insts.CoreOpsDerefDerefStr.deref
    (value : String) : Result Str :=
  alloc.string.String.as_bytes value

@[rust_fun
  "alloc::string::{core::convert::From<alloc::string::String, &'0 str>}::from"]
def alloc.string.String.Insts.CoreConvertFromShared0Str.from
    (value : Str) : Result String :=
  if value.val.length ≤ Isize.max.toNat then decodeStr value else fail .panic

end noble_contracts
