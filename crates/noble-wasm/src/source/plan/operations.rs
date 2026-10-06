#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; lowering mutates only the unpublished prospective compiler, preparation-owned meter and fresh plan buffers; caller-owned submissions and checked interfaces remain immutable."
)]

mod nominal;
mod quotation;

#[derive(Clone, Copy)]
pub(super) struct Input<'a> {
    pub(super) submission: &'a noble_kernel::execution::Submission,
    pub(super) body: &'a noble_kernel::execution::Body,
    pub(super) checked: &'a noble_kernel::untrusted::Checked,
    pub(super) arena: &'a super::Arena,
    pub(super) arenas: &'a [super::Arena],
    pub(super) owner: Option<u64>,
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
    program_id: usize,
    entries: &[noble_kernel::untrusted::NodeId],
    compiler: &mut super::super::Compiler,
    layout: &mut super::Layout,
    work: &mut super::super::Work,
) -> Result<alloc::vec::Vec<super::Operation>, crate::Diagnostic> {
    let mut operations = alloc::vec::Vec::with_capacity(entries.len());
    let mut index = 0usize;
    let mut failure = None;
    while index < entries.len() {
        match lower_node(input, program_id, entries[index], compiler, layout, work) {
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
    program_id: usize,
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
        noble_kernel::untrusted::Node::Invocation { def, .. } => {
            attempt!(input.lower_invocation(
                *def,
                nominal::Output {
                    compiler,
                    layout,
                    work,
                    env: &input.submission.environment,
                    interface,
                },
            ))
        }
        noble_kernel::untrusted::Node::SlotInvoke { site_id, .. } => {
            if !layout.live_slots || *site_id != id.0 {
                return Err(crate::Diagnostic::Invalid);
            }
            let site = match input.checked.live_sites.iter().find(|site| site.site_id == *site_id) {
                Some(site) => site,
                None => return Err(crate::Diagnostic::Invalid),
            };
            let ordinal = attempt!(super::number(layout.live_sites.len()));
            let input_signature = attempt!(compiler.signature(&site.stack_in, work));
            let output_signature = attempt!(compiler.signature(&site.stack_out, work));
            let effect_mask = attempt!(super::effect_mask(&site.allowed_effects));
            let mut forwarded_target_positions = alloc::vec::Vec::new();
            for (position, ty) in site.stack_in.iter().enumerate() {
                if matches!(ty, noble_kernel::types::Ty::LiveRef(_, _, _)) {
                    forwarded_target_positions.push(attempt!(super::number(position)));
                }
            }
            if forwarded_target_positions.len() != site.forwarded_ref_ordinals.len() {
                return Err(crate::Diagnostic::Invalid);
            }
            layout.live_sites.push(crate::source::LiveSiteMetadata {
                site_id: ordinal,
                caller_program_index: program_id,
                source_node: id,
                selected_ref_logical_position: site.root_logical_input_position,
                forwarded_source_positions: site.forwarded_ref_ordinals.clone(),
                forwarded_target_positions,
                target_input: site.stack_in.clone(),
                target_output: site.stack_out.clone(),
                effect_ceiling: site.allowed_effects.clone(),
                input_signature,
                output_signature,
                effect_mask,
            });
            super::Action::SlotInvoke(ordinal, site.ref_ordinal)
        }
    };
    Ok(super::Operation {
        node: id,
        action,
        input: attempt!(compiler.signature(&interface.stack_in, work)),
        output: attempt!(compiler.signature(&interface.stack_out, work)),
        effects: attempt!(super::effect_mask(&interface.effects)),
    })
}

impl Input<'_> {
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; private invocation dispatch reads the dynamic definition environment and may intern quotation signatures or nominal witnesses into the mutable prospective compiler, so it cannot be const."
    )]
    fn lower_invocation(
        self,
        def: noble_kernel::contracts::Definition,
        output: nominal::Output<'_>,
    ) -> Result<super::Action, crate::Diagnostic> {
        if def.0 == 8 {
            return quotation::lower(output.interface, output.compiler, output.work);
        }
        if self.submission.environment.text_cursor {
            if (22..=25).contains(&def.0) {
                return Err(crate::Diagnostic::Invalid);
            }
            if def.0 == 26 {
                return if self.submission.environment.kind(def)
                    == Some(noble_kernel::contracts::Behavior::TextByte)
                {
                    Ok(super::Action::TextByte)
                } else {
                    Err(crate::Diagnostic::Invalid)
                };
            }
        }
        if def.0 == 24 && super::super::builtin_count(&self.submission.environment) == 26 {
            return match self.owner {
                Some(owner) => Ok(super::Action::LivePropose(owner)),
                None => Err(crate::Diagnostic::Invalid),
            };
        }
        if def.0 == 25 && super::super::builtin_count(&self.submission.environment) == 26 {
            return Ok(super::Action::LiveGeneration);
        }
        if def.0 < super::super::builtin_count(&self.submission.environment) {
            return Ok(super::Action::Word(def.0));
        }
        match self.submission.environment.kind(def) {
            Some(noble_kernel::contracts::Behavior::Named) => self.named_call(def),
            Some(kind) => match kind {
                noble_kernel::contracts::Behavior::BoundEmit(slot) => output.bound_action(slot),
                noble_kernel::contracts::Behavior::BoundClock(slot) =>
                    output.clock_action(slot),
                noble_kernel::contracts::Behavior::NominalNew(id) => output.new_action(id),
                noble_kernel::contracts::Behavior::NominalInto(id) => {
                    let ty = attempt!(nominal::Output::checked_input(
                        output.env,
                        output.interface,
                        id,
                        1,
                    ));
                    output.into_action(id, ty)
                }
                noble_kernel::contracts::Behavior::NominalLeft(id) => {
                    output.variant_action(true, id)
                }
                noble_kernel::contracts::Behavior::NominalRight(id) => {
                    output.variant_action(false, id)
                }
                noble_kernel::contracts::Behavior::NominalMatch(id) => {
                    let ty = attempt!(nominal::Output::checked_input(
                        output.env,
                        output.interface,
                        id,
                        3,
                    ));
                    output.match_action(id, ty)
                }
                noble_kernel::contracts::Behavior::GenericLeft(id) => {
                    output.generic_variant_action(true, id)
                }
                noble_kernel::contracts::Behavior::GenericRight(id) => {
                    output.generic_variant_action(false, id)
                }
                noble_kernel::contracts::Behavior::GenericMatch(id) => {
                    output.generic_match_action(id)
                }
                noble_kernel::contracts::Behavior::Dup
                | noble_kernel::contracts::Behavior::Drop
                | noble_kernel::contracts::Behavior::Swap
                | noble_kernel::contracts::Behavior::Dip
                | noble_kernel::contracts::Behavior::Arith
                | noble_kernel::contracts::Behavior::Equals
                | noble_kernel::contracts::Behavior::Quote
                | noble_kernel::contracts::Behavior::Compose
                | noble_kernel::contracts::Behavior::Run
                | noble_kernel::contracts::Behavior::Reflect
                | noble_kernel::contracts::Behavior::Unit
                | noble_kernel::contracts::Behavior::Pair
                | noble_kernel::contracts::Behavior::Unpair
                | noble_kernel::contracts::Behavior::Inl
                | noble_kernel::contracts::Behavior::Inr
                | noble_kernel::contracts::Behavior::Case
                | noble_kernel::contracts::Behavior::If
                | noble_kernel::contracts::Behavior::Nil
                | noble_kernel::contracts::Behavior::Cons
                | noble_kernel::contracts::Behavior::ListCase
                | noble_kernel::contracts::Behavior::TestEmit
                | noble_kernel::contracts::Behavior::Reserved
                | noble_kernel::contracts::Behavior::TextByte
                | noble_kernel::contracts::Behavior::Named => Err(crate::Diagnostic::Invalid),
            },
            None => Err(crate::Diagnostic::Invalid),
        }
    }

    fn named_call(
        self,
        def: noble_kernel::contracts::Definition,
    ) -> Result<super::Action, crate::Diagnostic> {
        let target = attempt!(super::super::admission::definition_index(
            self.submission,
            def
        ));
        Ok(super::Action::Call(
            self.arenas[target].root,
            self.submission.definitions[target].identity,
        ))
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
            attempt!(lower_sequence(input, program, entries, compiler, layout, work));
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
        input.arena.root,
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
