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
    Quote(u32, u32, u32),
    Call(usize, u64),
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
    pub(super) operations: alloc::vec::Vec<Operation>,
    pub(super) depth: u32,
    pub(super) leaves: u32,
}

pub(super) struct Layout {
    pub(super) programs: alloc::vec::Vec<Program>,
    pub(super) order: alloc::vec::Vec<usize>,
    pub(super) first_function: u32,
    pub(super) functions: u32,
    pub(super) root: usize,
    pub(super) data: alloc::vec::Vec<(u32, alloc::vec::Vec<u8>)>,
    pub(super) descriptors: alloc::vec::Vec<(u32, u32)>,
    pub(super) types: super::types::Registry,
    pub(super) input_types: alloc::vec::Vec<u32>,
    pub(super) output_types: alloc::vec::Vec<u32>,
    pub(super) text_witness: u32,
}

struct Arena {
    root: usize,
    quotes: alloc::vec::Vec<Option<usize>>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the bounded effect set is checked exhaustively against the two admitted host identities, with Unsupported for any other identity instead of assertions on producer data."
)]
pub(super) fn effect_mask(effects: &noble_kernel::types::EffSet) -> Result<u32, crate::Diagnostic> {
    let mut mask = 0u32;
    let mut index = 0usize;
    let mut failure = None;
    while index < effects.as_slice().len() {
        match effects.as_slice()[index].0 {
            0 => mask |= 1,
            1 => mask |= 2,
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
    let layout = Layout {
        programs,
        order: alloc::vec::Vec::new(),
        first_function: compiler.functions,
        functions: compiler.functions,
        root: arenas[index].root,
        data: alloc::vec::Vec::new(),
        descriptors: alloc::vec::Vec::new(),
        types: super::types::Registry::new(),
        input_types: alloc::vec::Vec::new(),
        output_types: alloc::vec::Vec::new(),
        text_witness: 0,
    };
    Ok((layout, arenas))
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
    };
    attempt!(operations::fill(input, compiler, &mut layout, work));
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
