#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; lowering mutates only the unpublished prospective compiler, preparation-owned meter and fresh plan buffers; caller-owned submissions and checked interfaces remain immutable."
)]

fn signature(
    compiler: &mut super::super::Compiler,
    input: &[noble_kernel::types::Ty],
    output: &[noble_kernel::types::Ty],
    effects: &noble_kernel::types::EffSet,
    work: &mut super::super::Work,
) -> Result<super::Program, crate::Diagnostic> {
    Ok(super::Program {
        entry: 0,
        input: attempt!(compiler.signature(input, work)),
        output: attempt!(compiler.signature(output, work)),
        effects: attempt!(super::effect_mask(effects)),
        operations: alloc::vec::Vec::new(),
        depth: 0,
        leaves: 0,
    })
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; quotation allocation resolves admitted derivations and interns both signatures against the allocating live private pool with shared-budget charges; missing or non-Program interfaces return diagnostics."
)]
fn quotation(
    checked: &noble_kernel::untrusted::Checked,
    index: usize,
    compiler: &mut super::super::Compiler,
    work: &mut super::super::Work,
) -> Result<super::Program, crate::Diagnostic> {
    let interface = attempt!(super::operations::derivation(
        checked,
        noble_kernel::untrusted::NodeId(attempt!(super::number(index)))
    ));
    match interface.stack_out.last() {
        Some(noble_kernel::types::Ty::Program(input, output, effects)) => {
            signature(compiler, input, output, effects, work)
        }
        Some(_) | None => Err(crate::Diagnostic::Invalid),
    }
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; admission bounds quotation count by arena node count and this compiler-private program table must grow after reservation, requiring Vec; missing derivations and non-Program quotation interfaces return diagnostics instead of assertions."
)]
pub(super) fn arena(
    body: &noble_kernel::execution::Body,
    checked: &noble_kernel::untrusted::Checked,
    programs: &mut alloc::vec::Vec<super::Program>,
    compiler: &mut super::super::Compiler,
    work: &mut super::super::Work,
) -> Result<super::Arena, crate::Diagnostic> {
    programs.reserve(body.candidate.nodes.len().saturating_add(1));
    let root = programs.len();
    programs.push(attempt!(signature(
        compiler,
        &checked.interface.stack_in,
        &checked.interface.stack_out,
        &checked.interface.effects,
        work
    )));
    let mut quotes = alloc::vec![None; body.candidate.nodes.len()];
    let mut index = 0usize;
    let mut failure = None;
    while index < body.candidate.nodes.len() {
        if let noble_kernel::untrusted::Node::Quotation { .. } = &body.candidate.nodes[index] {
            match quotation(checked, index, compiler, work) {
                Ok(program) => {
                    quotes[index] = Some(programs.len());
                    programs.push(program);
                }
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        index += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(super::Arena { root, quotes }),
    }
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; each checked byte allocation appends one owned segment to the compiler-private data buffer after reserving capacity; a slice cannot express that growth."
)]
pub(super) fn bytes(
    text_end: &mut u32,
    data: &mut alloc::vec::Vec<(u32, alloc::vec::Vec<u8>)>,
    bytes: &[u8],
) -> Result<(u32, u32), crate::Diagnostic> {
    let length_bytes = attempt!(super::number(bytes.len()));
    let address_bytes = *text_end;
    let end_bytes = match address_bytes.checked_add(length_bytes) {
        Some(value) => value,
        None => return Err(crate::Diagnostic::Exhausted),
    };
    if end_bytes > super::super::MEMORY_BYTES {
        return Err(crate::Diagnostic::Exhausted);
    }
    *text_end = end_bytes;
    data.reserve(1);
    data.push((address_bytes, bytes.to_vec()));
    Ok((address_bytes, length_bytes))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; function allocation uses non-const checked usize-to-u32 conversion before reserving the owned program's table span; overflow and table exhaustion remain diagnostics."
)]
fn entry(program: &mut super::Program, functions: &mut u32) -> Result<(), crate::Diagnostic> {
    let count = attempt!(super::number(program.operations.len()));
    program.entry = if count == 0 { 0 } else { *functions };
    *functions = match functions.checked_add(count) {
        Some(value) => value,
        None => return Err(crate::Diagnostic::Exhausted),
    };
    if *functions > super::super::TABLE_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; checked function additions and byte allocations enforce table and memory limits while actual signature and type interning charges the shared meter; only the unpublished compiler and layout grow on success."
)]
pub(super) fn finish(
    layout: &mut super::Layout,
    compiler: &mut super::super::Compiler,
    checked: &noble_kernel::untrusted::Checked,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut failure = None;
    while index < layout.programs.len() {
        match entry(&mut layout.programs[index], &mut compiler.functions) {
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
    layout.functions = compiler.functions;
    layout.input_types = attempt!(layout
        .types
        .stack(&checked.interface.stack_in, compiler, work));
    layout.output_types =
        attempt!(layout
            .types
            .stack(&checked.interface.stack_out, compiler, work));
    layout.text_witness = attempt!(compiler.signature(&[noble_kernel::types::Ty::Text], work));
    index = compiler.descriptors.len();
    while index < compiler.signatures.keys.len() {
        match bytes(
            &mut compiler.text_end,
            &mut layout.data,
            &compiler.signatures.keys[index],
        ) {
            Ok(descriptor) => {
                compiler.descriptors.push(descriptor);
                index += 1;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => {
            layout.descriptors = compiler.descriptors.clone();
            Ok(())
        }
    }
}
