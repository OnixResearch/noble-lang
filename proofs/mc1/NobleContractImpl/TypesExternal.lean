module
public import Aeneas
public import NobleKernel.Types
@[expose] public section

/-!
External types of the actual `noble-contracts` extraction: every type of the
generated `TypesExternal_Template.lean`, plus `F32`, which the pinned Aeneas
library does not provide. Each model below names the Rust item and why it is
faithful within the inherited value-only Aeneas abstraction (no allocation
identity, no shared mutation, shared borrows are values).

The four `#[charon::opaque]` host plug-in handles (`ObserverHandle`,
`RankerHandle`, `MonotonicClock`, `OwnerVerifier`) wrap a caller-owned trait
object, closure or function pointer that the Rust source does not determine.
Each model therefore carries that host's behaviour as data, so a statement
about translated code that receives a handle is a statement about every host:

* an `&mut self` call (`observe`, `score`, `now_ns`) is answered from a
  prophecy sequence indexed by the number of such calls on the handle so far.
  The translated code is deterministic and uses each `&mut` handle linearly,
  so for any host the run is reproduced by the sequence of answers that host
  gives in that run, and every sequence is the run of the host which replays
  it; quantifying over handles is quantifying over hosts;
* an `&self` call (`model_identity`, `provenance_matches`, `verify`,
  `Debug::fmt`) is a function of the handle state, as Aeneas reads shared
  borrows as values, and of an injective `HostPlugin.Value` image of its
  arguments (the argument types are generated after this file, so they
  cannot be named here);
* every answer is a `Result`, so a host which panics or does not return is
  represented as well.

The replies of the scorer and of the owner's verifier are kept in reply
types below, which are in bijection with the generated Rust result types they
are converted to (`FunsExternal.lean`).
-/

open Aeneas Aeneas.Std
set_option linter.dupNamespace false

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

/-- Failed owned UTF-8 conversion retains the original bytes and the same
diagnostic as `str::from_utf8`; no replacement decoding takes place. -/
@[rust_type "alloc::string::FromUtf8Error"]
structure alloc.string.FromUtf8Error where
  bytes : alloc.vec.Vec U8
  error : core.str.error.Utf8Error

/-- The inherited checker owns effects; no second effect representation. -/
@[rust_type "noble_kernel::types::EffSet"]
abbrev noble_kernel.types.EffSet := _root_.noble_kernel.types.EffSet

/-- `core::array::iter::IntoIter<T, N>`: the inherited kernel model, which
keeps exactly the alive window `data[alive]` that Rust's `next` reads. The
contracts and kernel subjects share the same pinned `core`, so one model. -/
@[rust_type "core::array::iter::IntoIter"]
abbrev core.array.iter.IntoIter (T : Type) (N : Usize) :=
  _root_.noble_kernel.core.array.iter.IntoIter T N

/-- Rust's `f32` is IEEE 754 binary32 (round to nearest, ties to even, with
subnormals, infinities and NaN). Lean's core `Float32` is that format: its
logical model `Float32.Model` is the binary32 bit pattern with IEEE
arithmetic and comparisons, and compiled code uses the C `float`
operations, which on the audited x86_64 target are the same IEEE binary32
operations Rust uses. NaN payloads are not observable through any operation
the translated code calls (no `to_bits`). The pinned Aeneas library has no
float type; this is the only floating-point model in the subject. -/
abbrev F32 := Float32

/-- `core::cell::Cell<T>`: the current value of the cell (`Cell` is a
transparent wrapper of one `UnsafeCell<T>`). Writes through a shared
reference are not values in Aeneas; see `core.cell.Cell.set`. -/
@[rust_type "core::cell::Cell"]
structure core.cell.Cell (T : Type) where
  value : T

/-- `alloc::collections::btree::set::BTreeSet<T, A>`: the elements in
ascending order of the `Ord` that built the set, without duplicates, as
Rust's in-order iteration yields them. Tree shape, node capacity and the
allocator `A` (only `Global` here) are not observable through `new` and
`insert`, the only operations the translation calls. -/
@[rust_type "alloc::collections::btree::set::BTreeSet"]
structure alloc.collections.btree.set.BTreeSet (T A : Type) where
  elements : List T

/-- `alloc::rc::Rc<T>`: the shared value. Allocation identity and reference
counts are not part of Aeneas's value model; the only identity comparison
(`Rc::ptr_eq`) is outside it. -/
@[rust_type "alloc::rc::Rc"]
structure alloc.rc.Rc (T : Type) where
  value : T

/-- `alloc::sync::Arc<T>`: the shared value, as for `Rc`. The translation
uses `Arc<()>` only as an owner token compared with `Arc::ptr_eq`, which the
value model cannot answer (`alloc.sync.Arc.ptr_eq`). -/
@[rust_type "alloc::sync::Arc"]
structure alloc.sync.Arc (T : Type) where
  value : T

namespace HostPlugin

/-- An injective first-order image of a Rust value passed to a host plug-in:
numbers, text, tagged nodes, and the inherited kernel's own types and effect
sets (the contracts copies are mapped to them by the injective kernel
bridge). -/
inductive Value where
  | nat (value : Nat)
  | int (value : Int)
  | text (value : String)
  | ty (value : _root_.noble_kernel.types.Ty)
  | effects (value : _root_.noble_kernel.types.EffSet)
  | node (tag : Nat) (fields : List Value)

/-- A scorer's reply: `Ok(CandidateScoreResponse)` with the response fields
(the candidate id is its ordinal and digest), or `Err(ScorerFailure)` with
the failure's declaration index. -/
inductive ScoreReply where
  | response (request_identity model_identity : Array U8 32#usize)
      (ordinal : U32) (digest : Array U8 32#usize) (score : F32)
  | failure (kind : Fin 5)

/-- An owner verifier's attestation; `outcome` is the declaration index of
the `EvidenceOutcome`. -/
structure AttestationReply where
  verifier : String
  toolchain : alloc.vec.Vec String
  command_digest : Array U8 32#usize
  input_digest : Array U8 32#usize
  outcome : Fin 9

end HostPlugin

/-- `ObserverHandle<'a>(&'a mut dyn FnMut(&CandidateOutcome))`. Translated
code observes no value from the closure, only whether a call returns, so the
host is the sequence `deliveries` of call results; `delivered` counts the
calls made through the handle so far. -/
@[rust_type "noble_contracts::source::chunks::ObserverHandle"]
structure source.chunks.ObserverHandle where
  deliveries : Nat → Result Unit
  delivered : Nat

/-- `RankerHandle<'a>(&'a mut dyn CandidateRanker)`. `scored` counts the
`score` calls so far; the scorer state after them answers `model_identity`
and `provenance_matches` (an `&self` call, applied to the image of the
`RankingIdentity`), and `replies` is the prophecy sequence of `score`
results. -/
@[rust_type "noble_contracts::source::chunks::RankerHandle"]
structure source.chunks.RankerHandle where
  model_identity : Nat → Result (Array U8 32#usize)
  provenance : Nat → HostPlugin.Value → Result Bool
  replies : Nat → Result HostPlugin.ScoreReply
  scored : Nat

/-- `MonotonicClock(fn() -> u64)`. The host function pointer reads a host
clock; nothing in the Rust source fixes its values (monotonicity is the
host's obligation, not a guarantee), so the handle is the prophecy sequence
of readings, and `taken` counts the reads so far. -/
@[rust_type "noble_contracts::source::chunks::MonotonicClock"]
structure source.chunks.MonotonicClock where
  readings : Nat → Result U64
  taken : Nat

/-- `OwnerVerifier(Box<dyn EvidenceVerifier>)`. Every call is `&self`, so the
verifier is a fixed function of the images of the procedure record and the
claim, and its `Debug` result under the abstract Aeneas formatter. -/
@[rust_type "noble_contracts::source::procedures::acceptance::OwnerVerifier"]
structure source.procedures.acceptance.OwnerVerifier where
  verify : HostPlugin.Value → HostPlugin.Value →
    Result (Option HostPlugin.AttestationReply)
  debug : Result (core.result.Result Unit core.fmt.Error)

end noble_contracts
