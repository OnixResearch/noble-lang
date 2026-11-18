module
public import Aeneas
public import NobleContractImpl.Types
public import NobleKernel.FunsExternal
@[expose] public section

open Aeneas Aeneas.Std Result ControlFlow Error
open Aeneas.Data.ListN

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

As in Aeneas's `Vec` model, allocation identity, spare capacity, allocator
exhaustion, destructors, and type-layout-dependent allocation limits are not
observable. Reserve retains the capacity-overflow panic when the requested
element count exceeds `usize::MAX`. The erased allocator has no value on which
to invoke `split_off`'s `Clone A`; the generated calls all use the inherited
`CloneGlobal`, whose clone is a pure identity. Thus `split_off` models these
`Global` instantiations, not arbitrary effectful allocator implementations.
String allocation requests
retain Rust's `isize::MAX` capacity-overflow panic. Checked scalar construction
also rejects mathematical Lean strings too large for `usize`, rather than
wrapping their lengths. Invalid `Str` bytes cannot arise from safe Rust;
decoding them returns Aeneas's `undef`, not a replacement-character or
empty-string fallback. Owned UTF-8 failures retain both the original byte
vector and the precise Rust diagnostic.

Shared references are values in Aeneas. Mutable string borrows preserve their
byte length and UTF-8 validity in safe Rust, just as mutable slice borrows
preserve their length. `encode_utf8` uses that same boundary: its backward
function replaces only the borrowed prefix, retaining the untouched suffix.

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

@[rust_fun "core::array::equality::{core::cmp::PartialEq<&'0 [@T], [@U; @N]>}::eq"]
def Shared0Slice.Insts.CoreCmpPartialEqArray.eq
    {T U : Type} {N : Usize} (inst : core.cmp.PartialEq T U)
    (left : Slice T) (right : Array U N) : Result Bool :=
  Slice.Insts.CoreCmpPartialEqArray.eq inst left right

@[rust_fun "core::array::equality::{core::cmp::PartialEq<&'0 [@T], [@U; @N]>}::ne"]
def Shared0Slice.Insts.CoreCmpPartialEqArray.ne
    {T U : Type} {N : Usize} (inst : core.cmp.PartialEq T U)
    (left : Slice T) (right : Array U N) : Result Bool :=
  Slice.Insts.CoreCmpPartialEqArray.ne inst left right

@[rust_fun
  "alloc::vec::partial_eq::{core::cmp::PartialEq<&'0 [@T], alloc::vec::Vec<@U>>}::ne"]
def Shared0Slice.Insts.CoreCmpPartialEqVec.ne
    {T U : Type} (_A : Type) (inst : core.cmp.PartialEq T U)
    (left : Slice T) (right : alloc.vec.Vec U) : Result Bool :=
  core.slice.cmp.PartialEqSlice.ne inst left right.slice

@[rust_fun "core::cmp::impls::{core::cmp::PartialEq<bool, bool>}::ne"]
def Bool.Insts.CoreCmpPartialEqBool.ne (left right : Bool) : Result Bool :=
  _root_.noble_kernel.Bool.Insts.CoreCmpPartialEqBool.ne left right

/-- The audited Rust target is little-endian x86_64. Reinterpretation retains
all 64 bits, including negative two's-complement values; it is not a numeric
conversion through a nonnegative integer. -/
@[rust_fun "core::num::{u64}::to_ne_bytes"]
def core.num.U64.to_ne_bytes (value : U64) : Result (Array U8 8#usize) :=
  ok (_root_.Aeneas.Std.core.num.U64.to_le_bytes value)

@[rust_fun "core::num::{u64}::from_ne_bytes"]
def core.num.U64.from_ne_bytes (bytes : Array U8 8#usize) : Result U64 :=
  ok (_root_.Aeneas.Std.core.num.U64.from_le_bytes bytes)

@[rust_fun "core::num::{i64}::to_ne_bytes"]
def core.num.I64.to_ne_bytes (value : I64) : Result (Array U8 8#usize) :=
  core.num.U64.to_ne_bytes ⟨value.bv⟩

@[rust_fun "core::num::{i64}::from_ne_bytes"]
def core.num.I64.from_ne_bytes (bytes : Array U8 8#usize) : Result I64 := do
  let value ← core.num.U64.from_ne_bytes bytes
  ok ⟨value.bv⟩

@[rust_fun "core::char::convert::{core::convert::From<char, u8>}::from"]
def Char.Insts.CoreConvertFromU8.from (value : U8) : Result Char :=
  ok (_root_.Char.ofUInt8 (UInt8.ofBitVec value.bv))

@[rust_fun "core::char::methods::{char}::from_u32"]
def core.char.methods.Char.from_u32 (value : U32) : Result (Option Char) :=
  let scalar := UInt32.ofBitVec value.bv
  if h : _root_.isValidChar scalar then ok (some ⟨scalar, h⟩) else ok none

@[rust_fun "core::convert::num::{core::convert::From<i64, u8>}::from"]
def I64.Insts.CoreConvertFromU8.from (value : U8) : Result I64 :=
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

@[rust_fun "core::num::{u8}::is_ascii_lowercase"]
def core.num.U8.is_ascii_lowercase (value : U8) : Result Bool :=
  ok (decide (97 ≤ value.val ∧ value.val ≤ 122))

@[rust_fun "core::num::{u8}::is_ascii_alphanumeric"]
def core.num.U8.is_ascii_alphanumeric (value : U8) : Result Bool :=
  ok (decide ((65 ≤ value.val ∧ value.val ≤ 90) ∨
    (97 ≤ value.val ∧ value.val ≤ 122) ∨
    (48 ≤ value.val ∧ value.val ≤ 57)))

@[rust_fun "core::num::{u8}::is_ascii_digit"]
def core.num.U8.is_ascii_digit (value : U8) : Result Bool :=
  ok (decide (48 ≤ value.val ∧ value.val ≤ 57))

@[rust_fun "core::num::{u8}::is_ascii_graphic"]
def core.num.U8.is_ascii_graphic (value : U8) : Result Bool :=
  ok (decide (33 ≤ value.val ∧ value.val ≤ 126))

/-- Rust excludes vertical tab (0x0b) from ASCII whitespace. -/
@[rust_fun "core::num::{u8}::is_ascii_whitespace"]
def core.num.U8.is_ascii_whitespace (value : U8) : Result Bool :=
  ok (decide (value.val = 0x09 ∨ value.val = 0x0a ∨ value.val = 0x0c ∨
    value.val = 0x0d ∨ value.val = 0x20))

/-- The digit loop of Rust's `from_ascii_bytes_radix_impl` at radix 10, on the
magnitude. Each byte is checked as a digit before overflow is reported at that
digit, and an overflowing prefix stops before any later byte is examined.
Rust's two checked steps (`checked_mul` by ten, then `checked_add`, or
`checked_sub` for a negative value) overflow exactly when the new magnitude
exceeds `limit`, the largest magnitude of that sign, and both report the same
kind; the unchecked loop Rust uses for short inputs cannot overflow at all. -/
def decimalDigits (limit : Nat) (overflow : core.num.error.IntErrorKind) :
    List U8 → Nat → core.result.Result Nat core.num.error.ParseIntError
  | [], value => .Ok value
  | digit :: rest, value =>
    if 48 ≤ digit.val ∧ digit.val ≤ 57 then
      let next := value * 10 + (digit.val - 48)
      if next ≤ limit then decimalDigits limit overflow rest next
      else .Err { kind := overflow }
    else
      .Err { kind := .InvalidDigit }

/-- Rust's sign match, after the empty check: a lone `+` or `-` is an invalid
digit; `+` precedes a positive magnitude, and `-` a negative one only for a
signed type (for an unsigned type it stays in the digits, which refuse it).
The result is whether the value is positive, and its digits. -/
def decimalSign (signed : Bool) :
    List U8 → core.result.Result (Bool × List U8) core.num.error.ParseIntError
  | [] => .Err { kind := .Empty }
  | [sign] =>
    if sign.val = 43 ∨ sign.val = 45 then .Err { kind := .InvalidDigit }
    else .Ok (true, [sign])
  | sign :: rest =>
    if sign.val = 43 then .Ok (true, rest)
    else if sign.val = 45 ∧ signed then .Ok (false, rest)
    else .Ok (true, sign :: rest)

/-- Decimal `FromStr` for an unsigned type whose largest value is `max`: no
trimming, radix prefixes, underscores or Unicode digits. -/
def parseUnsignedDecimal (value : Str) (max : Nat) :
    core.result.Result Nat core.num.error.ParseIntError :=
  match decimalSign false value.val with
  | .Err error => .Err error
  | .Ok (_, digits) => decimalDigits max .PosOverflow digits 0

/-- Decimal `FromStr` for a signed type with range `-minimum ..= max`. -/
def parseSignedDecimal (value : Str) (max minimum : Nat) :
    core.result.Result Int core.num.error.ParseIntError :=
  match decimalSign true value.val with
  | .Err error => .Err error
  | .Ok (true, digits) =>
    match decimalDigits max .PosOverflow digits 0 with
    | .Ok magnitude => .Ok (Int.ofNat magnitude)
    | .Err error => .Err error
  | .Ok (false, digits) =>
    match decimalDigits minimum .NegOverflow digits 0 with
    | .Ok magnitude => .Ok (-Int.ofNat magnitude)
    | .Err error => .Err error

/-- Decimal `u32::from_str`: no trimming, radix prefixes, underscores, Unicode
digits, or minus sign; a single leading plus is permitted only before digits. -/
@[rust_fun
  "core::num::{core::str::traits::FromStr<u32, core::num::error::ParseIntError>}::from_str"]
def U32.Insts.CoreStrTraitsFromStrParseIntError.from_str (value : Str) :
    Result (core.result.Result U32 core.num.error.ParseIntError) :=
  match parseUnsignedDecimal value U32.max with
  | .Ok parsed => do ok (.Ok (← UScalar.tryMk .U32 parsed))
  | .Err error => ok (.Err error)

/-- Consume the exact finite word width, not an arbitrary recursion budget. -/
def countLowBits : Nat → Nat → Nat
  | 0, _ => 0
  | width + 1, value => value % 2 + countLowBits width (value / 2)

@[rust_fun "core::num::{u64}::count_ones"]
def core.num.U64.count_ones (value : U64) : Result U32 :=
  UScalar.tryMk .U32 (countLowBits 64 value.val)

/-- Checked shifts check the shift count, not the bits shifted out. -/
@[rust_fun "core::num::{u64}::checked_shl"]
def core.num.U64.checked_shl (value : U64) (shift : U32) : Result (Option U64) :=
  if shift.val < 64 then ok (some ⟨value.bv.shiftLeft shift.val⟩) else ok none

@[rust_fun "core::num::{u32}::saturating_mul"]
def core.num.U32.saturating_mul (left right : U32) : Result U32 :=
  UScalar.tryMk .U32 (min U32.max (left.val * right.val))

@[rust_fun "core::num::{usize}::saturating_mul"]
def core.num.Usize.saturating_mul (left right : Usize) : Result Usize :=
  _root_.noble_kernel.core.num.Usize.saturating_mul left right

@[rust_fun
  "core::ops::range::{core::clone::Clone<core::ops::range::Range<@Idx>>}::clone"]
def core.ops.range.Range.Insts.CoreCloneClone.clone {Idx : Type}
    (inst : core.clone.Clone Idx) (range : core.ops.range.Range Idx) :
    Result (core.ops.range.Range Idx) := do
  let start ← inst.clone range.start
  let stop ← inst.clone range.end
  ok { start := start, «end» := stop }

/-! ## Formatting, option, and result -/

@[rust_fun "core::fmt::{core::fmt::Display<&'0 @T>}::fmt"]
def Shared0T.Insts.CoreFmtDisplay.fmt {T : Type} (inst : core.fmt.Display T)
    (value : T) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  inst.fmt value formatter

@[rust_fun "core::fmt::{core::fmt::Display<str>}::fmt"]
def Str.Insts.CoreFmtDisplay.fmt (value : Str) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  core.fmt.Formatter.write_str formatter value

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

@[rust_fun "core::fmt::{core::fmt::Debug<(@U, @T)>}::fmt"]
def Pair.Insts.CoreFmtDebug.fmt {U T : Type}
    (first : core.fmt.Debug U) (second : core.fmt.Debug T)
    (value : U × T) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) := do
  let (result, formatter) ← first.fmt value.1 formatter
  match result with
  | .Err error => ok (.Err error, formatter)
  | .Ok () => second.fmt value.2 formatter

/-- Preserve Rust's ordered short-circuiting, including custom `ne` methods;
tuple inequality is not implemented by negating a call to `eq`. -/
@[rust_fun "core::tuple::{core::cmp::PartialEq<(@U, @T), (@U, @T)>}::eq"]
def Pair.Insts.CoreCmpPartialEqPair.eq {U T : Type}
    (first : core.cmp.PartialEq U U) (second : core.cmp.PartialEq T T)
    (left right : U × T) : Result Bool := do
  let equal ← first.eq left.1 right.1
  if equal then second.eq left.2 right.2 else ok false

@[rust_fun "core::tuple::{core::cmp::PartialEq<(@U, @T), (@U, @T)>}::ne"]
def Pair.Insts.CoreCmpPartialEqPair.ne {U T : Type}
    (first : core.cmp.PartialEq U U) (second : core.cmp.PartialEq T T)
    (left right : U × T) : Result Bool := do
  let different ← first.ne left.1 right.1
  if different then ok true else second.ne left.2 right.2

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

@[rust_fun "core::option::{core::option::Option<@T>}::as_ref"]
def core.option.Option.as_ref {T : Type} (value : Option T) : Result (Option T) :=
  ok value

/-- A mutable borrow cannot change the `Option` discriminant. If there was
no borrowed payload, or no replacement is returned, preserve the receiver. -/
@[rust_fun "core::option::{core::option::Option<@T>}::as_mut"]
def core.option.Option.as_mut {T : Type} (value : Option T) :
    Result (Option T × (Option T → Option T)) :=
  ok (value, fun replacement =>
    match value, replacement with
    | some _, some updated => some updated
    | _, _ => value)

@[rust_fun "core::option::{core::option::Option<@T>}::ok_or_else"]
def core.option.Option.ok_or_else {T E F : Type}
    (inst : core.ops.function.FnOnce F Unit E)
    (value : Option T) (function : F) : Result (core.result.Result T E) :=
  match value with
  | some payload => ok (.Ok payload)
  | none => do
    let error ← inst.call_once function ()
    ok (.Err error)

@[rust_fun "core::option::{core::option::Option<@T>}::as_deref"]
def core.option.Option.as_deref {T U : Type} (inst : core.ops.deref.Deref T U)
    (value : Option T) : Result (Option U) := do
  match value with
  | none => ok none
  | some value => return some (← inst.deref value)

@[rust_fun "core::option::{core::option::Option<@T>}::and_then"]
def core.option.Option.and_then {T U F : Type}
    (inst : core.ops.function.FnOnce F T (Option U))
    (value : Option T) (function : F) : Result (Option U) :=
  match value with
  | none => ok none
  | some value => inst.call_once function value

@[rust_fun "core::option::{core::option::Option<@T>}::or"]
def core.option.Option.or {T : Type} (value fallback : Option T) : Result (Option T) :=
  match value with
  | none => ok fallback
  | some _ => ok value

@[rust_fun
  "core::option::{core::option::Option<core::option::Option<@T>>}::flatten"]
def core.option.OptionOption.flatten {T : Type} (value : Option (Option T)) :
    Result (Option T) :=
  ok value.join

@[rust_fun "core::option::{core::option::Option<&'0 @T>}::copied"]
def core.option.OptionShared0T.copied {T : Type} (inst : core.marker.Copy T)
    (value : Option T) : Result (Option T) :=
  _root_.noble_kernel.core.option.OptionShared0T.copied inst value

@[rust_fun
  "core::option::{core::cmp::PartialEq<core::option::Option<@T>, core::option::Option<@T>>}::eq"]
def core.option.Option.Insts.CoreCmpPartialEqOption.eq {T : Type _}
    (inst : core.cmp.PartialEq T T) (left right : Option T) : Result Bool :=
  match left, right with
  | none, none => ok true
  | some left, some right => inst.eq left right
  | _, _ => ok false

@[rust_fun "alloc::boxed::{core::fmt::Debug<Box<@T>>}::fmt"]
def Box.Insts.CoreFmtDebug.fmt {T : Type} (_ : Type)
    (inst : core.fmt.Debug T) (value : T) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  inst.fmt value formatter

@[rust_fun "alloc::boxed::{core::convert::AsRef<Box<@T>, @T>}::as_ref"]
def Box.Insts.CoreConvertAsRef.as_ref {T : Type} (_A : Type)
    (value : T) : Result T :=
  ok value

@[rust_fun "core::option::{core::clone::Clone<core::option::Option<@T>>}::clone"]
def core.option.Option.Insts.CoreCloneClone.clone
    {T : Type} (inst : core.clone.Clone T) (value : Option T) : Result (Option T) :=
  _root_.noble_kernel.core.option.Option.Insts.CoreCloneClone.clone inst value

@[rust_fun "core::result::{core::result::Result<@T, @E>}::is_err"]
def core.result.Result.is_err
    {T E : Type} (value : core.result.Result T E) : Result Bool :=
  _root_.noble_kernel.core.result.Result.is_err value

@[rust_fun "core::result::{core::result::Result<@T, @E>}::is_ok_and"]
def core.result.Result.is_ok_and {T E F : Type}
    (inst : core.ops.function.FnOnce F T Bool)
    (value : core.result.Result T E) (predicate : F) : Result Bool :=
  match value with
  | .Ok payload => inst.call_once predicate payload
  | .Err _ => pure false

@[rust_fun "core::result::{core::result::Result<@T, @E>}::ok"]
def core.result.Result.ok
    {T E : Type} (value : core.result.Result T E) : Result (Option T) :=
  _root_.noble_kernel.core.result.Result.ok value

@[rust_fun "core::result::{core::result::Result<@T, @E>}::map"]
def core.result.Result.map {T E U F : Type}
    (inst : core.ops.function.FnOnce F T U)
    (value : core.result.Result T E) (function : F) :
    Result (core.result.Result U E) :=
  match value with
  | .Err error => pure (.Err error)
  | .Ok payload => do
    let mapped ← inst.call_once function payload
    pure (.Ok mapped)

/-- Evaluate `Default` before replacing the destination, preserving failure,
divergence, and any effects of the supplied implementation. -/
@[rust_fun "core::mem::take"]
def core.mem.take {T : Type} (inst : core.default.Default T) (value : T) :
    Result (T × T) := do
  let replacement ← inst.default
  ok (value, replacement)

@[rust_fun "core::hint::must_use"]
def core.hint.must_use {T : Type} (value : T) : Result T :=
  ok value

/-! ## Slice and vector endpoints -/

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::is_empty"]
def alloc.vec.Vec.is_empty {T : Type} (A : Type) (vector : alloc.vec.Vec T) : Result Bool :=
  _root_.noble_kernel.alloc.vec.Vec.is_empty A vector

@[rust_fun "core::slice::{[@T]}::first"]
def core.slice.Slice.first {T : Type} (slice : Slice T) : Result (Option T) :=
  ok slice.val.head?

@[rust_fun "core::slice::{[@T]}::last"]
def core.slice.Slice.last {T : Type} (slice : Slice T) : Result (Option T) :=
  _root_.noble_kernel.core.slice.Slice.last slice

def prefixEq {T : Type} (inst : core.cmp.PartialEq T T) {n m : Nat} :
    ListN T n → ListN T m → Result Bool
  | _, .nil => ok true
  | .nil, .cons _ _ => ok false
  | .cons left lefts, .cons right rights => do
    if ← inst.eq left right then prefixEq inst lefts rights else ok false

@[rust_fun "core::slice::{[@T]}::starts_with"]
def core.slice.Slice.starts_with {T : Type} (inst : core.cmp.PartialEq T T)
    (value needle : Slice T) : Result Bool :=
  prefixEq inst value.list needle.list

/-- A stable insertion order is one of the orders permitted by Rust's
`sort_unstable`: the API does not specify the order of equal keys. Invoke the
supplied `Ord` comparison only until the insertion point and propagate failure.
The length witness prevents a sorted slice from changing storage size. -/
def insertSorted {T : Type} (inst : core.cmp.Ord T) (item : T) :
    (items : List T) → Result { sorted : List T // sorted.length = items.length + 1 }
  | [] => ok ⟨[item], by simp⟩
  | first :: rest => do
    let order ← inst.cmp item first
    if order == .gt then
      let inserted ← insertSorted inst item rest
      ok ⟨first :: inserted.val, by simp [inserted.property]⟩
    else
      ok ⟨item :: first :: rest, by simp⟩

def sortItems {T : Type} (inst : core.cmp.Ord T) {n : Nat} :
    (items : ListN T n) → Result { sorted : List T // sorted.length = n }
  | .nil => ok ⟨[], rfl⟩
  | .cons first rest => do
    let sorted ← sortItems inst rest
    let inserted ← insertSorted inst first sorted.val
    ok ⟨inserted.val, by simp [inserted.property, sorted.property]⟩

@[rust_fun "core::slice::{[@T]}::sort_unstable"]
def core.slice.Slice.sort_unstable {T : Type} (inst : core.cmp.Ord T)
    (slice : Slice T) : Result (Slice T) :=
  if slice.length ≤ 1 then
    ok slice
  else do
    let sorted ← sortItems inst slice.list
    ok (.from sorted.val (by rw [sorted.property]; exact slice.bound))

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

@[rust_fun "alloc::vec::{core::default::Default<alloc::vec::Vec<@T>>}::default"]
def alloc.vec.Vec.Insts.CoreDefaultDefault.default (T : Type) : Result (alloc.vec.Vec T) :=
  ok (_root_.Aeneas.Std.alloc.vec.Vec.new T)

/-- Match the inherited layout-free vector model without wrapping the exact
requested element count. No elements or element order change. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::reserve"]
def alloc.vec.Vec.reserve {T : Type} (_A : Type) (vector : alloc.vec.Vec T)
    (additional : Usize) : Result (alloc.vec.Vec T) :=
  if vector.val.length + additional.val ≤ Usize.max then ok vector else fail .panic

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::truncate"]
def alloc.vec.Vec.truncate {T : Type} (A : Type) (vector : alloc.vec.Vec T)
    (length : Usize) : Result (alloc.vec.Vec T) :=
  _root_.noble_kernel.alloc.vec.Vec.truncate A vector length

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::as_slice"]
def alloc.vec.Vec.as_slice {T : Type} (A : Type) (vector : alloc.vec.Vec T) :
    Result (Slice T) :=
  _root_.noble_kernel.alloc.vec.Vec.as_slice A vector

/-- The `FnMut` callback is threaded left to right, including over elements
which are removed. Its returned closure state affects every later decision. -/
def retainedItems {T F : Type} (inst : core.ops.function.FnMut F T Bool)
    (function : F) {n : Nat} :
    (items : ListN T n) → Result { retained : List T // retained.length ≤ n }
  | .nil => ok ⟨[], by simp⟩
  | .cons item rest => do
    let (keep, function) ← inst.call_mut function item
    let retained ← retainedItems inst function rest
    if keep then
      ok ⟨item :: retained.val, by have := retained.property; simp; omega⟩
    else
      ok ⟨retained.val, by have := retained.property; simp; omega⟩

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::retain"]
def alloc.vec.Vec.retain {T : Type} (_A : Type) {F : Type}
    (inst : core.ops.function.FnMut F T Bool)
    (vector : alloc.vec.Vec T) (function : F) : Result (alloc.vec.Vec T) := do
  let retained ← retainedItems inst function vector.slice.list
  ok (.from retained.val (by
    have h := retained.property
    have bound := vector.slice.bound
    omega))

def generatedItems {T F : Type} (inst : core.ops.function.FnMut F Unit T)
    (function : F) :
    (amount : Nat) → Result { generated : List T // generated.length = amount }
  | 0 => ok ⟨[], rfl⟩
  | amount + 1 => do
    let (item, function) ← inst.call_mut function ()
    let generated ← generatedItems inst function amount
    ok ⟨item :: generated.val, by simp [generated.property]⟩

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::resize_with"]
def alloc.vec.Vec.resize_with {T : Type} (_A : Type) {F : Type}
    (inst : core.ops.function.FnMut F Unit T)
    (vector : alloc.vec.Vec T) (length : Usize) (function : F) :
    Result (alloc.vec.Vec T) :=
  if length.val < vector.val.length then
    _root_.noble_kernel.alloc.vec.Vec.truncate T vector length
  else if length.val = vector.val.length then
    ok vector
  else do
    let generated ← generatedItems inst function (length.val - vector.val.length)
    ok (.from (vector.val ++ generated.val) (by
      have generatedLength := generated.property
      simp only [List.length_append, generatedLength]
      scalar_tac))

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::pop"]
def alloc.vec.Vec.pop {T : Type} (A : Type) (vector : alloc.vec.Vec T) :
    Result (Option T × alloc.vec.Vec T) :=
  _root_.noble_kernel.alloc.vec.Vec.pop A vector

/-- Rust returns the suffix and leaves the prefix in `self`; Aeneas puts the
returned value before the updated receiver. `split_at` checks the index before
constructing either part, including the valid empty boundary splits. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::split_off"]
def alloc.vec.Vec.split_off {T A : Type} (_inst : core.clone.Clone A)
    (vector : alloc.vec.Vec T) (index : Usize) :
    Result (alloc.vec.Vec T × alloc.vec.Vec T) := do
  let (retained, suffix) ← core.slice.Slice.split_at vector.slice index
  ok (⟨suffix⟩, ⟨retained⟩)

/-! ## Rust UTF-8 diagnostics -/

def utf8Continuation (byte : U8) : Bool :=
  decide (0x80 ≤ byte.val ∧ byte.val ≤ 0xbf)

def utf8SecondOfThree (first second : U8) : Bool :=
  decide ((first.val = 0xe0 ∧ 0xa0 ≤ second.val ∧ second.val ≤ 0xbf) ∨
    (0xe1 ≤ first.val ∧ first.val ≤ 0xec ∧ 0x80 ≤ second.val ∧ second.val ≤ 0xbf) ∨
    (first.val = 0xed ∧ 0x80 ≤ second.val ∧ second.val ≤ 0x9f) ∨
    (0xee ≤ first.val ∧ first.val ≤ 0xef ∧ 0x80 ≤ second.val ∧ second.val ≤ 0xbf))

def utf8SecondOfFour (first second : U8) : Bool :=
  decide ((first.val = 0xf0 ∧ 0x90 ≤ second.val ∧ second.val ≤ 0xbf) ∨
    (0xf1 ≤ first.val ∧ first.val ≤ 0xf3 ∧ 0x80 ≤ second.val ∧ second.val ≤ 0xbf) ∨
    (first.val = 0xf4 ∧ 0x80 ≤ second.val ∧ second.val ≤ 0x8f))

/-- The scalar loop in Rust's `core::str::validations::run_utf8_validation`.
`none` means valid input; `some (offset, none)` is an incomplete trailing scalar;
`some (offset, some n)` is the `n`-byte invalid sequence before the first
non-continuation byte. Overlong encodings, surrogates, and scalars above
U+10FFFF fail at their lead byte with length one. The offset is deliberately
unbounded during traversal and checked once on export, so it cannot wrap. -/
def utf8ErrorAt (offset : Nat) (bytes : List U8) :
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

/-! ## UTF-8 byte ranges

Rust indexes `str` by bytes, never by character number. For valid UTF-8, zero,
the byte length, and bytes other than continuation bytes are exactly the
character boundaries. Raw pointer models retain Aeneas's value representation;
an invalid unchecked range is undefined behavior rather than a safe panic.
-/

def startsCharacterAt {n : Nat} (bytes : ListN U8 n) (index : Nat) : Bool :=
  match bytes with
  | .nil => false
  | .cons byte rest =>
    match index with
    | 0 => !utf8Continuation byte
    | index + 1 => startsCharacterAt rest index
termination_by structural bytes

def strBoundary (value : Str) (index : Nat) : Bool :=
  if index = 0 ∨ index = value.length then
    true
  else
    startsCharacterAt value.list index

def validStrRange (range : core.ops.range.Range Usize) (value : Str) : Bool :=
  decide (range.start.val ≤ range.end.val) &&
    strBoundary value range.start.val && strBoundary value range.end.val

/-- As at the inherited mutable-slice boundary, safe callers return a borrow
of the original length. Also retain the UTF-8 invariant of mutable `str`.
Out-of-domain backward inputs cannot resize the borrow or alter its suffix. -/
def replaceStrRange (range : core.ops.range.Range Usize)
    (value replacement : Str) : Str :=
  if replacement.length = range.end.val - range.start.val ∧
      (utf8ErrorAt 0 replacement.val).isNone then
    value.setSlice! range.start.val replacement.val
  else
    value

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::Range<usize>, str, str>}::get"]
def core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get
    (range : core.ops.range.Range Usize) (value : Str) : Result (Option Str) :=
  if validStrRange range value then
    core.slice.index.SliceIndexRangeUsizeSlice.get range value
  else
    ok none

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::Range<usize>, str, str>}::get_mut"]
def core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_mut
    (range : core.ops.range.Range Usize) (value : Str) :
    Result (Option Str × (Option Str → Str)) := do
  let borrowed ← core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get range value
  ok (borrowed, fun replacement =>
    match borrowed, replacement with
    | some _, some replacement => replaceStrRange range value replacement
    | _, _ => value)

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::Range<usize>, str, str>}::index"]
def core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.index
    (range : core.ops.range.Range Usize) (value : Str) : Result Str := do
  let borrowed ← core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get range value
  match borrowed with
  | some borrowed => ok borrowed
  | none => fail .panic

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::Range<usize>, str, str>}::index_mut"]
def core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.index_mut
    (range : core.ops.range.Range Usize) (value : Str) : Result (Str × (Str → Str)) := do
  let borrowed ← core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.index range value
  ok (borrowed, replaceStrRange range value)

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::Range<usize>, str, str>}::get_unchecked"]
def core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_unchecked
    (range : core.ops.range.Range Usize) (pointer : ConstRawPtr Str) :
    Result (ConstRawPtr Str) := do
  let borrowed ← core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get range pointer.v
  match borrowed with
  | some borrowed => ok ⟨borrowed⟩
  | none => fail .undef

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::Range<usize>, str, str>}::get_unchecked_mut"]
def core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_unchecked_mut
    (range : core.ops.range.Range Usize) (pointer : MutRawPtr Str) :
    Result (MutRawPtr Str) := do
  let borrowed ← core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get range pointer.v
  match borrowed with
  | some borrowed => ok ⟨borrowed⟩
  | none => fail .undef

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeFrom<usize>, str, str>}::get"]
def core.ops.range.RangeFromUsize.Insts.CoreSliceIndexSliceIndexStrStr.get
    (range : core.ops.range.RangeFrom Usize) (value : Str) : Result (Option Str) :=
  core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get
    { start := range.start, «end» := Slice.len value } value

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeFrom<usize>, str, str>}::get_mut"]
def core.ops.range.RangeFromUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_mut
    (range : core.ops.range.RangeFrom Usize) (value : Str) :
    Result (Option Str × (Option Str → Str)) :=
  core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_mut
    { start := range.start, «end» := Slice.len value } value

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeFrom<usize>, str, str>}::index"]
def core.ops.range.RangeFromUsize.Insts.CoreSliceIndexSliceIndexStrStr.index
    (range : core.ops.range.RangeFrom Usize) (value : Str) : Result Str :=
  core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.index
    { start := range.start, «end» := Slice.len value } value

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeFrom<usize>, str, str>}::index_mut"]
def core.ops.range.RangeFromUsize.Insts.CoreSliceIndexSliceIndexStrStr.index_mut
    (range : core.ops.range.RangeFrom Usize) (value : Str) : Result (Str × (Str → Str)) :=
  core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.index_mut
    { start := range.start, «end» := Slice.len value } value

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeFrom<usize>, str, str>}::get_unchecked"]
def core.ops.range.RangeFromUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_unchecked
    (range : core.ops.range.RangeFrom Usize) (pointer : ConstRawPtr Str) :
    Result (ConstRawPtr Str) :=
  core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_unchecked
    { start := range.start, «end» := Slice.len pointer.v } pointer

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeFrom<usize>, str, str>}::get_unchecked_mut"]
def core.ops.range.RangeFromUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_unchecked_mut
    (range : core.ops.range.RangeFrom Usize) (pointer : MutRawPtr Str) :
    Result (MutRawPtr Str) :=
  core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_unchecked_mut
    { start := range.start, «end» := Slice.len pointer.v } pointer

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeTo<usize>, str, str>}::get"]
def core.ops.range.RangeToUsize.Insts.CoreSliceIndexSliceIndexStrStr.get
    (range : core.ops.range.RangeTo Usize) (value : Str) : Result (Option Str) :=
  core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get
    { start := 0#usize, «end» := range.end } value

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeTo<usize>, str, str>}::get_mut"]
def core.ops.range.RangeToUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_mut
    (range : core.ops.range.RangeTo Usize) (value : Str) :
    Result (Option Str × (Option Str → Str)) :=
  core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_mut
    { start := 0#usize, «end» := range.end } value

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeTo<usize>, str, str>}::index"]
def core.ops.range.RangeToUsize.Insts.CoreSliceIndexSliceIndexStrStr.index
    (range : core.ops.range.RangeTo Usize) (value : Str) : Result Str :=
  core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.index
    { start := 0#usize, «end» := range.end } value

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeTo<usize>, str, str>}::index_mut"]
def core.ops.range.RangeToUsize.Insts.CoreSliceIndexSliceIndexStrStr.index_mut
    (range : core.ops.range.RangeTo Usize) (value : Str) : Result (Str × (Str → Str)) :=
  core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.index_mut
    { start := 0#usize, «end» := range.end } value

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeTo<usize>, str, str>}::get_unchecked"]
def core.ops.range.RangeToUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_unchecked
    (range : core.ops.range.RangeTo Usize) (pointer : ConstRawPtr Str) :
    Result (ConstRawPtr Str) :=
  core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_unchecked
    { start := 0#usize, «end» := range.end } pointer

@[rust_fun
  "core::str::traits::{core::slice::index::SliceIndex<core::ops::range::RangeTo<usize>, str, str>}::get_unchecked_mut"]
def core.ops.range.RangeToUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_unchecked_mut
    (range : core.ops.range.RangeTo Usize) (pointer : MutRawPtr Str) :
    Result (MutRawPtr Str) :=
  core.ops.range.RangeUsize.Insts.CoreSliceIndexSliceIndexStrStr.get_unchecked_mut
    { start := 0#usize, «end» := range.end } pointer

/-! ## Owned UTF-8 strings -/

@[rust_fun "core::str::{str}::len"]
def core.str.Str.len (value : Str) : Result Usize :=
  ok (Slice.len value)

@[rust_fun "core::str::{str}::is_empty"]
def core.str.Str.is_empty (value : Str) : Result Bool :=
  ok (value.length == 0)

@[rust_fun "core::str::{str}::as_bytes"]
def core.str.Str.as_bytes (value : Str) : Result (Slice U8) :=
  ok value

@[rust_fun "core::str::{str}::get"]
def core.str.Str.get {I Output : Type}
    (inst : core.slice.index.SliceIndex I Str Output) (value : Str) (index : I) :
    Result (Option Output) :=
  inst.get index value

@[rust_fun
  "core::str::traits::{core::ops::index::Index<str, @I, @Clause0_Output>}::index"]
def Str.Insts.CoreOpsIndexIndex.index {I Output : Type}
    (inst : core.slice.index.SliceIndex I Str Output)
    (value : Str) (index : I) : Result Output :=
  inst.index index value

@[rust_fun "core::str::{str}::parse"]
def core.str.Str.parse {F E : Type} (inst : core.str.traits.FromStr F E) (value : Str) :
    Result (core.result.Result F E) :=
  inst.from_str value

@[rust_fun "core::str::traits::{core::cmp::PartialEq<str, str>}::eq"]
def Str.Insts.CoreCmpPartialEqStr.eq (left right : Str) : Result Bool :=
  ok (left.val == right.val)

/-- Only safe, validating Lean UTF-8 decoding is used. `Str` itself omits Rust's
valid-UTF-8 invariant, hence the explicit failure on an invalid representation. -/
def decodeStr (value : Str) : Result String :=
  match _root_.String.fromUTF8?
      (value.val.map (fun byte => UInt8.ofBitVec byte.bv)).toByteArray with
  | some string => ok string
  | none => fail .undef

@[rust_fun "alloc::string::{alloc::string::String}::new"]
def alloc.string.String.new : Result String :=
  ok ""

@[rust_fun "alloc::string::{alloc::string::String}::from_utf8"]
def alloc.string.String.from_utf8 (bytes : alloc.vec.Vec U8) :
    Result (core.result.Result String alloc.string.FromUtf8Error) := do
  let validated ← core.str.converts.from_utf8 bytes.slice
  match validated with
  | .Ok string => return .Ok (← decodeStr string)
  | .Err error => ok (.Err { bytes := bytes, error := error })

@[rust_fun
  "alloc::string::{core::cmp::PartialEq<alloc::string::String, alloc::string::String>}::eq"]
def alloc.string.String.Insts.CoreCmpPartialEqString.eq
    (left right : String) : Result Bool :=
  ok (left == right)

@[rust_fun "alloc::string::{alloc::string::String}::is_empty"]
def alloc.string.String.is_empty (value : String) : Result Bool :=
  ok value.isEmpty

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

/-- Compare existing UTF-8 storage directly, without allocating a second
bounded slice merely to compare an owned and a borrowed string. -/
def stringBytesEq (owned : ByteArray) {n : Nat} (borrowed : ListN U8 n)
    (index : Nat) : Bool :=
  match borrowed with
  | .nil => index == owned.size
  | .cons byte rest =>
    if h : index < owned.size then
      byte.bv == owned[index].toBitVec && stringBytesEq owned rest (index + 1)
    else
      false
termination_by structural borrowed

@[rust_fun
  "alloc::string::{core::cmp::PartialEq<alloc::string::String, &'0 str>}::eq"]
def alloc.string.String.Insts.CoreCmpPartialEqShared0Str.eq
    (left : String) (right : Str) : Result Bool :=
  if left.utf8ByteSize ≤ Usize.max then
    ok (left.utf8ByteSize == right.length && stringBytesEq left.toUTF8 right.list 0)
  else
    fail .integerOverflow

@[rust_fun
  "alloc::string::{core::cmp::PartialEq<alloc::string::String, str>}::eq"]
def alloc.string.String.Insts.CoreCmpPartialEqStr.eq
    (left : String) (right : Str) : Result Bool :=
  alloc.string.String.Insts.CoreCmpPartialEqShared0Str.eq left right

@[rust_fun
  "alloc::string::{core::cmp::PartialEq<alloc::string::String, str>}::ne"]
def alloc.string.String.Insts.CoreCmpPartialEqStr.ne
    (left : String) (right : Str) : Result Bool := do
  let equal ← alloc.string.String.Insts.CoreCmpPartialEqStr.eq left right
  ok (!equal)

@[rust_fun
  "alloc::string::{core::cmp::PartialEq<&'0 str, alloc::string::String>}::eq"]
def Shared0Str.Insts.CoreCmpPartialEqString.eq
    (left : Str) (right : String) : Result Bool :=
  alloc.string.String.Insts.CoreCmpPartialEqShared0Str.eq right left

@[rust_fun "alloc::string::{core::fmt::Display<alloc::string::String>}::fmt"]
def alloc.string.String.Insts.CoreFmtDisplay.fmt
    (value : String) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) := do
  let bytes ← alloc.string.String.as_bytes value
  Str.Insts.CoreFmtDisplay.fmt bytes formatter

/-- Encode exactly one Unicode scalar into the borrowed prefix. A short buffer
panics before yielding a borrow. The backward function starts with the encoded
buffer, then copies back only the borrowed number of bytes; even an unreachable
overlong replacement cannot overwrite the original suffix. As in Aeneas's
slice backward functions, safe callers return a borrow of the original length
(and, for `str`, valid UTF-8). -/
@[rust_fun "core::char::methods::{char}::encode_utf8"]
def core.char.methods.Char.encode_utf8 (value : Char) (buffer : Slice U8) :
    Result (Str × (Str → Slice U8)) := do
  let encoded ← alloc.string.String.as_bytes (_root_.String.singleton value)
  if encoded.val.length ≤ buffer.val.length then
    let updated := buffer.setSlice! 0 encoded.val
    ok (encoded, fun replacement =>
      updated.setSlice! 0 (replacement.val.take encoded.val.length))
  else
    fail .panic

@[rust_fun "alloc::string::{alloc::string::String}::as_str"]
def alloc.string.String.as_str (value : String) : Result Str :=
  alloc.string.String.as_bytes value

@[rust_fun
  "alloc::string::{core::ops::index::Index<alloc::string::String, @I, @Clause0_Output>}::index"]
def alloc.string.String.Insts.CoreOpsIndexIndex.index {I Output : Type}
    (inst : core.slice.index.SliceIndex I Str Output)
    (value : String) (index : I) : Result Output := do
  let borrowed ← alloc.string.String.as_str value
  inst.index index borrowed

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

/-! ## Models of the regenerated subject (#97)

The standard-library items the regenerated subject calls besides the ones
above. Bodies follow the pinned Rust library (1.100.0-nightly 2026-09-17).
Where the inherited kernel already models the same Rust item over the same
Lean types, the model delegates to it by name. -/

/-! ### Delegations to the identical inherited kernel models -/

@[rust_fun
  "core::array::iter::{core::iter::traits::collect::IntoIterator<[@T; @N], @T, core::array::iter::IntoIter<@T, @N>>}::into_iter"]
def Array.Insts.CoreIterTraitsCollectIntoIteratorTIntoIter.into_iter {T : Type} {N : Usize}
    (array : Array T N) : Result (core.array.iter.IntoIter T N) :=
  _root_.noble_kernel.Array.Insts.CoreIterTraitsCollectIntoIteratorTIntoIter.into_iter array

@[rust_fun
  "core::array::iter::{core::iter::traits::iterator::Iterator<core::array::iter::IntoIter<@T, @N>, @T>}::next"]
def core.array.iter.IntoIter.Insts.CoreIterTraitsIteratorIterator.next {T : Type} {N : Usize}
    (iter : core.array.iter.IntoIter T N) : Result (Option T × core.array.iter.IntoIter T N) :=
  _root_.noble_kernel.core.array.iter.IntoIter.Insts.CoreIterTraitsIteratorIterator.next iter

@[rust_fun "core::bool::{bool}::then_some"]
def core.bool.Bool.then_some {T : Type} (condition : Bool) (value : T) : Result (Option T) :=
  _root_.noble_kernel.core.bool.Bool.then_some condition value

@[rust_fun
  "core::convert::num::{core::convert::TryFrom<u32, u64, core::num::error::TryFromIntError>}::try_from"]
def U32.Insts.CoreConvertTryFromU64TryFromIntError.try_from (value : U64) :
    Result (core.result.Result U32 core.num.error.TryFromIntError) :=
  _root_.noble_kernel.U32.Insts.CoreConvertTryFromU64TryFromIntError.try_from value

@[rust_fun "core::fmt::{core::fmt::Debug<str>}::fmt"]
def Str.Insts.CoreFmtDebug.fmt (value : Str) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  _root_.noble_kernel.Str.Insts.CoreFmtDebug.fmt value formatter

/-- Fails, as the inherited model does: the layout size is not determined by
the Rust source (see `NobleKernel.FunsExternal`). -/
@[rust_fun "core::mem::size_of"]
def core.mem.size_of (T : Type) : Result Usize :=
  _root_.noble_kernel.core.mem.size_of T

@[rust_fun "core::option::{core::option::Option<@T>}::is_none_or"]
def core.option.Option.is_none_or {T F : Type} (inst : core.ops.function.FnOnce F T Bool)
    (value : Option T) (predicate : F) : Result Bool :=
  _root_.noble_kernel.core.option.Option.is_none_or inst value predicate

@[rust_fun "core::option::{core::option::Option<@T>}::zip"]
def core.option.Option.zip {T U : Type} (left : Option T) (right : Option U) :
    Result (Option (T × U)) :=
  _root_.noble_kernel.core.option.Option.zip left right

@[rust_fun
  "core::option::{core::default::Default<core::option::Option<@T>>}::default"]
def core.option.Option.Insts.CoreDefaultDefault.default (T : Type) : Result (Option T) :=
  _root_.noble_kernel.core.option.Option.Insts.CoreDefaultDefault.default T

@[rust_fun
  "core::option::{core::ops::try_trait::Try<core::option::Option<@T>>}::branch"]
def core.option.Option.Insts.CoreOpsTry_traitTry.branch {T : Type} (value : Option T) :
    Result (core.ops.control_flow.ControlFlow (Option Never) T) :=
  _root_.noble_kernel.core.option.Option.Insts.CoreOpsTry_traitTry.branch value

@[rust_fun
  "core::option::{core::ops::try_trait::FromResidual<core::option::Option<@T>, core::option::Option<!>>}::from_residual"]
def core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionNever.from_residual (T : Type)
    (residual : Option Never) : Result (Option T) :=
  _root_.noble_kernel.core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionNever.from_residual
    T residual

@[rust_fun
  "core::result::{core::cmp::PartialEq<core::result::Result<@T, @E>, core::result::Result<@T, @E>>}::eq"]
def core.result.Result.Insts.CoreCmpPartialEqResult.eq {T E : Type}
    (valueEq : core.cmp.PartialEq T T) (errorEq : core.cmp.PartialEq E E)
    (left right : core.result.Result T E) : Result Bool :=
  _root_.noble_kernel.core.result.Result.Insts.CoreCmpPartialEqResult.eq
    valueEq errorEq left right

@[rust_fun "core::result::{core::result::Result<@T, @E>}::map_or"]
def core.result.Result.map_or {T E U F : Type} (inst : core.ops.function.FnOnce F T U)
    (value : core.result.Result T E) (fallback : U) (function : F) : Result U :=
  _root_.noble_kernel.core.result.Result.map_or inst value fallback function

@[rust_fun "alloc::boxed::{core::cmp::PartialEq<Box<@T>, Box<@T>>}::ne"]
def Box.Insts.CoreCmpPartialEqBox.ne {T : Type} (A : Type) (inst : core.cmp.PartialEq T T)
    (left right : T) : Result Bool :=
  _root_.noble_kernel.Box.Insts.CoreCmpPartialEqBox.ne A inst left right

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::remove"]
def alloc.vec.Vec.remove {T : Type} (A : Type) (vector : alloc.vec.Vec T) (index : Usize) :
    Result (T × alloc.vec.Vec T) :=
  _root_.noble_kernel.alloc.vec.Vec.remove A vector index

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::clear"]
def alloc.vec.Vec.clear {T : Type} (A : Type) (vector : alloc.vec.Vec T) :
    Result (alloc.vec.Vec T) :=
  _root_.noble_kernel.alloc.vec.Vec.clear A vector

@[rust_fun
  "alloc::vec::{core::iter::traits::collect::Extend<alloc::vec::Vec<@T>, @T>}::extend"]
def alloc.vec.Vec.Insts.CoreIterTraitsCollectExtend.extend {T : Type} (A : Type) {I : Type}
    {Clause0_IntoIter : Type}
    (inst : core.iter.traits.collect.IntoIterator I T Clause0_IntoIter)
    (vector : alloc.vec.Vec T) (items : I) : Result (alloc.vec.Vec T) :=
  _root_.noble_kernel.alloc.vec.Vec.Insts.CoreIterTraitsCollectExtend.extend A inst vector items

/-! ### `f32`

`F32` is Lean's IEEE 754 binary32 `Float32` (see `TypesExternal.lean`). Rust's
`f32` arithmetic never panics: overflow and invalid operations produce
infinities and NaN, as `Float32` does. -/

@[rust_fun "core::clone::impls::{core::clone::Clone<f32>}::clone"]
def F32.Insts.CoreCloneClone.clone (value : F32) : Result F32 :=
  ok value

/-- IEEE equality (`NaN ≠ NaN`, `0.0 = -0.0`), Rust's `f32 == f32`. -/
@[rust_fun "core::cmp::impls::{core::cmp::PartialEq<f32, f32>}::eq"]
def F32.Insts.CoreCmpPartialEqF32.eq (left right : F32) : Result Bool :=
  ok (Float32.beq left right)

@[rust_fun "core::cmp::impls::{core::cmp::PartialEq<f32, f32>}::ne"]
def F32.Insts.CoreCmpPartialEqF32.ne (left right : F32) : Result Bool :=
  ok (!Float32.beq left right)

/-- Rust's float `partial_cmp`: `match (self <= other, self >= other)`, so a
NaN operand compares as `None`. -/
@[rust_fun "core::cmp::impls::{core::cmp::PartialOrd<f32, f32>}::partial_cmp"]
def F32.Insts.CoreCmpPartialOrdF32.partial_cmp (left right : F32) : Result (Option Ordering) :=
  ok (match Float32.le left right, Float32.le right left with
    | false, false => none
    | false, true => some .gt
    | true, false => some .lt
    | true, true => some .eq)

/-- `u8 as f32` is exact. -/
@[rust_fun "core::convert::num::{core::convert::From<f32, u8>}::from"]
def F32.Insts.CoreConvertFromU8.from (value : U8) : Result F32 :=
  ok (UInt8.toFloat32 (UInt8.ofBitVec value.bv))

/-- `u16 as f32` is exact. -/
@[rust_fun "core::convert::num::{core::convert::From<f32, u16>}::from"]
def F32.Insts.CoreConvertFromU16.from (value : U16) : Result F32 :=
  ok (UInt16.toFloat32 (UInt16.ofBitVec value.bv))

/-- Rust converts through `bool as u8`: `1.0` or `0.0`. -/
@[rust_fun "core::convert::num::{core::convert::From<f32, bool>}::from"]
def F32.Insts.CoreConvertFromBool.from (value : Bool) : Result F32 :=
  ok (UInt8.toFloat32 (if value then 1 else 0))

/-- The digits are formatter text, which the Aeneas formatter erases; float
formatting itself cannot fail, so this is a successful step that preserves
the formatter, as for every other leaf `Debug` model. -/
@[rust_fun "core::fmt::float::{core::fmt::Debug<f32>}::fmt"]
def F32.Insts.CoreFmtDebug.fmt (_ : F32) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  ok (.Ok (), formatter)

@[rust_fun "core::f32::{f32}::is_finite"]
def core.f32.F32.is_finite (value : F32) : Result Bool :=
  ok (Float32.isFinite value)

@[rust_fun "core::ops::arith::{core::ops::arith::Add<f32, f32, f32>}::add"]
def F32.Insts.CoreOpsArithAddF32F32.add (left right : F32) : Result F32 :=
  ok (Float32.add left right)

@[rust_fun "core::ops::arith::{core::ops::arith::Mul<f32, f32, f32>}::mul"]
def F32.Insts.CoreOpsArithMulF32F32.mul (left right : F32) : Result F32 :=
  ok (Float32.mul left right)

@[rust_fun "core::ops::arith::{core::ops::arith::Div<f32, f32, f32>}::div"]
def F32.Insts.CoreOpsArithDivF32F32.div (left right : F32) : Result F32 :=
  ok (Float32.div left right)

/-! ### Integers, characters, booleans and orderings -/

@[rust_fun "core::cmp::impls::{core::cmp::Ord<bool>}::cmp"]
def Bool.Insts.CoreCmpOrd.cmp (left right : Bool) : Result Ordering :=
  ok (match left, right with
    | false, true => .lt
    | true, false => .gt
    | _, _ => .eq)

@[rust_fun
  "core::cmp::{core::cmp::PartialEq<core::cmp::Ordering, core::cmp::Ordering>}::eq"]
def core.cmp.Ordering.Insts.CoreCmpPartialEqOrdering.eq (left right : Ordering) :
    Result Bool :=
  ok (decide (left = right))

/-- The closure runs only when the receiver is `Equal`. -/
@[rust_fun "core::cmp::{core::cmp::Ordering}::then_with"]
def core.cmp.Ordering.then_with {F : Type} (inst : core.ops.function.FnOnce F Unit Ordering)
    (order : Ordering) (function : F) : Result Ordering :=
  match order with
  | .eq => inst.call_once function ()
  | _ => ok order

@[rust_fun "core::char::methods::{char}::len_utf8"]
def core.char.methods.Char.len_utf8 (value : Char) : Result Usize :=
  UScalar.tryMk .Usize value.utf8Size

@[rust_fun
  "core::convert::{core::convert::TryFrom<@T, @U, !>}::try_from"]
def core.convert.TryFromTUNever.Blanket.try_from {T U : Type} (inst : core.convert.Into U T)
    (value : U) : Result (core.result.Result T Never) := do
  ok (.Ok (← inst.into value))

@[rust_fun
  "core::convert::num::ptr_try_from_impls::{core::convert::TryFrom<u16, usize, core::num::error::TryFromIntError>}::try_from"]
def U16.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from (value : Usize) :
    Result (core.result.Result U16 core.num.error.TryFromIntError) :=
  core.num.tryFromUScalar .U16 value

@[rust_fun
  "core::convert::num::{core::convert::TryFrom<u8, u64, core::num::error::TryFromIntError>}::try_from"]
def U8.Insts.CoreConvertTryFromU64TryFromIntError.try_from (value : U64) :
    Result (core.result.Result U8 core.num.error.TryFromIntError) :=
  core.num.tryFromUScalar .U8 value

@[rust_fun
  "core::convert::num::ptr_try_from_impls::{core::convert::TryFrom<u8, usize, core::num::error::TryFromIntError>}::try_from"]
def U8.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from (value : Usize) :
    Result (core.result.Result U8 core.num.error.TryFromIntError) :=
  core.num.tryFromUScalar .U8 value

@[rust_fun
  "core::convert::num::{core::convert::TryFrom<u8, u32, core::num::error::TryFromIntError>}::try_from"]
def U8.Insts.CoreConvertTryFromU32TryFromIntError.try_from (value : U32) :
    Result (core.result.Result U8 core.num.error.TryFromIntError) :=
  core.num.tryFromUScalar .U8 value

@[rust_fun
  "core::convert::num::{core::convert::TryFrom<u8, u16, core::num::error::TryFromIntError>}::try_from"]
def U8.Insts.CoreConvertTryFromU16TryFromIntError.try_from (value : U16) :
    Result (core.result.Result U8 core.num.error.TryFromIntError) :=
  core.num.tryFromUScalar .U8 value

deriving instance DecidableEq for core.num.error.IntErrorKind

@[rust_fun
  "core::num::error::{core::cmp::PartialEq<core::num::error::ParseIntError, core::num::error::ParseIntError>}::eq"]
def core.num.error.ParseIntError.Insts.CoreCmpPartialEqParseIntError.eq
    (left right : core.num.error.ParseIntError) : Result Bool :=
  ok (decide (left.kind = right.kind))

@[rust_fun "core::num::{u8}::is_ascii"]
def core.num.U8.is_ascii (value : U8) : Result Bool :=
  ok (decide (value.val < 128))

@[rust_fun
  "core::num::{core::str::traits::FromStr<i64, core::num::error::ParseIntError>}::from_str"]
def I64.Insts.CoreStrTraitsFromStrParseIntError.from_str (value : Str) :
    Result (core.result.Result I64 core.num.error.ParseIntError) :=
  match parseSignedDecimal value I64.max.toNat (-I64.min).toNat with
  | .Ok parsed => do ok (.Ok (← IScalar.tryMk .I64 parsed))
  | .Err error => ok (.Err error)

@[rust_fun
  "core::num::{core::str::traits::FromStr<usize, core::num::error::ParseIntError>}::from_str"]
def Usize.Insts.CoreStrTraitsFromStrParseIntError.from_str (value : Str) :
    Result (core.result.Result Usize core.num.error.ParseIntError) :=
  match parseUnsignedDecimal value Usize.max with
  | .Ok parsed => do ok (.Ok (← UScalar.tryMk .Usize parsed))
  | .Err error => ok (.Err error)

/-- `⌊log₁₀ value⌋` for a positive value; `fuel` bounds the divisions, and
`value` itself is always enough fuel. -/
def floorLog10 : Nat → Nat → Nat
  | 0, _ => 0
  | fuel + 1, value => if value < 10 then 0 else floorLog10 fuel (value / 10) + 1

/-- `usize::ilog10` panics on zero (`panic_for_nonpositive_argument`). -/
@[rust_fun "core::num::{usize}::ilog10"]
def core.num.Usize.ilog10 (value : Usize) : Result U32 :=
  if value.val = 0 then fail .panic
  else UScalar.tryMk .U32 (floorLog10 value.val value.val)

@[rust_fun "core::num::{u64}::saturating_mul"]
def core.num.U64.saturating_mul (left right : U64) : Result U64 :=
  UScalar.tryMk .U64 (min U64.max (left.val * right.val))

/-- Two's-complement negation modulo `2^64`. -/
@[rust_fun "core::num::{u64}::wrapping_neg"]
def core.num.U64.wrapping_neg (value : U64) : Result U64 :=
  ok ⟨-value.bv⟩

/-! ### Ranges -/

/-- `RangeBounds::contains` for `start..end`: `start <= item && item < end`,
short-circuiting, through the two mixed `PartialOrd` instances. -/
@[rust_fun "core::ops::range::{core::ops::range::Range<@Idx>}::contains"]
def core.ops.range.Range.contains {Idx U : Type} (_ : core.cmp.PartialOrd Idx Idx)
    (idxItem : core.cmp.PartialOrd Idx U) (itemIdx : core.cmp.PartialOrd U Idx)
    (range : core.ops.range.Range Idx) (item : U) : Result Bool := do
  if ← idxItem.le range.start item then itemIdx.lt item range.end else ok false

/-- `RangeBounds::contains` for `start..=end`. An exhausted range reports the
end bound `Excluded(start)`, so it contains nothing. -/
@[rust_fun
  "core::ops::range::{core::ops::range::RangeInclusive<@Idx>}::contains"]
def core.ops.range.RangeInclusive.contains {Idx U : Type} (_ : core.cmp.PartialOrd Idx Idx)
    (idxItem : core.cmp.PartialOrd Idx U) (itemIdx : core.cmp.PartialOrd U Idx)
    (range : core.ops.range.RangeInclusive Idx) (item : U) : Result Bool := do
  if ← idxItem.le range.start item then
    if range.exhausted then itemIdx.lt item range.start else itemIdx.le item range.end
  else ok false

@[rust_fun
  "core::ops::range::{core::ops::range::RangeBounds<core::ops::range::RangeToInclusive<@T>, @T>}::end_bound"]
def core.ops.range.RangeToInclusive.Insts.CoreOpsRangeRangeBounds.end_bound {T : Type}
    (range : core.ops.range.RangeToInclusive T) : Result (core.ops.range.Bound T) :=
  ok (.Included range.end)

@[rust_fun
  "core::ops::range::{core::ops::range::RangeBounds<core::ops::range::RangeToInclusive<@T>, @T>}::start_bound"]
def core.ops.range.RangeToInclusive.Insts.CoreOpsRangeRangeBounds.start_bound {T : Type}
    (_ : core.ops.range.RangeToInclusive T) : Result (core.ops.range.Bound T) :=
  ok .Unbounded

/-! ### Options, results, borrows and boxes -/

/-- Rust: `(Some(to), Some(from)) => to.clone_from(from)`, otherwise
`*to = from.clone()`. -/
@[rust_fun
  "core::option::{core::clone::Clone<core::option::Option<@T>>}::clone_from"]
def core.option.Option.Insts.CoreCloneClone.clone_from {T : Type} (inst : core.clone.Clone T)
    (target source : Option T) : Result (Option T) :=
  match target, source with
  | some target, some source => do ok (some (← inst.clone_from target source))
  | _, _ => core.option.Option.Insts.CoreCloneClone.clone inst source

@[rust_fun "core::option::Option::Some"]
def core.option.Option.Some.constructor {T : Type} (value : T) : Result (Option T) :=
  ok (some value)

/-- `FnOnce` of the `Option::Some` constructor item, the only value of its
zero-sized type (the generated instance passes
`core.option.Option.Some.constructor`). The template's signature misprints the
function argument without parentheses; the generated instance requires
`(T → Result (Option T)) → T → Result (Option T)`. -/
@[rust_fun
  "core::option::Option::{core::ops::function::FnOnce<@, (@T), core::option::Option<@T>>}::call_once"]
def P.Insts.CoreOpsFunctionFnOnceTupleTOption.call_once {T : Type}
    (_ : T → Result (Option T)) (value : T) : Result (Option T) :=
  ok (some value)

@[rust_fun "core::option::{core::option::Option<@T>}::map_or"]
def core.option.Option.map_or {T U F : Type} (inst : core.ops.function.FnOnce F T U)
    (value : Option T) (fallback : U) (function : F) : Result U :=
  match value with
  | some payload => inst.call_once function payload
  | none => ok fallback

@[rust_fun "core::option::{core::option::Option<@T>}::filter"]
def core.option.Option.filter {T P : Type} (inst : core.ops.function.FnOnce P T Bool)
    (value : Option T) (predicate : P) : Result (Option T) :=
  match value with
  | some payload => do
    if ← inst.call_once predicate payload then ok (some payload) else ok none
  | none => ok none

@[rust_fun "core::option::{core::option::Option<&'0 @T>}::cloned"]
def core.option.OptionShared0T.cloned {T : Type} (inst : core.clone.Clone T)
    (value : Option T) : Result (Option T) :=
  match value with
  | some payload => do ok (some (← inst.clone payload))
  | none => ok none

@[rust_fun
  "core::option::{core::option::Option<core::result::Result<@T, @E>>}::transpose"]
def core.option.OptionResult.transpose {T E : Type} (value : Option (core.result.Result T E)) :
    Result (core.result.Result (Option T) E) :=
  match value with
  | none => ok (.Ok none)
  | some (.Ok payload) => ok (.Ok (some payload))
  | some (.Err error) => ok (.Err error)

@[rust_fun "core::result::{core::result::Result<@T, @E>}::and_then"]
def core.result.Result.and_then {T E U F : Type}
    (inst : core.ops.function.FnOnce F T (core.result.Result U E))
    (value : core.result.Result T E) (function : F) : Result (core.result.Result U E) :=
  match value with
  | .Ok payload => inst.call_once function payload
  | .Err error => pure (.Err error)

@[rust_fun "core::borrow::{core::borrow::Borrow<@T, @T>}::borrow"]
def core.borrow.Borrow.Blanket.borrow {T : Type} (value : T) : Result T :=
  ok value

/-- `impl<T: Clone> ToOwned for T`: `self.clone()`. -/
@[rust_fun "alloc::borrow::{alloc::borrow::ToOwned<@T, @T>}::to_owned"]
def alloc.borrow.ToOwned.Blanket.to_owned {T : Type} (inst : core.clone.Clone T) (value : T) :
    Result T :=
  inst.clone value

/-- `(**self).clone_from(&**source)`; a box is its value in Aeneas. -/
@[rust_fun "alloc::boxed::{core::clone::Clone<Box<@T>>}::clone_from"]
def Box.Insts.CoreCloneClone.clone_from {T A : Type} (inst : core.clone.Clone T)
    (_ : core.clone.Clone A) (target source : T) : Result T :=
  inst.clone_from target source

/-! ### Cells, shared pointers and sets -/

@[rust_fun "core::cell::{core::cell::Cell<@T>}::new"]
def core.cell.Cell.new {T : Type} (value : T) : Result (core.cell.Cell T) :=
  ok ⟨value⟩

/-- A `Copy` read of the current value. -/
@[rust_fun "core::cell::{core::cell::Cell<@T>}::get"]
def core.cell.Cell.get {T : Type} (_ : core.marker.Copy T) (cell : core.cell.Cell T) : Result T :=
  ok cell.value

/-- `Cell::set` writes through a shared reference. Aeneas reads shared
references as values, so the generated signature returns no updated cell and
the write cannot be carried to the other holders of the cell (the only use is
the shared `Rc<Cell<u32>>` request-work pool, debited by `Meter::charge`).
No value-returning model is faithful: returning `ok ()` would drop the debit
and let later reads see unspent work. As with `core::mem::size_of`, the
model fails, so no run that writes a cell yields an `ok` result, and every
run that does not write one reads the cells' Rust values exactly. -/
@[rust_fun "core::cell::{core::cell::Cell<@T>}::set"]
def core.cell.Cell.set {T : Type} (_ : core.cell.Cell T) (_ : T) : Result Unit :=
  fail .undef

/-- `debug_struct("Cell").field("value", &self.get()).finish()`: the value's
`Debug` runs; text and layout are formatter state the abstraction erases. -/
@[rust_fun "core::fmt::{core::fmt::Debug<core::cell::Cell<@T>>}::fmt"]
def core.cell.Cell.Insts.CoreFmtDebug.fmt {T : Type} (_ : core.marker.Copy T)
    (inst : core.fmt.Debug T) (cell : core.cell.Cell T) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  inst.fmt cell.value formatter

@[rust_fun "alloc::rc::{alloc::rc::Rc<@T>}::new"]
def alloc.rc.Rc.new {T : Type} (value : T) : Result (alloc.rc.Rc T) :=
  ok ⟨value⟩

@[rust_fun "alloc::rc::{core::ops::deref::Deref<alloc::rc::Rc<@T>, @T>}::deref"]
def alloc.rc.Rc.Insts.CoreOpsDerefDeref.deref {T : Type} (_A : Type) (rc : alloc.rc.Rc T) :
    Result T :=
  ok rc.value

/-- A clone shares the allocation, hence the value. -/
@[rust_fun "alloc::rc::{core::clone::Clone<alloc::rc::Rc<@T>>}::clone"]
def alloc.rc.Rc.Insts.CoreCloneClone.clone {T : Type} {A : Type _}
    (_ : core.alloc.AllocatorClone A) (rc : alloc.rc.Rc T) : Result (alloc.rc.Rc T) :=
  ok rc

/-- `fmt::Debug::fmt(&**self, f)`. -/
@[rust_fun "alloc::rc::{core::fmt::Debug<alloc::rc::Rc<@T>>}::fmt"]
def alloc.rc.Rc.Insts.CoreFmtDebug.fmt {T : Type} (_A : Type) (inst : core.fmt.Debug T)
    (rc : alloc.rc.Rc T) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  inst.fmt rc.value formatter

@[rust_fun "alloc::sync::{alloc::sync::Arc<@T>}::new"]
def alloc.sync.Arc.new {T : Type} (value : T) : Result (alloc.sync.Arc T) :=
  ok ⟨value⟩

/-- `Arc::ptr_eq` compares allocation identity, which Aeneas's value model does
not have: two `Arc::new(())` owners and two clones of one owner are the same
value, yet Rust answers `false` and `true`. No function of the values is
faithful, so, as with `core::mem::size_of`, the model fails; no run that
compares owners yields an `ok` result. The only caller is
`Session::is_current_namespace` (live-slot commit), never `prepare`. -/
@[rust_fun "alloc::sync::{alloc::sync::Arc<@T>}::ptr_eq"]
def alloc.sync.Arc.ptr_eq {T : Type} (_A : Type) (_ _ : alloc.sync.Arc T) : Result Bool :=
  fail .undef

@[rust_fun "alloc::sync::{core::clone::Clone<alloc::sync::Arc<@T>>}::clone"]
def alloc.sync.Arc.Insts.CoreCloneClone.clone {T : Type} {A : Type _}
    (_ : core.alloc.AllocatorClone A) (arc : alloc.sync.Arc T) : Result (alloc.sync.Arc T) :=
  ok arc

@[rust_fun "alloc::sync::{core::fmt::Debug<alloc::sync::Arc<@T>>}::fmt"]
def alloc.sync.Arc.Insts.CoreFmtDebug.fmt {T : Type} (_A : Type) (inst : core.fmt.Debug T)
    (arc : alloc.sync.Arc T) (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  inst.fmt arc.value formatter

@[rust_fun
  "alloc::collections::btree::set::{alloc::collections::btree::set::BTreeSet<@T, alloc::alloc::Global>}::new"]
def alloc.collections.btree.set.BTreeSetTGlobal.new (T : Type) :
    Result (alloc.collections.btree.set.BTreeSet T Global) :=
  ok ⟨[]⟩

/-- Insert into the ascending element list. An element equal to `value` is
kept and `value` dropped, as in Rust. For a `cmp` that cannot fail (the only
instantiation, `String`) the result is Rust's; the comparisons themselves are
made along the list rather than along B-tree nodes. -/
def btreeInsert {T : Type} (inst : core.cmp.Ord T) (value : T) :
    List T → Result (Bool × List T)
  | [] => ok (true, [value])
  | first :: rest => do
    match ← inst.cmp value first with
    | .lt => ok (true, value :: first :: rest)
    | .eq => ok (false, first :: rest)
    | .gt => do
      let (inserted, rest) ← btreeInsert inst value rest
      ok (inserted, first :: rest)

@[rust_fun
  "alloc::collections::btree::set::{alloc::collections::btree::set::BTreeSet<@T, @A>}::insert"]
def alloc.collections.btree.set.BTreeSet.insert {T A : Type} (_ : core.alloc.AllocatorClone A)
    (inst : core.cmp.Ord T) (set : alloc.collections.btree.set.BTreeSet T A) (value : T) :
    Result (Bool × alloc.collections.btree.set.BTreeSet T A) := do
  let (inserted, elements) ← btreeInsert inst value set.elements
  ok (inserted, ⟨elements⟩)

/-! ### Slices, arrays and vectors -/

/-- Lexicographic comparison by the element `cmp`, then by length (Rust's
`SliceOrd::compare`); stops at the first unequal pair. -/
def lexCompare {T : Type _} (compare : T → T → Result Ordering) :
    List T → List T → Result Ordering
  | [], [] => ok .eq
  | [], _ :: _ => ok .lt
  | _ :: _, [] => ok .gt
  | left :: lefts, right :: rights => do
    match ← compare left right with
    | .eq => lexCompare compare lefts rights
    | order => ok order

/-- The same with the element `partial_cmp` (Rust's `SlicePartialOrd`): any
result other than `Some(Equal)` ends the comparison. -/
def lexPartialCompare {T : Type _} (compare : T → T → Result (Option Ordering)) :
    List T → List T → Result (Option Ordering)
  | [], [] => ok (some .eq)
  | [], _ :: _ => ok (some .lt)
  | _ :: _, [] => ok (some .gt)
  | left :: lefts, right :: rights => do
    match ← compare left right with
    | some .eq => lexPartialCompare compare lefts rights
    | order => ok order

/-- `[T; N]::partial_cmp` is the slice comparison. -/
@[rust_fun "core::array::{core::cmp::PartialOrd<[@T; @N], [@T; @N]>}::partial_cmp"]
def Array.Insts.CoreCmpPartialOrdArray.partial_cmp {T : Type _} {N : Usize}
    (inst : core.cmp.PartialOrd T T) (left right : Array T N) : Result (Option Ordering) :=
  lexPartialCompare inst.partial_cmp left.val right.val

/-- `[T; N]::ge` is the slice `__chaining_ge`: the first element pair that
is not `Some(Equal)` decides (`None` is `false`), and equal lengths are `true`. -/
@[rust_fun "core::array::{core::cmp::PartialOrd<[@T; @N], [@T; @N]>}::ge"]
def Array.Insts.CoreCmpPartialOrdArray.ge {T : Type _} {N : Usize}
    (inst : core.cmp.PartialOrd T T) (left right : Array T N) : Result Bool := do
  match ← lexPartialCompare inst.partial_cmp left.val right.val with
  | some .gt => ok true
  | some .eq => ok true
  | _ => ok false

@[rust_fun "core::array::{core::cmp::Ord<[@T; @N]>}::cmp"]
def Array.Insts.CoreCmpOrd.cmp {T : Type _} {N : Usize} (inst : core.cmp.Ord T)
    (left right : Array T N) : Result Ordering :=
  lexCompare inst.cmp left.val right.val

/-- Call the closure on `index, index + 1, …`, threading its state. -/
def generatedArrayItems {T F : Type} (inst : core.ops.function.FnMut F Usize T) :
    (count index : Nat) → F → Result { items : List T // items.length = count }
  | 0, _, _ => ok ⟨[], rfl⟩
  | count + 1, index, function => do
    let position ← UScalar.tryMk .Usize index
    let (item, function) ← inst.call_mut function position
    let rest ← generatedArrayItems inst count (index + 1) function
    ok ⟨item :: rest.val, by simp [rest.property]⟩

/-- `array::from_fn` calls the closure on `0, 1, …, N - 1` in order. -/
@[rust_fun "core::array::from_fn"]
def core.array.from_fn {T F : Type} (N : Usize) (inst : core.ops.function.FnMut F Usize T)
    (function : F) : Result (Array T N) := do
  let items ← generatedArrayItems inst N.val 0 function
  ok (Array.from items.val items.property)

@[rust_fun "core::slice::{[@T]}::split_first"]
def core.slice.Slice.split_first {T : Type} (slice : Slice T) : Result (Option (T × Slice T)) :=
  match h : slice.val with
  | [] => ok none
  | first :: rest =>
    ok (some (first, .from rest (by
      have bound := slice.property
      rw [h] at bound
      simp only [List.length_cons] at bound
      omega)))

/-- As for `last_mut`: the backward function updates only the first element,
and `none` leaves the slice unchanged. -/
@[rust_fun "core::slice::{[@T]}::first_mut"]
def core.slice.Slice.first_mut {T : Type} (slice : Slice T) :
    Result (Option T × (Option T → Slice T)) :=
  ok (slice.val.head?, fun replacement =>
    match replacement with
    | none => slice
    | some value => slice.setAtNat 0 value)

/-- `m >= n && needle == &self[m - n..]`, comparing `needle` on the left. -/
@[rust_fun "core::slice::{[@T]}::ends_with"]
def core.slice.Slice.ends_with {T : Type} (inst : core.cmp.PartialEq T T)
    (slice needle : Slice T) : Result Bool :=
  if needle.length ≤ slice.length then
    core.slice.cmp.PartialEqSlice.eq inst needle
      (.from (slice.val.drop (slice.length - needle.length)) (by
        have bound := slice.property
        simp only [List.length_drop]
        omega))
  else
    ok false

/-- The loop of the pinned `binary_search_by`: no early exit on `Equal`;
`base` moves to `mid` unless the comparison is `Greater`, and `size` loses
`size / 2` per step. `fuel` starts at the initial size; since `size` drops by
at least one per step while it exceeds one, the fuel never ends the loop. -/
def binarySearchSteps {T F : Type} (compare : F → T → Result (Ordering × F))
    (items : List T) : Nat → Nat → Nat → F → Result (Nat × F)
  | 0, _, base, function => ok (base, function)
  | fuel + 1, size, base, function =>
    if 1 < size then
      match items[base + size / 2]? with
      | none => fail .undef
      | some item => do
        let (order, function) ← compare function item
        binarySearchSteps compare items fuel (size - size / 2)
          (if order = .gt then base else base + size / 2) function
    else
      ok (base, function)

/-- `binary_search_by`: `Err(0)` on an empty slice; after the loop one more
comparison at `base` gives `Ok(base)` on `Equal`, else
`Err(base + (order == Less))`. The closure's final state is dropped. -/
def binarySearchBy {T F : Type} (compare : F → T → Result (Ordering × F))
    (slice : Slice T) (function : F) : Result (core.result.Result Usize Usize) := do
  if slice.length = 0 then return .Err 0#usize
  let (base, function) ← binarySearchSteps compare slice.val slice.length slice.length 0 function
  match slice.val[base]? with
  | none => fail .undef
  | some item =>
    let (order, _) ← compare function item
    if order = .eq then return .Ok (← UScalar.tryMk .Usize base)
    return .Err (← UScalar.tryMk .Usize (if order = .lt then base + 1 else base))

/-- `binary_search_by(|p| p.cmp(x))`. -/
@[rust_fun "core::slice::{[@T]}::binary_search"]
def core.slice.Slice.binary_search {T : Type} (inst : core.cmp.Ord T) (slice : Slice T)
    (target : T) : Result (core.result.Result Usize Usize) :=
  binarySearchBy (fun (_ : Unit) item => do ok (← inst.cmp item target, ())) slice ()

/-- `binary_search_by(|k| f(k).cmp(b))`, threading the key closure's state. -/
@[rust_fun "core::slice::{[@T]}::binary_search_by_key"]
def core.slice.Slice.binary_search_by_key {T B F : Type}
    (key : core.ops.function.FnMut F T B) (inst : core.cmp.Ord B) (slice : Slice T)
    (target : B) (function : F) : Result (core.result.Result Usize Usize) :=
  binarySearchBy (fun function item => do
      let (itemKey, function) ← key.call_mut function item
      ok (← inst.cmp itemKey target, function)) slice function

/-- Stable insertion with a stateful comparator, as in `insertSorted`: `item`
goes before the first element it does not compare `Greater` to, and every
comparison threads the closure state. -/
def insertSortedBy {T F : Type} (compare : F → T → T → Result (Ordering × F)) (item : T) :
    (items : List T) → F →
      Result ({ sorted : List T // sorted.length = items.length + 1 } × F)
  | [], function => ok (⟨[item], by simp⟩, function)
  | first :: rest, function => do
    let (order, function) ← compare function item first
    if order = .gt then
      let (inserted, function) ← insertSortedBy compare item rest function
      ok (⟨first :: inserted.val, by simp [inserted.property]⟩, function)
    else
      ok (⟨item :: first :: rest, by simp⟩, function)

def sortItemsBy {T F : Type} (compare : F → T → T → Result (Ordering × F)) :
    (items : List T) → F → Result ({ sorted : List T // sorted.length = items.length } × F)
  | [], function => ok (⟨[], rfl⟩, function)
  | first :: rest, function => do
    let (sorted, function) ← sortItemsBy compare rest function
    let (inserted, function) ← insertSortedBy compare first sorted.val function
    ok (⟨inserted.val, by simp [inserted.property, sorted.property]⟩, function)

/-- A stable sort; Rust returns before any comparison when the length is below
two. A stable order is the unique sorted order of `sort_by` and one of the
orders `sort_unstable_by` permits (it leaves the order of equal elements
unspecified). -/
def sortSliceBy {T F : Type} (compare : F → T → T → Result (Ordering × F))
    (slice : Slice T) (function : F) : Result (Slice T) :=
  if slice.length ≤ 1 then
    ok slice
  else do
    let (sorted, _) ← sortItemsBy compare slice.val function
    ok (.from sorted.val (by rw [sorted.property]; exact slice.property))

@[rust_fun "core::slice::{[@T]}::sort_unstable_by"]
def core.slice.Slice.sort_unstable_by {T F : Type}
    (inst : core.ops.function.FnMut F (T × T) Ordering) (slice : Slice T) (function : F) :
    Result (Slice T) :=
  sortSliceBy (fun function item first => inst.call_mut function (item, first)) slice function

/-- Rust compares by `f(a).lt(&f(b))`: the key closure runs twice per
comparison and `K`'s `lt` decides; `item` passes `first` exactly when
`first`'s key is less. -/
@[rust_fun "core::slice::{[@T]}::sort_unstable_by_key"]
def core.slice.Slice.sort_unstable_by_key {T K F : Type} (key : core.ops.function.FnMut F T K)
    (inst : core.cmp.Ord K) (slice : Slice T) (function : F) : Result (Slice T) :=
  sortSliceBy (fun function item first => do
      let (firstKey, function) ← key.call_mut function first
      let (itemKey, function) ← key.call_mut function item
      let passes ← inst.partialOrdInst.lt firstKey itemKey
      ok (if passes then .gt else .lt, function)) slice function

/-- Stable `sort_by`. -/
@[rust_fun "alloc::slice::{[@T]}::sort_by"]
def alloc.slice.Slice.sort_by {T F : Type} (inst : core.ops.function.FnMut F (T × T) Ordering)
    (slice : Slice T) (function : F) : Result (Slice T) :=
  sortSliceBy (fun function item first => inst.call_mut function (item, first)) slice function

@[rust_fun "core::slice::raw::from_ref"]
def core.slice.raw.from_ref {T : Type} (value : T) : Result (Slice T) :=
  ok (.from [value] (by simp only [List.length_singleton]; scalar_tac))

@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::append"]
def alloc.vec.Vec.append {T : Type} (_A : Type) (vector other : alloc.vec.Vec T) :
    Result (alloc.vec.Vec T × alloc.vec.Vec T) :=
  if h : vector.val.length + other.val.length ≤ Usize.max then
    ok (.from (vector.val ++ other.val) (by simp only [List.length_append]; exact h),
      .from [] (by simp))
  else
    fail .panic

/-- The general `SpecCloneIntoVec` path: `clone_from` on the common prefix in
index order, then clones of the remaining source elements; surplus target
elements are dropped. -/
def cloneIntoItems {T : Type} (inst : core.clone.Clone T) :
    (targets sources : List T) → Result { cloned : List T // cloned.length = sources.length }
  | target :: targets, source :: sources => do
    let first ← inst.clone_from target source
    let rest ← cloneIntoItems inst targets sources
    ok ⟨first :: rest.val, by simp [rest.property]⟩
  | _, [] => ok ⟨[], rfl⟩
  | [], source :: sources => do
    let first ← inst.clone source
    let rest ← cloneIntoItems inst [] sources
    ok ⟨first :: rest.val, by simp [rest.property]⟩

@[rust_fun "alloc::vec::{core::clone::Clone<alloc::vec::Vec<@T>>}::clone_from"]
def alloc.vec.Vec.Insts.CoreCloneClone.clone_from {T A : Type} (inst : core.clone.Clone T)
    (_ : core.clone.Clone A) (vector source : alloc.vec.Vec T) : Result (alloc.vec.Vec T) := do
  let cloned ← cloneIntoItems inst vector.val source.val
  ok (.from cloned.val (by rw [cloned.property]; exact source.slice.property))

@[rust_fun
  "alloc::vec::{core::iter::traits::collect::IntoIterator<&'a alloc::vec::Vec<@T>, &'a @T, core::slice::iter::Iter<'a, @T>>}::into_iter"]
def SharedAVec.Insts.CoreIterTraitsCollectIntoIteratorSharedATIter.into_iter {T : Type}
    (_A : Type) (vector : alloc.vec.Vec T) : Result (core.slice.iter.Iter T) :=
  core.slice.Slice.iter vector.slice

@[rust_fun "alloc::vec::{core::cmp::Ord<alloc::vec::Vec<@T>>}::cmp"]
def alloc.vec.Vec.Insts.CoreCmpOrd.cmp {T : Type} (_A : Type) (inst : core.cmp.Ord T)
    (left right : alloc.vec.Vec T) : Result Ordering :=
  lexCompare inst.cmp left.val right.val

@[rust_fun
  "alloc::vec::partial_eq::{core::cmp::PartialEq<alloc::vec::Vec<@T>, [@U; @N]>}::eq"]
def alloc.vec.Vec.Insts.CoreCmpPartialEqArray.eq {T U : Type} (_A : Type) {N : Usize}
    (inst : core.cmp.PartialEq T U) (left : alloc.vec.Vec T) (right : Array U N) : Result Bool :=
  core.slice.cmp.PartialEqSlice.eq inst left.slice right.to_slice

@[rust_fun
  "alloc::vec::partial_eq::{core::cmp::PartialEq<alloc::vec::Vec<@T>, [@U; @N]>}::ne"]
def alloc.vec.Vec.Insts.CoreCmpPartialEqArray.ne {T U : Type} (_A : Type) {N : Usize}
    (inst : core.cmp.PartialEq T U) (left : alloc.vec.Vec T) (right : Array U N) : Result Bool :=
  core.slice.cmp.PartialEqSlice.ne inst left.slice right.to_slice

@[rust_fun
  "alloc::vec::partial_eq::{core::cmp::PartialEq<alloc::vec::Vec<@T>, [@U]>}::eq"]
def alloc.vec.Vec.Insts.CoreCmpPartialEqSlice.eq {T U : Type} (_A : Type)
    (inst : core.cmp.PartialEq T U) (left : alloc.vec.Vec T) (right : Slice U) : Result Bool :=
  core.slice.cmp.PartialEqSlice.eq inst left.slice right

@[rust_fun
  "alloc::vec::partial_eq::{core::cmp::PartialEq<alloc::vec::Vec<@T>, [@U]>}::ne"]
def alloc.vec.Vec.Insts.CoreCmpPartialEqSlice.ne {T U : Type} (_A : Type)
    (inst : core.cmp.PartialEq T U) (left : alloc.vec.Vec T) (right : Slice U) : Result Bool :=
  core.slice.cmp.PartialEqSlice.ne inst left.slice right

@[rust_fun
  "alloc::vec::partial_eq::{core::cmp::PartialEq<alloc::vec::Vec<@T>, &'0 [@U]>}::ne"]
def alloc.vec.Vec.Insts.CoreCmpPartialEqShared0Slice.ne {T U : Type} (_A : Type)
    (inst : core.cmp.PartialEq T U) (left : alloc.vec.Vec T) (right : Slice U) : Result Bool :=
  core.slice.cmp.PartialEqSlice.ne inst left.slice right

/-! ### Strings -/

/-- Byte-wise lexicographic order, then length: Rust's `Ord` for `[u8]`, which
`str` and `String` (a derived `Ord` over its `Vec<u8>`) use. -/
def bytesCompare : List Nat → List Nat → Ordering
  | [], [] => .eq
  | [], _ :: _ => .lt
  | _ :: _, [] => .gt
  | left :: lefts, right :: rights =>
    if left < right then .lt else if right < left then .gt else bytesCompare lefts rights

/-- The UTF-8 bytes of an owned string. -/
def stringBytes (value : String) : List Nat :=
  value.toUTF8.toList.map UInt8.toNat

@[rust_fun "core::str::traits::{core::cmp::Ord<str>}::cmp"]
def Str.Insts.CoreCmpOrd.cmp (left right : Str) : Result Ordering :=
  ok (bytesCompare (left.val.map (·.val)) (right.val.map (·.val)))

@[rust_fun "alloc::string::{core::cmp::Ord<alloc::string::String>}::cmp"]
def alloc.string.String.Insts.CoreCmpOrd.cmp (left right : String) : Result Ordering :=
  ok (bytesCompare (stringBytes left) (stringBytes right))

@[rust_fun
  "alloc::string::{core::cmp::PartialOrd<alloc::string::String, alloc::string::String>}::partial_cmp"]
def alloc.string.String.Insts.CoreCmpPartialOrdString.partial_cmp (left right : String) :
    Result (Option Ordering) :=
  ok (some (bytesCompare (stringBytes left) (stringBytes right)))

@[rust_fun "alloc::string::{core::cmp::Eq<alloc::string::String>}::assert_fields_are_eq"]
def alloc.string.String.Insts.CoreCmpEq.assert_fields_are_eq (_ : String) : Result Unit :=
  ok ()

@[rust_fun "alloc::string::{alloc::string::String}::into_bytes"]
def alloc.string.String.into_bytes (value : String) : Result (alloc.vec.Vec U8) := do
  ok ⟨← alloc.string.String.as_bytes value⟩

/-- `String::replace_range`: `slice::range(range, ..len)` reads `start_bound`
then `end_bound`, resolves and checks the end, then the start, panicking on
`end > len` or `start > end`; both ends must be character boundaries; growth
beyond `isize::MAX` bytes panics in `reserve`. The bytes are then spliced. -/
@[rust_fun "alloc::string::{alloc::string::String}::replace_range"]
def alloc.string.String.replace_range {R : Type} (inst : core.ops.range.RangeBounds R Usize)
    (value : String) (range : R) (replacement : Str) : Result String := do
  let startBound ← inst.start_bound range
  let endBound ← inst.end_bound range
  let bytes ← alloc.string.String.as_bytes value
  let stop ← match endBound with
    | .Included index => if bytes.length ≤ index.val then fail .panic else ok (index.val + 1)
    | .Excluded index => if bytes.length < index.val then fail .panic else ok index.val
    | .Unbounded => ok bytes.length
  let start ← match startBound with
    | .Excluded index => if stop ≤ index.val then fail .panic else ok (index.val + 1)
    | .Included index => if stop < index.val then fail .panic else ok index.val
    | .Unbounded => ok 0
  if strBoundary bytes start && strBoundary bytes stop then
    let spliced := bytes.val.take start ++ replacement.val ++ bytes.val.drop stop
    if spliced.length ≤ Isize.max.toNat then
      if h : spliced.length ≤ Usize.max then decodeStr (.from spliced h) else fail .panic
    else
      fail .panic
  else
    fail .panic

@[rust_fun
  "alloc::string::{core::clone::Clone<alloc::string::String>}::clone_from"]
def alloc.string.String.Insts.CoreCloneClone.clone_from (_ source : String) : Result String :=
  ok source

@[rust_fun
  "alloc::string::{core::cmp::PartialEq<alloc::string::String, &'0 str>}::ne"]
def alloc.string.String.Insts.CoreCmpPartialEqShared0Str.ne (left : String) (right : Str) :
    Result Bool := do
  let equal ← alloc.string.String.Insts.CoreCmpPartialEqShared0Str.eq left right
  ok (!equal)

@[rust_fun
  "alloc::string::{core::cmp::PartialEq<&'0 str, alloc::string::String>}::ne"]
def Shared0Str.Insts.CoreCmpPartialEqString.ne (left : Str) (right : String) : Result Bool := do
  let equal ← Shared0Str.Insts.CoreCmpPartialEqString.eq left right
  ok (!equal)

@[rust_fun
  "alloc::string::{core::default::Default<alloc::string::String>}::default"]
def alloc.string.String.Insts.CoreDefaultDefault.default : Result String :=
  ok ""

end noble_contracts
