module
public import Aeneas
public import NobleWasmImpl.Types
public import NobleWasmImpl.StdModels
public import NobleWasmImpl.KernelBridge
public import NobleWasmImpl.ContractsBridge
@[expose] public section

open Aeneas Aeneas.Std Result

namespace noble_wasm

/-!
The crate's one reviewed `#[charon::opaque]` function. The standard-library
models are in `StdModels.lean` and the inherited kernel and frontend boundaries
in `KernelBridge.lean` and `ContractsBridge.lean`; this module gathers them for
the generated `Funs.lean`.
-/

/-- [noble_wasm::source::debug_origin]: renders an `OriginOperand`,
`OriginProgramValue` or `OriginFixedExpression` exactly as `#[derive(Debug)]`
did, through `debug_tuple`/`debug_struct` fields. Its body is opaque to the
extraction because the three types recurse through `Rc`, so each nested value
is formatted through its own `Debug` impl, which calls back into this function.
Under the inherited abstract `Formatter` every write returns the formatter
unchanged, and every field it formats is either a scalar (`NodeId`, `u32`,
`usize`, `i64`, the C-like `OriginFixedArithmetic`), whose `Debug` model is a
successful pass-through, or a nested origin value: an `OriginI64` through its
derived `Debug` (scalars and an `Rc` recipe), or a value behind `Rc`, whose
`Debug` delegates to the payload and so back to this function. By induction
over the finite value the rendering therefore succeeds and returns the
formatter unchanged, which is what this model returns, like the kernel's opaque
`debug_stack` and `debug_parts`. It makes no claim about the rendered text. -/
def source.debug_origin (_rendering : source.OriginRendering)
    (formatter : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) :=
  ok (.Ok (), formatter)

end noble_wasm
