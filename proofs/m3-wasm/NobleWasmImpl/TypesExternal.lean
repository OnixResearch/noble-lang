import Aeneas
import NobleKernel.Types

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

end noble_wasm
