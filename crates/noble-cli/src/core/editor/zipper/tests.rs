//! Unit tests for the cursor and its document encoding.
//!
//! These cover the pure data-structure semantics; the transport comparison
//! lives in the integration measurement (`tests/editor_zipper/`).

use super::document::encode;
use super::{Cursor, Refusal};
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
/// `Node` deliberately carries no `PartialEq`, and the encoding is the form
/// the editor transport itself reads back.
fn same(left: &[Node], right: &[Node]) -> bool {
    encode(left) == encode(right)
}

fn single(node: &Node) -> std::vec::Vec<Node> {
    std::vec![node.clone()]
}

/// Every address in the tree, in depth-first order.
#[allow(
    tigerstyle::no_recursion,
    // Justification: the walk descends one level per quotation and the
    // ingress depth limit bounds that, so recursion is bounded and readable.
)]
fn addresses(tree: &[Node], prefix: &[u32], into: &mut std::vec::Vec<std::vec::Vec<u32>>) {
    let mut at = 0;
    while at < tree.len() {
        let mut path = Vec::with_capacity(prefix.len().saturating_add(1));
        path.extend_from_slice(prefix);
        path.push(u32::try_from(at).unwrap_or(u32::MAX));
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
fn moves_the_focus_and_keeps_the_tree() {
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
    assert_eq!(first.enter().err(), Some(Refusal::NotQuotation));
    assert_eq!(first.leave().err(), Some(Refusal::AtRoot));
    let word = Cursor::at(&tree, &[1, 0]).expect("a word focuses");
    assert_eq!(word.enter().err(), Some(Refusal::NotQuotation));
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
    let classes = [
        Refusal::OffEnd.class(),
        Refusal::NotQuotation.class(),
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
