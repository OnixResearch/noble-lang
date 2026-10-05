#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; lowering mutates only the unpublished prospective compiler, preparation-owned meter and fresh plan buffers; caller-owned submissions and checked interfaces remain immutable."
)]

mod allocation;
mod operations;
mod topology;

#[derive(Clone, Copy)]
pub(super) enum Action {
    I64(i64),
    Bool(bool),
    Unit,
    Text(u32, u32),
    Program(usize),
    Word(u32),
    TextByte,
    Quote(u32, u32, u32),
    Call(usize, u64),
    NominalNew(u64, u32, u32),
    NominalInto(u64, u32, u32),
    NominalLeft(u64, u32, u32),
    NominalRight(u64, u32, u32),
    NominalMatch(u64, u32, u32, u32, u32),
    EmitBound(u32),
    ClockBound(u32),
    LivePropose(u64),
    LiveGeneration,
    SlotInvoke(u32, u32),
}

pub(super) struct Operation {
    pub(super) action: Action,
    pub(super) input: u32,
    pub(super) output: u32,
    pub(super) effects: u32,
}

pub(super) struct Program {
    pub(super) entry: u32,
    pub(super) input: u32,
    pub(super) output: u32,
    pub(super) effects: u32,
    /// Moved into published metadata only in the opt-in profile.
    pub(super) logical: Option<ProgramTypes>,
    pub(super) operations: alloc::vec::Vec<Operation>,
    pub(super) depth: u32,
    pub(super) leaves: u32,
}

pub(super) struct ProgramTypes {
    pub(super) stack_in: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub(super) stack_out: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub(super) effects: noble_kernel::types::EffSet,
}

pub(super) struct Layout {
    pub(super) programs: alloc::vec::Vec<Program>,
    /// Accepted definition index to its emitted, immutable Program global.
    /// The submission root may be a wrapper invocation, not the proved value.
    pub(super) definition_roots: alloc::vec::Vec<usize>,
    pub(super) order: alloc::vec::Vec<usize>,
    pub(super) first_function: u32,
    pub(super) functions: u32,
    pub(super) root: usize,
    pub(super) data: alloc::vec::Vec<(u32, alloc::vec::Vec<u8>)>,
    pub(super) descriptors: alloc::vec::Vec<(u32, u32)>,
    pub(super) types: super::types::Registry,
    pub(super) input_types: alloc::vec::Vec<u32>,
    pub(super) output_types: alloc::vec::Vec<u32>,
    pub(super) borrowed_input_positions: alloc::vec::Vec<u32>,
    pub(super) text_witness: u32,
    pub(super) declared_modules: bool,
    pub(super) live: bool,
    pub(super) live_slots: bool,
    pub(super) live_sites: alloc::vec::Vec<super::LiveSiteMetadata>,
    pub(super) has_nominals: bool,
    pub(super) has_core_emit: bool,
    pub(super) has_core_abort: bool,
    pub(super) has_text_byte: bool,
    pub(super) has_bound_emit: bool,
    pub(super) has_bound_clock: bool,
    pub(super) has_live_propose: bool,
    pub(super) has_live_generation: bool,
}

struct Arena {
    root: usize,
    quotes: alloc::vec::Vec<Option<usize>>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the bounded effect set is checked exhaustively against the admitted test and live effect identities, with Unsupported for any other identity instead of assertions on producer data."
)]
pub(super) fn effect_mask(effects: &noble_kernel::types::EffSet) -> Result<u32, crate::Diagnostic> {
    let mut mask = 0u32;
    let mut index = 0usize;
    let mut failure = None;
    while index < effects.as_slice().len() {
        match effects.as_slice()[index].0 {
            0 => mask |= 1,
            1 => mask |= 2,
            2 => mask |= 4,
            3 => mask |= 8,
            5 => mask |= 32,
            4 => mask |= 16,
            _ => {
                failure = Some(crate::Diagnostic::Unsupported);
                break;
            }
        }
        index += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(mask),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; admission constructs one checked body per definition and root, and this owned table reserves those bodies in the same order; metered signature interning failures propagate without asserting on input."
)]
fn start(
    submission: &noble_kernel::execution::Submission,
    checked: &super::admission::Accepted,
    compiler: &mut super::Compiler,
    work: &mut super::Work,
) -> Result<(Layout, alloc::vec::Vec<Arena>), crate::Diagnostic> {
    let body_count = submission.definitions.len().saturating_add(1);
    let mut programs = alloc::vec::Vec::with_capacity(body_count);
    let mut arenas = alloc::vec::Vec::with_capacity(body_count);
    let mut index = 0usize;
    let mut failure = None;
    while index < submission.definitions.len() {
        match allocation::arena(
            &submission.definitions[index].body,
            &checked.definitions[index],
            &mut programs,
            compiler,
            work,
        ) {
            Ok(arena) => {
                arenas.push(arena);
                index += 1;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    arenas.push(attempt!(allocation::arena(
        &submission.body,
        &checked.root,
        &mut programs,
        compiler,
        work
    )));
    let definition_roots = if submission.environment.live_slots {
        arenas[..index].iter().map(|arena| arena.root).collect()
    } else {
        alloc::vec::Vec::new()
    };
    let layout = Layout {
        programs,
        definition_roots,
        order: alloc::vec::Vec::new(),
        first_function: compiler.functions,
        functions: compiler.functions,
        root: arenas[index].root,
        data: alloc::vec::Vec::new(),
        descriptors: alloc::vec::Vec::new(),
        types: super::types::Registry::new(),
        input_types: alloc::vec::Vec::new(),
        output_types: alloc::vec::Vec::new(),
        borrowed_input_positions: alloc::vec::Vec::new(),
        text_witness: 0,
        declared_modules: submission.environment.declared_modules,
        live: submission.environment.effects.contains(&noble_kernel::types::EffId(3)),
        live_slots: submission.environment.live_slots,
        live_sites: alloc::vec::Vec::new(),
        has_nominals: !submission.environment.nominals.is_empty()
            || !submission.environment.generic_variants.is_empty(),
        has_core_emit: false,
        has_core_abort: false,
        has_text_byte: false,
        has_bound_emit: false,
        has_bound_clock: false,
        has_live_propose: false,
        has_live_generation: false,
    };
    Ok((layout, arenas))
}

// Every lowered program is emitted, including named definitions and quotation
// bodies that the root does not invoke yet. Only executable operations grant
// host imports; an interface's effect set or an unused binding does not.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the complete private program table is traversed after checked lowering, and forbidden ambient operations reject before any module is emitted."
)]
fn host_footprint(layout: &mut Layout) -> Result<(), crate::Diagnostic> {
    for program in &layout.programs {
        for operation in &program.operations {
            match operation.action {
                Action::Word(22 | 23) if layout.declared_modules || layout.live => {
                    return Err(crate::Diagnostic::Invalid);
                }
                Action::Word(22) => layout.has_core_emit = true,
                Action::Word(23) => layout.has_core_abort = true,
                Action::TextByte => layout.has_text_byte = true,
                Action::EmitBound(_) => layout.has_bound_emit = true,
                Action::ClockBound(_) => layout.has_bound_clock = true,
                Action::LivePropose(_) => layout.has_live_propose = true,
                Action::LiveGeneration => layout.has_live_generation = true,
                _ => {}
            }
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; start creates aligned private arenas for every admitted body, then lowering, topology and bounded allocation propagate diagnostics; no incomplete plan or prospective compiler is published."
)]
pub(super) fn lower(
    submission: &noble_kernel::execution::Submission,
    checked: &super::admission::Accepted,
    compiler: &mut super::Compiler,
    work: &mut super::Work,
) -> Result<Layout, crate::Diagnostic> {
    let (mut layout, arenas) = attempt!(start(submission, checked, compiler, work));
    let mut index = 0usize;
    let mut failure = None;
    while index < submission.definitions.len() {
        let input = operations::Input {
            submission,
            body: &submission.definitions[index].body,
            checked: &checked.definitions[index],
            arena: &arenas[index],
            arenas: &arenas,
            owner: Some(submission.definitions[index].identity),
        };
        match operations::fill(input, compiler, &mut layout, work) {
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
    let input = operations::Input {
        submission,
        body: &submission.body,
        checked: &checked.root,
        arena: &arenas[index],
        arenas: &arenas,
        owner: None,
    };
    attempt!(operations::fill(input, compiler, &mut layout, work));
    attempt!(host_footprint(&mut layout));
    attempt!(topology::order(&mut layout));
    attempt!(allocation::finish(
        &mut layout,
        compiler,
        &checked.root,
        work
    ));
    Ok(layout)
}

pub(super) fn number(value: usize) -> Result<u32, crate::Diagnostic> {
    match u32::try_from(value) {
        Ok(value) => Ok(value),
        Err(_) => Err(crate::Diagnostic::Exhausted),
    }
}
