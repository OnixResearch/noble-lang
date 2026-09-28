fn left_quote(ty: noble_kernel::types::Ty, left_effects: &[u32]) -> noble_kernel::untrusted::Node {
    crate::support::quote_node(
        vec![],
        vec![ty],
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::I64],
        left_effects,
    )
}

fn right_quote(ty: noble_kernel::types::Ty, left_effects: &[u32]) -> noble_kernel::untrusted::Node {
    let left_program = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::I64],
        crate::support::ids(left_effects),
    );
    crate::support::quote_node(
        vec![2, 3],
        vec![ty, left_program],
        vec![noble_kernel::types::Ty::Text],
        vec![noble_kernel::types::Ty::I64],
        &[],
    )
}

fn text_duplicate() -> noble_kernel::untrusted::Node {
    crate::support::invocation(
        noble_kernel::contracts::Definition(1),
        vec![
            crate::support::segment(vec![]),
            crate::support::value(noble_kernel::types::Ty::Text),
        ],
    )
}

fn matcher_invocation(
    matcher: noble_kernel::contracts::Definition,
    left_effects: &[u32],
) -> noble_kernel::untrusted::Node {
    crate::support::invocation(
        matcher,
        vec![
            crate::support::segment(vec![]),
            crate::support::segment(vec![noble_kernel::types::Ty::I64]),
            crate::support::effect_binding(left_effects),
            crate::support::effect_binding(&[]),
        ],
    )
}

pub(super) fn match_candidate(
    matcher: noble_kernel::contracts::Definition,
    ty: noble_kernel::types::Ty,
    right: bool,
    left_effects: &[u32],
) -> noble_kernel::untrusted::Candidate {
    let nodes = vec![
        left_quote(ty.clone(), left_effects),
        right_quote(ty, left_effects),
        text_duplicate(),
        crate::support::lit_node(noble_kernel::untrusted::Lit::I64(7), vec![]),
        matcher_invocation(matcher, left_effects),
    ];
    crate::support::candidate(nodes, if right { vec![0, 1, 4] } else { vec![0, 4] })
}
