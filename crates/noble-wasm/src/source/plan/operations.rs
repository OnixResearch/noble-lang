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
    pub(super) owner_definition: Option<noble_kernel::contracts::Definition>,
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
    let mut found: Option<usize> = None;
    let mut failure = None;
    while index < checked.derivations.len() {
        match derivation_step(checked, node, found, index) {
            Ok(next) => {
                found = next;
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
        None => match found {
            Some(at) => Ok(&checked.derivations[at].interface),
            None => Err(crate::Diagnostic::Invalid),
        },
    }
}

/// Fold derivation `index` into the last derivation of `node` found before it.
fn derivation_step(
    checked: &noble_kernel::untrusted::Checked,
    node: noble_kernel::untrusted::NodeId,
    found: Option<usize>,
    index: usize,
) -> Result<Option<usize>, crate::Diagnostic> {
    let item = &checked.derivations[index];
    if item.node != node {
        return Ok(found);
    }
    if let Some(previous) = found {
        let previous = &checked.derivations[previous].interface;
        if previous.stack_in != item.interface.stack_in
            || previous.stack_out != item.interface.stack_out
            || previous.effects != item.interface.effects
        {
            return Err(crate::Diagnostic::Invalid);
        }
    }
    Ok(Some(index))
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; the call passes the complete checked input, plan and shared-budget coordinates, and an options struct would only rename the same borrowed fields."
)]
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

// Every branch below either ends the function or calls a helper whose own
// branches end it, so no continuation is shared by two paths: symbolic
// execution never duplicates the code that follows a branch.
#[allow(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; exact node lowering allocates owned text and interns complete interfaces against the live private pool with shared-budget charges, so it cannot be const; checked lookup and quota failures remain diagnostics."
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; the call passes the complete checked input, plan and shared-budget coordinates, and an options struct would only rename the same borrowed fields."
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
    let node = attempt!(super::super::admission::nodes::node(
        &input.body.candidate,
        id
    ));
    let action = attempt!(node_action(
        input,
        program_id,
        id,
        node,
        nominal::Output {
            compiler,
            layout,
            work,
            env: &input.submission.environment,
            interface,
        },
    ));
    let (input_signature, output_signature) =
        attempt!(operation_signatures(compiler, action, interface, work));
    Ok(super::Operation {
        node: id,
        action,
        input: input_signature,
        output: output_signature,
        effects: attempt!(super::effect_mask(&interface.effects)),
    })
}

/// The action one checked node lowers to.
fn node_action(
    input: Input<'_>,
    program_id: usize,
    id: noble_kernel::untrusted::NodeId,
    node: &noble_kernel::untrusted::Node,
    sink: nominal::Output<'_>,
) -> Result<super::Action, crate::Diagnostic> {
    match node {
        noble_kernel::untrusted::Node::Literal {
            lit: noble_kernel::untrusted::Lit::I64(value),
            ..
        } => Ok(super::Action::I64(*value)),
        noble_kernel::untrusted::Node::Literal {
            lit: noble_kernel::untrusted::Lit::Bool(value),
            ..
        } => Ok(super::Action::Bool(*value)),
        noble_kernel::untrusted::Node::Literal {
            lit: noble_kernel::untrusted::Lit::Unit,
            ..
        } => Ok(super::Action::Unit),
        noble_kernel::untrusted::Node::Literal {
            lit: noble_kernel::untrusted::Lit::Text,
            ..
        } => text_action(input, id, sink),
        noble_kernel::untrusted::Node::Quotation { .. } => {
            match input.arena.quotes[attempt!(crate::admission::index(id))] {
                Some(program) => Ok(super::Action::Program(program)),
                None => Err(crate::Diagnostic::Defective),
            }
        }
        noble_kernel::untrusted::Node::Invocation { def, .. } => input.lower_invocation(*def, sink),
        noble_kernel::untrusted::Node::SignedSelf { def, .. } => {
            if input.owner_definition != Some(*def) {
                return Err(crate::Diagnostic::Invalid);
            }
            input.named_call(*def, sink.interface)
        }
        noble_kernel::untrusted::Node::SlotInvoke { site_id, .. } => {
            slot_invoke_action(input, program_id, id, *site_id, sink)
        }
    }
}

/// Allocate a Text literal's bytes and return its address and length.
fn text_action(
    input: Input<'_>,
    id: noble_kernel::untrusted::NodeId,
    sink: nominal::Output<'_>,
) -> Result<super::Action, crate::Diagnostic> {
    let nominal::Output {
        compiler,
        layout,
        work,
        ..
    } = sink;
    let text = attempt!(super::super::admission::nodes::text(input.body, id));
    if compiler.calculator {
        let (address, length) = attempt!(super::allocation::literal(
            compiler,
            &mut layout.data,
            text,
            work
        ));
        return Ok(super::Action::Text(address, length));
    }
    let (address, length) = attempt!(super::allocation::bytes(
        &mut compiler.text_end,
        &mut layout.data,
        text,
        false
    ));
    Ok(super::Action::Text(address, length))
}

/// Record a checked live-slot dispatch site and return its invocation.
fn slot_invoke_action(
    input: Input<'_>,
    program_id: usize,
    id: noble_kernel::untrusted::NodeId,
    site_id: u32,
    sink: nominal::Output<'_>,
) -> Result<super::Action, crate::Diagnostic> {
    let nominal::Output {
        compiler,
        layout,
        work,
        ..
    } = sink;
    if !layout.live_slots || site_id != id.0 {
        return Err(crate::Diagnostic::Invalid);
    }
    let site =
        match crate::scan::find_first(&input.checked.live_sites, |site| site.site_id == site_id) {
            Some(site) => site,
            None => return Err(crate::Diagnostic::Invalid),
        };
    let ordinal = attempt!(super::number(layout.live_sites.len()));
    let input_signature = attempt!(compiler.signature(&site.stack_in, work));
    let output_signature = attempt!(compiler.signature(&site.stack_out, work));
    let effect_mask = attempt!(super::effect_mask(&site.allowed_effects));
    let forwarded_target_positions = attempt!(live_ref_positions(&site.stack_in));
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
    Ok(super::Action::SlotInvoke(ordinal, site.ref_ordinal))
}

/// The stack signatures an operation records. Literal and quotation-push atoms
/// never store an operation interface: the root/child Program already records
/// its complete typed signatures, and executable invocation atoms still retain
/// both exact stack keys.
fn operation_signatures(
    compiler: &mut super::super::Compiler,
    action: super::Action,
    interface: &noble_kernel::untrusted::Interface,
    work: &mut super::super::Work,
) -> Result<(u32, u32), crate::Diagnostic> {
    if is_unrecorded_atom(compiler.calculator, action) {
        return Ok((0, 0));
    }
    let input = attempt!(compiler.signature(&interface.stack_in, work));
    let output = attempt!(compiler.signature(&interface.stack_out, work));
    Ok((input, output))
}

/// Whether a calculator atom records no operation interface.
fn is_unrecorded_atom(is_calculator: bool, action: super::Action) -> bool {
    is_calculator && !action.records_interface()
}

/// The positions of the live references in a dispatch site's input stack.
fn live_ref_positions(
    stack_in: &[noble_kernel::types::Ty],
) -> Result<alloc::vec::Vec<u32>, crate::Diagnostic> {
    let mut positions = alloc::vec::Vec::with_capacity(stack_in.len());
    let mut position = 0usize;
    let mut failure = None;
    while position < stack_in.len() {
        if matches!(
            stack_in[position],
            noble_kernel::types::Ty::LiveRef(_, _, _)
        ) {
            match super::number(position) {
                Ok(number) => positions.push(number),
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        position += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(positions),
    }
}

impl Input<'_> {
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; private invocation dispatch reads the dynamic definition environment and may intern quotation signatures or nominal witnesses into the mutable prospective compiler, so it cannot be const."
    )]
    fn lower_invocation(
        self,
        def: noble_kernel::contracts::Definition,
        sink: nominal::Output<'_>,
    ) -> Result<super::Action, crate::Diagnostic> {
        if def.0 == 8 {
            return quotation::lower(sink.interface, sink.compiler, sink.work);
        }
        if let Some(action) = builtin_action(&self.submission.environment, def, self.owner) {
            return action;
        }
        self.defined_action(def, sink)
    }

    /// The action of a definition the environment declares by behaviour.
    fn defined_action(
        self,
        def: noble_kernel::contracts::Definition,
        sink: nominal::Output<'_>,
    ) -> Result<super::Action, crate::Diagnostic> {
        let Some(kind) = self.submission.environment.kind(def) else {
            return Err(crate::Diagnostic::Invalid);
        };
        if is_named(kind) {
            return self.named_call(def, sink.interface);
        }
        sink.behavior_action(kind)
    }

    fn named_call(
        self,
        def: noble_kernel::contracts::Definition,
        site: &noble_kernel::untrusted::Interface,
    ) -> Result<super::Action, crate::Diagnostic> {
        let target = attempt!(super::super::admission::definition_index(
            self.submission,
            def
        ));
        // A generic signed owner may have multiple independently checked
        // specializations. This call can dispatch only to the precise
        // concrete ABI compiled for this target, even when another
        // instantiation of its rank-1 Scheme is well typed by the kernel.
        let expected = &self.submission.definitions[target].expected;
        if site.stack_in != expected.stack_in
            || site.stack_out != expected.stack_out
            || site.effects != expected.allowed_effects
        {
            return Err(crate::Diagnostic::Invalid);
        }
        Ok(super::Action::Call(
            self.arenas[target].root,
            self.submission.definitions[target].identity,
        ))
    }
}

// Invocation dispatch tries each profile's reserved rows in a fixed order.
// Every stage is its own function whose tests all end it, so no path ever
// rejoins another and no dispatch code follows a branch twice.

/// Whether a declared behaviour is a named definition call.
fn is_named(kind: noble_kernel::contracts::Behavior) -> bool {
    matches!(kind, noble_kernel::contracts::Behavior::Named)
}

/// The action of a builtin word or profile-reserved row, or `None` when the
/// environment's declared behaviour decides the invocation.
fn builtin_action(
    env: &noble_kernel::contracts::Env,
    def: noble_kernel::contracts::Definition,
    owner: Option<u64>,
) -> Option<Result<super::Action, crate::Diagnostic>> {
    if let Some(action) = cursor_action(env, def) {
        return Some(action);
    }
    if let Some(action) = calculator_action(env, def) {
        return Some(Ok(action));
    }
    if let Some(action) = checked_i64_action(env, def) {
        return Some(Ok(action));
    }
    live_action(env, def, owner)
}

/// Standalone cursor and calculator reserve 22..25 and share text.byte.
fn cursor_action(
    env: &noble_kernel::contracts::Env,
    def: noble_kernel::contracts::Definition,
) -> Option<Result<super::Action, crate::Diagnostic>> {
    if !is_cursor_profile(env) {
        return None;
    }
    if (22..=25).contains(&def.0) {
        return Some(Err(crate::Diagnostic::Invalid));
    }
    if def.0 == 26 {
        return Some(text_byte_action(env, def));
    }
    // The combined declared-text profile alone owns 27 as signed `<`;
    // the calculator's 27 is its own separately admitted iterate row.
    if is_declared_less_row(env, def) {
        return Some(declared_less_action(env, def));
    }
    None
}

/// Whether the standalone cursor or calculator profile reserves its rows.
fn is_cursor_profile(env: &noble_kernel::contracts::Env) -> bool {
    env.text_cursor || env.calculator
}

/// Row 26 is text.byte exactly when the environment declares it so.
fn text_byte_action(
    env: &noble_kernel::contracts::Env,
    def: noble_kernel::contracts::Definition,
) -> Result<super::Action, crate::Diagnostic> {
    if env.kind(def) == Some(noble_kernel::contracts::Behavior::TextByte) {
        Ok(super::Action::TextByte)
    } else {
        Err(crate::Diagnostic::Invalid)
    }
}

/// Whether row 27 is the declared-text profile's signed `<`.
fn is_declared_less_row(
    env: &noble_kernel::contracts::Env,
    def: noble_kernel::contracts::Definition,
) -> bool {
    def.0 == 27 && !env.calculator
}

/// Row 27 is signed `<` exactly when declared modules declare it so.
fn declared_less_action(
    env: &noble_kernel::contracts::Env,
    def: noble_kernel::contracts::Definition,
) -> Result<super::Action, crate::Diagnostic> {
    if env.declared_modules && env.kind(def) == Some(noble_kernel::contracts::Behavior::I64Less) {
        Ok(super::Action::I64Less)
    } else {
        Err(crate::Diagnostic::Invalid)
    }
}

/// The calculator's own separately admitted rows 27..31.
fn calculator_action(
    env: &noble_kernel::contracts::Env,
    def: noble_kernel::contracts::Definition,
) -> Option<super::Action> {
    if !env.calculator {
        return None;
    }
    match def.0 {
        27 => Some(super::Action::CalculatorIterate),
        28 => Some(super::Action::CalculatorCompare),
        29 => Some(super::Action::CalculatorDivRem),
        30 => Some(super::Action::CalculatorTextFromAscii),
        31 => Some(super::Action::CalculatorTextLength),
        _ => None,
    }
}

/// The checked-I64 profile's rows 23..29.
fn checked_i64_action(
    env: &noble_kernel::contracts::Env,
    def: noble_kernel::contracts::Definition,
) -> Option<super::Action> {
    if !env.has_checked_i64() {
        return None;
    }
    match def.0 {
        23 => Some(super::Action::CheckedI64Add),
        24 => Some(super::Action::CheckedI64Sub),
        25 => Some(super::Action::CheckedI64Mul),
        26 => Some(super::Action::CheckedI64Neg),
        27 => Some(super::Action::CheckedI64Abs),
        28 => Some(super::Action::CheckedI64Less),
        29 => Some(super::Action::CheckedI64DivRem),
        _ => None,
    }
}

/// Core live sessions' two checked operations at 24/25, then the fixed builtin
/// prefix; `None` past the prefix.
fn live_action(
    env: &noble_kernel::contracts::Env,
    def: noble_kernel::contracts::Definition,
    owner: Option<u64>,
) -> Option<Result<super::Action, crate::Diagnostic>> {
    let builtins = super::super::builtin_count(env);
    if is_live_row(def, 24, builtins) {
        return Some(propose_action(owner));
    }
    if is_live_row(def, 25, builtins) {
        return Some(Ok(super::Action::LiveGeneration));
    }
    if def.0 < builtins {
        return Some(Ok(super::Action::Word(def.0)));
    }
    None
}

/// Whether `def` is live row `row` of a core live session's 26-word prefix.
fn is_live_row(def: noble_kernel::contracts::Definition, row: u32, builtins: u32) -> bool {
    def.0 == row && builtins == 26
}

/// A live proposal needs the owner the session was opened for.
fn propose_action(owner: Option<u64>) -> Result<super::Action, crate::Diagnostic> {
    match owner {
        Some(owner) => Ok(super::Action::LivePropose(owner)),
        None => Err(crate::Diagnostic::Invalid),
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
        layout.programs[program].operations = attempt!(lower_sequence(
            input, program, entries, compiler, layout, work
        ));
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
