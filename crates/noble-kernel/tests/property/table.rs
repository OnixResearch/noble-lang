//! The oracle's contract table and binding accessors (DX-PROPERTY-01).
//!
//! One arm per word of the documented table in [verification/m2-fragment.md]
//! "Environment contracts": stack variables join as prefixes, value
//! variables fill single positions, and effect variables carry latent
//! bounds. `None` rejects — the kernel's instantiation-kind, arity, and
//! unknown-word paths.

#[path = "table/branches.rs"]
mod branches;
#[path = "table/data.rs"]
mod data;
#[path = "table/primitives.rs"]
mod primitives;
#[path = "table/programs.rs"]
mod programs;

/// One node's derived interface under the oracle's rules.
pub struct Interface {
    pub input: std::vec::Vec<super::otypes::Type>,
    pub output: std::vec::Vec<super::otypes::Type>,
    pub latent: std::vec::Vec<u32>,
}

/// The stack bound to one stack variable, if the binding has that kind.
pub fn stack_at(
    inst: &noble_kernel::words::Inst,
    index: u32,
) -> Option<std::vec::Vec<super::otypes::Type>> {
    let index = usize::try_from(index).ok()?;
    match inst.bindings.get(index) {
        Some(noble_kernel::words::Binding::Stack(segment)) => {
            Some(super::otypes::oty_stack(segment))
        }
        Some(noble_kernel::words::Binding::Value(_))
        | Some(noble_kernel::words::Binding::Effect(_))
        | Some(noble_kernel::words::Binding::Ref(_))
        | None => None,
    }
}

/// The type bound to one value variable, if the binding has that kind.
pub fn value_at(inst: &noble_kernel::words::Inst, index: u32) -> Option<super::otypes::Type> {
    let index = usize::try_from(index).ok()?;
    match inst.bindings.get(index) {
        Some(noble_kernel::words::Binding::Value(ty)) => Some(super::otypes::oty(ty)),
        Some(noble_kernel::words::Binding::Stack(_))
        | Some(noble_kernel::words::Binding::Effect(_))
        | Some(noble_kernel::words::Binding::Ref(_))
        | None => None,
    }
}

/// The identities bound to one effect variable, if the binding has that kind.
pub fn effects_at(inst: &noble_kernel::words::Inst, index: u32) -> Option<std::vec::Vec<u32>> {
    let index = usize::try_from(index).ok()?;
    match inst.bindings.get(index) {
        Some(noble_kernel::words::Binding::Effect(set)) => {
            Some(set.as_slice().iter().map(|id| id.0).collect())
        }
        Some(noble_kernel::words::Binding::Stack(_))
        | Some(noble_kernel::words::Binding::Value(_))
        | Some(noble_kernel::words::Binding::Ref(_))
        | None => None,
    }
}

/// Append one tail to a copied stack.
pub fn append(
    mut stack: std::vec::Vec<super::otypes::Type>,
    tail: &[super::otypes::Type],
) -> std::vec::Vec<super::otypes::Type> {
    stack.extend_from_slice(tail);
    stack
}

/// Whether the word places a `Data` side condition on its value variable.
pub fn requires_data(def: u32) -> bool {
    def == 0 || def == 1 || def == 8
}

/// Whether the witness carries exactly the wanted number of bindings.
pub fn exact_arity(inst: &noble_kernel::words::Inst, wanted: u32) -> Option<()> {
    let wanted = usize::try_from(wanted).ok()?;
    if inst.bindings.len() == wanted {
        Some(())
    } else {
        None
    }
}

/// The documented contract table, one arm per word; `None` rejects.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; word_face rejects unknown definitions and wrong witness arities through Option before independent per-word decoding; the dedicated pool-coverage control detects missing arms."
)]
pub fn word_face(def: u32, inst: &noble_kernel::words::Inst) -> Option<Interface> {
    // The documented table fixes each word's variable count; a witness with
    // missing or extra bindings cannot instantiate it.
    exact_arity(
        inst,
        match def {
            0 | 1 | 19 | 20 => 2,
            2 | 8 | 10 | 13 | 14 | 15 | 16 => 3,
            3 | 11 | 18 => 4,
            4..=7 | 12 | 22 | 23 => 1,
            9 | 17 => 6,
            21 => 5,
            _ => return None,
        },
    )?;
    match def {
        0 => primitives::duplicate(inst),
        1 => primitives::discard(inst),
        2 => primitives::swap(inst),
        3 => programs::dip(inst),
        4..=6 => primitives::integer_result(inst, super::otypes::Type::I64),
        7 => primitives::integer_result(inst, super::otypes::Type::Bool),
        8 => programs::quote(inst),
        9 => programs::compose(inst),
        10 => programs::apply(inst),
        11 => programs::reflect(inst),
        12 => primitives::unit(inst),
        13 => data::pair(inst),
        14 => data::unpair(inst),
        15 => data::left(inst),
        16 => data::right(inst),
        17 => branches::sum_cases(inst),
        18 => branches::select(inst),
        19 => data::empty_list(inst),
        20 => data::prepend(inst),
        21 => branches::list_cases(inst),
        22 => primitives::print(inst),
        23 => primitives::resource(inst),
        _ => None,
    }
}
