impl super::materialize::State {
    pub(super) fn finish(
        mut self,
        term: super::Term,
        effects: noble_kernel::types::EffSet,
        generics: &[noble_kernel::contracts::GenericVariantDecl],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<Self, crate::Diagnostic> {
        let b = attempt!(self.pop(span));
        let material = match term {
            super::Term::List(_) => list(b, span),
            super::Term::Pair(_, _) | super::Term::Sum(_, _) => {
                let a = attempt!(self.pop(span));
                pair(matches!(term, super::Term::Pair(_, _)), a, b, span)
            }
            super::Term::Program(_, _) => {
                let a = attempt!(self.pop(span));
                program(a, b, effects, span)
            }
            super::Term::GenericNominal(id, _, _) => {
                let a = attempt!(self.pop(span));
                generic_nominal(id, a, b, generics, span, meter)
            }
            super::Term::Push(_, _) => {
                let a = attempt!(self.pop(span));
                push(a, b, span)
            }
            super::Term::Hole(_)
            | super::Term::RigidValue(_)
            | super::Term::RigidStack(_)
            | super::Term::Link(_)
            | super::Term::Unit
            | super::Term::Bool
            | super::Term::I64
            | super::Term::Text
            | super::Term::Syntax
            | super::Term::Contract
            | super::Term::Evidence
            | super::Term::Certified
            | super::Term::Resource(_)
            | super::Term::Nominal(_)
            | super::Term::Empty => Err(crate::internal(span)),
        };
        self.values.push(attempt!(material));
        Ok(self)
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameter list is the internal call shape; bundling it would change the extracted subject"
)]
/// Build one generic instance after both of its limits are known to hold.
///
/// The instance owns its two ordered arguments *and* a canonical descriptor
/// holding a second copy of the selected payload arguments. The descriptor
/// copies are charged to the work meter and the resulting owned node count is
/// checked against `TYPE_NODE_CAP` before anything is cloned, so nesting
/// cannot double host allocation per level beneath a linear logical size.
fn generic_nominal(
    id: noble_kernel::types::NominalTypeId,
    first: super::materialize::Material,
    second: super::materialize::Material,
    generics: &[noble_kernel::contracts::GenericVariantDecl],
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<super::materialize::Material, crate::Diagnostic> {
    let Some(declaration) = crate::scan::find_first(generics, |decl| decl.id == id) else {
        return Err(crate::internal(span));
    };
    let (
        super::materialize::Material::Value(a, a_extent),
        super::materialize::Material::Value(b, b_extent),
    ) = (first, second)
    else {
        return Err(crate::internal(span));
    };
    let extents = [a_extent, b_extent];
    let (Some(left), Some(right)) = (
        extents.get(usize::from(declaration.payload_params[0])),
        extents.get(usize::from(declaration.payload_params[1])),
    ) else {
        return Err(crate::internal(span));
    };
    let constructors = attempt!(type_size(
        a_extent
            .constructors
            .saturating_add(b_extent.constructors)
            .saturating_add(1),
        span
    ));
    let copied = attempt!(type_nodes(left.nodes, right.nodes, span));
    let retained = attempt!(type_nodes(a_extent.nodes, b_extent.nodes, span));
    let nodes = attempt!(type_nodes(
        attempt!(type_nodes(retained, copied, span)),
        1,
        span
    ));
    attempt!(meter.charge(copied, span));
    let arguments = [a, b];
    let shape = noble_kernel::types::NominalShape::Variant(
        alloc::boxed::Box::new(arguments[usize::from(declaration.payload_params[0])].clone()),
        alloc::boxed::Box::new(arguments[usize::from(declaration.payload_params[1])].clone()),
    );
    Ok(super::materialize::Material::Value(
        noble_kernel::types::Ty::GenericNominal(
            id,
            alloc::boxed::Box::new(arguments),
            alloc::boxed::Box::new(shape),
        ),
        super::materialize::Extent {
            constructors,
            nodes,
        },
    ))
}

fn list(
    item: super::materialize::Material,
    span: crate::Span,
) -> Result<super::materialize::Material, crate::Diagnostic> {
    match item {
        super::materialize::Material::Value(ty, extent) => Ok(super::materialize::Material::Value(
            noble_kernel::types::Ty::List(alloc::boxed::Box::new(ty)),
            super::materialize::Extent {
                constructors: attempt!(type_size(extent.constructors.saturating_add(1), span)),
                nodes: attempt!(type_nodes(extent.nodes, 1, span)),
            },
        )),
        super::materialize::Material::Stack(_, _) => Err(crate::internal(span)),
    }
}

fn pair(
    is_pair: bool,
    a: super::materialize::Material,
    b: super::materialize::Material,
    span: crate::Span,
) -> Result<super::materialize::Material, crate::Diagnostic> {
    match (a, b) {
        (
            super::materialize::Material::Value(a, sa),
            super::materialize::Material::Value(b, sb),
        ) => {
            let extent = attempt!(joined(sa, sb, span));
            let ty = if is_pair {
                noble_kernel::types::Ty::Pair(alloc::boxed::Box::new(a), alloc::boxed::Box::new(b))
            } else {
                noble_kernel::types::Ty::Sum(alloc::boxed::Box::new(a), alloc::boxed::Box::new(b))
            };
            Ok(super::materialize::Material::Value(ty, extent))
        }
        (super::materialize::Material::Stack(_, _), _)
        | (super::materialize::Material::Value(_, _), super::materialize::Material::Stack(_, _)) => {
            Err(crate::internal(span))
        }
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; Ty::program owns its allocated input/output stacks and constructor-limit errors allocate diagnostics."
)]
fn program(
    inputs: super::materialize::Material,
    outputs: super::materialize::Material,
    effects: noble_kernel::types::EffSet,
    span: crate::Span,
) -> Result<super::materialize::Material, crate::Diagnostic> {
    match (inputs, outputs) {
        (
            super::materialize::Material::Stack(a, sa),
            super::materialize::Material::Stack(b, sb),
        ) => Ok(super::materialize::Material::Value(
            noble_kernel::types::Ty::program(a, b, effects),
            attempt!(joined(sa, sb, span)),
        )),
        (super::materialize::Material::Value(_, _), _)
        | (super::materialize::Material::Stack(_, _), super::materialize::Material::Value(_, _)) => {
            Err(crate::internal(span))
        }
    }
}

/// Extent of a value built from one new constructor over two children.
fn joined(
    a: super::materialize::Extent,
    b: super::materialize::Extent,
    span: crate::Span,
) -> Result<super::materialize::Extent, crate::Diagnostic> {
    Ok(super::materialize::Extent {
        constructors: attempt!(type_size(
            a.constructors
                .saturating_add(b.constructors)
                .saturating_add(1),
            span
        )),
        nodes: attempt!(type_nodes(
            attempt!(type_nodes(a.nodes, b.nodes, span)),
            1,
            span
        )),
    })
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; pushing a materialized value grows a Vec and stack-cap errors construct diagnostics."
)]
fn push(
    stack: super::materialize::Material,
    value: super::materialize::Material,
    span: crate::Span,
) -> Result<super::materialize::Material, crate::Diagnostic> {
    match (stack, value) {
        (
            super::materialize::Material::Stack(mut stack, sa),
            super::materialize::Material::Value(ty, sb),
        ) => {
            if stack.len() >= attempt!(crate::offset(super::STACK_CAP, span)) {
                return Err(crate::Diagnostic::new(
                    crate::DiagnosticKind::Exhausted,
                    span,
                    "stack height limit (256 values) exceeded",
                ));
            }
            // A stack holds at most STACK_CAP values, each within
            // TYPE_NODE_CAP, so its owned node total is bounded without a
            // separate cap; any enclosing program type is checked again.
            let nodes = match sa.nodes.checked_add(sb.nodes) {
                Some(nodes) => nodes,
                None => return Err(node_limit(span)),
            };
            stack.push(ty);
            Ok(super::materialize::Material::Stack(
                stack,
                super::materialize::Extent {
                    constructors: sa.constructors.saturating_add(sb.constructors),
                    nodes,
                },
            ))
        }
        (super::materialize::Material::Value(_, _), _)
        | (super::materialize::Material::Stack(_, _), super::materialize::Material::Stack(_, _)) => {
            Err(crate::internal(span))
        }
    }
}

/// Count, and charge one work unit per node for, the owned `Ty` nodes of a
/// registered nominal descriptor before a materialized occurrence clones it.
/// Each node is counted before the walk descends into it, so the walk is
/// never more than `TYPE_NODE_CAP + 1` nodes deep, whatever the descriptor.
/// Every node costs the same charge and count, so the outcome does not depend
/// on the visiting order. The recursive frames carry only references: the
/// first refusal is parked in the walk state and each frame returns a flag.
pub(super) fn reserve_descriptor(
    shape: &noble_kernel::types::NominalShape,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<u32, crate::Diagnostic> {
    let mut walk = Reservation {
        nodes: 0,
        failure: None,
    };
    if reserve_shape(shape, &mut walk, span, meter) {
        return Ok(walk.nodes);
    }
    match walk.failure {
        Some(problem) => Err(problem),
        None => Err(crate::internal(span)),
    }
}

/// Owned state of one descriptor reservation.
struct Reservation {
    nodes: u32,
    failure: Option<crate::Diagnostic>,
}

/// Charge one work unit and count one node; record the refusal and return
/// `false` when either limit is reached.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; metering is non-const and a refusal stores an owned diagnostic."
)]
fn reserve_node(walk: &mut Reservation, span: crate::Span, meter: &mut crate::Meter) -> bool {
    if let Err(problem) = meter.charge(1, span) {
        walk.failure = Some(problem);
        return false;
    }
    match type_nodes(walk.nodes, 1, span) {
        Ok(nodes) => {
            walk.nodes = nodes;
            true
        }
        Err(problem) => {
            walk.failure = Some(problem);
            false
        }
    }
}

/// Reserve the payload types of one nominal descriptor, in order; `false`
/// once a refusal is recorded.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the reservation charges the non-const work meter for every node."
)]
fn reserve_shape(
    shape: &noble_kernel::types::NominalShape,
    walk: &mut Reservation,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> bool {
    match shape {
        noble_kernel::types::NominalShape::Opaque(representation) => {
            reserve_ty(representation, walk, span, meter)
        }
        noble_kernel::types::NominalShape::Variant(left, right) => {
            reserve_ty(left, walk, span, meter) && reserve_ty(right, walk, span, meter)
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; every node is charged and counted against TYPE_NODE_CAP before the walk descends into it; direct recursion avoids a borrowed work stack the pinned Aeneas cannot translate."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the reservation charges the non-const work meter for every node."
)]
/// Charge and count one node, then reserve the nodes it owns; `false` once a
/// refusal is recorded.
fn reserve_ty(
    ty: &noble_kernel::types::Ty,
    walk: &mut Reservation,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> bool {
    if !reserve_node(walk, span, meter) {
        return false;
    }
    match ty {
        noble_kernel::types::Ty::Pair(left, right) | noble_kernel::types::Ty::Sum(left, right) => {
            reserve_ty(left, walk, span, meter) && reserve_ty(right, walk, span, meter)
        }
        noble_kernel::types::Ty::List(item) => reserve_ty(item, walk, span, meter),
        noble_kernel::types::Ty::Program(input, output, _)
        | noble_kernel::types::Ty::LiveRef(input, output, _) => {
            reserve_stack(input, walk, span, meter) && reserve_stack(output, walk, span, meter)
        }
        noble_kernel::types::Ty::Nominal(_, nested) => reserve_shape(nested, walk, span, meter),
        noble_kernel::types::Ty::GenericNominal(_, arguments, nested) => {
            reserve_ty(&arguments[0], walk, span, meter)
                && reserve_ty(&arguments[1], walk, span, meter)
                && reserve_shape(nested, walk, span, meter)
        }
        noble_kernel::types::Ty::Unit
        | noble_kernel::types::Ty::Bool
        | noble_kernel::types::Ty::I64
        | noble_kernel::types::Ty::Text
        | noble_kernel::types::Ty::Syntax
        | noble_kernel::types::Ty::Contract
        | noble_kernel::types::Ty::Evidence
        | noble_kernel::types::Ty::Certified
        | noble_kernel::types::Ty::Resource(_) => true,
    }
}

#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; each call consumes one entry, and every entry is a node counted against TYPE_NODE_CAP before it is walked, so the depth stays within the node cap; recursion instead of a loop keeps the reservation cycle free of loops, which the pinned Aeneas cannot prove monotone."
)]
/// Reserve every type of a stack in order, stopping at the first refusal.
fn reserve_stack(
    items: &[noble_kernel::types::Ty],
    walk: &mut Reservation,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> bool {
    match items.split_first() {
        Some((first, rest)) => {
            reserve_ty(first, walk, span, meter) && reserve_stack(rest, walk, span, meter)
        }
        None => true,
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; constructor-limit failure constructs an owned diagnostic String."
)]
fn type_size(constructors: u32, span: crate::Span) -> Result<u32, crate::Diagnostic> {
    if constructors > crate::syntax::TYPE_CAP {
        Err(crate::Diagnostic::new(
            crate::DiagnosticKind::Exhausted,
            span,
            "inferred type exceeds the 256-constructor limit",
        ))
    } else {
        Ok(constructors)
    }
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
/// Checked sum of owned `Ty` node counts, refused above `TYPE_NODE_CAP`.
/// This physical bound is independent of the logical constructor limit.
pub(super) fn type_nodes(
    left: u32,
    right: u32,
    span: crate::Span,
) -> Result<u32, crate::Diagnostic> {
    match left.checked_add(right) {
        Some(nodes) if nodes <= super::TYPE_NODE_CAP => Ok(nodes),
        Some(_) | None => Err(node_limit(span)),
    }
}

fn node_limit(span: crate::Span) -> crate::Diagnostic {
    crate::Diagnostic::new(
        crate::DiagnosticKind::Exhausted,
        span,
        "inferred type representation exceeds the 4096-node limit",
    )
}

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
#[cfg(test)]
mod tests {
    use super::super::materialize::Material;
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
    )]
    use super::super::{Arena, Term};
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
    )]
    use noble_kernel::types::{NominalShape, NominalTypeId, Ty};

    const SPAN: crate::Span = crate::Span { start: 0, end: 0 };
    const FAMILY: NominalTypeId = NominalTypeId {
        module: 1,
        ordinal: 0,
    };
    const ORDERS: [[u8; 2]; 2] = [[0, 1], [1, 0]];

    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
    )]
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; the walk is bounded by the type-node budget it charges, and the recursive form states the traversal directly"
    )]
    /// Every owned `Ty` node, including retained generic arguments and their
    /// separately boxed descriptor copies.
    fn owned_nodes(ty: &Ty) -> u32 {
        let shape = |shape: &NominalShape| match shape {
            NominalShape::Opaque(inner) => owned_nodes(inner),
            NominalShape::Variant(left, right) => {
                (owned_nodes(left)).saturating_add(owned_nodes(right))
            }
        };
        1u32.saturating_add(match ty {
            Ty::Pair(left, right) | Ty::Sum(left, right) => {
                (owned_nodes(left)).saturating_add(owned_nodes(right))
            }
            Ty::List(item) => owned_nodes(item),
            Ty::Program(input, output, _) | Ty::LiveRef(input, output, _) => {
                input.iter().chain(output.iter()).map(owned_nodes).sum()
            }
            Ty::Nominal(_, descriptor) => shape(descriptor),
            Ty::GenericNominal(_, arguments, descriptor) => (owned_nodes(&arguments[0]))
                .saturating_add(owned_nodes(&arguments[1]))
                .saturating_add(shape(descriptor)),
            _ => 0,
        })
    }

    /// `T0 = I64`, `T(n+1) = G<Tn, I64>` as a small acyclic symbolic arena.
    fn nested(depth: u32, payload_params: [u8; 2], meter: &mut crate::Meter) -> (Arena, u32) {
        let mut arena = Arena::new();
        arena
            .generic_nominals
            .push(noble_kernel::contracts::GenericVariantDecl {
                id: FAMILY,
                payload_params,
                exported: true,
                public: [true, true],
            });
        let leaf = arena.add(Term::I64, SPAN, meter).expect("leaf fits");
        let mut root = leaf;
        for _ in 0..depth {
            root = arena
                .add(Term::GenericNominal(FAMILY, root, leaf), SPAN, meter)
                .expect("symbolic arena fits");
        }
        (arena, root)
    }

    struct Materialized {
        constructors: u32,
        accounted: u32,
        owned: u32,
        charged: u32,
    }

    fn materialize(
        depth: u32,
        payload_params: [u8; 2],
        limits: crate::Limits,
    ) -> Result<Materialized, crate::Diagnostic> {
        let mut meter = crate::Meter::new(limits);
        let (arena, root) = nested(depth, payload_params, &mut meter);
        let before = meter.work;
        let material = arena.materialize(root, SPAN, &mut meter)?;
        let Material::Value(ty, extent) = material else {
            panic!("a generic instance materializes as a value");
        };
        Ok(Materialized {
            constructors: extent.constructors,
            accounted: extent.nodes,
            owned: owned_nodes(&ty),
            charged: (before).saturating_sub(meter.work),
        })
    }

    #[test]
    fn nested_generic_descriptor_copies_are_counted_and_charged_before_cloning() {
        for payload_params in ORDERS {
            for depth in [1, 2, 4, 6, 8, 10] {
                let built = materialize(depth, payload_params, crate::Limits::default())
                    .expect("depth within the representation bound");
                // Logical size grows linearly; owned nodes double per level.
                assert_eq!(
                    built.constructors,
                    2u32.saturating_mul(depth).saturating_add(1)
                );
                assert_eq!(
                    built.owned,
                    4u32.saturating_mul(1u32 << depth).saturating_sub(3)
                );
                assert_eq!(
                    built.accounted, built.owned,
                    "{payload_params:?} depth {depth}"
                );
                assert!(
                    built.charged >= built.owned,
                    "{payload_params:?} depth {depth}: {} owned nodes for {} work",
                    built.owned,
                    built.charged
                );
            }
        }
    }

    #[test]
    fn generic_descriptor_node_cap_refuses_before_cloning() {
        for payload_params in ORDERS {
            let largest = materialize(10, payload_params, crate::Limits::default())
                .expect("4093 owned nodes fit the representation bound");
            assert!(largest.owned <= super::super::TYPE_NODE_CAP);
            let refused = materialize(11, payload_params, crate::Limits::default())
                .err()
                .expect("8189 owned nodes exceed the representation bound");
            assert_eq!(refused.kind, crate::DiagnosticKind::Exhausted);
            assert_eq!(
                refused.message,
                "inferred type representation exceeds the 4096-node limit"
            );
        }
    }

    #[test]
    fn fixed_work_budget_bounds_descriptor_allocation() {
        let limits = crate::Limits {
            bytes: 65_536,
            nodes: 256,
            depth: 64,
            work: 256,
        };
        for payload_params in ORDERS {
            let mut accepted = 0;
            for depth in 0..=12 {
                match materialize(depth, payload_params, limits) {
                    Ok(built) => {
                        assert!(built.owned <= limits.work);
                        accepted = depth;
                    }
                    Err(refused) => {
                        assert_eq!(refused.kind, crate::DiagnosticKind::Exhausted);
                        assert_eq!(refused.message, "frontend work limit exceeded");
                    }
                }
            }
            // Formerly depth 12 (16381 owned nodes) materialized for 62 work.
            assert!(accepted < 6, "{payload_params:?} accepted depth {accepted}");
        }
    }

    #[test]
    fn registered_descriptor_reservation_counts_and_charges_every_owned_node() {
        let nested = NominalShape::Opaque(alloc::boxed::Box::new(Ty::Bool));
        let shape = NominalShape::Variant(
            alloc::boxed::Box::new(Ty::program(
                alloc::vec![Ty::I64, Ty::Nominal(FAMILY, alloc::boxed::Box::new(nested))],
                alloc::vec![Ty::List(alloc::boxed::Box::new(Ty::I64))],
                noble_kernel::types::EffSet::empty(),
            )),
            alloc::boxed::Box::new(Ty::Pair(
                alloc::boxed::Box::new(Ty::I64),
                alloc::boxed::Box::new(Ty::Unit),
            )),
        );
        let limits = |work| crate::Limits {
            work,
            ..crate::Limits::default()
        };
        let mut meter = crate::Meter::new(limits(9));
        assert_eq!(
            super::reserve_descriptor(&shape, SPAN, &mut meter).ok(),
            Some(9)
        );
        assert_eq!(meter.work, 0);
        let mut meter = crate::Meter::new(limits(8));
        let refused = super::reserve_descriptor(&shape, SPAN, &mut meter)
            .expect_err("nine owned nodes need nine work units");
        assert_eq!(refused.kind, crate::DiagnosticKind::Exhausted);
        assert_eq!(refused.message, "frontend work limit exceeded");
        assert_eq!(meter.work, 0);
    }

    /// `base` with `values` pushed in order, so the last value is on top.
    fn stack_of(arena: &mut Arena, base: Term, values: &[Term], meter: &mut crate::Meter) -> u32 {
        let mut stack = arena.add(base, SPAN, meter).expect("stack base fits");
        for value in values {
            let value = arena.add(*value, SPAN, meter).expect("stack value fits");
            stack = arena
                .add(Term::Push(stack, value), SPAN, meter)
                .expect("stack push fits");
        }
        stack
    }

    #[test]
    fn stack_difference_reports_the_first_differing_position_from_the_top() {
        let mut meter = crate::Meter::new(crate::Limits::default());
        let mut arena = Arena::new();
        let expected = stack_of(
            &mut arena,
            Term::Empty,
            &[Term::Bool, Term::I64],
            &mut meter,
        );
        let actual = stack_of(
            &mut arena,
            Term::Empty,
            &[Term::Text, Term::I64],
            &mut meter,
        );
        let alike = stack_of(
            &mut arena,
            Term::Empty,
            &[Term::Bool, Term::I64],
            &mut meter,
        );
        let (shared, expected_text, actual_text) = arena
            .stack_difference(expected, actual, SPAN, &mut meter)
            .expect("both stacks render")
            .expect("the second position differs");
        assert_eq!(shared, 1);
        assert_ne!(expected_text, actual_text);
        assert!(arena
            .stack_difference(expected, alike, SPAN, &mut meter)
            .expect("both stacks render")
            .is_none());
        let mut starved = crate::Meter::new(crate::Limits {
            work: 0,
            ..crate::Limits::default()
        });
        let refused = arena
            .stack_difference(expected, actual, SPAN, &mut starved)
            .expect_err("the walk is charged");
        assert_eq!(refused.message, "frontend work limit exceeded");
    }

    #[test]
    fn editor_stack_suffix_lists_the_solved_suffix_bottom_first() {
        let mut meter = crate::Meter::new(crate::Limits::default());
        let mut arena = Arena::new();
        let open = stack_of(
            &mut arena,
            Term::Hole(super::super::Sort::Stack),
            &[Term::I64, Term::Bool],
            &mut meter,
        );
        let bottom = arena.add(Term::I64, SPAN, &mut meter).expect("value fits");
        let top = arena.add(Term::Bool, SPAN, &mut meter).expect("value fits");
        let suffix = arena
            .editor_stack_suffix(open, SPAN, &mut meter)
            .expect("an open stack has a solved suffix");
        assert_eq!(suffix.len(), 2);
        assert_eq!(
            suffix[0],
            arena
                .describe_stack(bottom, SPAN, &mut meter)
                .expect("renders")
        );
        assert_eq!(
            suffix[1],
            arena
                .describe_stack(top, SPAN, &mut meter)
                .expect("renders")
        );
        let malformed = stack_of(&mut arena, Term::Unit, &[Term::I64], &mut meter);
        let refused = arena
            .editor_stack_suffix(malformed, SPAN, &mut meter)
            .expect_err("a value is not a stack base");
        assert_eq!(refused.kind, crate::DiagnosticKind::Internal);
    }

    #[test]
    fn types_and_stacks_build_children_first_with_one_charge_per_step() {
        let mut meter = crate::Meter::new(crate::Limits::default());
        let mut arena = Arena::new();
        let pair = Ty::Pair(
            alloc::boxed::Box::new(Ty::I64),
            alloc::boxed::Box::new(Ty::List(alloc::boxed::Box::new(Ty::Bool))),
        );
        // Four type steps and two joins, each charged once, plus four nodes.
        let root = arena.ty(&pair, SPAN, &mut meter).expect("pair builds");
        assert_eq!(meter.spent(), 10);
        assert_eq!(root, 3);
        assert!(matches!(arena.get(0, SPAN), Ok(Term::I64)));
        assert!(matches!(arena.get(1, SPAN), Ok(Term::Bool)));
        assert!(matches!(arena.get(2, SPAN), Ok(Term::List(1))));
        assert!(matches!(arena.get(3, SPAN), Ok(Term::Pair(0, 2))));

        let mut meter = crate::Meter::new(crate::Limits::default());
        let mut arena = Arena::new();
        // The base, two entry scans, two entries and two pushes.
        let root = arena
            .stack(&[Ty::I64, Ty::Bool], SPAN, &mut meter)
            .expect("stack builds");
        assert_eq!(meter.spent(), 12);
        assert_eq!(root, 4);
        assert!(matches!(arena.get(0, SPAN), Ok(Term::Empty)));
        assert!(matches!(arena.get(1, SPAN), Ok(Term::I64)));
        assert!(matches!(arena.get(2, SPAN), Ok(Term::Push(0, 1))));
        assert!(matches!(arena.get(3, SPAN), Ok(Term::Bool)));
        assert!(matches!(arena.get(4, SPAN), Ok(Term::Push(2, 3))));

        let mut starved = crate::Meter::new(crate::Limits {
            work: 11,
            ..crate::Limits::default()
        });
        let refused = Arena::new()
            .stack(&[Ty::I64, Ty::Bool], SPAN, &mut starved)
            .expect_err("the final push node needs the twelfth unit");
        assert_eq!(refused.message, "frontend work limit exceeded");
    }

    /// `List` nested `depth` times around `I64`: `depth + 1` owned nodes in one chain.
    fn list_chain(depth: u32) -> Ty {
        let mut ty = Ty::I64;
        for _ in 0..depth {
            ty = Ty::List(alloc::boxed::Box::new(ty));
        }
        ty
    }

    #[test]
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-contracts; TYPE_NODE_CAP is the constant 4096, so cap - 1 cannot underflow."
    )]
    fn deepest_descriptor_the_node_cap_admits_is_reserved_and_one_deeper_is_refused() {
        // Reservation counts each node before descending into it, so its
        // recursion is never deeper than TYPE_NODE_CAP + 1 nodes.
        let cap = super::super::TYPE_NODE_CAP;
        let deepest = NominalShape::Opaque(alloc::boxed::Box::new(list_chain(cap - 1)));
        let mut meter = crate::Meter::new(crate::Limits::default());
        assert_eq!(
            super::reserve_descriptor(&deepest, SPAN, &mut meter).ok(),
            Some(cap)
        );
        let deeper = NominalShape::Opaque(alloc::boxed::Box::new(list_chain(cap)));
        let mut meter = crate::Meter::new(crate::Limits::default());
        let refused = super::reserve_descriptor(&deeper, SPAN, &mut meter)
            .expect_err("one node past the cap");
        assert_eq!(
            refused.message,
            "inferred type representation exceeds the 4096-node limit"
        );

        // The recursion over a stack's entries is bounded the same way: a
        // Program with cap - 1 inputs owns exactly cap nodes.
        let widest = |inputs: u32| {
            let mut stack = alloc::vec::Vec::new();
            for _ in 0..inputs {
                stack.push(Ty::I64);
            }
            NominalShape::Opaque(alloc::boxed::Box::new(Ty::program(
                stack,
                alloc::vec::Vec::new(),
                noble_kernel::types::EffSet::empty(),
            )))
        };
        let mut meter = crate::Meter::new(crate::Limits::default());
        assert_eq!(
            super::reserve_descriptor(&widest(cap - 1), SPAN, &mut meter).ok(),
            Some(cap)
        );
        let mut meter = crate::Meter::new(crate::Limits::default());
        let refused = super::reserve_descriptor(&widest(cap), SPAN, &mut meter)
            .expect_err("one entry past the cap");
        assert_eq!(
            refused.message,
            "inferred type representation exceeds the 4096-node limit"
        );
    }

    #[test]
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-contracts; TYPE_CAP is the constant 256, so TYPE_CAP - 1 cannot underflow."
    )]
    fn deepest_admitted_type_at_the_bottom_of_a_full_stack_builds() {
        // Admitted types have at most TYPE_CAP (256) constructors and stacks
        // at most STACK_CAP (256) entries; the bottom entry is built at the
        // deepest scan, so this is the deepest recursion those limits admit.
        let mut entries = alloc::vec![list_chain(crate::syntax::TYPE_CAP - 1)];
        for _ in 1..super::super::STACK_CAP {
            entries.push(Ty::I64);
        }
        let mut meter = crate::Meter::new(crate::Limits::default());
        let mut arena = Arena::new();
        let root = arena
            .stack(&entries, SPAN, &mut meter)
            .expect("a full stack of admitted types builds");
        assert!(matches!(arena.get(root, SPAN), Ok(Term::Push(_, _))));

        // Program nesting adds stack frames per constructor; 255 nested
        // Programs around I64 are the deepest such type of 256 constructors.
        let mut nested = Ty::I64;
        for _ in 1..crate::syntax::TYPE_CAP {
            nested = Ty::program(
                alloc::vec![nested],
                alloc::vec::Vec::new(),
                noble_kernel::types::EffSet::empty(),
            );
        }
        let mut meter = crate::Meter::new(crate::Limits::default());
        let root = Arena::new()
            .ty(&nested, SPAN, &mut meter)
            .expect("the deepest admitted Program type builds");
        assert!(root > 0);
    }
}
