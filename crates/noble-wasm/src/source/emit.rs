#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; emission mutates only the fresh private output sink; checked plans and caller inputs remain immutable and no partial output escapes preparation."
)]

mod core_live;
mod functions;
mod imports;
mod initialize;
mod live;
mod metadata;
mod reflection;
mod worker;

pub(super) fn runtime_bridge(
    descriptors: &[(u32, u32)],
    declared_modules: bool,
) -> Result<alloc::vec::Vec<u8>, crate::Diagnostic> {
    let mut out = crate::output::Buffer::new(32_768);
    attempt!(imports::bridge(&mut out));
    attempt!(imports::bridge_runtime(&mut out, declared_modules));
    attempt!(metadata::descriptors(&mut out, descriptors, None));
    attempt!(metadata::signature_count(&mut out, descriptors.len()));
    attempt!(out.append(
        b"(export \"core_empty\" (func $empty_entry))\n\
(export \"core_quote\" (func $quote_entry))\n\
(export \"core_compose\" (func $compose_entry))\n\
(export \"core_restore\" (func $restore_entry))\n)\n",
    ));
    Ok(out.finish())
}

fn global(out: &mut crate::output::Buffer, id: usize) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"$s"));
    out.index(id)
}

fn get_global(out: &mut crate::output::Buffer, id: usize) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(global.get "));
    attempt!(global(out, id));
    out.append(b")")
}

fn programs(
    out: &mut crate::output::Buffer,
    programs: &[super::plan::Program],
    calculator: bool,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut failure = None;
    while index < programs.len() {
        match program(out, index, &programs[index], calculator) {
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

fn program(
    out: &mut crate::output::Buffer,
    index: usize,
    program: &super::plan::Program,
    calculator: bool,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(global "));
    attempt!(global(out, index));
    attempt!(out.append(b" (mut i32) (i32.const 0))\n"));
    functions::write(out, program, calculator)
}

fn code_elements(
    out: &mut crate::output::Buffer,
    plan: &super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(elem $code func"));
    let mut function = plan.first_function;
    let mut failure = None;
    while function < plan.functions {
        match function_element(out, function) {
            Ok(()) => function += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => out.append(b")\n"),
    }
}

fn function_element(
    out: &mut crate::output::Buffer,
    function: u32,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b" $f"));
    out.number(u64::from(function))
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-wasm-maintainers; the selected lowering or preflight stage keeps its checked step order in one reviewable unit, and splitting it would separate the emitted sequence from its bounds checks."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the admitted layout bounds every function, segment and type descriptor before emission; the private sink propagates output exhaustion and returns bytes only after all fragments succeed, without asserting on input."
)]
pub(super) fn module(
    plan: &super::plan::Layout,
    generation: u32,
    signature_base: usize,
    signature_keys: &[alloc::vec::Vec<u8>],
) -> Result<alloc::vec::Vec<u8>, crate::Diagnostic> {
    // The calculator's bounded multi-definition closure may serialize above
    // the ordinary one-MiB module cap. Keep that cap on all other profiles
    // and on individual structural signature keys.
    let cap = if plan.calculator {
        4 * crate::OUTPUT_BYTE_LIMIT
    } else {
        crate::OUTPUT_BYTE_LIMIT
    };
    let mut out = crate::output::Buffer::with_limit(32_768, cap);
    attempt!(imports::write(&mut out, plan));
    attempt!(imports::entry::runtime(&mut out, plan));
    attempt!(plan.types.emit(&mut out, plan.calculator));
    attempt!(programs(&mut out, &plan.programs, plan.calculator));
    if generation == 0 {
        if plan.calculator {
            attempt!(out.append(
                b"(elem $core func $empty_entry $quote_entry $compose_entry $restore_entry $iterate_resume)\n"
            ));
        } else {
            attempt!(out.append(
                b"(elem $core func $empty_entry $quote_entry $compose_entry $restore_entry)\n"
            ));
        }
    }
    if plan.functions > plan.first_function {
        attempt!(code_elements(&mut out, plan));
    }
    let mut index = 0usize;
    let mut failure = None;
    while index < plan.data.len() {
        match metadata::data_segment(&mut out, index, &plan.data[index].1) {
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
    // Ordinary profiles emit their complete descriptor table. A calculator
    // module carries only IDs `signature_base..` interned since the last
    // published (committed) compiler state; the trusted host retains earlier
    // IDs, so this metadata does not grow with the session's history.
    if (!plan.calculator && signature_base != 0)
        || signature_base.checked_add(signature_keys.len()) != Some(plan.descriptors.len())
    {
        return Err(crate::Diagnostic::Defective);
    }
    if plan.calculator {
        for (offset, key) in signature_keys.iter().enumerate() {
            attempt!(metadata::signature_segment(
                &mut out,
                signature_base + offset,
                key
            ));
        }
    }
    attempt!(metadata::descriptors(
        &mut out,
        &plan.descriptors,
        if plan.calculator {
            Some((signature_base, signature_keys))
        } else {
            None
        },
    ));
    if plan.core_live {
        // Core live shares one signature catalogue with its runtime bridge.
        attempt!(metadata::signature_count(&mut out, plan.descriptors.len()));
    }
    attempt!(initialize::write(&mut out, plan, generation));
    attempt!(metadata::stack_check(
        &mut out,
        b"$check_input",
        &plan.input_types
    ));
    attempt!(metadata::stack_check(
        &mut out,
        b"$check_output",
        &plan.output_types
    ));
    if plan.live_slots {
        attempt!(live::sites(&mut out, &plan.live_sites));
        attempt!(live::root_check(&mut out, &plan.borrowed_input_positions));
        attempt!(live::program_accessors(&mut out, plan.programs.len()));
        attempt!(live::exports(&mut out, plan, generation));
        attempt!(out.append(b")\n"));
        return Ok(out.finish());
    }
    if plan.worker_inputs {
        attempt!(worker::exports(&mut out, plan, generation));
    }
    if plan.core_live {
        attempt!(core_live::release_module_roots(
            &mut out,
            plan.programs.len()
        ));
    }
    if plan.declared_text {
        attempt!(out.append(b"(func (export \"module_program_count\") (result i32) "));
        attempt!(out.i32(attempt!(
            u32::try_from(plan.programs.len()).map_err(|_| crate::Diagnostic::Exhausted)
        )));
        attempt!(out.append(
            b")\n(func (export \"module_program_handle\") (param $index i32) (result i32)\n"
        ));
        for index in 0..plan.programs.len() {
            attempt!(out.append(b"(if (i32.eq (local.get $index) "));
            attempt!(out.i32(attempt!(
                u32::try_from(index).map_err(|_| crate::Diagnostic::Exhausted)
            )));
            attempt!(out.append(b") (then (return "));
            attempt!(get_global(&mut out, index));
            attempt!(out.append(b")))\n"));
        }
        attempt!(out
            .append(b"(i32.const 0))\n(func (export \"release_module_programs\") (result i32)\n"));
        for index in 0..plan.programs.len() {
            attempt!(out.append(b"(global.set "));
            attempt!(global(&mut out, index));
            attempt!(out.append(b" (i32.const 0))\n"));
        }
        // An idle signed/Text worker may retire its source epoch only with no
        // failure, operand, continuation, recipe or observed Program root.
        attempt!(out.append(
            b"(i32.const 0))\n\
(func (export \"retire_source_epoch\") (result i32)\n\
 (if (i32.or (global.get $failure) (global.get $sp)) (then (return (i32.const 4))))\n\
 (if (i32.or (global.get $cp) (global.get $rp)) (then (return (i32.const 4))))\n\
 (if (global.get $observed_program) (then (return (i32.const 4))))\n\
 (call $rollback (i32.const 0))\n\
 (global.set $heap_baseline (i32.const 0))\n\
 (i32.const 0))\n"
        ));
    }
    attempt!(out.append(b"(func (export \"submit\") (result i32)\n(if (global.get $failure) (then (return (global.get $failure))))\n(if (i32.or (global.get $cp) (i32.ne (global.get $generation) "));
    // A calculator module does not bake its preparation generation into its
    // bytes: the trusted engine binds the exact compiler generation that the
    // host validated this module against as an immutable per-instance import,
    // so byte-identical modules can be reused across preparations while the
    // guard still refuses any arena whose submission count differs.
    if plan.calculator {
        attempt!(out.append(b"(global.get $submit_generation)"));
    } else {
        attempt!(out.i32(generation));
    }
    attempt!(out.append(b")) (then (call $fail (i32.const 4)) (return (global.get $failure))))\n"));
    // A calculator module is entered exactly once. Reset after host injection,
    // immediately before validating its first input, even if the host observed
    // the previous stack (and changed reflection_steps) between begin/submit.
    if plan.calculator {
        attempt!(out.append(b"(call $type_certificate_reset)\n"));
    }
    attempt!(out.append(b"(call $check_input)\n(if (global.get $failure) (then (return (global.get $failure))))\n(global.set $phase (i32.const 0))\n(call $initialize)\n(if (global.get $failure) (then (return (global.get $failure))))\n(global.set $phase (i32.const 1))\n(call $enqueue_program "));
    attempt!(get_global(&mut out, plan.root));
    attempt!(out.append(b")\n(call $dispatch)\n(if (i32.eqz (global.get $failure)) (then (call $check_output)))\n(if (i32.eqz (global.get $failure)) (then\n (global.set $generation (i32.add (global.get $generation) (i32.const 1)))\n"));
    if plan.core_live {
        attempt!(out.append(b" (global.set $core_live_module_installed (i32.const 1))\n (i32.store (i32.const 61440) (i32.const 0))\n"));
    }
    attempt!(out.append(b"))\n(global.get $failure))\n)\n"));
    Ok(out.finish())
}
