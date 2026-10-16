//! `noble ngraph`: the opt-in `Numeric-Graph-v1` entry point (design D7–D11
//! of `.cairn/changes/numeric-graph-v1`). It reads only the files named by
//! its arguments, writes only its `--out` file, and runs no subprocess, Wasm
//! or network operation. Graph semantics live in the pure `noble-ngraph`
//! library; this module supplies bytes, budgets and the BLAKE3 identity
//! scheme, and renders `noble-ngraph-report/v1` lines.
//!
//! Non-claims: no performance, GPU or backend-selection claim (the Phase C
//! measurement recommended stop); NG-PO-A..H stay open; agreement between
//! evaluators and replay are finite differential evidence.

mod arguments;
mod files;
mod report;

use crate::workflow::encoding::{string, Json};
use arguments::{Kind, Parsed, Spec};
use noble_ngraph::format::snapshot::SnapshotFile;
use noble_ngraph::{CheckedGraph, Code, Diagnostic, IdentityHasher, IdentityScheme, Site};
use report::Line;

pub(crate) const USAGE: &str = "usage:
  noble ngraph check GRAPH [--budget-KIND N]...
  noble ngraph extract SOURCE --inputs T[,T...] --out GRAPH [--framed] [--profile ngraph-i64/0]
      [--budget-KIND N]...
  noble ngraph eval GRAPH VECTORS [--reference] [--snapshot FILE [--snapshot-id HEX]] [--fuel N]
      [--budget-KIND N]...
  noble ngraph train MODEL --loss LOSS --data DATA --policy ngraph-deriv-real/0 --optimizer adam/0
      --seed U64 --steps N --out SNAPSHOT [--from PARENT] [--opt-in SPEC]...
      [--adam-lr X] [--adam-beta1 X] [--adam-beta2 X] [--adam-eps X] [--budget-KIND N]...
  noble ngraph verify GRAPH SNAPSHOT [--snapshot-id HEX] [--replay --loss LOSS --data DATA
      [--from PARENT]] [--budget-KIND N]...
  T: i64 | bool. SPEC: %N:boolean=straight-through | %N:boolean=clamped-linear:0xBITS |
      %N:index=zero | %N:index=backward-difference. X: 0x<binary64 bits> or a decimal.
  KIND: work expansion inline-depth live-nodes layers inputs outputs affine-terms table-length
      table-entries facts graph-bytes snapshot-bytes forward-scratch backward-scratch steps
      (each subcommand accepts the kinds it charges; a value may only lower the /0 default).
  Reports are noble-ngraph-report/v1 JSON lines. Exit: 0 normal, 1 trap, 2 reject or usage,
      4 unsupported, 5 exhausted.";

/// BLAKE3-256, the identity scheme of `Numeric-Graph-v1` reports and files.
struct Blake3;

static IDS: Blake3 = Blake3;

struct Running(blake3::Hasher);

impl IdentityHasher for Running {
    fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
    fn finish(self: Box<Self>) -> [u8; 32] {
        *self.0.finalize().as_bytes()
    }
}

impl IdentityScheme for Blake3 {
    fn name(&self) -> &'static str {
        "blake3"
    }
    fn start(&self) -> Box<dyn IdentityHasher + '_> {
        Box::new(Running(blake3::Hasher::new()))
    }
}

/// Why a subcommand stopped before its normal report.
enum Stop {
    /// One coded or host refusal line.
    Report(Line),
    /// A usage error (stderr usage, exit 2, empty stdout).
    Usage(std::string::String),
}

impl From<Line> for Stop {
    fn from(line: Line) -> Self {
        Self::Report(line)
    }
}

fn usage<T>(message: impl Into<std::string::String>) -> Result<T, Stop> {
    Err(Stop::Usage(message.into()))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; ngraph routing reports usage errors on stderr and every other outcome as report lines with their exit status. Malformed input must not panic."
)]
pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let rest = arguments.get(2..).unwrap_or(&[]);
    let result = match arguments.get(1).and_then(|s| s.to_str()) {
        Some("check") => check(rest),
        Some("extract") => extract(rest),
        Some("eval") => eval(rest),
        Some("train") => train(rest),
        Some("verify") => verify(rest),
        _ => usage("expected check, extract, eval, train or verify"),
    };
    let lines = match result {
        Ok(lines) => lines,
        Err(Stop::Report(line)) => std::vec![line],
        Err(Stop::Usage(message)) => {
            eprintln!("noble ngraph: {message}\n{USAGE}");
            return std::process::ExitCode::from(2);
        }
    };
    let exit = lines.iter().find(|l| !l.is_normal()).map_or(0, Line::exit);
    let mut out = std::io::stdout().lock();
    for line in &lines {
        if let Err(error) = std::io::Write::write_all(&mut out, (line.encode() + "\n").as_bytes()) {
            eprintln!("noble ngraph: report output failed: {error}");
            return std::process::ExitCode::from(2);
        }
    }
    if let Err(error) = std::io::Write::flush(&mut out) {
        eprintln!("noble ngraph: report output failed: {error}");
        return std::process::ExitCode::from(2);
    }
    std::process::ExitCode::from(exit)
}

fn kinds(groups: &[&[Kind]]) -> std::vec::Vec<Kind> {
    groups.iter().flat_map(|g| g.iter().copied()).collect()
}

/// The `/0` policy: a raised budget is `E-PROFILE`.
fn policy(command: &'static str, budgets: &noble_ngraph::Budgets) -> Result<(), Line> {
    budgets
        .validate_policy()
        .map_err(|d| Line::diagnostic(command, "request", &d))
}

fn read(
    command: &'static str,
    path: &std::path::Path,
    limit: u64,
    what: &str,
) -> Result<std::vec::Vec<u8>, Line> {
    files::read(path, limit, what).map_err(|m| Line::host(command, "input", m))
}

/// Prefixes a diagnostic's detail with the file it concerns.
fn about(mut d: Diagnostic, what: &str) -> Diagnostic {
    d.detail = std::format!("{what}: {}", d.detail);
    d
}

fn text(
    command: &'static str,
    bytes: std::vec::Vec<u8>,
    what: &str,
    code: Code,
) -> Result<std::string::String, Line> {
    std::string::String::from_utf8(bytes).map_err(|_| {
        Line::diagnostic(
            command,
            "input",
            &Diagnostic::new(code, Site::Graph, std::format!("{what} is not UTF-8 text")),
        )
    })
}

/// Reads, parses and checks a graph file.
fn load_graph(
    command: &'static str,
    path: &std::path::Path,
    budgets: &noble_ngraph::Budgets,
    what: &str,
) -> Result<CheckedGraph, Line> {
    let bytes = attempt!(read(command, path, files::GRAPH_BYTES, what));
    let source = attempt!(text(
        command,
        bytes,
        what,
        Code::GraphMalformed(noble_ngraph::Malformed::Syntax)
    ));
    let graph = attempt!(noble_ngraph::format::graph::parse(&IDS, &source)
        .map_err(|d| Line::diagnostic(command, "input", &about(d, what))));
    noble_ngraph::check_with(&IDS, graph, budgets)
        .map_err(|d| Line::diagnostic(command, "check", &about(d, what)))
}

/// Reads and parses a snapshot file (identities recomputed).
fn load_snapshot(
    command: &'static str,
    path: &std::path::Path,
    budgets: &noble_ngraph::Budgets,
    what: &str,
) -> Result<SnapshotFile, Line> {
    let bytes = attempt!(read(command, path, files::SNAPSHOT_BYTES, what));
    let source = attempt!(text(command, bytes, what, Code::Format));
    SnapshotFile::parse(&IDS, &source, budgets.snapshot_bytes)
        .map_err(|d| Line::diagnostic(command, "input", &about(d, what)))
}

fn type_list(types: impl Iterator<Item = noble_ngraph::ScalarType>) -> Json {
    Json::Array(types.map(|t| string(t.name())).collect())
}

fn input_types(c: &CheckedGraph) -> std::vec::Vec<noble_ngraph::ScalarType> {
    c.graph()
        .slots
        .iter()
        .filter(|s| s.kind == noble_ngraph::ir::SlotKind::Input)
        .map(|s| s.ty)
        .collect()
}

/// The D7 `check` fields.
fn check_fields(line: Line, c: &CheckedGraph) -> Line {
    let g = c.graph();
    let outputs = g
        .outputs
        .iter()
        .map(|o| c.ty(*o).unwrap_or(noble_ngraph::ScalarType::F32));
    line.with("graph", report::digest(&c.id()))
        .with("profile", string(g.profile.name()))
        .with(
            "subject",
            string(noble_ngraph::format::graph::subject_text(&g.subject)),
        )
        .with(
            "closure",
            Json::Array(g.subject.closure.iter().map(|d| Json::Number(*d)).collect()),
        )
        .with("inputs", type_list(input_types(c).into_iter()))
        .with("params", report::count(c.param_slots().len()))
        .with("outputs", type_list(outputs))
        .with("live_nodes", report::count(c.node_count()))
        .with("layers", Json::Number(u64::from(c.layer_count())))
        .with("facts", report::count(g.facts.len()))
        .with("param_layout", report::digest(&c.param_layout()))
}

fn interface(command: &'static str, detail: std::string::String) -> Line {
    Line::diagnostic(
        command,
        "input",
        &Diagnostic::new(Code::Interface, Site::Graph, detail),
    )
}

fn names(types: &[noble_ngraph::ScalarType]) -> std::string::String {
    let n: std::vec::Vec<&str> = types.iter().map(|t| t.name()).collect();
    std::format!("[{}]", n.join(" "))
}

fn check(arguments: &[std::ffi::OsString]) -> Result<std::vec::Vec<Line>, Stop> {
    const C: &str = "check";
    let args = attempt!(arguments::parse(
        &Spec {
            positional: &["GRAPH"],
            values: &[],
            repeated: &[],
            switches: &[],
            budgets: &arguments::CHECKER,
        },
        arguments
    )
    .map_err(Stop::Usage));
    attempt!(policy(C, &args.budgets));
    let graph = attempt!(load_graph(C, &args.path(0), &args.budgets, "graph file"));
    Ok(std::vec![check_fields(Line::normal(C, "check"), &graph)])
}

/// The submissions of a Core source file: newline-separated (blank lines
/// skipped) or `noble session` framing.
fn submissions(
    source: &[u8],
    framed: bool,
) -> Result<std::vec::Vec<std::vec::Vec<u8>>, std::string::String> {
    let mut reader = std::io::Cursor::new(source);
    let mut out = std::vec::Vec::new();
    loop {
        match crate::core::framing::read_submission(&mut reader, framed, files::SUBMISSION_BYTES) {
            Ok(Some(s)) => {
                if framed || !s.iter().all(u8::is_ascii_whitespace) {
                    out.push(s);
                }
            }
            Ok(None) => return Ok(out),
            Err(_) => {
                return Err(std::format!(
                    "the source is not valid {} with submissions of at most {} bytes",
                    if framed { "session framing" } else { "text" },
                    files::SUBMISSION_BYTES
                ))
            }
        }
    }
}

fn subject_refusal(command: &'static str, detail: std::string::String) -> Line {
    Line::diagnostic(
        command,
        "admission",
        &Diagnostic::new(Code::Subject, Site::Graph, detail),
    )
}

fn extract(arguments: &[std::ffi::OsString]) -> Result<std::vec::Vec<Line>, Stop> {
    const C: &str = "extract";
    let budget_kinds = kinds(&[&arguments::EXTRACTION, &arguments::CHECKER]);
    let args = attempt!(arguments::parse(
        &Spec {
            positional: &["SOURCE"],
            values: &["--inputs", "--out", "--profile"],
            repeated: &[],
            switches: &["--framed"],
            budgets: &budget_kinds,
        },
        arguments
    )
    .map_err(Stop::Usage));
    let inputs_text = attempt!(args.required("--inputs").map_err(Stop::Usage));
    let mut inputs = std::vec::Vec::new();
    for t in inputs_text.split(',') {
        inputs.push(match t {
            "i64" => noble_kernel::types::Ty::I64,
            "bool" => noble_kernel::types::Ty::Bool,
            _ => return usage(std::format!("--inputs takes i64 or bool types, not {t:?}")),
        });
    }
    let Some(out) = args.path_value("--out") else {
        return usage("--out is required");
    };
    let profile = attempt!(args.text("--profile").map_err(Stop::Usage))
        .unwrap_or_else(|| "ngraph-i64/0".to_owned());
    let budgets = args.budgets.clone();
    attempt!(policy(C, &budgets));
    let source = attempt!(read(C, &args.path(0), files::SOURCE_BYTES, "source file"));
    let parts = attempt!(
        submissions(&source, args.switch("--framed")).map_err(|m| Line::host(C, "input", m))
    );
    let mut texts = std::vec::Vec::with_capacity(parts.len());
    for p in parts {
        texts.push(attempt!(std::string::String::from_utf8(p).map_err(|_| {
            subject_refusal(C, "a source submission is not UTF-8 text".into())
        })));
    }
    let Some((root, definitions)) = texts.split_last() else {
        return Err(subject_refusal(C, "the source has no root submission".into()).into());
    };
    let definitions: std::vec::Vec<&str> = definitions
        .iter()
        .map(std::string::String::as_str)
        .collect();
    let prepared = attempt!(
        noble_ngraph::extract::source::prepare(&definitions, root, &inputs)
            .map_err(|e| subject_refusal(C, e))
    );
    let admitted = attempt!(noble_ngraph::extract::admit(
        &IDS,
        &prepared.submission,
        &noble_ngraph::extract::Request {
            profile: &profile,
            claimed_closure: &prepared.claimed_closure,
            budget: noble_ngraph::extract::Budget {
                work_units: budgets.work_units,
                expanded_nodes: budgets.expanded_nodes,
                inline_depth: budgets.inline_depth,
            },
        },
    )
    .map_err(|d| Line::diagnostic(C, "admission", &d)));
    let extraction = attempt!(noble_ngraph::extract::extract(&IDS, &admitted, &budgets)
        .map_err(|d| Line::diagnostic(C, "extraction", &d)));
    let graph_text = noble_ngraph::format::graph::print(extraction.checked.graph());
    attempt!(files::publish(&out, graph_text.as_bytes()).map_err(|m| Line::host(C, "output", m)));
    let s = extraction.stats;
    Ok(std::vec![check_fields(
        Line::normal(C, "extraction"),
        &extraction.checked
    )
    .with("work_units", Json::Number(s.work_units))
    .with("expanded_nodes", Json::Number(s.expanded_nodes))
    .with("pruned_nodes", Json::Number(s.pruned_nodes))
    .with("inline_depth", Json::Number(s.max_inline_depth))])
}

fn eval(arguments: &[std::ffi::OsString]) -> Result<std::vec::Vec<Line>, Stop> {
    const C: &str = "eval";
    let budget_kinds = kinds(&[
        &arguments::CHECKER,
        &[arguments::FORWARD_SCRATCH, arguments::SNAPSHOT_BYTES],
    ]);
    let args = attempt!(arguments::parse(
        &Spec {
            positional: &["GRAPH", "VECTORS"],
            values: &["--snapshot", "--snapshot-id", "--fuel"],
            repeated: &[],
            switches: &["--reference"],
            budgets: &budget_kinds,
        },
        arguments
    )
    .map_err(Stop::Usage));
    let fuel = match attempt!(args.text("--fuel").map_err(Stop::Usage)) {
        Some(t) => Some(attempt!(
            arguments::unsigned("--fuel", &t).map_err(Stop::Usage)
        )),
        None => None,
    };
    let pinned = attempt!(pinned(&args));
    if pinned.is_some() && !args.has("--snapshot") {
        return usage("--snapshot-id needs --snapshot");
    }
    attempt!(policy(C, &args.budgets));
    let graph = attempt!(load_graph(C, &args.path(0), &args.budgets, "graph file"));
    let snapshot = match args.path_value("--snapshot") {
        Some(p) => {
            let f = attempt!(load_snapshot(C, &p, &args.budgets, "snapshot file"));
            attempt!(f
                .bind(&IDS, &graph, pinned.as_ref())
                .map_err(|d| Line::diagnostic(C, "binding", &d)));
            Some(f)
        }
        None if !graph.param_slots().is_empty() => {
            return Err(Line::diagnostic(
                C,
                "binding",
                &Diagnostic::new(
                    Code::SnapshotLayout,
                    Site::Graph,
                    "the graph has param slots but no --snapshot was given",
                ),
            )
            .into())
        }
        None => None,
    };
    let bytes = attempt!(read(C, &args.path(1), files::VECTORS_BYTES, "vector file"));
    let source = attempt!(text(C, bytes, "vector file", Code::Format));
    let vectors = attempt!(noble_ngraph::format::vectors::parse(&source)
        .map_err(|d| Line::diagnostic(C, "input", &about(d, "vector file"))));
    let expected = input_types(&graph);
    if vectors.types != expected {
        return Err(interface(
            C,
            std::format!(
                "the vector file declares types {}; the graph's inputs are {}",
                names(&vectors.types),
                names(&expected)
            ),
        )
        .into());
    }
    let control = noble_ngraph::Control {
        fuel,
        forward_scratch: Some(args.budgets.forward_scratch),
        ..noble_ngraph::Control::default()
    };
    let param_snapshot = snapshot.as_ref().map(|f| &f.snapshot);
    let mut lines = std::vec![Line::normal(C, "evaluation")
        .with("graph", report::digest(&graph.id()))
        .with(
            "snapshot",
            param_snapshot.map_or(Json::Null, |s| report::digest(&s.id()))
        )];
    let compiled =
        (!args.switch("--reference")).then(|| noble_ngraph::compiled::Compiled::new(&IDS, &graph));
    let mut workspace = compiled
        .as_ref()
        .map(noble_ngraph::compiled::Compiled::workspace);
    for (i, row) in vectors.rows.iter().enumerate() {
        let obs = match (&compiled, &mut workspace) {
            (Some(c), Some(w)) => c.evaluate_in(w, row, param_snapshot, &control).obs,
            _ => noble_ngraph::reference(&graph, row, param_snapshot, &control).obs,
        };
        lines.push(match &obs {
            noble_ngraph::Obs::Normal(values) => Line::normal(C, "evaluation")
                .with("vector", report::count(i))
                .with(
                    "outputs",
                    Json::Array(
                        values
                            .iter()
                            .map(|v| string(noble_ngraph::format::value::render(*v)))
                            .collect(),
                    ),
                ),
            other => Line::observation(C, other).with("vector", report::count(i)),
        });
    }
    Ok(lines)
}

fn pinned(args: &Parsed) -> Result<Option<[u8; 32]>, Stop> {
    match attempt!(args.text("--snapshot-id").map_err(Stop::Usage)) {
        Some(t) => arguments::digest("--snapshot-id", &t)
            .map(Some)
            .map_err(Stop::Usage),
        None => Ok(None),
    }
}

/// Parses `--opt-in` values into `deriv`, validated against `model`.
fn opt_ins(
    specs: &[&str],
    model: &CheckedGraph,
) -> Result<noble_ngraph::grad::DerivOptions, std::string::String> {
    use noble_ngraph::grad::{BooleanPolicy, IndexPolicy};
    use noble_ngraph::ir::{NodeId, Op};
    let mut deriv = noble_ngraph::grad::DerivOptions::default();
    for spec in specs {
        let bad = || std::format!("--opt-in {spec:?} is not a derivative opt-in");
        let (node, rest) = attempt!(spec
            .strip_prefix('%')
            .and_then(|s| s.split_once(':'))
            .ok_or_else(bad));
        let node = NodeId(attempt!(node.parse::<u32>().map_err(|_| bad())));
        let op = &attempt!(model
            .graph()
            .nodes
            .get(node.index())
            .ok_or_else(|| std::format!("--opt-in {spec:?} names no node of the model")))
        .op;
        let (kind, rule) = attempt!(rest.split_once('=').ok_or_else(bad));
        let inapplicable = || {
            std::format!(
                "--opt-in {spec:?} does not apply to %{} ({})",
                node.0,
                op.name()
            )
        };
        match kind {
            "boolean" => {
                let policy = match rule.split_once(':') {
                    Some(("clamped-linear", w)) => {
                        let bits = attempt!(w
                            .strip_prefix("0x")
                            .filter(|h| !h.is_empty() && h.len() <= 8)
                            .and_then(|h| u32::from_str_radix(h, 16).ok())
                            .ok_or_else(bad));
                        if !matches!(op, Op::F32Step(_)) {
                            return Err(inapplicable());
                        }
                        BooleanPolicy::ClampedLinear(f32::from_bits(bits))
                    }
                    None if rule == "straight-through" => {
                        if !matches!(op, Op::F32Step(_) | Op::F32Select { .. }) {
                            return Err(inapplicable());
                        }
                        BooleanPolicy::StraightThrough
                    }
                    _ => return Err(bad()),
                };
                if deriv.boolean.insert(node, policy).is_some() {
                    return Err(std::format!(
                        "--opt-in gives %{} two boolean policies",
                        node.0
                    ));
                }
            }
            "index" => {
                let policy = match rule {
                    "zero" => IndexPolicy::Zero,
                    "backward-difference" => IndexPolicy::BackwardDifference,
                    _ => return Err(bad()),
                };
                if !matches!(op, Op::F32Index { .. }) {
                    return Err(inapplicable());
                }
                if deriv.index.insert(node, policy).is_some() {
                    return Err(std::format!(
                        "--opt-in gives %{} two index policies",
                        node.0
                    ));
                }
            }
            _ => return Err(bad()),
        }
    }
    Ok(deriv)
}

/// Reads training data and checks its types against the model and loss.
fn load_points(
    command: &'static str,
    path: &std::path::Path,
    model: &CheckedGraph,
    loss: &CheckedGraph,
) -> Result<std::vec::Vec<noble_ngraph::train::Point>, Line> {
    let bytes = attempt!(read(command, path, files::DATA_BYTES, "data file"));
    let source = attempt!(text(command, bytes, "data file", Code::Format));
    let data = attempt!(
        noble_ngraph::format::data::parse(&source).map_err(|d| Line::diagnostic(
            command,
            "input",
            &about(d, "data file")
        ))
    );
    let inputs = input_types(model);
    let targets: std::vec::Vec<noble_ngraph::ScalarType> = loss
        .graph()
        .slots
        .iter()
        .skip(model.graph().outputs.len())
        .map(|s| s.ty)
        .collect();
    if data.inputs != inputs || data.targets != targets {
        return Err(interface(
            command,
            std::format!(
                "the data file declares types {} ; {}; the model's inputs are {} and the loss's targets {}",
                names(&data.inputs),
                names(&data.targets),
                names(&inputs),
                names(&targets)
            ),
        ));
    }
    data.to_points()
        .map_err(|d| Line::diagnostic(command, "input", &about(d, "data file")))
}

fn train(arguments: &[std::ffi::OsString]) -> Result<std::vec::Vec<Line>, Stop> {
    const C: &str = "train";
    let budget_kinds = kinds(&[
        &arguments::CHECKER,
        &[
            arguments::SNAPSHOT_BYTES,
            arguments::BACKWARD_SCRATCH,
            arguments::STEPS,
        ],
    ]);
    let args = attempt!(arguments::parse(
        &Spec {
            positional: &["MODEL"],
            values: &[
                "--loss",
                "--data",
                "--policy",
                "--optimizer",
                "--seed",
                "--steps",
                "--out",
                "--from",
                "--adam-lr",
                "--adam-beta1",
                "--adam-beta2",
                "--adam-eps",
            ],
            repeated: &["--opt-in"],
            switches: &[],
            budgets: &budget_kinds,
        },
        arguments
    )
    .map_err(Stop::Usage));
    let (Some(loss_path), Some(data_path), Some(out)) = (
        args.path_value("--loss"),
        args.path_value("--data"),
        args.path_value("--out"),
    ) else {
        return usage("--loss, --data and --out are required");
    };
    let policy_name = attempt!(args.required("--policy").map_err(Stop::Usage));
    let optimizer_name = attempt!(args.required("--optimizer").map_err(Stop::Usage));
    let seed_text = attempt!(args.required("--seed").map_err(Stop::Usage));
    let seed = attempt!(arguments::unsigned("--seed", &seed_text).map_err(Stop::Usage));
    let steps_text = attempt!(args.required("--steps").map_err(Stop::Usage));
    let Ok(steps) = steps_text.parse::<u64>() else {
        return usage("--steps takes an unsigned 64-bit integer");
    };
    let mut adam = noble_ngraph::train::Adam::default();
    for (flag, slot) in [
        ("--adam-lr", &mut adam.lr),
        ("--adam-beta1", &mut adam.beta1),
        ("--adam-beta2", &mut adam.beta2),
        ("--adam-eps", &mut adam.eps),
    ] {
        if let Some(t) = attempt!(args.text(flag).map_err(Stop::Usage)) {
            *slot = attempt!(arguments::binary64(flag, &t).map_err(Stop::Usage));
        }
    }
    let budgets = args.budgets.clone();
    attempt!(policy(C, &budgets));
    if policy_name != noble_ngraph::grad::DERIV_POLICY {
        return Err(
            profile_refusal(C, std::format!("unknown derivative policy {policy_name:?}")).into(),
        );
    }
    if optimizer_name != noble_ngraph::train::ADAM {
        return Err(
            profile_refusal(C, std::format!("unknown optimizer {optimizer_name:?}")).into(),
        );
    }
    let model = attempt!(load_graph(C, &args.path(0), &budgets, "model graph"));
    let loss = attempt!(load_graph(C, &loss_path, &budgets, "loss graph"));
    let deriv = attempt!(opt_ins(&args.all("--opt-in"), &model).map_err(Stop::Usage));
    let params = model.param_slots().to_vec();
    attempt!(noble_ngraph::train::admit(&model, &loss, &params, &deriv)
        .map_err(|d| Line::diagnostic(C, "admission", &d)));
    let data = attempt!(load_points(C, &data_path, &model, &loss));
    let parent = match args.path_value("--from") {
        Some(p) => {
            let f = attempt!(load_snapshot(C, &p, &budgets, "parent snapshot"));
            attempt!(f.bind(&IDS, &model, None).map_err(|d| Line::diagnostic(
                C,
                "binding",
                &about(d, "parent snapshot")
            )));
            Some(f)
        }
        None => None,
    };
    let contract = noble_ngraph::train::Contract {
        model,
        loss,
        params,
        deriv,
        adam,
        seed,
        data,
    };
    let control = noble_ngraph::train::TrainControl {
        budgets,
        ..noble_ngraph::train::TrainControl::default()
    };
    let trained = attempt!(noble_ngraph::train::train(
        &IDS,
        &contract,
        parent.as_ref().map(|f| &f.snapshot),
        steps,
        &control
    )
    .map_err(|s| Line::diagnostic(C, "training", &s.diagnostic)
        .with("published_steps", Json::Number(s.published_steps))));
    let losses = (
        trained.losses.first().copied().unwrap_or(f64::NAN),
        trained.losses.last().copied().unwrap_or(f64::NAN),
    );
    let file = SnapshotFile {
        snapshot: trained.snapshot,
        provenance: trained.provenance,
    };
    attempt!(
        files::publish(&out, file.print(&IDS).as_bytes()).map_err(|m| Line::host(C, "output", m))
    );
    Ok(std::vec![Line::normal(C, "training")
        .with("snapshot", report::digest(&file.snapshot.id()))
        .with("graph", report::digest(&file.snapshot.graph()))
        .with("parent", report::digest(&file.provenance.parent))
        .with("steps", Json::Number(file.provenance.steps))
        .with("loss_initial", report::bits64(losses.0))
        .with("loss_final", report::bits64(losses.1))])
}

fn profile_refusal(command: &'static str, detail: std::string::String) -> Line {
    Line::diagnostic(
        command,
        "request",
        &Diagnostic::new(Code::Profile, Site::Graph, detail),
    )
}

fn verify(arguments: &[std::ffi::OsString]) -> Result<std::vec::Vec<Line>, Stop> {
    const C: &str = "verify";
    let budget_kinds = kinds(&[
        &arguments::CHECKER,
        &[
            arguments::SNAPSHOT_BYTES,
            arguments::BACKWARD_SCRATCH,
            arguments::STEPS,
        ],
    ]);
    let args = attempt!(arguments::parse(
        &Spec {
            positional: &["GRAPH", "SNAPSHOT"],
            values: &["--snapshot-id", "--loss", "--data", "--from"],
            repeated: &[],
            switches: &["--replay"],
            budgets: &budget_kinds,
        },
        arguments
    )
    .map_err(Stop::Usage));
    let replay = args.switch("--replay");
    if !replay && (args.has("--loss") || args.has("--data") || args.has("--from")) {
        return usage("--loss, --data and --from need --replay");
    }
    if replay && !(args.has("--loss") && args.has("--data")) {
        return usage("--replay needs --loss and --data");
    }
    let pinned = attempt!(pinned(&args));
    let budgets = args.budgets.clone();
    attempt!(policy(C, &budgets));
    let graph = attempt!(load_graph(C, &args.path(0), &budgets, "graph file"));
    let file = attempt!(load_snapshot(C, &args.path(1), &budgets, "snapshot file"));
    attempt!(file
        .bind(&IDS, &graph, pinned.as_ref())
        .map_err(|d| Line::diagnostic(C, "binding", &d)));
    if replay {
        let (Some(loss_path), Some(data_path)) =
            (args.path_value("--loss"), args.path_value("--data"))
        else {
            return usage("--replay needs --loss and --data");
        };
        let loss = attempt!(load_graph(C, &loss_path, &budgets, "loss graph"));
        let data = attempt!(load_points(C, &data_path, &graph, &loss));
        let parent = match args.path_value("--from") {
            Some(p) => {
                let f = attempt!(load_snapshot(C, &p, &budgets, "parent snapshot"));
                attempt!(f.bind(&IDS, &graph, None).map_err(|d| Line::diagnostic(
                    C,
                    "binding",
                    &about(d, "parent snapshot")
                )));
                Some(f)
            }
            None => None,
        };
        let p = &file.provenance;
        let contract = noble_ngraph::train::Contract {
            params: graph.param_slots().to_vec(),
            model: graph.clone(),
            loss,
            deriv: p.deriv.clone(),
            adam: p.optimizer,
            seed: p.seed,
            data,
        };
        let control = noble_ngraph::train::TrainControl {
            budgets,
            ..noble_ngraph::train::TrainControl::default()
        };
        attempt!(noble_ngraph::train::replay(
            &IDS,
            &contract,
            p,
            parent.as_ref().map(|f| &f.snapshot),
            &control
        )
        .map_err(|s| Line::diagnostic(C, "replay", &s.diagnostic)));
    }
    Ok(std::vec![Line::normal(
        C,
        if replay { "replay" } else { "binding" }
    )
    .with("snapshot", report::digest(&file.snapshot.id()))
    .with("graph", report::digest(&graph.id()))
    .with("bound", Json::Bool(true))
    .with("replayed", Json::Bool(replay))])
}
