#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; emission mutates only the fresh private output sink; checked plans and caller inputs remain immutable and no partial output escapes preparation."
)]

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; every closed Action variant must select its exact immutable recipe atom rather than a fallback; writing the allocating bounded sink cannot be const and output failures propagate diagnostics."
)]
fn atom(
    out: &mut crate::output::Buffer,
    operation: &super::super::plan::Operation,
    witness: u32,
) -> Result<(), crate::Diagnostic> {
    let atom = super::super::plan::atom_metadata(operation, witness);
    if let Some(text) = atom.text {
        return text_atom(out, text, witness);
    }
    attempt!(out.append(b"(local.set $atom (call $new (i32.const 8) "));
    attempt!(out.i64(atom.payload));
    attempt!(out.append(b" "));
    match atom.child_program {
        Some(child) => attempt!(super::get_global(out, child)),
        None => attempt!(out.i32(0)),
    }
    attempt!(out.append(b" (i32.const 0) (i32.const 0) "));
    attempt!(out.i32(atom.tag));
    attempt!(out.append(b" "));
    attempt!(out.i32(atom.input));
    attempt!(out.append(b" "));
    attempt!(out.i32(atom.output));
    attempt!(out.append(b" "));
    attempt!(out.i32(atom.effects));
    attempt!(out.append(b" "));
    attempt!(out.i32(atom.ordinal));
    out.append(b"))\n")
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the checked layout contains bounded table/data spans and topologically ordered programs; initialization emits runtime quota checks in source order while sink exhaustion returns a diagnostic, not an assertion."
)]
pub(super) fn write(
    out: &mut crate::output::Buffer,
    plan: &super::super::plan::Layout,
    generation: u32,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func $initialize (local $recipe i32) (local $atom i32)\n"));
    if plan.live_slots && generation == 0 {
        attempt!(
            out.append(b"(if (i32.eqz (global.get $heap_cursor)) (then (call $storage_init)))\n")
        );
    }
    if generation == 0 {
        if plan.calculator {
            attempt!(out.append(b"(table.init $core (i32.const 0) (i32.const 0) (i32.const 5))\n"));
        } else {
            attempt!(out.append(b"(table.init $core (i32.const 0) (i32.const 0) (i32.const 4))\n"));
        }
    }
    if plan.functions > plan.first_function {
        attempt!(out.append(b"(table.init $code "));
        attempt!(out.i32(plan.first_function));
        attempt!(out.append(b" (i32.const 0) "));
        attempt!(out.i32(plan.functions.saturating_sub(plan.first_function)));
        attempt!(out.append(b")\n"));
    }
    let mut segment = 0usize;
    let mut failure = None;
    while segment < plan.data.len() {
        match memory_segment(out, plan, segment) {
            Ok(()) => segment += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    let mut at = 0usize;
    let mut failure = None;
    while at < plan.order.len() {
        match program(out, plan, plan.order[at]) {
            Ok(()) => at += 1,
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

fn memory_segment(
    out: &mut crate::output::Buffer,
    plan: &super::super::plan::Layout,
    segment: usize,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(if (i32.eqz (call $runtime_tick)) (then (return)))\n(memory.init $d"));
    attempt!(out.index(segment));
    attempt!(out.append(b" "));
    attempt!(out.i32(plan.data[segment].0));
    attempt!(out.append(b" (i32.const 0) "));
    attempt!(out.i32(attempt!(super::super::plan::number(
        plan.data[segment].1.len()
    ))));
    out.append(b")\n")
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the admitted program is topologically ordered and bounded before emission; recipe quota checks precede construction and the private sink propagates exhaustion rather than asserting on input."
)]
fn program(
    out: &mut crate::output::Buffer,
    plan: &super::super::plan::Layout,
    id: usize,
) -> Result<(), crate::Diagnostic> {
    let program = &plan.programs[id];
    attempt!(out.append(b"(if (i32.eqz (call $runtime_tick)) (then (return)))\n"));
    // Charge semantic size before any recipe construction for this body.
    attempt!(out.append(b"(if (i32.gt_u "));
    attempt!(out.i32(program.leaves));
    attempt!(
        out.append(b" (global.get $recipe_limit)) (then (call $quota (i32.const 2)) (return)))\n")
    );
    attempt!(out.append(b"(if (i32.gt_u "));
    attempt!(out.i32(program.depth));
    attempt!(out.append(b" (global.get $depth_limit)) (then (call $quota (i32.const 3)) (return)))\n(local.set $recipe (i32.const 0))\n"));
    let mut index = 0usize;
    let mut failure = None;
    while index < program.operations.len() {
        match operation(out, &program.operations[index], plan.text_witness) {
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
    attempt!(out.append(b"(global.set "));
    attempt!(super::global(out, id));
    attempt!(out.append(b" (call $program "));
    attempt!(out.i32(program.entry));
    attempt!(out.append(b" "));
    attempt!(out.i32(program.input));
    attempt!(out.append(b" "));
    attempt!(out.i32(program.output));
    attempt!(out.append(b" (local.get $recipe) "));
    attempt!(out.i64(i64::from(program.effects)));
    attempt!(out.append(b" (i32.const 0) (i32.const 0) "));
    attempt!(out.i32(program.depth));
    attempt!(out.append(b" "));
    attempt!(out.i32(program.leaves));
    attempt!(out.append(b"))\n(if (global.get $failure) (then (return)))\n"));
    if plan.live_slots {
        // The module's immutable global is a real backend owner, independent
        // of registry reachability and separately saved Program cells. Root
        // every program, including quotation/named bodies, before publication.
        attempt!(out.append(b"(if (call $storage_root_program "));
        attempt!(super::get_global(out, id));
        attempt!(out.append(b") (then (call $fail (i32.const 4)) (return)))\n"));
    }
    if plan.core_live {
        // Core's append-only heap has distinct host and per-module root bytes.
        // A quotation child stays owned after its wrapper has left the stack.
        attempt!(out.append(b"(if (call $core_live_module_root "));
        attempt!(super::get_global(out, id));
        attempt!(out.append(b") (then (call $fail (i32.const 4)) (return)))\n"));
    }
    Ok(())
}

fn operation(
    out: &mut crate::output::Buffer,
    operation: &super::super::plan::Operation,
    witness: u32,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(if (i32.eqz (call $runtime_tick)) (then (return)))\n"));
    attempt!(atom(out, operation, witness));
    out.append(b"(if (global.get $failure) (then (return)))\n(local.set $recipe (call $concat_recipe (local.get $recipe) (local.get $atom)))\n(if (global.get $failure) (then (return)))\n")
}

fn text_atom(
    out: &mut crate::output::Buffer,
    text: (u32, u32),
    witness: u32,
) -> Result<(), crate::Diagnostic> {
    let (address_bytes, length_bytes) = text;
    attempt!(out.append(b"(local.set $atom (call $new (i32.const 8) (i64.const 0) (call $text "));
    attempt!(out.i32(address_bytes));
    attempt!(out.append(b" "));
    attempt!(out.i32(length_bytes));
    attempt!(out.append(b") (i32.const 0) (i32.const 0) (i32.const 8) "));
    attempt!(out.i32(witness));
    out.append(b" (i32.const 0) (i32.const 0) (i32.const 0)))\n")
}
