//! Opt-in zipper cursor over the five editor node kinds (issue #142).
//!
//! A cursor is one focused [`Node`] plus a trail of quotation frames: for each
//! enclosing quotation, the siblings of the quotation node that holds the
//! focus. Foci are addressed with the editor transport's existing `node_path`
//! scheme, the index of each enclosing quotation followed by the focus's own
//! index, so a cursor and an `editor analyze` failure name the same node.
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

use noble_contracts::source::editor::{Candidate, Node, FORMAT};

/// Why a cursor move was refused.
///
/// Every refusal happens before any submission: it leaves the borrowed cursor
/// untouched, no tree is produced, and no editor command is invoked on an
/// illegal tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// `next` on the last sibling, or `previous` on the first.
    OffEnd,
    /// `enter` when the focus is not a quotation.
    NotAQuotation,
    /// `enter` on a quotation that has no child to focus.
    EmptyQuotation,
    /// `leave` when the focus already sits in the root node list.
    AtRoot,
    /// A `node_path` that addresses no node of this tree.
    NoSuchNode,
}

impl Refusal {
    /// The stable class name the frozen corpus and the receipt record.
    pub(crate) const fn class(self) -> &'static str {
        match self {
            Self::OffEnd => "off-end",
            Self::NotAQuotation => "not-a-quotation",
            Self::EmptyQuotation => "empty-quotation",
            Self::AtRoot => "at-root",
            Self::NoSuchNode => "no-such-node",
        }
    }
}

/// The siblings of one enclosing quotation node, in the list one level up.
#[derive(Clone)]
struct Frame {
    before: Vec<Node>,
    after: Vec<Node>,
}

/// A focused editor node with the trail of quotation frames around it.
///
/// `before` and `after` are the focus's own siblings in its list; the trail is
/// outermost first, and an empty trail means the focus is a root node.
pub(crate) struct Cursor {
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
        let mut trail: Vec<Frame> = Vec::new();
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
            path.push(index(frame.before.len()));
        }
        path.push(index(self.before.len()));
        path
    }

    /// Focus the first child of a focused quotation.
    pub(crate) fn enter(&self) -> Result<Self, Refusal> {
        let Node::Quotation(children) = &self.focus else {
            return Err(Refusal::NotAQuotation);
        };
        let Some((first, rest)) = children.split_first() else {
            return Err(Refusal::EmptyQuotation);
        };
        let mut trail = self.trail.clone();
        trail.push(Frame {
            before: self.before.clone(),
            after: self.after.clone(),
        });
        Ok(Self {
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
        let mut children = self.before.clone();
        children.push(self.focus.clone());
        children.extend_from_slice(&self.after);
        Ok(Self {
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
        let mut before = self.before.clone();
        before.push(self.focus.clone());
        Ok(Self {
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
            before: rest.to_vec(),
            focus: last.clone(),
            after,
            trail: self.trail.clone(),
        })
    }

    /// Replace the focus with one of the five editor node kinds.
    pub(crate) fn replace(&self, node: Node) -> Self {
        Self {
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
            format: FORMAT,
            nodes: self.reconstruct(),
        }
    }
}

/// A sibling count as an editor path index.
///
/// The ingress bounds a tree at 16 384 nodes, so a count in a cursor the
/// transport produced always fits `u32`; the clamp is the harness-visible
/// fallback for a list built outside that bound.
#[expect(
    tigerstyle::fallible_int_fallback,
    reason = "Owner: noble-cli; a cursor sibling count is bounded by the ingress node limit, and the clamp only applies to a tree no transport can produce"
)]
fn index(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

/// Encode a node list as the editor transport's format-1 JSON document.
pub(crate) fn encode(tree: &[Node]) -> String {
    let mut out = String::from("{\"format\":1,\"nodes\":[");
    encode_list(tree, &mut out);
    out.push_str("]}");
    out
}

fn encode_list(nodes: &[Node], out: &mut String) {
    let mut at = 0;
    while at < nodes.len() {
        if at > 0 {
            out.push(',');
        }
        if let Some(node) = nodes.get(at) {
            encode_node(node, out);
        }
        at = at.saturating_add(1);
    }
}

fn encode_node(node: &Node, out: &mut String) {
    match node {
        Node::Integer(value) => {
            out.push_str("{\"kind\":\"integer\",\"value\":");
            out.push_str(&std::format!("{value}"));
            out.push('}');
        }
        Node::Boolean(value) => {
            out.push_str("{\"kind\":\"boolean\",\"value\":");
            out.push_str(if *value { "true" } else { "false" });
            out.push('}');
        }
        Node::Word(word) => {
            out.push_str("{\"kind\":\"word\",\"value\":");
            encode_text(word, out);
            out.push('}');
        }
        Node::Quotation(children) => {
            out.push_str("{\"kind\":\"quotation\",\"nodes\":[");
            encode_list(children, out);
            out.push_str("]}");
        }
        Node::Hole => out.push_str("{\"kind\":\"hole\"}"),
    }
}

fn encode_text(text: &str, out: &mut String) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => match u32::from(other) {
                0x00..=0x1f => out.push_str(&std::format!("\\u{:04x}", u32::from(other))),
                _ => out.push(other),
            },
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::{encode, Cursor, Refusal};
    use noble_contracts::source::editor::Node;

    fn sample() -> Vec<Node> {
        std::vec![
            Node::Integer(1),
            Node::Quotation(std::vec![
                Node::Word("dup".into()),
                Node::Quotation(std::vec![Node::Hole, Node::Boolean(false)]),
                Node::Word("+".into()),
            ]),
            Node::Word("test.emit".into()),
        ]
    }

    /// Node trees are compared through their canonical transport encoding:
    /// `Node` deliberately carries no `PartialEq`, and the encoding is the
    /// form the editor transport itself reads back.
    fn same(left: &[Node], right: &[Node]) -> bool {
        encode(left) == encode(right)
    }

    fn single(node: &Node) -> std::vec::Vec<Node> {
        std::vec![node.clone()]
    }

    /// Every address in the tree, in depth-first order.
    fn addresses(tree: &[Node], prefix: &[u32], into: &mut std::vec::Vec<std::vec::Vec<u32>>) {
        let mut at = 0;
        while at < tree.len() {
            let mut path = prefix.to_vec();
            path.push(super::index(at));
            into.push(path.clone());
            if let Some(Node::Quotation(children)) = tree.get(at) {
                addresses(children, &path, into);
            }
            at = at.saturating_add(1);
        }
    }

    #[test]
    fn identity_reconstruction_covers_every_address() {
        let trees = std::vec![
            sample(),
            std::vec![Node::Hole],
            std::vec![Node::Quotation(std::vec![])],
            std::vec![
                Node::Quotation(std::vec![Node::Quotation(std::vec![Node::Integer(-2)])]),
                Node::Boolean(true),
            ],
        ];
        for tree in &trees {
            let original = encode(tree);
            let mut paths = std::vec::Vec::new();
            addresses(tree, &[], &mut paths);
            for path in paths {
                let cursor = Cursor::at(tree, &path).expect("every enumerated address focuses");
                assert_eq!(
                    cursor.node_path(),
                    path,
                    "the cursor reports its own address"
                );
                assert_eq!(encode(&cursor.reconstruct()), original, "{path:?}");
                assert_eq!(encode(&cursor.candidate().nodes), original, "{path:?}");
            }
        }
    }

    #[test]
    fn navigation_moves_the_focus_and_keeps_the_tree() {
        let tree = sample();
        let original = encode(&tree);
        let root = Cursor::at(&tree, &[1]).expect("root quotation focuses");
        assert_eq!(root.node_path(), std::vec![1]);
        let first = root.enter().expect("a quotation has a first child");
        assert_eq!(first.node_path(), std::vec![1, 0]);
        assert!(same(
            &single(first.focus()),
            &single(&Node::Word("dup".into()))
        ));
        let second = first.next().expect("a quotation has a second child");
        assert_eq!(second.node_path(), std::vec![1, 1]);
        let inside = second.enter().expect("the nested quotation has a child");
        assert_eq!(inside.node_path(), std::vec![1, 1, 0]);
        let sibling = inside.next().expect("the nested quotation has a sibling");
        assert_eq!(sibling.node_path(), std::vec![1, 1, 1]);
        assert!(same(
            &single(sibling.focus()),
            &single(&Node::Boolean(false))
        ));
        let back = sibling.previous().expect("the hole precedes the boolean");
        assert_eq!(back.node_path(), std::vec![1, 1, 0]);
        let quoted = sibling.leave().expect("the focus is inside a quotation");
        assert_eq!(quoted.node_path(), std::vec![1, 1]);
        assert!(
            same(&quoted.reconstruct(), &tree),
            "navigation changes no node"
        );
        assert_eq!(encode(&quoted.reconstruct()), original);

        // Stepping on from the enclosing quotation reaches the next word.
        let tail = quoted
            .next()
            .expect("the quotation has a following sibling");
        assert_eq!(tail.node_path(), std::vec![1, 2]);
        assert!(same(&tail.reconstruct(), &tree));
        let root_again = tail.leave().expect("the quotation is a root node");
        assert_eq!(root_again.node_path(), std::vec![1]);
        assert!(same(&root_again.reconstruct(), &tree));
    }

    #[test]
    fn illegal_moves_are_refused_with_distinct_classes() {
        let tree = sample();
        let last = Cursor::at(&tree, &[2]).expect("the last root node focuses");
        assert_eq!(last.next().err(), Some(Refusal::OffEnd));
        let first = Cursor::at(&tree, &[0]).expect("the first root node focuses");
        assert_eq!(first.previous().err(), Some(Refusal::OffEnd));
        assert_eq!(first.enter().err(), Some(Refusal::NotAQuotation));
        assert_eq!(first.leave().err(), Some(Refusal::AtRoot));
        let word = Cursor::at(&tree, &[1, 0]).expect("a word focuses");
        assert_eq!(word.enter().err(), Some(Refusal::NotAQuotation));
        let empty = std::vec![Node::Quotation(std::vec![])];
        let quotation = Cursor::at(&empty, &[0]).expect("an empty quotation focuses");
        assert_eq!(quotation.enter().err(), Some(Refusal::EmptyQuotation));
        assert_eq!(Cursor::at(&tree, &[3]).err(), Some(Refusal::NoSuchNode));
        assert_eq!(Cursor::at(&tree, &[1, 3]).err(), Some(Refusal::NoSuchNode));
        assert_eq!(Cursor::at(&tree, &[0, 0]).err(), Some(Refusal::NoSuchNode));
        assert_eq!(Cursor::at(&tree, &[]).err(), Some(Refusal::NoSuchNode));
        assert_eq!(
            Cursor::at(&std::vec![], &[0]).err(),
            Some(Refusal::NoSuchNode)
        );
        // The five kinds have five distinct refusal classes and no catch-all.
        let classes = [
            Refusal::OffEnd.class(),
            Refusal::NotAQuotation.class(),
            Refusal::EmptyQuotation.class(),
            Refusal::AtRoot.class(),
            Refusal::NoSuchNode.class(),
        ];
        assert_eq!(
            classes,
            [
                "off-end",
                "not-a-quotation",
                "empty-quotation",
                "at-root",
                "no-such-node"
            ]
        );
    }

    #[test]
    fn replacement_changes_only_the_focused_node() {
        let tree = std::vec![Node::Quotation(std::vec![
            Node::Integer(1),
            Node::Word("+".into())
        ])];
        let focus = Cursor::at(&tree, &[0, 1]).expect("the word focuses");
        let cases = [
            Node::Integer(i64::MIN),
            Node::Boolean(true),
            Node::Word("drop".into()),
            Node::Quotation(std::vec![Node::Hole]),
            Node::Hole,
        ];
        for replacement in cases {
            let replaced = focus.replace(replacement.clone());
            assert_eq!(replaced.node_path(), std::vec![0, 1]);
            assert!(same(&single(replaced.focus()), &single(&replacement)));
            let expected = std::vec![Node::Quotation(std::vec![Node::Integer(1), replacement,])];
            assert!(same(&replaced.reconstruct(), &expected));
        }
        let quoted = Cursor::at(&tree, &[0]).expect("the quotation focuses");
        let replaced = quoted.replace(Node::Hole);
        assert!(same(&replaced.reconstruct(), &std::vec![Node::Hole]));
    }

    #[test]
    fn encoding_is_the_transport_document() {
        assert_eq!(encode(&[]), "{\"format\":1,\"nodes\":[]}");
        assert_eq!(
            encode(&[
                Node::Integer(i64::MAX),
                Node::Integer(i64::MIN),
                Node::Boolean(true),
                Node::Boolean(false),
                Node::Word("+".into()),
                Node::Hole,
                Node::Quotation(std::vec![]),
                Node::Quotation(std::vec![Node::Hole]),
            ]),
            "{\"format\":1,\"nodes\":[\
             {\"kind\":\"integer\",\"value\":9223372036854775807},\
             {\"kind\":\"integer\",\"value\":-9223372036854775808},\
             {\"kind\":\"boolean\",\"value\":true},\
             {\"kind\":\"boolean\",\"value\":false},\
             {\"kind\":\"word\",\"value\":\"+\"},\
             {\"kind\":\"hole\"},\
             {\"kind\":\"quotation\",\"nodes\":[]},\
             {\"kind\":\"quotation\",\"nodes\":[{\"kind\":\"hole\"}]}\
             ]}"
        );
        // Text is escaped rather than trusted: a word that would break the
        // document is still encoded as one JSON string.
        assert_eq!(
            encode(&[Node::Word("a\"b\\c\nd\t".into())]),
            "{\"format\":1,\"nodes\":[{\"kind\":\"word\",\"value\":\"a\\\"b\\\\c\\nd\\t\"}]}"
        );
        assert_eq!(
            encode(&[Node::Word("\u{1}".into())]),
            "{\"format\":1,\"nodes\":[{\"kind\":\"word\",\"value\":\"\\u0001\"}]}"
        );
    }
}
