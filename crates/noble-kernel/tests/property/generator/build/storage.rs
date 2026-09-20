//! Owned arena storage: every append checks its externally visible node id.

pub(super) struct Arena {
    pub nodes: Vec<noble_kernel::untrusted::Node>,
}

/// The surrounding stack and claimed body interface of one quotation.
pub(super) struct Claim<'a> {
    pub around: &'a [noble_kernel::types::Ty],
    pub start: &'a [noble_kernel::types::Ty],
    pub end: &'a [noble_kernel::types::Ty],
    pub effects: &'a [u32],
}

impl Arena {
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-maintainers; this constant capacity formula is 168 on every supported usize width, derived from six outer steps and three nine-node children; reassess when those generator bounds change."
    )]
    pub fn new() -> Self {
        // Six outer steps, each a quotation with three nine-node
        // eliminators in its child, plus the quotation node itself.
        Self {
            nodes: Vec::with_capacity(6 * (3 * 9 + 1)),
        }
    }

    pub fn push(
        &mut self,
        node: noble_kernel::untrusted::Node,
    ) -> Result<noble_kernel::untrusted::NodeId, String> {
        let index = u32::try_from(self.nodes.len())
            .map_err(|error| format!("generated arena index exceeds u32: {error}"))?;
        self.nodes.push(node);
        Ok(noble_kernel::untrusted::NodeId(index))
    }

    pub fn literal(
        &mut self,
        lit: noble_kernel::untrusted::Lit,
        under: &[noble_kernel::types::Ty],
    ) -> Result<noble_kernel::untrusted::NodeId, String> {
        self.push(noble_kernel::untrusted::Node::Literal {
            lit,
            inst: noble_kernel::words::Inst {
                bindings: vec![crate::fit::stack(under.to_vec())],
            },
        })
    }

    pub fn word(
        &mut self,
        def: u32,
        bindings: Vec<noble_kernel::words::Binding>,
    ) -> Result<noble_kernel::untrusted::NodeId, String> {
        self.push(noble_kernel::untrusted::Node::Invocation {
            def: noble_kernel::contracts::Definition(def),
            inst: noble_kernel::words::Inst { bindings },
        })
    }

    pub fn quotation(
        &mut self,
        body: Vec<noble_kernel::untrusted::NodeId>,
        claim: crate::gen::storage::Claim<'_>,
    ) -> Result<noble_kernel::untrusted::NodeId, String> {
        self.push(noble_kernel::untrusted::Node::Quotation {
            body,
            inst: noble_kernel::words::Inst {
                bindings: vec![
                    crate::fit::stack(claim.around.to_vec()),
                    crate::fit::stack(claim.start.to_vec()),
                    crate::fit::stack(claim.end.to_vec()),
                    crate::fit::effects(claim.effects),
                ],
            },
        })
    }
}
