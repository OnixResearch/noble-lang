//! Strict versioned editor ingress. This transport never adds `.noble` syntax.

use crate::workflow::encoding::{object, string, Json};
use noble_contracts::source::editor::{Candidate, Node};

type Value = super::report::Value;

fn fields<'a>(
    value: &'a Value,
    expected: &[&str],
) -> Result<&'a std::collections::BTreeMap<String, Value>, String> {
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
            let Frame {
                nodes,
                next: parent_next,
                decoded: mut parent_decoded,
            } = parent;
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
        let kind = value
            .member("kind")
            .and_then(Value::text)
            .ok_or("editor node kind is missing")?;
        if kind == "quotation" {
            let fields = fields(value, &["kind", "nodes"])?;
            let children = fields
                .get("nodes")
                .and_then(Value::items)
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
            stack.push(Frame {
                nodes: current,
                next,
                decoded,
            });
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
    let nodes = envelope
        .get("nodes")
        .and_then(Value::items)
        .ok_or("editor nodes must be an array")?;
    Ok(Candidate {
        format: 1,
        nodes: decode_nodes(nodes)?,
    })
}

const SOURCE_STAGE: [&str; 5] = ["parse", "resolve", "link", "check", "acceptance"];

fn span_json(span: noble_contracts::Span) -> Json {
    object([
        ("kind", string("generated")),
        ("start", Json::Number(u64::from(span.start))),
        ("end", Json::Number(u64::from(span.end))),
    ])
}

/// One structured analysis failure, with the bounded editor node path the
/// synthetic span came from. A span the emitter never produced is reported as
/// unmapped rather than guessed, and no span is ever presented as a
/// `.noble`-file or caller-JSON offset.
fn failure_object(
    error: &noble_contracts::source::Error,
    paths: &[(noble_contracts::Span, Vec<u32>)],
) -> Json {
    let diagnostic = error.diagnostic();
    let source_stage = SOURCE_STAGE[error.stage() as usize];
    let lookup = paths
        .iter()
        .find(|(span, _)| *span == diagnostic.span)
        .map(|(_, path)| path.clone());
    let node_path = match &lookup {
        Some(path) => {
            let indices: Vec<Json> = path
                .iter()
                .map(|index| Json::Number(u64::from(*index)))
                .collect();
            Json::Array(indices)
        }
        None => Json::Null,
    };
    let join = match diagnostic.join() {
        Some(join) => object([
            ("failing_word", string(&join.word)),
            ("expected_stack", string(&join.expected_stack)),
            ("actual_stack", string(&join.actual_stack)),
            ("constraint", string(join.constraint)),
            (
                "value_origin",
                match join.value_origin {
                    Some(origin) => span_json(origin),
                    None => Json::Null,
                },
            ),
        ]),
        None => Json::Null,
    };
    object([
        ("source_stage", string(source_stage)),
        ("category", string(diagnostic.kind.category())),
        ("span", span_json(diagnostic.span)),
        (
            "node_path_status",
            string(if lookup.is_some() {
                "mapped"
            } else {
                "unmapped"
            }),
        ),
        ("node_path", node_path),
        ("join", join),
    ])
}

/// One editor refusal: the operation stage, the English diagnostic kept for
/// compatibility, and the structured source-analysis failure when the
/// candidate reached the contracts layer.
struct Refusal {
    stage: &'static str,
    message: String,
    failure: Option<Json>,
}

fn rejection(stage: &'static str, message: &str) -> Refusal {
    Refusal {
        stage,
        message: message.into(),
        failure: None,
    }
}

fn refusal_report(refusal: Refusal) -> Json {
    let mut fields: std::vec::Vec<(&'static str, Json)> = std::vec![
        ("schema", string("noble-editor-report/v1")),
        ("stage", string(refusal.stage)),
        ("outcome", string("reject")),
        ("diagnostic", string(&refusal.message)),
    ];
    if let Some(failure) = refusal.failure {
        fields.push(("failure", failure));
    }
    fields.push(("accepted_program_created", Json::Bool(false)));
    fields.push(("guest_requests", Json::Number(0)));
    fields.push(("protected_operations", Json::Number(0)));
    Json::Object(fields)
}

fn print(report: Json) {
    println!("{}", report.encode());
}

pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let result = run_inner(arguments);
    match result {
        Ok(code) => std::process::ExitCode::from(code),
        Err(refusal) => {
            print(refusal_report(refusal));
            std::process::ExitCode::from(2)
        }
    }
}

fn run_inner(arguments: &[std::ffi::OsString]) -> Result<u8, Refusal> {
    if !matches!(arguments.len(), 3 | 5) {
        return Err(rejection(
            "transport",
            "usage: noble editor analyze|admit JSON_FILE [--emit NEW_DIR]",
        ));
    }
    let Some(action) = arguments.get(1).and_then(|value| value.to_str()) else {
        return Err(rejection("transport", "editor action is not text"));
    };
    if !matches!(action, "analyze" | "admit") {
        return Err(rejection(
            "transport",
            "usage: noble editor analyze|admit JSON_FILE",
        ));
    }
    let Some(path) = arguments.get(2) else {
        return Err(rejection("transport", "editor input file is missing"));
    };
    let emit = if arguments.len() == 5
        && action == "admit"
        && arguments.get(3).is_some_and(|value| value == "--emit")
    {
        arguments.get(4).map(std::path::PathBuf::from)
    } else if arguments.len() == 3 {
        None
    } else {
        return Err(rejection(
            "transport",
            "--emit NEW_DIR is only valid for editor admission",
        ));
    };
    let bytes =
        super::framing::read_file(&std::path::PathBuf::from(path), super::SOURCE_LIMITS.bytes)
            .map_err(|error| rejection("transport", &error.message))?;
    let candidate = decode(&bytes).map_err(|error| rejection("transport", &error))?;
    if action == "analyze" {
        let session = noble_contracts::source::Session::new();
        let analysis = match session.analyze_editor(&candidate, &[], super::SOURCE_LIMITS) {
            Ok(analysis) => analysis,
            Err(error) => {
                let paths = candidate
                    .editor_paths(super::SOURCE_LIMITS)
                    .unwrap_or_default();
                return Err(Refusal {
                    stage: "analysis",
                    message: error.diagnostic().message.clone(),
                    failure: Some(failure_object(&error, paths.as_slice())),
                });
            }
        };
        let holes = analysis
            .holes
            .iter()
            .map(|hole| {
                object([
                    ("input_stack", string(&hole.input_stack)),
                    ("required_output_stack", string(&hole.output_stack)),
                    (
                        "required_output_suffix",
                        Json::Array(hole.required_output_suffix.iter().map(string).collect()),
                    ),
                    (
                        "effect",
                        object([
                            (
                                "known",
                                Json::Array(
                                    hole.known_effects
                                        .iter()
                                        .map(|id| Json::Number(u64::from(*id)))
                                        .collect(),
                                ),
                            ),
                            ("unresolved", Json::Bool(hole.unresolved_effect)),
                        ]),
                    ),
                ])
            })
            .collect();
        print(object([
            ("schema", string("noble-editor-report/v1")),
            ("stage", string("analysis")),
            ("outcome", string("constraints")),
            ("holes", Json::Array(holes)),
            (
                "effect",
                object([
                    (
                        "known",
                        Json::Array(
                            analysis
                                .known_effects
                                .iter()
                                .map(|id| Json::Number(u64::from(*id)))
                                .collect(),
                        ),
                    ),
                    ("unresolved", Json::Bool(analysis.unresolved_effect)),
                ]),
            ),
            ("accepted_program_created", Json::Bool(false)),
            ("guest_requests", Json::Number(0)),
            ("protected_operations", Json::Number(0)),
        ]));
        return Ok(0);
    }
    let source = match candidate.admitted_source(super::SOURCE_LIMITS) {
        Ok(source) => source,
        Err(error) => {
            let paths = candidate
                .editor_paths(super::SOURCE_LIMITS)
                .unwrap_or_default();
            return Err(Refusal {
                stage: "admission",
                message: error.diagnostic().message.clone(),
                failure: Some(failure_object(&error, paths.as_slice())),
            });
        }
    };
    let mut core_args = vec![
        std::ffi::OsString::from("run"),
        std::ffi::OsString::from("editor-generated.noble"),
    ];
    if let Some(path) = &emit {
        core_args.push(std::ffi::OsString::from("--emit"));
        core_args.push(path.as_os_str().to_os_string());
    }
    let options = super::arguments::parse(&core_args)
        .map_err(|error| rejection("transport", &error.message))?;
    if let Some(path) = &emit {
        std::fs::create_dir(path).map_err(|error| rejection("transport", &error.to_string()))?;
    }
    let mut session =
        super::Session::new(&options).map_err(|error| rejection("admission", &error.message))?;
    let report = session
        .submit(&source, &options)
        .map_err(|error| rejection("admission", &error.message))?;
    super::output::print(&report).map_err(|error| rejection("transport", &error.message))?;
    Ok(report.exit())
}

// Controlled injection: no safe deterministic editor fixture produces an
// unsupported, exhausted or internal analysis outcome, so the reporting
// boundary is exercised directly with captured diagnostics.
#[cfg(test)]
mod tests {
    use super::*;
    use noble_contracts::source::Stage;

    fn field<'a>(json: &'a Json, key: &str) -> &'a Json {
        let Json::Object(entries) = json else {
            panic!("expected an object");
        };
        entries
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("missing field {key}"))
    }

    fn text(json: &Json) -> &str {
        let Json::String(text) = json else {
            panic!("expected a string");
        };
        text
    }

    fn failure(kind: noble_contracts::DiagnosticKind) -> Json {
        let error = noble_contracts::source::Error::reported(
            Stage::Check,
            noble_contracts::Diagnostic::new(
                kind,
                noble_contracts::Span { start: 3, end: 7 },
                "diagnostic prose that clients must not parse",
            ),
        );
        failure_object(&error, &[])
    }

    #[test]
    fn every_analysis_category_survives_the_reporting_boundary() {
        let categories = [
            (noble_contracts::DiagnosticKind::Invalid, "invalid"),
            (noble_contracts::DiagnosticKind::Unsupported, "unsupported"),
            (noble_contracts::DiagnosticKind::Exhausted, "exhausted"),
            (noble_contracts::DiagnosticKind::Internal, "internal"),
        ];
        let mut seen = std::vec::Vec::new();
        for (kind, category) in categories {
            let report = failure(kind);
            assert_eq!(text(field(&report, "category")), category);
            assert_eq!(text(field(&report, "source_stage")), "check");
            assert_eq!(text(field(&report, "node_path_status")), "unmapped");
            assert_eq!(text(field(field(&report, "span"), "kind")), "generated");
            // The structured failure carries no prose for clients to parse.
            assert!(matches!(report, Json::Object(ref entries)
                if !entries.iter().any(|(name, _)| *name == "diagnostic")));
            assert!(!seen.contains(&category));
            seen.push(category);
        }
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn join_provenance_and_origin_survive_the_reporting_boundary() {
        let word = "héllo☃ héllo☃";
        let span = noble_contracts::Span { start: 11, end: 20 };
        let diagnostic = noble_contracts::Diagnostic::new(
            noble_contracts::DiagnosticKind::Invalid,
            noble_contracts::Span { start: 0, end: 20 },
            "two words did not join",
        )
        .join_with(noble_contracts::JoinDiagnostic {
            word: word.into(),
            expected_stack: "?stack I64 I64".into(),
            actual_stack: "?stack Text".into(),
            constraint: "joining:stack",
            value_origin: Some(span),
        });
        let error = noble_contracts::source::Error::reported(Stage::Acceptance, diagnostic);
        let report = failure_object(
            &error,
            &[(noble_contracts::Span { start: 0, end: 20 }, std::vec![0, 1])],
        );
        assert_eq!(text(field(&report, "source_stage")), "acceptance");
        assert_eq!(text(field(&report, "node_path_status")), "mapped");
        let join = field(&report, "join");
        assert_eq!(text(field(join, "failing_word")), word);
        assert_eq!(text(field(join, "expected_stack")), "?stack I64 I64");
        assert_eq!(text(field(join, "actual_stack")), "?stack Text");
        assert_eq!(text(field(join, "constraint")), "joining:stack");
        assert!(matches!(
            field(field(join, "value_origin"), "start"),
            Json::Number(11)
        ));

        // An unavailable origin is reported as unavailable rather than guessed.
        let diagnostic = noble_contracts::Diagnostic::new(
            noble_contracts::DiagnosticKind::Invalid,
            noble_contracts::Span { start: 0, end: 9 },
            "no origin",
        )
        .join_with(noble_contracts::JoinDiagnostic {
            word: "dup dup".into(),
            expected_stack: "?stack".into(),
            actual_stack: "?stack".into(),
            constraint: "joining:stack",
            value_origin: None,
        });
        let error = noble_contracts::source::Error::reported(Stage::Resolve, diagnostic);
        let report = failure_object(&error, &[]);
        let join = field(&report, "join");
        assert!(matches!(field(join, "value_origin"), Json::Null));
        assert_eq!(text(field(&report, "node_path_status")), "unmapped");
        let Json::Null = field(&report, "node_path") else {
            panic!("an unmapped failure must not claim a node path");
        };
    }
}
