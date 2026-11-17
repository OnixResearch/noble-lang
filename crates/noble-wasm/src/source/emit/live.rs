//! Opt-in checked dispatch site and borrowed-root emission.

fn site_name(out: &mut crate::output::Buffer, id: u32) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"$slot_site_"));
    out.number(u64::from(id))
}

pub(super) fn sites(
    out: &mut crate::output::Buffer,
    sites: &[super::super::LiveSiteMetadata],
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut failure = None;
    while index < sites.len() {
        match write_site(out, &sites[index]) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

fn write_site(
    out: &mut crate::output::Buffer,
    site: &super::super::LiveSiteMetadata,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func "));
    attempt!(site_name(out, site.site_id));
    attempt!(out.append(b" (local $handle i32) (local $ordinal i32) (local $frame i32)\n"));
    attempt!(out.append(b"(local.set $ordinal (call $live_frame_get "));
    attempt!(out.i32(site.selected_ref_logical_position));
    attempt!(out.append(b"))\n(if (global.get $failure) (then (return)))\n"));
    attempt!(out.append(b"(global.set $failure (i32.const 5))\n(local.set $handle (call $host_live_select (local.get $ordinal) "));
    attempt!(out.i32(site.site_id));
    attempt!(out.append(b"))\n(if (i32.eqz (local.get $handle)) (then (global.set $failure (i32.const 4)) (return)))\n(global.set $failure (i32.const 0))\n"));
    // The host has already checked selected-version provenance. Recheck the
    // actual preadmitted Program cell before it enters the Wasm trampoline.
    attempt!(out.append(b"(if (i32.eqz (call $is_live (local.get $handle))) (then (call $fail (i32.const 4)) (return)))\n"));
    attempt!(out.append(b"(if (i32.ne (call $kind (local.get $handle)) (i32.const 4)) (then (call $fail (i32.const 4)) (return)))\n"));
    attempt!(out.append(b"(if (i32.or (i32.ne (call $y (local.get $handle)) "));
    attempt!(out.i32(site.input_signature));
    attempt!(out.append(b") (i32.ne (call $z (local.get $handle)) "));
    attempt!(out.i32(site.output_signature));
    attempt!(out.append(b")) (then (call $fail (i32.const 3)) (return)))\n"));
    attempt!(out.append(b"(if (i64.ne (i64.and (call $payload (local.get $handle)) "));
    attempt!(out.i64(!i64::from(site.effect_mask)));
    attempt!(out.append(b") (i64.const 0)) (then (call $fail (i32.const 3)) (return)))\n"));
    attempt!(out.append(b"(local.set $frame (call $live_frame_new "));
    attempt!(out.i32(attempt!(u32::try_from(
        site.forwarded_source_positions.len()
    )
    .map_err(|_| crate::Diagnostic::Exhausted))));
    attempt!(out.append(b" (local.get $handle)))\n(if (global.get $failure) (then (return)))\n"));
    attempt!(frame_binds(
        out,
        &site.forwarded_source_positions,
        &site.forwarded_target_positions
    ));
    attempt!(out.append(b"(call $live_frame_register (local.get $frame) "));
    attempt!(out.i32(site.site_id));
    out.append(b")\n(if (global.get $failure) (then (return)))\n(call $enqueue_program (local.get $handle)))\n")
}

/// Bind each forwarded source position to its target slot in the site's new
/// frame, pairwise and in order, up to the shorter of the two lists.
fn frame_binds(
    out: &mut crate::output::Buffer,
    sources: &[u32],
    targets: &[u32],
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut failure = None;
    while index < sources.len() && index < targets.len() {
        match frame_bind(out, index, sources[index], targets[index]) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

/// Bind forwarded source position `source` to frame slot `index` at `target`.
fn frame_bind(
    out: &mut crate::output::Buffer,
    index: usize,
    origin: u32,
    target: u32,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call $live_frame_bind (local.get $frame) "));
    attempt!(out.i32(attempt!(
        u32::try_from(index).map_err(|_| crate::Diagnostic::Exhausted)
    )));
    attempt!(out.append(b" "));
    attempt!(out.i32(target));
    attempt!(out.append(b" (call $live_frame_get "));
    attempt!(out.i32(origin));
    out.append(b"))\n(if (global.get $failure) (then (return)))\n")
}

pub(super) fn root_check(
    out: &mut crate::output::Buffer,
    positions: &[u32],
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func $check_borrows\n(if (i32.ne (call $live_root_count) "));
    attempt!(out.i32(attempt!(
        u32::try_from(positions.len()).map_err(|_| crate::Diagnostic::Exhausted)
    )));
    attempt!(out.append(b") (then (call $fail (i32.const 3)) (return)))\n"));
    let mut index = 0usize;
    let mut failure = None;
    while index < positions.len() {
        match root_position_check(out, index, positions[index]) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    out.append(b")\n")
}

/// Check that borrowed root `index` arrived at logical input `position`.
fn root_position_check(
    out: &mut crate::output::Buffer,
    index: usize,
    position: u32,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(if (i32.ne (call $live_root_position "));
    attempt!(out.i32(attempt!(
        u32::try_from(index).map_err(|_| crate::Diagnostic::Exhausted)
    )));
    attempt!(out.append(b") "));
    attempt!(out.i32(position));
    out.append(b") (then (call $fail (i32.const 3)) (return)))\n")
}

pub(super) fn root_frame_for(
    out: &mut crate::output::Buffer,
    root: usize,
    positions: &[u32],
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(local.set $root_frame (call $live_frame_new "));
    attempt!(out.i32(attempt!(
        u32::try_from(positions.len()).map_err(|_| crate::Diagnostic::Exhausted)
    )));
    attempt!(out.append(b" "));
    attempt!(super::get_global(out, root));
    attempt!(out.append(b"))\n(if (global.get $failure) (then (return (global.get $failure))))\n"));
    let mut index = 0usize;
    let mut failure = None;
    while index < positions.len() {
        match root_frame_bind(out, index, positions[index]) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    out.append(b"(call $live_frame_register (local.get $root_frame) (i32.const -1))\n(if (global.get $failure) (then (return (global.get $failure))))\n")
}

/// Bind borrowed root `index` to the root frame slot at logical input
/// `position`.
fn root_frame_bind(
    out: &mut crate::output::Buffer,
    index: usize,
    position: u32,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call $live_frame_bind (local.get $root_frame) "));
    attempt!(out.i32(attempt!(
        u32::try_from(index).map_err(|_| crate::Diagnostic::Exhausted)
    )));
    attempt!(out.append(b" "));
    attempt!(out.i32(position));
    attempt!(out.append(b" (call $live_root_ordinal "));
    attempt!(out.i32(attempt!(
        u32::try_from(index).map_err(|_| crate::Diagnostic::Exhausted)
    )));
    out.append(b"))\n(if (global.get $failure) (then (return (global.get $failure))))\n")
}

pub(super) fn program_accessors(
    out: &mut crate::output::Buffer,
    programs: usize,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func (export \"module_program_count\") (result i32) "));
    attempt!(out.i32(attempt!(
        u32::try_from(programs).map_err(|_| crate::Diagnostic::Exhausted)
    )));
    attempt!(out.append(b")\n(func (export \"module_program_handle\") (param $index i32) (result i32)\n(if (i32.eqz (global.get $live_module_installed)) (then (return (i32.const 0))))\n"));
    attempt!(super::program_handle_cases(out, programs));
    out.append(b"(i32.const 0))\n")
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-wasm-maintainers; the selected lowering or preflight stage keeps its checked step order in one reviewable unit, and splitting it would separate the emitted sequence from its bounds checks."
)]
pub(super) fn exports(
    out: &mut crate::output::Buffer,
    plan: &super::super::plan::Layout,
    generation: u32,
) -> Result<(), crate::Diagnostic> {
    // Installing a target constructs checked immutable code only. It does
    // not enqueue an entry or execute the candidate body.
    attempt!(out.append(
        b"(func (export \"install_target\") (result i32)\n\
(if (global.get $failure) (then (return (i32.const 0))))\n\
(if (i32.or (global.get $cp) (i32.or (global.get $sp) (global.get $live_module_installed)))\
 (then (call $fail (i32.const 4)) (return (i32.const 0))))\n\
(if (i32.ne (global.get $generation) "
    ));
    attempt!(out.i32(generation));
    attempt!(out.append(
        b") (then (call $fail (i32.const 4)) (return (i32.const 0))))\n\
(global.set $phase (i32.const 0))\n\
(call $initialize)\n\
(if (global.get $failure) (then (return (i32.const 0))))\n\
(global.set $live_module_installed (i32.const 1))\n\
(global.set $generation (i32.add (global.get $generation) (i32.const 1)))\n"
    ));
    attempt!(super::get_global(out, plan.root));
    attempt!(out.append(b")\n"));

    // A saved generic caller is reusable with a newly issued root borrow set
    // and newly pinned registry map on every independent invocation.
    attempt!(out.append(b"(func $invoke_live_internal (export \"invoke_live\") (result i32) (local $root_frame i32)\n\
(if (global.get $failure) (then (return (global.get $failure))))\n\
(if (i32.or (global.get $cp) (i32.eqz (global.get $live_module_installed)))\
 (then (call $fail (i32.const 4)) (return (global.get $failure))))\n\
(call $check_input)\n\
(if (global.get $failure) (then (return (global.get $failure))))\n\
(call $check_borrows)\n\
(if (global.get $failure) (then (return (global.get $failure))))\n\
(global.set $phase (i32.const 1))\n"));
    attempt!(root_frame_for(
        out,
        plan.root,
        &plan.borrowed_input_positions
    ));
    attempt!(out.append(b"(call $enqueue_program "));
    attempt!(super::get_global(out, plan.root));
    attempt!(out.append(
        b")\n(call $dispatch)\n\
(call $live_frame_restore (i32.const 0))\n\
(if (i32.eqz (global.get $failure)) (then (call $check_output)))\n\
(global.get $failure))\n"
    ));

    // Preserve the one-shot submit boundary for a newly prepared module,
    // while the host may instead install once and call invoke_live repeatedly.
    attempt!(out.append(
        b"(func (export \"submit\") (result i32)\n\
(if (global.get $failure) (then (return (global.get $failure))))\n\
(if (i32.or (global.get $cp) (global.get $live_module_installed))\
 (then (call $fail (i32.const 4)) (return (global.get $failure))))\n\
(call $check_input)\n\
(if (global.get $failure) (then (return (global.get $failure))))\n\
(call $check_borrows)\n\
(if (global.get $failure) (then (return (global.get $failure))))\n\
(if (i32.ne (global.get $generation) "
    ));
    attempt!(out.i32(generation));
    attempt!(out.append(
        b") (then (call $fail (i32.const 4)) (return (global.get $failure))))\n\
(global.set $phase (i32.const 0))\n(call $initialize)\n\
(if (global.get $failure) (then (return (global.get $failure))))\n\
(global.set $live_module_installed (i32.const 1))\n\
(global.set $generation (i32.add (global.get $generation) (i32.const 1)))\n\
(call $invoke_live_internal))\n"
    ));
    // Releasing the module's own immutable Program globals is distinct from
    // retiring its table span: a saved exact cell or map/frame owner may still
    // keep that code reachable. The host reads code_owners before table.clear.
    attempt!(out.append(
        b"(func (export \"release_module_roots\") (result i32)\n\
(if (i32.or (i32.or (global.get $cp) (global.get $sp))\
 (i32.or (global.get $rp) (global.get $live_frame)))\
 (then (return (i32.const 4))))\n\
(if (i32.eqz (global.get $live_module_installed))\
 (then (return (i32.const 4))))\n"
    ));
    attempt!(super::program_global_lines(
        out,
        plan.programs.len(),
        b"(if (call $storage_unroot_program ",
        b") (then (return (i32.const 4))))\n",
    ));
    out.append(b"(global.set $live_module_installed (i32.const 0))\n(call $storage_collect))\n")
}
