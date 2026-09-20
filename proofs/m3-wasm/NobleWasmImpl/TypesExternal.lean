import Aeneas
import NobleKernel.Types

open Aeneas Aeneas.Std

namespace noble_wasm

/-- Effects use the inherited kernel's complete ordered representation. -/
@[rust_type "noble_kernel::types::EffSet"]
abbrev noble_kernel.types.EffSet := _root_.noble_kernel.types.EffSet

end noble_wasm
