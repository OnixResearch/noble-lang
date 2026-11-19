//! Opt-in zipper cursor over the five editor node kinds (issue #142).
//!
//! A cursor is one focused [`Node`] plus a trail of quotation frames: for each
//! enclosing quotation, the index and siblings of the quotation node that
//! holds the focus. Foci are addressed with the editor transport's existing
//! `node_path` scheme, the index of each enclosing quotation followed by the
//! focus's own index, so a cursor and an `editor analyze` failure name the same
//! node. The cursor carries its own address rather than recomputing it from
//! sibling counts, so `node_path` is the address it was built from or moved to.
//!
//! The module is declared `#[cfg(test)] mod zipper;` and nothing else reaches
//! it: the shipped CLI contains none of this code, no editor command
//! constructs a cursor, and the JSON ingress, its exit codes, the generated
//! spans and `noble-editor-report/v1` are untouched. Navigation and
//! replacement build node values only; every submission still goes through
//! `noble editor analyze` / `noble editor admit`.
//!
//! The zipper shape follows the ordinary focus-and-context idea the issue
//! cites. Nothing here reproduces the cited paper's Haskell modules, Lean
//! development, prefetching workload or cost bounds, and none of the authors'
//! numbers are gates for this module.

#[path = "zipper/document.rs"]
pub(crate) mod document;

#[cfg(test)]
#[path = "zipper/tests.rs"]
mod tests;

use noble_contracts::source::editor::{Candidate, Node};

/// Why a cursor move was refused.
///
/// The five variants are the frozen corpus's refusal vocabulary; `class`
/// spells each one as the corpus writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// `next` on the last sibling, or `previous` on the first.
    OffEnd,
    /// `enter` when the focus is not a quotation.
    NotQuotation,
    /// `enter` on a quotation that has no child to focus.
    EmptyQuotation,
    /// `leave` when the focus already sits in the root node list.
    AtRoot,
    /// A `node_path` that addresses no node of this tree.
    NoSuchNode,
}

impl Refusal {
    /// The stable class name the frozen corpus and the receipt record.
    #[allow(
        tigerstyle::fragile_exhaustive_enum_match,
        // Justification: the five refusal classes are a closed vocabulary the
        // committed corpus pins by name, so an exhaustive match is the point:
        // a sixth variant must reach this match and the corpus together.
    )]
    pub(crate) const fn class(self) -> &'static str {
        match self {
            Self::OffEnd => "off-end",
            Self::NotQuotation => "not-a-quotation",
            Self::EmptyQuotation => "empty-quotation",
            Self::AtRoot => "at-root",
            Self::NoSuchNode => "no-such-node",
        }
    }
}

/// One enclosing quotation: its own index in the parent list, and the siblings
/// of that quotation there.
#[derive(Clone)]
struct Frame {
    index: u32,
    before: Vec<Node>,
    after: Vec<Node>,
}

/// A focused editor node with the trail of quotation frames around it.
///
/// `focus_index`, `before` and `after` describe the focus's place in its own
/// list; the trail is outermost first and an empty trail means the focus is a
/// root node.
pub(crate) struct Cursor {
    focus_index: u32,
    before: Vec<Node>,
    focus: Node,
    after: Vec<Node>,
    trail: Vec<Frame>,
}

impl Cursor {
    /// Focus the node a `node_path` addresses, or refuse an address this tree
    /// does not contain. Descending through a non-quotation, an index past the
    /// end of a list and an address with no index at all are all
    /// `no-such-node`; the empty root list has no node to focus.
    pub(crate) fn at(tree: &[Node], path: &[u32]) -> Result<Self, Refusal> {
        let mut list = tree;
        let mut trail: Vec<Frame> = Vec::with_capacity(path.len());
        let mut rest = path;
        loop {
            let Some((index, tail)) = rest.split_first() else {
                return Err(Refusal::NoSuchNode);
            };
            let at = usize::try_from(*index).map_err(|_| Refusal::NoSuchNode)?;
            let Some((before, from_focus)) = list.split_at_checked(at) else {
                return Err(Refusal::NoSuchNode);
            };
            let Some((node, after)) = from_focus.split_first() else {
                return Err(Refusal::NoSuchNode);
            };
            if tail.is_empty() {
                return Ok(Self {
                    focus_index: *index,
                    before: before.to_vec(),
                    focus: node.clone(),
                    after: after.to_vec(),
                    trail,
                });
            }
            let Node::Quotation(children) = node else {
                return Err(Refusal::NoSuchNode);
            };
            trail.push(Frame {
                index: *index,
                before: before.to_vec(),
                after: after.to_vec(),
            });
            list = children;
            rest = tail;
        }
    }

    /// The focused node.
    pub(crate) const fn focus(&self) -> &Node {
        &self.focus
    }

    /// The `node_path` address of the focus in this cursor's tree.
    pub(crate) fn node_path(&self) -> Vec<u32> {
        let mut path = Vec::with_capacity(self.trail.len().saturating_add(1));
        for frame in &self.trail {
            path.push(frame.index);
        }
        path.push(self.focus_index);
        path
    }

    /// Focus the first child of a focused quotation.
    pub(crate) fn enter(&self) -> Result<Self, Refusal> {
        let Node::Quotation(children) = &self.focus else {
            return Err(Refusal::NotQuotation);
        };
        let Some((first, rest)) = children.split_first() else {
            return Err(Refusal::EmptyQuotation);
        };
        let mut trail = Vec::with_capacity(self.trail.len().saturating_add(1));
        trail.extend_from_slice(&self.trail);
        trail.push(Frame {
            index: self.focus_index,
            before: self.before.clone(),
            after: self.after.clone(),
        });
        Ok(Self {
            focus_index: 0,
            before: Vec::new(),
            focus: first.clone(),
            after: rest.to_vec(),
            trail,
        })
    }

    /// Focus the quotation that contains the current focus.
    pub(crate) fn leave(&self) -> Result<Self, Refusal> {
        let Some((frame, above)) = self.trail.split_last() else {
            return Err(Refusal::AtRoot);
        };
        let mut children = Vec::with_capacity(self.before.len().saturating_add(1));
        children.extend_from_slice(&self.before);
        children.push(self.focus.clone());
        children.extend_from_slice(&self.after);
        Ok(Self {
            focus_index: frame.index,
            before: frame.before.clone(),
            focus: Node::Quotation(children),
            after: frame.after.clone(),
            trail: above.to_vec(),
        })
    }

    /// Focus the next sibling in the enclosing list.
    pub(crate) fn next(&self) -> Result<Self, Refusal> {
        let Some((head, rest)) = self.after.split_first() else {
            return Err(Refusal::OffEnd);
        };
        let mut before = Vec::with_capacity(self.before.len().saturating_add(1));
        before.extend_from_slice(&self.before);
        before.push(self.focus.clone());
        Ok(Self {
            focus_index: self.focus_index.saturating_add(1),
            before,
            focus: head.clone(),
            after: rest.to_vec(),
            trail: self.trail.clone(),
        })
    }

    /// Focus the previous sibling in the enclosing list.
    pub(crate) fn previous(&self) -> Result<Self, Refusal> {
        let Some((last, rest)) = self.before.split_last() else {
            return Err(Refusal::OffEnd);
        };
        let mut after = Vec::with_capacity(self.after.len().saturating_add(1));
        after.push(self.focus.clone());
        after.extend_from_slice(&self.after);
        Ok(Self {
            focus_index: self.focus_index.saturating_sub(1),
            before: rest.to_vec(),
            focus: last.clone(),
            after,
            trail: self.trail.clone(),
        })
    }

    /// Replace the focus with one of the five editor node kinds.
    pub(crate) fn replace(&self, node: Node) -> Self {
        Self {
            focus_index: self.focus_index,
            before: self.before.clone(),
            focus: node,
            after: self.after.clone(),
            trail: self.trail.clone(),
        }
    }

    /// Fold the trail back into the root node list.
    pub(crate) fn reconstruct(&self) -> Vec<Node> {
        let mut node = self.focus.clone();
        let mut before = self.before.clone();
        let mut after = self.after.clone();
        for frame in self.trail.iter().rev() {
            let mut children = before;
            children.push(node);
            children.extend(after);
            node = Node::Quotation(children);
            before = frame.before.clone();
            after = frame.after.clone();
        }
        let mut tree = before;
        tree.push(node);
        tree.extend(after);
        tree
    }

    /// The reconstructed tree as an editor transport candidate.
    pub(crate) fn candidate(&self) -> Candidate {
        Candidate {
            format: noble_contracts::source::editor::FORMAT,
            nodes: self.reconstruct(),
        }
    }
}
