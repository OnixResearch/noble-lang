//! Frozen-corpus measurement for the opt-in zipper cursor (issue #142).
//!
//! Every measurement here goes through the real editor transport: the
//! documents are handed to `noble editor analyze` / `noble editor admit` as a
//! child process, and the verdicts compared are the ones those commands print.
//! The cursor itself is the same source file the `noble-cli` test build
//! declares (`src/core/editor/zipper.rs`), included here with `#[path]` so the
//! measured code is byte-identical to the reviewed module.
//!
//! What this lane is not: it is not a theorem, a refinement, or a
//! correspondence between a model and this code. The cited paper's cost
//! bounds, completeness results and timings are the authors' claims and are
//! never thresholds here (see `verification/editor-zipper/criteria.json`).
//!
//! Usage (from the repository root, receipt produced from a clean commit):
//!   NOBLE_EDITOR_ZIPPER_RECEIPT=<absolute path outside the repository> \
//!     cargo test -p noble-cli --test editor -- editor_zipper
//! Without that variable the same assertions run and nothing is written.

#[path = "../../src/core/editor/zipper.rs"]
mod cursor;

use cursor::{Cursor, Refusal};
use noble_contracts::source::editor::Node;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::vec::Vec;

type Error = Box<dyn std::error::Error>;
type Document = serde_json::Value;

const CORPUS_PATH: &str = "verification/editor-zipper/corpus.json";
const CRITERIA_PATH: &str = "verification/editor-zipper/criteria.json";
const IMPLEMENTATION: [&str; 3] = [
    "crates/noble-cli/src/core/editor/zipper.rs",
    "crates/noble-cli/tests/editor_zipper/measurement.rs",
    "crates/noble-cli/tests/editor.rs",
];
const RECEIPT_VARIABLE: &str = "NOBLE_EDITOR_ZIPPER_RECEIPT";

// The refusal-class literals exist only in the test-only module; the shipped
// binary must contain none of them, which is how the default-off criterion is
// measured on the actual artifact rather than asserted from the source layout.
// A schema literal that the shipped CLI does contain is the positive control,
// so an empty search cannot pass for an absent cursor.
const CURSOR_MARKERS: [&str; 2] = ["empty-quotation", "no-such-node"];
const SHIPPED_CONTROL: &str = "noble-editor-report/v1";

#[derive(serde::Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    origin: String,
    tree: Document,
    #[serde(default)]
    sequences: Vec<Sequence>,
    #[serde(default)]
    replacements: Vec<Replacement>,
    #[serde(default)]
    refusals: Vec<RefusalCell>,
}

#[derive(serde::Deserialize)]
struct Sequence {
    name: String,
    at: Vec<u32>,
    steps: Vec<Step>,
    addresses: Vec<Vec<u32>>,
    tree: Option<Document>,
}

#[derive(serde::Deserialize)]
struct Step {
    op: String,
}

#[derive(serde::Deserialize)]
struct Replacement {
    name: String,
    at: Vec<u32>,
    node: Document,
    tree: Document,
}

#[derive(serde::Deserialize)]
struct RefusalCell {
    name: String,
    at: Vec<u32>,
    op: String,
    class: String,
}

/// One cursor step the corpus can document.
#[derive(Clone, Copy)]
enum Operation {
    Focus,
    Enter,
    Leave,
    Next,
    Previous,
}

fn operation(name: &str) -> Result<Operation, Error> {
    match name {
        "focus" => Ok(Operation::Focus),
        "enter" => Ok(Operation::Enter),
        "leave" => Ok(Operation::Leave),
        "next" => Ok(Operation::Next),
        "previous" => Ok(Operation::Previous),
        other => Err(std::format!("unknown corpus operation {other}").into()),
    }
}

fn refused(refusal: Refusal) -> Error {
    std::format!("cursor refused with {}", refusal.class()).into()
}

/// Address a node and, unless the step only addresses it, apply one move.
fn attempt(tree: &[Node], address: &[u32], step: Operation) -> Result<Cursor, Refusal> {
    let cursor = Cursor::at(tree, address)?;
    match step {
        Operation::Focus => Ok(cursor),
        Operation::Enter => cursor.enter(),
        Operation::Leave => cursor.leave(),
        Operation::Next => cursor.next(),
        Operation::Previous => cursor.previous(),
    }
}

fn focus(tree: &[Node], address: &[u32]) -> Result<Cursor, Error> {
    Cursor::at(tree, address).map_err(refused)
}

fn decode(document: &Document) -> Result<Vec<Node>, Error> {
    document
        .get("nodes")
        .and_then(Document::as_array)
        .ok_or_else(|| Error::from("a corpus document needs a nodes array"))
        .and_then(|nodes| decode_list(nodes))
}

fn decode_list(nodes: &[Document]) -> Result<Vec<Node>, Error> {
    let mut decoded = Vec::with_capacity(nodes.len());
    for node in nodes {
        decoded.push(decode_node(node)?);
    }
    Ok(decoded)
}

fn decode_node(value: &Document) -> Result<Node, Error> {
    let object = value
        .as_object()
        .ok_or("a corpus node must be a JSON object")?;
    let kind = object
        .get("kind")
        .and_then(Document::as_str)
        .ok_or("a corpus node needs a string kind")?;
    match kind {
        "integer" => {
            required_fields(object, &["kind", "value"])?;
            let value = object
                .get("value")
                .and_then(Document::as_i64)
                .ok_or("a corpus integer needs an I64 value")?;
            Ok(Node::Integer(value))
        }
        "boolean" => {
            required_fields(object, &["kind", "value"])?;
            let value = object
                .get("value")
                .and_then(Document::as_bool)
                .ok_or("a corpus boolean needs a boolean value")?;
            Ok(Node::Boolean(value))
        }
        "word" => {
            required_fields(object, &["kind", "value"])?;
            let value = object
                .get("value")
                .and_then(Document::as_str)
                .ok_or("a corpus word needs a text value")?;
            Ok(Node::Word(value.into()))
        }
        "hole" => {
            required_fields(object, &["kind"])?;
            Ok(Node::Hole)
        }
        "quotation" => {
            required_fields(object, &["kind", "nodes"])?;
            let children = object
                .get("nodes")
                .and_then(Document::as_array)
                .ok_or("a corpus quotation needs a nodes array")?;
            Ok(Node::Quotation(decode_list(children)?))
        }
        other => Err(std::format!("unsupported corpus node kind {other}").into()),
    }
}

fn required_fields(
    object: &serde_json::Map<String, Document>,
    expected: &[&str],
) -> Result<(), Error> {
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err("a corpus node has missing or unexpected fields".into());
    }
    Ok(())
}

fn collect_addresses(tree: &[Node], prefix: &[u32], into: &mut Vec<Vec<u32>>) -> Result<(), Error> {
    for (at, node) in tree.iter().enumerate() {
        let mut address = prefix.to_vec();
        address.push(u32::try_from(at)?);
        into.push(address.clone());
        if let Node::Quotation(children) = node {
            collect_addresses(children, &address, into)?;
        }
    }
    Ok(())
}

/// The `node_path` of the first hole in source order, as the transport names
/// the first hole of a refused tree.
fn first_hole(tree: &[Node], prefix: &[u32], into: &mut Option<Vec<u32>>) -> Result<(), Error> {
    for (at, node) in tree.iter().enumerate() {
        let mut address = prefix.to_vec();
        address.push(u32::try_from(at)?);
        match node {
            Node::Hole => {
                if into.is_none() {
                    *into = Some(address);
                }
            }
            Node::Quotation(children) => first_hole(children, &address, into)?,
            Node::Integer(_) | Node::Boolean(_) | Node::Word(_) => {}
        }
    }
    Ok(())
}

fn contains_hole(tree: &[Node]) -> bool {
    let mut found = None;
    let _ = first_hole(tree, &[], &mut found);
    found.is_some()
}

/// The editor transport as a child process, with a counter that proves a
/// refused move never reaches it.
struct Editor {
    binary: PathBuf,
    temporary: PathBuf,
    invocations: std::cell::Cell<usize>,
    next: std::cell::Cell<usize>,
}

struct Outcome {
    exit: i32,
    report: Document,
    emitted: Option<Vec<u8>>,
}

impl Editor {
    fn new() -> Result<Self, Error> {
        let temporary =
            std::env::temp_dir().join(std::format!("noble-editor-zipper-{}", std::process::id()));
        if temporary.exists() {
            std::fs::remove_dir_all(&temporary)?;
        }
        std::fs::create_dir_all(&temporary)?;
        Ok(Self {
            binary: PathBuf::from(env!("CARGO_BIN_EXE_noble")),
            temporary,
            invocations: std::cell::Cell::new(0),
            next: std::cell::Cell::new(0),
        })
    }

    fn run(&self, action: &str, document: &str, emit: bool) -> Result<Outcome, Error> {
        let number = self.next.get();
        self.next.set(number.saturating_add(1));
        let input = self.temporary.join(std::format!("input-{number}.json"));
        std::fs::write(&input, document)?;
        let directory = self.temporary.join(std::format!("emit-{number}"));
        let mut command = Command::new(&self.binary);
        command.args(["editor", action]).arg(&input);
        if emit {
            command.arg("--emit").arg(&directory);
        }
        let output = command.output()?;
        self.invocations
            .set(self.invocations.get().saturating_add(1));
        std::fs::remove_file(&input)?;
        let text = std::str::from_utf8(&output.stdout)?.trim();
        let report: Document = if text.is_empty() {
            Document::Null
        } else {
            serde_json::from_str(text)?
        };
        let emitted = if emit {
            let path = directory.join("engine").join("module-1.noble");
            if path.exists() {
                Some(std::fs::read(path)?)
            } else {
                None
            }
        } else {
            None
        };
        Ok(Outcome {
            exit: output.status.code().unwrap_or(-1),
            report,
            emitted,
        })
    }
}

fn field(report: &Document, name: &str) -> Document {
    report.get(name).cloned().unwrap_or(Document::Null)
}

fn failure_field(report: &Document, name: &str) -> Document {
    report
        .get("failure")
        .and_then(|failure| failure.get(name))
        .cloned()
        .unwrap_or(Document::Null)
}

fn verdict(outcome: &Outcome) -> Document {
    serde_json::json!({
        "exit": outcome.exit,
        "stage": field(&outcome.report, "stage"),
        "outcome": field(&outcome.report, "outcome"),
        "category": failure_field(&outcome.report, "category"),
    })
}

fn digest(report: &Document, name: &str) -> Document {
    report
        .get("module")
        .and_then(|module| module.get(name))
        .cloned()
        .unwrap_or(Document::Null)
}

fn same_document(left: &Document, right: &Document, what: &str) -> Result<(), Error> {
    if left == right {
        return Ok(());
    }
    Err(std::format!("{what} differs: cursor {left} edit {right}").into())
}

fn repository_root() -> Result<PathBuf, Error> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(root.canonicalize()?)
}

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

fn sha256(path: &Path) -> Result<String, Error> {
    let output = Command::new("sha256sum").arg(path).output()?;
    let text = std::str::from_utf8(&output.stdout)?;
    let digest = text
        .split_whitespace()
        .next()
        .ok_or("sha256sum printed no digest")?;
    if digest.len() != 64 {
        return Err(std::format!("sha256sum printed no digest for {}", path.display()).into());
    }
    Ok(digest.to_owned())
}

fn git(root: &Path, arguments: &[&str]) -> Result<String, Error> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()?;
    if !output.status.success() {
        return Err(std::format!("git {arguments:?} failed").into());
    }
    Ok(std::str::from_utf8(&output.stdout)?.to_owned())
}

fn tool_version(tool: &str) -> Document {
    match Command::new(tool).arg("-V").output() {
        Ok(output) if output.status.success() => match std::str::from_utf8(&output.stdout) {
            Ok(text) => Document::String(text.trim().to_owned()),
            Err(_) => Document::Null,
        },
        _ => Document::Null,
    }
}

/// One paired cell: the same resulting tree twice, once encoded from the
/// cursor and once as the corpus's independent JSON edit.
fn pair(
    editor: &Editor,
    cell: &str,
    cursor_tree: &[Node],
    edit_document: &str,
) -> Result<Document, Error> {
    let cursor_document = cursor::encode(cursor_tree);
    let edit_tree = decode(&serde_json::from_str::<Document>(edit_document)?)?;
    if contains_hole(cursor_tree) != contains_hole(&edit_tree) {
        return Err(std::format!("{cell}: the two documents disagree about holes").into());
    }

    let analyze_cursor = editor.run("analyze", &cursor_document, false)?;
    let analyze_edit = editor.run("analyze", edit_document, false)?;
    let admit_cursor = editor.run("admit", &cursor_document, false)?;
    let admit_edit = editor.run("admit", edit_document, false)?;

    if analyze_cursor.exit != analyze_edit.exit {
        return Err(std::format!("{cell}: analyze exit codes differ").into());
    }
    same_document(&analyze_cursor.report, &analyze_edit.report, cell)?;
    if admit_cursor.exit != admit_edit.exit {
        return Err(std::format!("{cell}: admit exit codes differ").into());
    }
    same_document(&admit_cursor.report, &admit_edit.report, cell)?;

    // C6: the report contract itself is unchanged. An analysis answers with
    // the editor report; an admission answers with the core report or the
    // editor refusal, and never with a third shape.
    let analyze_schema = field(&analyze_cursor.report, "schema");
    let admit_schema = field(&admit_cursor.report, "schema");
    if analyze_schema != Document::String("noble-editor-report/v1".into())
        || (admit_schema != Document::String("noble-core-report/v1".into())
            && admit_schema != Document::String("noble-editor-report/v1".into()))
    {
        return Err(std::format!(
            "{cell}: unexpected report schema {analyze_schema} / {admit_schema}"
        )
        .into());
    }

    let mut admission = Document::Null;
    if admit_cursor.exit == 0 {
        let emitted_cursor = editor.run("admit", &cursor_document, true)?;
        let emitted_edit = editor.run("admit", edit_document, true)?;
        let bytes_cursor = emitted_cursor
            .emitted
            .as_ref()
            .ok_or_else(|| Error::from(std::format!("{cell}: cursor admit emitted no source")))?;
        let bytes_edit = emitted_edit
            .emitted
            .as_ref()
            .ok_or_else(|| Error::from(std::format!("{cell}: edit admit emitted no source")))?;
        if bytes_cursor != bytes_edit {
            return Err(std::format!("{cell}: admitted source bytes differ").into());
        }
        admission = serde_json::json!({
            "bytes_equal": true,
            "bytes": bytes_cursor.len(),
            "source_sha256": digest(&admit_cursor.report, "source_sha256"),
            "edit_source_sha256": digest(&admit_edit.report, "source_sha256"),
            "wasm_sha256": digest(&admit_cursor.report, "wasm_sha256"),
            "edit_wasm_sha256": digest(&admit_edit.report, "wasm_sha256"),
        });
        same_document(
            &digest(&admit_cursor.report, "source_sha256"),
            &digest(&admit_edit.report, "source_sha256"),
            cell,
        )?;
        same_document(
            &digest(&admit_cursor.report, "wasm_sha256"),
            &digest(&admit_edit.report, "wasm_sha256"),
            cell,
        )?;
    } else if contains_hole(cursor_tree) {
        // A hole-bearing tree must refuse before any submission. When its own
        // node syntax builds, the refusal is the hole refusal and it names the
        // first hole's own node path; a tree whose syntax refuses earlier
        // never reaches hole acceptance at all.
        if admit_cursor.exit != 2
            || field(&admit_cursor.report, "accepted_program_created") != Document::Bool(false)
            || field(&admit_cursor.report, "guest_requests") != serde_json::json!(0)
        {
            return Err(std::format!(
                "{cell}: a hole-bearing tree was not refused as before: {}",
                admit_cursor.report
            )
            .into());
        }
        let stage = failure_field(&admit_cursor.report, "source_stage");
        if stage == Document::String("acceptance".into()) {
            let mut hole = None;
            first_hole(cursor_tree, &[], &mut hole)?;
            let address: Vec<Document> = hole
                .iter()
                .flatten()
                .map(|index| serde_json::json!(index))
                .collect();
            if failure_field(&admit_cursor.report, "node_path_status")
                != Document::String("mapped".into())
                || failure_field(&admit_cursor.report, "node_path") != Document::Array(address)
            {
                return Err(std::format!(
                    "{cell}: the hole refusal does not name the first hole: {}",
                    admit_cursor.report
                )
                .into());
            }
        }
        admission = serde_json::json!({
            "hole_refusal": stage,
            "node_path": failure_field(&admit_cursor.report, "node_path"),
            "guest_requests": 0,
        });
    }

    Ok(serde_json::json!({
        "cell": cell,
        "schemas": {
            "analyze": analyze_schema,
            "admit": admit_schema,
        },
        "analyze": {
            "cursor": verdict(&analyze_cursor),
            "edit": verdict(&analyze_edit),
            "equal": true,
        },
        "admit": {
            "cursor": verdict(&admit_cursor),
            "edit": verdict(&admit_edit),
            "equal": true,
        },
        "admission": admission,
    }))
}

#[test]
fn editor_zipper_corpus_measurement() -> Result<(), Error> {
    let root = repository_root()?;
    let corpus_text = std::fs::read_to_string(root.join(CORPUS_PATH))?;
    let corpus: Corpus = serde_json::from_str(&corpus_text)?;
    let criteria_text = std::fs::read_to_string(root.join(CRITERIA_PATH))?;
    let criteria: Document = serde_json::from_str(&criteria_text)?;
    let editor = Editor::new()?;
    let mut cells: Vec<Document> = Vec::new();
    let mut addresses_checked = 0_usize;
    let mut sequences_checked = 0_usize;
    let mut replacements_checked = 0_usize;
    let mut refusals_checked = 0_usize;
    let mut pairs_checked = 0_usize;

    // C6: the shipped artifact carries no cursor code, so no default path can
    // reach it. The class literals are the module's own strings.
    let shipped = std::fs::read(env!("CARGO_BIN_EXE_noble"))?;
    if !contains(&shipped, SHIPPED_CONTROL.as_bytes()) {
        return Err(
            "the searched artifact is not the shipped editor CLI: its own schema literal is absent"
                .into(),
        );
    }
    let module = std::fs::read_to_string(root.join(IMPLEMENTATION[0]))?;
    let mut markers = serde_json::Map::new();
    for marker in CURSOR_MARKERS {
        if !module.contains(marker) {
            return Err(std::format!("the cursor module does not spell {marker}").into());
        }
        if contains(&shipped, marker.as_bytes()) {
            return Err(
                std::format!("the shipped binary contains the cursor marker {marker}").into(),
            );
        }
        markers.insert(marker.to_owned(), serde_json::json!(0));
    }
    cells.push(serde_json::json!({
        "cell": "default-off:shipped-binary",
        "declaration": "#[cfg(test)] mod zipper;",
        "markers": Document::Object(markers),
        "control": SHIPPED_CONTROL,
        "cursor_code_in_shipped_artifact": false,
    }));

    for case in &corpus.cases {
        let tree = decode(&case.tree)?;
        let original = cursor::encode(&tree);

        // C1: every address reconstructs the original tree, and the cursor
        // reports the address it was given.
        let mut addresses: Vec<Vec<u32>> = Vec::new();
        collect_addresses(&tree, &[], &mut addresses)?;
        for address in &addresses {
            let at = focus(&tree, address)?;
            if at.node_path() != *address {
                return Err(std::format!(
                    "{}: the cursor reported {:?} for {address:?}",
                    case.name,
                    at.node_path()
                )
                .into());
            }
            let reconstructed = cursor::encode(&at.reconstruct());
            if reconstructed != original {
                return Err(std::format!(
                    "{}: reconstruction at {address:?} is not the original tree",
                    case.name
                )
                .into());
            }
            if cursor::encode(&at.candidate().nodes) != original {
                return Err(std::format!(
                    "{}: the candidate at {address:?} is not the original tree",
                    case.name
                )
                .into());
            }
            addresses_checked = addresses_checked.saturating_add(1);
        }
        cells.push(serde_json::json!({
            "cell": std::format!("identity:{}", case.name),
            "origin": case.origin,
            "addresses": addresses.len(),
            "reconstruction": "original",
            "declared": true,
        }));

        // The identity pair: the corpus document and the cursor's own
        // identity reconstruction are the same tree, with the same verdicts.
        let identity = match addresses.first() {
            Some(first) => cursor::encode(&focus(&tree, first)?.reconstruct()),
            None => original.clone(),
        };
        cells.push(pair(
            &editor,
            &std::format!("original:{}", case.name),
            &tree,
            &identity,
        )?);
        pairs_checked = pairs_checked.saturating_add(1);

        // C2: documented navigation reaches the documented addresses and
        // reconstructs the documented tree.
        for sequence in &case.sequences {
            let mut at = focus(&tree, &sequence.at)?;
            if sequence.steps.len() != sequence.addresses.len() {
                return Err(std::format!(
                    "{}:{}: steps and addresses differ in length",
                    case.name,
                    sequence.name
                )
                .into());
            }
            for (step, address) in sequence.steps.iter().zip(sequence.addresses.iter()) {
                let moved =
                    attempt(&tree, &at.node_path(), operation(&step.op)?).map_err(|refusal| {
                        std::format!(
                            "{}:{}: {} refused with {}",
                            case.name,
                            sequence.name,
                            step.op,
                            refusal.class()
                        )
                    })?;
                if moved.node_path() != *address {
                    return Err(std::format!(
                        "{}:{}: {} reached {:?}, not {address:?}",
                        case.name,
                        sequence.name,
                        step.op,
                        moved.node_path()
                    )
                    .into());
                }
                at = moved;
            }
            let expected = match &sequence.tree {
                Some(document) => document.to_string(),
                None => original.clone(),
            };
            let expected: Document = serde_json::from_str(&expected)?;
            let reconstructed: Document = serde_json::from_str(&cursor::encode(&at.reconstruct()))?;
            same_document(&reconstructed, &expected, &sequence.name)?;
            sequences_checked = sequences_checked.saturating_add(1);
        }
        cells.push(serde_json::json!({
            "cell": std::format!("sequence:{}", case.name),
            "sequences": case.sequences.len(),
            "tree": "reconstructed",
        }));

        // C3: each documented replacement equals the independent JSON edit and
        // the two resulting trees have equal verdicts.
        for replacement in &case.replacements {
            let at = focus(&tree, &replacement.at)?;
            let replaced = at.replace(decode_node(&replacement.node)?);
            if replaced.node_path() != replacement.at {
                return Err(std::format!(
                    "{}:{}: the replacement moved the focus to {:?}",
                    case.name,
                    replacement.name,
                    replaced.node_path()
                )
                .into());
            }
            let produced: Document =
                serde_json::from_str(&cursor::encode(&replaced.reconstruct()))?;
            same_document(&produced, &replacement.tree, &replacement.name)?;
            cells.push(pair(
                &editor,
                &std::format!("replacement:{}:{}", case.name, replacement.name),
                &replaced.reconstruct(),
                &replacement.tree.to_string(),
            )?);
            replacements_checked = replacements_checked.saturating_add(1);
            pairs_checked = pairs_checked.saturating_add(1);
        }

        // C5: every illegal move refuses with its own class and reaches no
        // editor command.
        for refusal in &case.refusals {
            let before = editor.invocations.get();
            let step = operation(&refusal.op)?;
            let outcome = attempt(&tree, &refusal.at, step);
            if editor.invocations.get() != before {
                return Err(std::format!(
                    "{}:{}: a refused move reached an editor command",
                    case.name,
                    refusal.name
                )
                .into());
            }
            match outcome {
                Ok(_) => {
                    return Err(std::format!(
                        "{}:{}: {} was not refused",
                        case.name,
                        refusal.name,
                        refusal.op
                    )
                    .into())
                }
                Err(class) => {
                    if class.class() != refusal.class {
                        return Err(std::format!(
                            "{}:{}: refused with {}, not {}",
                            case.name,
                            refusal.name,
                            class.class(),
                            refusal.class
                        )
                        .into());
                    }
                }
            }
            refusals_checked = refusals_checked.saturating_add(1);
        }
        cells.push(serde_json::json!({
            "cell": std::format!("refusal:{}", case.name),
            "refusals": case.refusals.len(),
            "editor_commands": 0,
        }));
    }

    // The corpus is the frozen set of cases; a case that silently disappears
    // would otherwise shrink the measurement.
    if corpus.cases.len() != 12 {
        return Err(std::format!("the frozen corpus has {} cases", corpus.cases.len()).into());
    }
    if refusals_checked == 0 || sequences_checked == 0 || replacements_checked == 0 {
        return Err("the frozen corpus left a cell family unexercised".into());
    }

    let summary = serde_json::json!({
        "cases": corpus.cases.len(),
        "addresses": addresses_checked,
        "sequences": sequences_checked,
        "replacements": replacements_checked,
        "refusals": refusals_checked,
        "verdict_pairs": pairs_checked,
        "editor_commands": editor.invocations.get(),
        "mismatches": 0,
    });

    let receipt = serde_json::json!({
        "schema": "noble-editor-zipper-receipt/v1",
        "issue": 142,
        "lane": "verification/editor-zipper",
        "criteria": {
            "path": CRITERIA_PATH,
            "sha256": sha256(&root.join(CRITERIA_PATH))?,
            "frozen": criteria.get("frozen").cloned().unwrap_or(Document::Null),
            "go_rule": criteria.get("go_rule").cloned().unwrap_or(Document::Null),
        },
        "corpus": {
            "path": CORPUS_PATH,
            "sha256": sha256(&root.join(CORPUS_PATH))?,
        },
        "implementation": sources(&root)?,
        "source": {
            "commit": git(&root, &["rev-parse", "HEAD"])?.trim().to_owned(),
            "tree": git(&root, &["rev-parse", "HEAD^{tree}"])?.trim().to_owned(),
            "clean": git(&root, &["status", "--porcelain"])?.trim().is_empty(),
        },
        "toolchain": {
            "rustc": tool_version("rustc"),
            "cargo": tool_version("cargo"),
        },
        "command": std::env::args().collect::<Vec<std::string::String>>(),
        "summary": summary,
        "cells": cells,
        "non_claims": [
            "a passing corpus is not a theorem, a refinement or a model-to-implementation correspondence",
            "the cited paper's cost bounds, completeness results and timings are the authors' claims and are not measured or gated here",
            "the cursor is measured only over this frozen corpus of five node kinds; nothing here claims coverage of another editor tree",
            "the receipt compares verdicts of the unchanged editor transport; it does not establish anything about the kernel, the wasm backend or admission semantics beyond those comparisons",
        ],
    });

    if let Ok(destination) = std::env::var(RECEIPT_VARIABLE) {
        let path = PathBuf::from(&destination);
        if !path.is_absolute() || path.starts_with(&root) {
            return Err(std::format!(
                "{RECEIPT_VARIABLE} must be an absolute path outside the repository"
            )
            .into());
        }
        if !git(&root, &["status", "--porcelain"])?.trim().is_empty() {
            return Err("a receipt must come from a clean commit".into());
        }
        let text = serde_json::to_string_pretty(&receipt)?;
        std::fs::write(&path, std::format!("{text}\n"))?;
    }

    std::fs::remove_dir_all(&editor.temporary)?;
    Ok(())
}

/// The implementation files the receipt binds, each with its digest.
fn sources(root: &Path) -> Result<Vec<Document>, Error> {
    let mut listed: Vec<Document> = Vec::with_capacity(IMPLEMENTATION.len());
    for path in IMPLEMENTATION {
        listed.push(serde_json::json!({
            "path": path,
            "sha256": sha256(&root.join(path))?,
        }));
    }
    Ok(listed)
}
