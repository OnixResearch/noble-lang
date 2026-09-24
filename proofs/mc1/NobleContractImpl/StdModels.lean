import Aeneas
import NobleContractImpl.Types
import NobleKernel.FunsExternal

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

/-- Check each digit before reporting overflow at that digit, as Rust does;
an already overflowing prefix stops before examining any later bytes. -/
private def parseU32Digits {n : Nat} (digits : ListN U8 n) (value : U32) :
    Result (core.result.Result U32 core.num.error.ParseIntError) :=
  match digits with
  | .nil => ok (.Ok value)
  | .cons digit rest =>
    if 48 ≤ digit.val ∧ digit.val ≤ 57 then
      let next := value.val * 10 + (digit.val - 48)
      if next ≤ U32.max then do
        let value ← UScalar.tryMk .U32 next
        parseU32Digits rest value
      else
        ok (.Err { kind := .PosOverflow })
    else
      ok (.Err { kind := .InvalidDigit })
termination_by structural digits

private def parseU32Start {n : Nat} (digits : ListN U8 n) :
    Result (core.result.Result U32 core.num.error.ParseIntError) :=
  match digits with
  | .nil => ok (.Err { kind := .Empty })
  | .cons first rest =>
    if first.val = 43 then
      match rest with
      | .nil => ok (.Err { kind := .InvalidDigit })
      | .cons _ _ => parseU32Digits rest 0#u32
    else
      parseU32Digits digits 0#u32

/-- Decimal `u32::from_str`: no trimming, radix prefixes, underscores, Unicode
digits, or minus sign; a single leading plus is permitted only before digits. -/
@[rust_fun
  "core::num::{core::str::traits::FromStr<u32, core::num::error::ParseIntError>}::from_str"]
def U32.Insts.CoreStrTraitsFromStrParseIntError.from_str (value : Str) :
    Result (core.result.Result U32 core.num.error.ParseIntError) :=
  parseU32Start value.list

/-- Consume the exact finite word width, not an arbitrary recursion budget. -/
private def countLowBits : Nat → Nat → Nat
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

@[rust_fun "core::fmt::{core::fmt::Formatter<'a>}::debug_c_like_enum_write_str"]
def core.fmt.Formatter.debug_c_like_enum_write_str (formatter : core.fmt.Formatter)
    (names : Str) (offsets : Slice Usize) (index : Usize) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  _root_.noble_kernel.core.fmt.Formatter.debug_c_like_enum_write_str
    formatter names offsets index

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

@[rust_fun "core::option::{core::option::Option<@T>}::map"]
def core.option.Option.map {T U F : Type} (inst : core.ops.function.FnOnce F T U)
    (value : Option T) (function : F) : Result (Option U) :=
  _root_.noble_kernel.core.option.Option.map inst value function

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

@[rust_fun "core::option::{core::option::Option<&'0 @T>}::copied"]
def core.option.OptionShared0T.copied {T : Type} (inst : core.marker.Copy T)
    (value : Option T) : Result (Option T) :=
  _root_.noble_kernel.core.option.OptionShared0T.copied inst value

@[rust_fun
  "core::option::{core::cmp::PartialEq<core::option::Option<@T>, core::option::Option<@T>>}::eq"]
def core.option.Option.Insts.CoreCmpPartialEqOption.eq {T : Type}
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

@[rust_fun "core::option::{core::clone::Clone<core::option::Option<@T>>}::clone"]
def core.option.Option.Insts.CoreCloneClone.clone
    {T : Type} (inst : core.clone.Clone T) (value : Option T) : Result (Option T) :=
  _root_.noble_kernel.core.option.Option.Insts.CoreCloneClone.clone inst value

@[rust_fun "core::result::{core::result::Result<@T, @E>}::is_err"]
def core.result.Result.is_err
    {T E : Type} (value : core.result.Result T E) : Result Bool :=
  _root_.noble_kernel.core.result.Result.is_err value

@[rust_fun "core::result::{core::result::Result<@T, @E>}::ok"]
def core.result.Result.ok
    {T E : Type} (value : core.result.Result T E) : Result (Option T) :=
  _root_.noble_kernel.core.result.Result.ok value

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

/-- Traverse existing slice storage without materializing another list. Every
callback receives the state returned by its predecessor; skipped items do not
invoke it. The cursor includes the item that triggers short-circuiting. -/
private def scanIterator {T F : Type} (inst : core.ops.function.FnMut F T Bool)
    (stopOn : Bool) {n : Nat} (items : ListN T n) (skip cursor : Nat)
    (function : F) : Result (Bool × Nat) :=
  match items with
  | .nil => ok (!stopOn, cursor)
  | .cons value rest =>
    match skip with
    | skip + 1 => scanIterator inst stopOn rest skip cursor function
    | 0 => do
      let (result, function) ← inst.call_mut function value
      if result == stopOn then
        ok (result, cursor + 1)
      else
        scanIterator inst stopOn rest 0 (cursor + 1) function
termination_by structural items

private def foldIterator {T B F : Type} (inst : core.ops.function.FnMut F (B × T) B)
    {n : Nat} (items : ListN T n) (skip : Nat) (accumulator : B)
    (function : F) : Result B :=
  match items with
  | .nil => ok accumulator
  | .cons value rest =>
    match skip with
    | skip + 1 => foldIterator inst rest skip accumulator function
    | 0 => do
      let (accumulator, function) ← inst.call_mut function (accumulator, value)
      foldIterator inst rest 0 accumulator function
termination_by structural items

@[rust_fun
  "core::slice::iter::{core::iter::traits::iterator::Iterator<core::slice::iter::Iter<'a, @T>, &'a @T>}::fold"]
def core.slice.iter.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.fold
    {T B F : Type} (inst : core.ops.function.FnMut F (B × T) B)
    (iterator : core.slice.iter.Iter T) (initial : B) (function : F) : Result B :=
  foldIterator inst iterator.slice.list iterator.i initial function

@[rust_fun
  "core::slice::iter::{core::iter::traits::iterator::Iterator<core::slice::iter::Iter<'a, @T>, &'a @T>}::any"]
def core.slice.iter.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.any
    {T F : Type} (inst : core.ops.function.FnMut F T Bool)
    (iterator : core.slice.iter.Iter T) (function : F) :
    Result (Bool × core.slice.iter.Iter T) := do
  let (result, cursor) ←
    scanIterator inst true iterator.slice.list iterator.i iterator.i function
  ok (result, { iterator with i := cursor })

@[rust_fun
  "core::slice::iter::{core::iter::traits::iterator::Iterator<core::slice::iter::Iter<'a, @T>, &'a @T>}::position"]
def core.slice.iter.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.position
    {T P : Type} (inst : core.ops.function.FnMut P T Bool)
    (iterator : core.slice.iter.Iter T) (predicate : P) :
    Result (Option Usize × core.slice.iter.Iter T) := do
  let (found, cursor) ←
    scanIterator inst true iterator.slice.list iterator.i iterator.i predicate
  let updated := { iterator with i := cursor }
  if found then do
    let position ← UScalar.tryMk .Usize (cursor - iterator.i - 1)
    ok (some position, updated)
  else
    ok (none, updated)

@[rust_fun
  "core::str::iter::{core::iter::traits::iterator::Iterator<core::str::iter::Bytes<'0>, u8>}::all"]
def core.str.iter.Bytes.Insts.CoreIterTraitsIteratorIteratorU8.all
    {F : Type} (inst : core.ops.function.FnMut F U8 Bool)
    (iterator : core.str.iter.Bytes) (function : F) :
    Result (Bool × core.str.iter.Bytes) := do
  let (result, cursor) ←
    scanIterator inst false iterator.iter.slice.list iterator.iter.i iterator.iter.i function
  ok (result, { iter := { iterator.iter with i := cursor } })

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

/-! ## UTF-8 byte ranges

Rust indexes `str` by bytes, never by character number. For valid UTF-8, zero,
the byte length, and bytes other than continuation bytes are exactly the
character boundaries. Raw pointer models retain Aeneas's value representation;
an invalid unchecked range is undefined behavior rather than a safe panic.
-/

private def startsCharacterAt {n : Nat} (bytes : ListN U8 n) (index : Nat) : Bool :=
  match bytes with
  | .nil => false
  | .cons byte rest =>
    match index with
    | 0 => !utf8Continuation byte
    | index + 1 => startsCharacterAt rest index
termination_by structural bytes

private def strBoundary (value : Str) (index : Nat) : Bool :=
  if index = 0 ∨ index = value.length then
    true
  else
    startsCharacterAt value.list index

private def validStrRange (range : core.ops.range.Range Usize) (value : Str) : Bool :=
  decide (range.start.val ≤ range.end.val) &&
    strBoundary value range.start.val && strBoundary value range.end.val

/-- As at the inherited mutable-slice boundary, safe callers return a borrow
of the original length. Also retain the UTF-8 invariant of mutable `str`.
Out-of-domain backward inputs cannot resize the borrow or alter its suffix. -/
private def replaceStrRange (range : core.ops.range.Range Usize)
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

@[rust_fun "core::str::{str}::bytes"]
def core.str.Str.bytes (value : Str) : Result core.str.iter.Bytes :=
  ok { iter := { slice := value, i := 0 } }

@[rust_fun "core::str::{str}::parse"]
def core.str.Str.parse {F E : Type} (inst : core.str.traits.FromStr F E) (value : Str) :
    Result (core.result.Result F E) :=
  inst.from_str value

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
private def stringBytesEq (owned : ByteArray) {n : Nat} (borrowed : ListN U8 n)
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
