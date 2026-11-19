//! Editor document codec and widget gesture value grammars.
//!
//! The document codec mirrors the strict `noble editor` ingress: the same
//! `{"format":1,"nodes":[...]}` envelope, the same five node kinds and the same
//! exact field sets. It exists so the experiment can re-encode a rendered tree
//! and hand the bytes to the real CLI; it is never an admission route and is
//! not compiled into the noble binary.

use noble_contracts::source::editor::Node;

use super::Refusal;

/// Largest editor JSON document the CLI ingress reads (`SOURCE_LIMITS.bytes`).
pub const MAX_ENVELOPE_BYTES: usize = 65_536;

/// Encode one node list as the versioned editor document.
pub fn encode(nodes: &[Node]) -> Result<String, Refusal> {
    let document = serde_json::json!({
        "format": 1,
        "nodes": node_array(nodes),
    });
    let text = serde_json::to_string(&document)
        .map_err(|error| Refusal::new("encode", error.to_string()))?;
    if text.len() > MAX_ENVELOPE_BYTES {
        return Err(Refusal::new(
            "byte-limit",
            "editor JSON byte limit exceeded",
        ));
    }
    Ok(text)
}

fn node_array(nodes: &[Node]) -> serde_json::Value {
    serde_json::Value::Array(nodes.iter().map(node_value).collect())
}

fn node_value(node: &Node) -> serde_json::Value {
    match node {
        Node::Integer(value) => serde_json::json!({ "kind": "integer", "value": value }),
        Node::Boolean(value) => serde_json::json!({ "kind": "boolean", "value": value }),
        Node::Word(text) => serde_json::json!({ "kind": "word", "value": text }),
        Node::Hole => serde_json::json!({ "kind": "hole" }),
        Node::Quotation(children) => {
            serde_json::json!({ "kind": "quotation", "nodes": node_array(children) })
        }
    }
}

/// Decode one versioned editor document. The frozen corpus is trusted input;
/// the field sets still match the CLI ingress exactly so a decoded tree is the
/// tree the CLI would decode from the same bytes.
pub fn decode(text: &str) -> Result<Vec<Node>, Refusal> {
    let document: serde_json::Value =
        serde_json::from_str(text).map_err(|error| Refusal::new("decode", error.to_string()))?;
    let serde_json::Value::Object(envelope) = document else {
        return Err(Refusal::new(
            "decode",
            "editor document must be a JSON object",
        ));
    };
    exact(&envelope, &["format", "nodes"])?;
    if envelope.get("format").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err(Refusal::new("decode", "unsupported editor format"));
    }
    let some_nodes = envelope.get("nodes");
    let Some(serde_json::Value::Array(items)) = some_nodes else {
        return Err(Refusal::new("decode", "editor nodes must be an array"));
    };
    items.iter().map(node_from).collect()
}

fn node_from(value: &serde_json::Value) -> Result<Node, Refusal> {
    let serde_json::Value::Object(fields) = value else {
        return Err(Refusal::new("decode", "editor node must be a JSON object"));
    };
    let kind = fields
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Refusal::new("decode", "editor node kind is missing"))?;
    match kind {
        "integer" => {
            exact(fields, &["kind", "value"])?;
            let value = fields
                .get("value")
                .and_then(serde_json::Value::as_i64)
                .ok_or_else(|| Refusal::new("decode", "editor integer must be I64"))?;
            Ok(Node::Integer(value))
        }
        "boolean" => {
            exact(fields, &["kind", "value"])?;
            let value = fields
                .get("value")
                .and_then(serde_json::Value::as_bool)
                .ok_or_else(|| Refusal::new("decode", "editor boolean must be Boolean"))?;
            Ok(Node::Boolean(value))
        }
        "word" => {
            exact(fields, &["kind", "value"])?;
            let value = fields
                .get("value")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| Refusal::new("decode", "editor word must be text"))?;
            Ok(Node::Word(value.to_owned()))
        }
        "hole" => {
            exact(fields, &["kind"])?;
            Ok(Node::Hole)
        }
        "quotation" => {
            exact(fields, &["kind", "nodes"])?;
            let Some(serde_json::Value::Array(items)) = fields.get("nodes") else {
                return Err(Refusal::new(
                    "decode",
                    "editor quotation nodes must be an array",
                ));
            };
            let children: Result<Vec<Node>, Refusal> = items.iter().map(node_from).collect();
            Ok(Node::Quotation(children?))
        }
        _ => Err(Refusal::new("decode", "unsupported editor node kind")),
    }
}

fn exact(
    fields: &serde_json::Map<String, serde_json::Value>,
    expected: &[&str],
) -> Result<(), Refusal> {
    if fields.len() != expected.len() || expected.iter().any(|key| !fields.contains_key(*key)) {
        return Err(Refusal::new(
            "decode",
            "editor object has missing or unexpected fields",
        ));
    }
    Ok(())
}

/// The numeric control's I64 decimal grammar: an optional `-` and at least one
/// ASCII digit, parsed with a checked I64 range. Anything else is refused rather
/// than wrapped, truncated or reinterpreted.
pub fn integer_value(text: &str) -> Result<i64, Refusal> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Refusal::new(
            "integer-not-decimal",
            "the numeric control accepts only decimal I64 text",
        ));
    }
    text.parse::<i64>()
        .map_err(|_| Refusal::new("integer-out-of-range", "the value is outside the I64 range"))
}

/// The checkbox's decode: only the two literal spellings, never a truthiness
/// guess, so a non-boolean gesture cannot become a Boolean node.
pub fn boolean_value(text: &str) -> Result<bool, Refusal> {
    match text {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(Refusal::new(
            "boolean-not-boolean",
            "the checkbox accepts only `true` or `false`",
        )),
    }
}

/// The word control's grammar, mirroring the source lexer's word classifier:
/// ASCII only, an initial character from `[A-Za-z_+*/=<>?!-]`, later bytes from
/// that set plus digits and `.`, and never a full token the lexer classifies as
/// something else (`def`, `true`, `false`, or an integer literal). A word the
/// renderer accepts therefore cannot inject quotation or source syntax, and the
/// generated source refuses nothing the JSON path would accept.
pub fn source_word(text: &str) -> Result<(), Refusal> {
    if text.is_empty() || !text.is_ascii() {
        return Err(Refusal::new(
            "word-syntax",
            "the word control accepts non-empty ASCII text",
        ));
    }
    if matches!(text, "def" | "true" | "false") || integer_shaped(text) {
        return Err(Refusal::new(
            "word-syntax",
            "the text is a source keyword or literal, not a word",
        ));
    }
    let mut bytes = text.bytes();
    let Some(first) = bytes.next() else {
        return Err(Refusal::new("word-syntax", "the word control is empty"));
    };
    if !word_initial(first) {
        return Err(Refusal::new(
            "word-syntax",
            "a word starts with an ASCII letter or one of _ + - * / = < > ? !",
        ));
    }
    if !bytes.all(|byte| word_initial(byte) || byte.is_ascii_digit() || byte == b'.') {
        return Err(Refusal::new(
            "word-syntax",
            "a word continues with ASCII letters, digits and _ + - * / = < > ? ! .",
        ));
    }
    Ok(())
}

fn word_initial(byte: u8) -> bool {
    byte.is_ascii_alphabetic()
        || matches!(
            byte,
            b'_' | b'+' | b'-' | b'*' | b'/' | b'=' | b'<' | b'>' | b'?' | b'!'
        )
}

fn integer_shaped(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

/// Node-tree equality for the foreign contract type.
pub fn nodes_equal(left: &[Node], right: &[Node]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| node_equal(a, b))
}

fn node_equal(left: &Node, right: &Node) -> bool {
    match (left, right) {
        (Node::Integer(a), Node::Integer(b)) => a == b,
        (Node::Boolean(a), Node::Boolean(b)) => a == b,
        (Node::Word(a), Node::Word(b)) => a == b,
        (Node::Hole, Node::Hole) => true,
        (Node::Quotation(a), Node::Quotation(b)) => nodes_equal(a, b),
        _ => false,
    }
}

/// Whether a tree still carries an editor hole anywhere.
pub fn contains_hole(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Hole => true,
        Node::Quotation(children) => contains_hole(children),
        _ => false,
    })
}
