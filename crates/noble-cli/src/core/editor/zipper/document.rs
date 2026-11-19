//! The editor transport's format-1 document encoding for one node list.
//!
//! The encoding is the only way a cursor's tree reaches the editor commands,
//! so it is deliberately total: every node kind and every text value encodes
//! to a document the transport's own decoder reads back.

use noble_contracts::source::editor::Node;

/// Encode a node list as the editor transport's format-1 JSON document.
pub(crate) fn encode(tree: &[Node]) -> String {
    let mut document = Writer {
        out: String::from("{\"format\":1,\"nodes\":["),
    };
    document.list(tree);
    document.out.push_str("]}");
    document.out
}

/// One document under construction.
struct Writer {
    out: String,
}

// Justification: every method appends to a heap `String` (or formats an
// integer), so none of them can be `const fn`; the advisory heuristic only
// reads the signature.
#[allow(tigerstyle::missing_const_fn)]
impl Writer {
    fn list(&mut self, nodes: &[Node]) {
        let mut at = 0;
        while at < nodes.len() {
            if at > 0 {
                self.out.push(',');
            }
            if let Some(node) = nodes.get(at) {
                self.node(node);
            }
            at = at.saturating_add(1);
        }
    }

    fn node(&mut self, node: &Node) {
        match node {
            Node::Integer(value) => {
                self.out.push_str("{\"kind\":\"integer\",\"value\":");
                self.out.push_str(&std::format!("{value}"));
                self.out.push('}');
            }
            Node::Boolean(value) => {
                self.out.push_str("{\"kind\":\"boolean\",\"value\":");
                self.out.push_str(if *value { "true" } else { "false" });
                self.out.push('}');
            }
            Node::Word(word) => {
                self.out.push_str("{\"kind\":\"word\",\"value\":");
                self.text(word);
                self.out.push('}');
            }
            Node::Quotation(children) => {
                self.out.push_str("{\"kind\":\"quotation\",\"nodes\":[");
                self.list(children);
                self.out.push_str("]}");
            }
            Node::Hole => self.out.push_str("{\"kind\":\"hole\"}"),
        }
    }

    fn text(&mut self, text: &str) {
        self.out.push('"');
        for character in text.chars() {
            match character {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                '\t' => self.out.push_str("\\t"),
                other => match u32::from(other) {
                    0x00..=0x1f => self
                        .out
                        .push_str(&std::format!("\\u{:04x}", u32::from(other))),
                    _ => self.out.push(other),
                },
            }
        }
        self.out.push('"');
    }
}
