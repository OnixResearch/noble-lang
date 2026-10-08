//! Separate opt-in Worker source runtime. Installation creates the exact
//! checked named Program cells without executing the wrapper root. Host code
//! must compare their fields to `NamedTargetMetadata` before exposing a WIT
//! resource; these i32 cell addresses are never themselves WIT capabilities.

pub(super) fn exports(
    out: &mut crate::output::Buffer,
    plan: &super::super::plan::Layout,
    generation: u32,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(global $worker_installed (mut i32) (i32.const 0))\n\
(func (export \"worker_install\") (result i32)\n\
(if (global.get $failure) (then (return (global.get $failure))))\n\
(if (i32.or (global.get $worker_installed) (i32.or (global.get $cp) (global.get $sp)))\n (then (call $fail (i32.const 4)) (return (global.get $failure))))\n\
(if (i32.ne (global.get $generation) "));
    attempt!(out.i32(generation));
    attempt!(out.append(b") (then (call $fail (i32.const 4)) (return (global.get $failure))))\n\
(global.set $phase (i32.const 0))\n(call $initialize)\n\
(if (i32.eqz (global.get $failure)) (then\n\
 (global.set $worker_installed (i32.const 1))\n\
 (global.set $generation (i32.add (global.get $generation) (i32.const 1)))))\n\
(global.get $failure))\n\
(func (export \"worker_named_count\") (result i32) "));
    attempt!(out.i32(attempt!(u32::try_from(plan.definition_roots.len())
        .map_err(|_| crate::Diagnostic::Exhausted))));
    attempt!(out.append(b")\n\
(func (export \"worker_named_handle\") (param $index i32) (result i32)\n\
(if (i32.eqz (global.get $worker_installed)) (then (return (i32.const 0))))\n"));
    for (index, program) in plan.definition_roots.iter().enumerate() {
        attempt!(out.append(b"(if (i32.eq (local.get $index) "));
        attempt!(out.i32(attempt!(u32::try_from(index)
            .map_err(|_| crate::Diagnostic::Exhausted))));
        attempt!(out.append(b") (then (return "));
        attempt!(super::get_global(out, *program));
        attempt!(out.append(b")))\n"));
    }
    attempt!(out.append(b"(i32.const 0))\n\
(func (export \"worker_program_handle\") (param $index i32) (result i32)\n\
(if (i32.eqz (global.get $worker_installed)) (then (return (i32.const 0))))\n"));
    for index in 0..plan.programs.len() {
        attempt!(out.append(b"(if (i32.eq (local.get $index) "));
        attempt!(out.i32(attempt!(u32::try_from(index)
            .map_err(|_| crate::Diagnostic::Exhausted))));
        attempt!(out.append(b") (then (return "));
        attempt!(super::get_global(out, index));
        attempt!(out.append(b")))\n"));
    }
    out.append(b"(i32.const 0))\n\
(func (export \"worker_invoke\") (param $handle i32) (param $operand i64) (result i64)\n\
(if (i32.or (i32.eqz (global.get $worker_installed))\n (i32.or (global.get $sp) (global.get $cp)))\n (then (call $fail (i32.const 4)) (return (i64.const 0))))\n\
(if (global.get $failure) (then (return (i64.const 0))))\n\
;; Only the trusted host calls this source module after validating the owner.\n\
(if (i32.ne (call $kind (local.get $handle)) (i32.const 4))\n (then (call $fail (i32.const 4)) (return (i64.const 0))))\n\
(global.set $phase (i32.const 1))\n\
(call $push_i64 (local.get $operand))\n\
(if (global.get $failure) (then (return (i64.const 0))))\n\
(call $enqueue_program (local.get $handle))\n(call $dispatch)\n\
(if (global.get $failure) (then (return (i64.const 0))))\n\
(if (i32.ne (global.get $sp) (i32.const 1))\n (then (call $fail (i32.const 4)) (return (i64.const 0))))\n\
(call $pop_kind (i32.const 1)))\n\
(func (export \"worker_compose\") (param $left i32) (param $right i32) (result i32)\n\
(if (i32.or (i32.eqz (global.get $worker_installed))\n\
 (i32.or (global.get $sp) (global.get $cp)))\n\
 (then (call $fail (i32.const 4)) (return (i32.const 0))))\n\
(if (global.get $failure) (then (return (i32.const 0))))\n\
(call $compose (local.get $left) (local.get $right)))\n")
}
