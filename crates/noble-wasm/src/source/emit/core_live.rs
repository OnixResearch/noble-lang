//! Explicit Core-live module-global ownership, separate from live-slot roots.

pub(super) fn release_module_roots(
    out: &mut crate::output::Buffer,
    programs: usize,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(
        b"(func (export \"release_module_roots\") (result i32)\n\
(if (i32.or (global.get $failure) (call $core_live_busy))\
 (then (return (i32.const 4))))\n\
(if (i32.eqz (global.get $core_live_module_installed))\
 (then (return (i32.const 4))))\n"
    ));
    attempt!(out.append(
        b"(if (global.get $core_live_adapter_leases)\
 (then (return (i32.const 4))))\n"
    ));
    // Validate the complete immutable module-global set before touching any
    // shared root byte; a wrong/old handle cannot release a partial module.
    for id in 0..programs {
        attempt!(out.append(b"(if (i32.eqz (call $core_live_module_rooted "));
        attempt!(super::get_global(out, id));
        attempt!(out.append(b")) (then (return (i32.const 4))))\n"));
    }
    for id in 0..programs {
        attempt!(out.append(b"(call $core_live_unroot_module "));
        attempt!(super::get_global(out, id));
        attempt!(out.append(b")\n"));
    }
    out.append(b"(global.set $core_live_module_installed (i32.const 0))\n(i32.const 0))\n")
}
