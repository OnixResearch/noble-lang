import Aeneas
import NobleKernel.Types

open Aeneas Aeneas.Std

namespace noble_contracts

/-- Initialization state of a standard-library temporary. -/
@[rust_type "core::mem::maybe_uninit::MaybeUninit"]
abbrev core.mem.maybe_uninit.MaybeUninit (T : Type) := Option T

/-- The standard integer parser distinguishes syntax from signed overflow.
Unsigned decimal parsing produces only the first three constructors. -/
@[rust_type "core::num::error::IntErrorKind"]
inductive core.num.error.IntErrorKind where
  | Empty
  | InvalidDigit
  | PosOverflow
  | NegOverflow
  | Zero

@[rust_type "core::num::error::ParseIntError"]
structure core.num.error.ParseIntError where
  kind : core.num.error.IntErrorKind

/-- Rust UTF-8 diagnostics carry a byte offset and an optional invalid length. -/
@[rust_type "core::str::error::Utf8Error"]
structure core.str.error.Utf8Error where
  offset : Usize
  error_length : Option Usize

/-- `Bytes` shares the standard slice iterator's original storage and cursor. -/
@[rust_type "core::str::iter::Bytes"]
structure core.str.iter.Bytes where
  iter : core.slice.iter.Iter U8

/-- Failed owned UTF-8 conversion retains the original bytes and the same
diagnostic as `str::from_utf8`; no replacement decoding takes place. -/
@[rust_type "alloc::string::FromUtf8Error"]
structure alloc.string.FromUtf8Error where
  bytes : alloc.vec.Vec U8
  error : core.str.error.Utf8Error

/-- The inherited checker owns effects; no second effect representation. -/
@[rust_type "noble_kernel::types::EffSet"]
abbrev noble_kernel.types.EffSet := _root_.noble_kernel.types.EffSet

end noble_contracts
