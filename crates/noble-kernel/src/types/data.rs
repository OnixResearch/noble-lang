//! Iterative eligibility walk for the fragment's `Data` constraint.

impl super::Ty {
    /// A resource anywhere inside a payload makes the whole value ineligible,
    /// including through `Pair`, `Sum`, and `List` alternatives.
    #[expect(
        tigerstyle::fragile_exhaustive_enum_match,
        reason = "Owner: noble-maintainers; each Ty constructor must explicitly declare whether Data eligibility inspects children, accepts directly or rejects resources; new types must not inherit a fallback eligibility rule."
    )]
    pub fn is_data(&self) -> bool {
        let mut work: alloc::vec::Vec<super::Ty> = alloc::vec::Vec::with_capacity(8);
        work.push(self.clone());
        let mut is_data = true;
        while let Some(node) = work.pop() {
            if work.len() >= super::WORK_CAP {
                is_data = false;
                break;
            }
            match node {
                super::Ty::Resource(_) => {
                    is_data = false;
                    break;
                }
                super::Ty::Pair(left, right) | super::Ty::Sum(left, right) => {
                    work.push(*left);
                    work.push(*right);
                }
                super::Ty::List(item) => work.push(*item),
                super::Ty::Nominal(_, shape) => match *shape {
                    super::NominalShape::Opaque(representation) => work.push(*representation),
                    super::NominalShape::Variant(left, right) => {
                        work.push(*left);
                        work.push(*right);
                    }
                },
                super::Ty::Unit
                | super::Ty::Bool
                | super::Ty::I64
                | super::Ty::Text
                | super::Ty::Syntax
                | super::Ty::Contract
                | super::Ty::Evidence
                | super::Ty::Certified
                | super::Ty::Program(_, _, _) => {}
            }
        }
        is_data
    }
}
