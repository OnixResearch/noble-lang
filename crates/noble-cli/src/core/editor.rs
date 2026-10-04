//! Strict versioned editor ingress. This transport never adds `.noble` syntax.

use crate::workflow::encoding::{object, string, Json};
use noble_contracts::source::editor::{Candidate, Node};

type Value = super::report::Value;

fn fields<'a>(value: &'a Value, expected: &[&str]) -> Result<&'a std::collections::BTreeMap<String, Value>, String> {
    let Value::Object(fields) = value else {
        return Err("editor node must be a JSON object".into());
    };
    if fields.len() != expected.len() || expected.iter().any(|key| !fields.contains_key(*key)) {
        return Err("editor object has missing or unexpected fields".into());
    }
    Ok(fields)
}

fn decode_terminal_node(value: &Value, kind: &str) -> Result<Node, String> {
    match kind {
        "integer" => {
            let fields = fields(value, &["kind", "value"])?;
            match fields.get("value") {
                Some(Value::Number(number)) => Ok(Node::Integer(*number)),
                _ => Err("editor integer must be I64".into()),
            }
        }
        "boolean" => {
            let fields = fields(value, &["kind", "value"])?;
            match fields.get("value") {
                Some(Value::Bool(boolean)) => Ok(Node::Boolean(*boolean)),
                _ => Err("editor boolean must be Boolean".into()),
            }
        }
        "word" => {
            let fields = fields(value, &["kind", "value"])?;
            match fields.get("value") {
                Some(Value::Text(word)) => Ok(Node::Word(word.clone())),
                _ => Err("editor word must be text".into()),
            }
        }
        "hole" => {
            fields(value, &["kind"])?;
            Ok(Node::Hole)
        }
        _ => Err("unsupported editor node kind".into()),
    }
}

fn decode_nodes(nodes: &[Value]) -> Result<Vec<Node>, String> {
    struct Frame<'a> {
        nodes: &'a [Value],
        next: usize,
        decoded: Vec<Node>,
    }

    let mut stack: Vec<Frame<'_>> = Vec::new();
    let mut current = nodes;
    let mut next = 0_usize;
    let mut decoded = Vec::with_capacity(nodes.len());
    let mut count = 0_usize;
    while next < current.len() || !stack.is_empty() {
        if next == current.len() {
            let Some(parent) = stack.pop() else {
                break;
            };
            let Frame { nodes, next: parent_next, decoded: mut parent_decoded } = parent;
            parent_decoded.push(Node::Quotation(decoded));
            current = nodes;
            next = parent_next;
            decoded = parent_decoded;
            continue;
        }

        // Each saved parent is one quotation enclosing the current siblings.
        let depth = stack.len();
        let value = &current[next];
        next = next.saturating_add(1);
        count = count.saturating_add(1);
        let nodes_limit = usize::try_from(super::SOURCE_LIMITS.nodes)
            .map_err(|_| "editor syntax node or depth limit exceeded".to_owned())?;
        let depth_limit = usize::try_from(super::SOURCE_LIMITS.depth)
            .map_err(|_| "editor syntax node or depth limit exceeded".to_owned())?;
        if count > nodes_limit || depth > depth_limit {
            return Err("editor syntax node or depth limit exceeded".into());
        }
        let kind = value.member("kind").and_then(Value::text)
            .ok_or("editor node kind is missing")?;
        if kind == "quotation" {
            let fields = fields(value, &["kind", "nodes"])?;
            let children = fields.get("nodes").and_then(Value::items)
                .ok_or("editor quotation nodes must be an array")?;
            if children.is_empty() {
                decoded.push(Node::Quotation(std::vec![]));
                continue;
            }
            // A nonempty quotation must visit its first child at the next depth.
            // Empty quotations have no child and remain valid at the limit.
            if stack.len() >= depth_limit {
                return Err("editor syntax node or depth limit exceeded".into());
            }
            stack.push(Frame { nodes: current, next, decoded });
            current = children;
            next = 0;
            decoded = Vec::with_capacity(children.len());
            continue;
        }
        decoded.push(decode_terminal_node(value, kind)?);
    }
    Ok(decoded)
}

fn decode(bytes: &[u8]) -> Result<Candidate, String> {
    let bytes_limit = usize::try_from(super::SOURCE_LIMITS.bytes)
        .map_err(|_| "editor JSON byte limit exceeded".to_owned())?;
    if bytes.len() > bytes_limit {
        return Err("editor JSON byte limit exceeded".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "editor JSON is not UTF-8")?;
    let value = super::report::parse(text)?;
    let envelope = fields(&value, &["format", "nodes"])?;
    if envelope.get("format").and_then(Value::number) != Some(1) {
        return Err("unsupported editor format".into());
    }
    let nodes = envelope.get("nodes").and_then(Value::items)
        .ok_or("editor nodes must be an array")?;
    Ok(Candidate { format: 1, nodes: decode_nodes(nodes)? })
}

fn refusal(stage: &str, message: &str) -> Json {
    object([
        ("schema", string("noble-editor-report/v1")),
        ("stage", string(stage)),
        ("outcome", string("reject")),
        ("diagnostic", string(message)),
        ("accepted_program_created", Json::Bool(false)),
        ("guest_requests", Json::Number(0)),
        ("protected_operations", Json::Number(0)),
    ])
}

fn print(report: Json) {
    println!("{}", report.encode());
}

pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let result = run_inner(arguments);
    match result {
        Ok(code) => std::process::ExitCode::from(code),
        Err((stage, error)) => {
            print(refusal(stage, &error));
            std::process::ExitCode::from(2)
        }
    }
}

fn run_inner(arguments: &[std::ffi::OsString]) -> Result<u8, (&'static str, String)> {
    if !matches!(arguments.len(), 3 | 5) {
        return Err(("transport", "usage: noble editor analyze|admit JSON_FILE [--emit NEW_DIR]".into()));
    }
    let Some(action) = arguments.get(1).and_then(|value| value.to_str()) else {
        return Err(("transport", "editor action is not text".into()));
    };
    if !matches!(action, "analyze" | "admit") {
        return Err(("transport", "usage: noble editor analyze|admit JSON_FILE".into()));
    }
    let Some(path) = arguments.get(2) else {
        return Err(("transport", "editor input file is missing".into()));
    };
    let emit = if arguments.len() == 5 && action == "admit"
        && arguments.get(3).is_some_and(|value| value == "--emit") {
        arguments.get(4).map(std::path::PathBuf::from)
    } else if arguments.len() == 3 {
        None
    } else {
        return Err(("transport", "--emit NEW_DIR is only valid for editor admission".into()));
    };
    let bytes = super::framing::read_file(&std::path::PathBuf::from(path), super::SOURCE_LIMITS.bytes)
        .map_err(|error| ("transport", error.message))?;
    let candidate = decode(&bytes).map_err(|error| ("transport", error))?;
    if action == "analyze" {
        let session = noble_contracts::source::Session::new();
        let analysis = session.analyze_editor(&candidate, &[], super::SOURCE_LIMITS)
            .map_err(|error| ("analysis", error.diagnostic().message.clone()))?;
        let holes = analysis.holes.iter().map(|hole| object([
            ("input_stack", string(&hole.input_stack)),
            ("required_output_stack", string(&hole.output_stack)),
            ("required_output_suffix", Json::Array(hole.required_output_suffix.iter()
                .map(string).collect())),
            ("effect", object([
                ("known", Json::Array(hole.known_effects.iter()
                    .map(|id| Json::Number(u64::from(*id))).collect())),
                ("unresolved", Json::Bool(hole.unresolved_effect)),
            ])),
        ])).collect();
        print(object([
            ("schema", string("noble-editor-report/v1")),
            ("stage", string("analysis")),
            ("outcome", string("constraints")),
            ("holes", Json::Array(holes)),
            ("effect", object([
                ("known", Json::Array(analysis.known_effects.iter()
                    .map(|id| Json::Number(u64::from(*id))).collect())),
                ("unresolved", Json::Bool(analysis.unresolved_effect)),
            ])),
            ("accepted_program_created", Json::Bool(false)),
            ("guest_requests", Json::Number(0)),
            ("protected_operations", Json::Number(0)),
        ]));
        return Ok(0);
    }
    let source = candidate.admitted_source(super::SOURCE_LIMITS)
        .map_err(|error| ("admission", error.diagnostic().message.clone()))?;
    let mut core_args = vec![
        std::ffi::OsString::from("run"),
        std::ffi::OsString::from("editor-generated.noble"),
    ];
    if let Some(path) = &emit {
        core_args.push(std::ffi::OsString::from("--emit"));
        core_args.push(path.as_os_str().to_os_string());
    }
    let options = super::arguments::parse(&core_args)
        .map_err(|error| ("transport", error.message))?;
    if let Some(path) = &emit {
        std::fs::create_dir(path)
            .map_err(|error| ("transport", error.to_string()))?;
    }
    let mut session = super::Session::new(&options).map_err(|error| ("admission", error.message))?;
    let report = session.submit(&source, &options)
        .map_err(|error| ("admission", error.message))?;
    super::output::print(&report).map_err(|error| ("transport", error.message))?;
    Ok(report.exit())
}
