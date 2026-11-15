-- Aeneas-generated declarations, selected verbatim by region-cut-generate.mjs.

-- The bridge below is the explicit signed-conversion external model, not a Rust translation.

import M5Resources



open Aeneas Aeneas.Std Result ControlFlow Error

namespace noble_kernel



/-- [noble_kernel::region_read::Refusal]
    Source: 'crates/noble-kernel/src/region/read.rs', lines 7:0-12:1
    Visibility: public -/
@[discriminant isize]
inductive region_read.Refusal where
| Representation : region_read.Refusal
| NoOwner : region_read.Refusal
| Identity : region_read.Refusal
| Bounds : region_read.Refusal

/-- [noble_kernel::region_read::Request]
    Source: 'crates/noble-kernel/src/region/read.rs', lines 15:0-21:1
    Visibility: public -/
structure region_read.Request where
  representation : Std.U32
  claim : resources.Handle
  retained : Option resources.Handle
  offset_bytes : Std.I64
  length_bytes : Std.I64

/-- [noble_kernel::region_read::Range]
    Source: 'crates/noble-kernel/src/region/read.rs', lines 24:0-27:1
    Visibility: public -/
structure region_read.Range where
  start : Std.Usize
  finish : Std.Usize

/-- [noble_kernel::region_read::MAX_REGION_BYTES]
    Source: 'crates/noble-kernel/src/region/read.rs', lines 4:0-4:41
    Visibility: public -/
@[global_simps, irreducible]
def region_read.MAX_REGION_BYTES : Std.Usize := 4096#usize

-- The only new external model: Rust i64 -> usize refuses negative or out-of-range values.
-- The two other externals are the already audited M5 option equality and Result.is_err.
def Usize.Insts.CoreConvertTryFromI64TryFromIntError.try_from
    (value : Std.I64) :
    Result (core.result.Result Std.Usize core.num.error.TryFromIntError) :=
  if value.val < 0 then ok (.Err ())
  else match UScalar.tryMkOpt .Usize value.val.toNat with
    | some converted => ok (.Ok converted)
    | none => ok (.Err ())

/-- [noble_kernel::region_read::preflight]:
    Source: 'crates/noble-kernel/src/region/read.rs', lines 39:0-70:1
    Visibility: public -/
def region_read.preflight
  (table : resources.table.Table) (required : resources.Requirement)
  (request : region_read.Request) (region_len : Std.Usize) :
  Result (core.result.Result region_read.Range region_read.Refusal)
  := do
  if request.representation != 1#u32
  then ok (core.result.Result.Err region_read.Refusal.Representation)
  else
    match request.retained with
    | none => ok (core.result.Result.Err region_read.Refusal.NoOwner)
    | some actual =>
      let r ← resources.table.Table.validate table request.claim required
      let b ← core.result.Result.is_err r
      if b
      then ok (core.result.Result.Err region_read.Refusal.Identity)
      else
        let b1 ←
          core.cmp.PartialEq.ne.trait_default
            resources.Handle.Insts.CoreCmpPartialEqHandle request.claim actual
        if b1
        then ok (core.result.Result.Err region_read.Refusal.Identity)
        else
          let o ←
            lift (I64.checked_add request.offset_bytes request.length_bytes)
          match o with
          | none => ok (core.result.Result.Err region_read.Refusal.Bounds)
          | some «end» =>
            let r1 ←
              Usize.Insts.CoreConvertTryFromI64TryFromIntError.try_from
                request.offset_bytes
            match r1 with
            | core.result.Result.Ok start =>
              let r2 ←
                Usize.Insts.CoreConvertTryFromI64TryFromIntError.try_from
                  request.length_bytes
              match r2 with
              | core.result.Result.Ok count =>
                let r3 ←
                  Usize.Insts.CoreConvertTryFromI64TryFromIntError.try_from
                    «end»
                match r3 with
                | core.result.Result.Ok finish =>
                  if start > region_len
                  then ok (core.result.Result.Err region_read.Refusal.Bounds)
                  else
                    if finish > region_len
                    then ok (core.result.Result.Err region_read.Refusal.Bounds)
                    else
                      if count > region_read.MAX_REGION_BYTES
                      then
                        ok (core.result.Result.Err region_read.Refusal.Bounds)
                      else ok (core.result.Result.Ok { start, finish })
                | core.result.Result.Err _ =>
                  ok (core.result.Result.Err region_read.Refusal.Bounds)
              | core.result.Result.Err _ =>
                ok (core.result.Result.Err region_read.Refusal.Bounds)
            | core.result.Result.Err _ =>
              ok (core.result.Result.Err region_read.Refusal.Bounds)

/-- [noble_kernel::region_read::authorize_copy]:
    Source: 'crates/noble-kernel/src/region/read.rs', lines 78:0-96:1
    Visibility: public -/
def region_read.authorize_copy
  (access : core.result.Result resources.Snapshot resources.Error)
  (claim : resources.Handle) (scope : resources.Scope)
  (range : region_read.Range) (region_len : Std.Usize) :
  Result Bool
  := do
  match access with
  | core.result.Result.Ok snapshot =>
    let b ←
      resources.Handle.Insts.CoreCmpPartialEqHandle.eq snapshot.handle claim
    if b
    then
      let b1 ←
        resources.Handle.Insts.CoreCmpPartialEqHandle.eq scope.handle claim
      if b1
      then
        let b2 ←
          resources.State.Insts.CoreCmpPartialEqState.eq snapshot.state
            (resources.State.Busy scope.serial)
        if b2
        then
          let b3 ←
            core.option.Option.Insts.CoreCmpPartialEqOption.eq
              core.cmp.PartialEqU64 snapshot.last_scope (some scope.serial)
          if b3
          then
            if range.start <= range.finish
            then ok (range.finish <= region_len)
            else ok false
          else ok false
        else ok false
      else ok false
    else ok false
  | core.result.Result.Err _ => ok false

end noble_kernel

