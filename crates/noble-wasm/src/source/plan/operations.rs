#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; lowering mutates only the unpublished prospective compiler, preparation-owned meter and fresh plan buffers; caller-owned submissions and checked interfaces remain immutable."
)]

#[derive(Clone, Copy)]
pub(super) struct Input<'a> {
    pub(super) submission: &'a noble_kernel::execution::Submission,
    pub(super) body: &'a noble_kernel::execution::Body,
    pub(super) checked: &'a noble_kernel::untrusted::Checked,
    pub(super) arena: &'a super::Arena,
    pub(super) arenas: &'a [super::Arena],
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; compilation reserves the bounded derivation scans before lowering; missing or inconsistent complete interfaces reject with Invalid rather than asserting on accepted producer witnesses."
)]
pub(super) fn derivation(
    checked: &noble_kernel::untrusted::Checked,
    node: noble_kernel::untrusted::NodeId,
) -> Result<&noble_kernel::untrusted::Interface, crate::Diagnostic> {
    let mut index = 0usize;
    let mut found: Option<&noble_kernel::untrusted::Interface> = None;
    let mut failure = None;
    while index < checked.derivations.len() {
        let item = &checked.derivations[index];
        if item.node != node {
            index += 1;
            continue;
        }
        if let Some(previous) = found {
            if previous.stack_in != item.interface.stack_in
                || previous.stack_out != item.interface.stack_out
                || previous.effects != item.interface.effects
            {
                failure = Some(crate::Diagnostic::Invalid);
                break;
            }
        }
        found = Some(&item.interface);
        index += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => match found {
            Some(value) => Ok(value),
            None => Err(crate::Diagnostic::Invalid),
        },
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; admitted sequence entries lower in source order into a fresh reserved operation buffer, and live-pool interning and owned allocation preserve the first shared-budget failure instead of asserting."
)]
fn lower_sequence(
    input: Input<'_>,
    entries: &[noble_kernel::untrusted::NodeId],
    compiler: &mut super::super::Compiler,
    layout: &mut super::Layout,
    work: &mut super::super::Work,
) -> Result<alloc::vec::Vec<super::Operation>, crate::Diagnostic> {
    let mut operations = alloc::vec::Vec::with_capacity(entries.len());
    let mut index = 0usize;
    let mut failure = None;
    while index < entries.len() {
        match lower_node(input, entries[index], compiler, layout, work) {
            Ok(operation) => {
                operations.push(operation);
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
        None => Ok(operations),
    }
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; exact node lowering allocates owned text and interns complete interfaces against the live private pool with shared-budget charges, so it cannot be const; checked lookup and quota failures remain diagnostics."
)]
fn lower_node(
    input: Input<'_>,
    id: noble_kernel::untrusted::NodeId,
    compiler: &mut super::super::Compiler,
    layout: &mut super::Layout,
    work: &mut super::super::Work,
) -> Result<super::Operation, crate::Diagnostic> {
    let interface = attempt!(derivation(input.checked, id));
    let node = attempt!(super::super::admission::node(&input.body.candidate, id));
    let action = match node {
        noble_kernel::untrusted::Node::Literal {
            lit: noble_kernel::untrusted::Lit::I64(value),
            ..
        } => super::Action::I64(*value),
        noble_kernel::untrusted::Node::Literal {
            lit: noble_kernel::untrusted::Lit::Bool(value),
            ..
        } => super::Action::Bool(*value),
        noble_kernel::untrusted::Node::Literal {
            lit: noble_kernel::untrusted::Lit::Unit,
            ..
        } => super::Action::Unit,
        noble_kernel::untrusted::Node::Literal {
            lit: noble_kernel::untrusted::Lit::Text,
            ..
        } => {
            let (address, length) = attempt!(super::allocation::bytes(
                &mut compiler.text_end,
                &mut layout.data,
                attempt!(super::super::admission::text(input.body, id))
            ));
            super::Action::Text(address, length)
        }
        noble_kernel::untrusted::Node::Quotation { .. } => {
            match input.arena.quotes[attempt!(crate::admission::index(id))] {
                Some(program) => super::Action::Program(program),
                None => return Err(crate::Diagnostic::Defective),
            }
        }
        noble_kernel::untrusted::Node::Invocation { def, .. } if def.0 == 8 => {
            attempt!(quote(interface, compiler, work))
        }
        noble_kernel::untrusted::Node::Invocation { def, .. } if def.0 < 24 => {
            super::Action::Word(def.0)
        }
        noble_kernel::untrusted::Node::Invocation { def, .. } => {
            let target = attempt!(super::super::admission::definition_index(
                input.submission,
                *def
            ));
            super::Action::Call(
                input.arenas[target].root,
                input.submission.definitions[target].identity,
            )
        }
    };
    Ok(super::Operation {
        action,
        input: attempt!(compiler.signature(&interface.stack_in, work)),
        output: attempt!(compiler.signature(&interface.stack_out, work)),
        effects: attempt!(super::effect_mask(&interface.effects)),
    })
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; quotation lowering interns the captured witness and exact Program signatures in the allocating prospective pool; missing operands, effects and metering failures return diagnostics instead of assertions."
)]
fn quote(
    interface: &noble_kernel::untrusted::Interface,
    compiler: &mut super::super::Compiler,
    work: &mut super::super::Work,
) -> Result<super::Action, crate::Diagnostic> {
    let captured = match interface.stack_in.last() {
        Some(value) => value,
        None => return Err(crate::Diagnostic::Invalid),
    };
    let witness = attempt!(compiler.signature(core::slice::from_ref(captured), work));
    match interface.stack_out.last() {
        Some(noble_kernel::types::Ty::Program(input, output, effects)) => {
            if !effects.is_empty() {
                return Err(crate::Diagnostic::Invalid);
            }
            Ok(super::Action::Quote(
                attempt!(compiler.signature(input, work)),
                attempt!(compiler.signature(output, work)),
                witness,
            ))
        }
        Some(_) | None => Err(crate::Diagnostic::Invalid),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; allocation fixes each private quotation index before its source-order operation buffer is filled through live-pool interning and owned text allocation; missing mappings and shared-budget failures remain diagnostics."
)]
fn fill_quotation(
    input: Input<'_>,
    index: usize,
    compiler: &mut super::super::Compiler,
    layout: &mut super::Layout,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    if let noble_kernel::untrusted::Node::Quotation { body: entries, .. } =
        &input.body.candidate.nodes[index]
    {
        let program = match input.arena.quotes[index] {
            Some(value) => value,
            None => return Err(crate::Diagnostic::Defective),
        };
        layout.programs[program].operations =
            attempt!(lower_sequence(input, entries, compiler, layout, work));
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; arena allocation fixes root and quotation indices before filling the owned program table; exact source-order lowering propagates missing mapping, unsupported node and metering diagnostics without publishing partial state."
)]
pub(super) fn fill(
    input: Input<'_>,
    compiler: &mut super::super::Compiler,
    layout: &mut super::Layout,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    layout.programs[input.arena.root].operations = attempt!(lower_sequence(
        input,
        &input.body.candidate.body,
        compiler,
        layout,
        work
    ));
    let mut index = 0usize;
    let mut failure = None;
    while index < input.body.candidate.nodes.len() {
        match fill_quotation(input, index, compiler, layout, work) {
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
