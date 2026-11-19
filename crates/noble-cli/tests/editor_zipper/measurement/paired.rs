//! One paired cell: the same resulting tree twice, once encoded from the
//! cursor and once as the corpus's independent JSON edit.
//!
//! The pair is the whole measurement. Both documents go to the real editor
//! commands, and nothing here decides a verdict: it compares what the commands
//! printed, and for admitted trees it compares the bytes they emitted.

use super::corpus::{contains_hole, decode, first_hole_address};
use super::navigation::document::encode;
use super::transport::{digest, failure_field, field, same_document, verdict, Editor, Outcome};
use super::{Document, Error};
use noble_contracts::source::editor::Node;

type OutcomePair<'a> = (&'a Outcome, &'a Outcome);
type DocumentPair<'a> = (&'a str, &'a str);

pub(super) fn pair(
    editor: &Editor,
    cell: &str,
    cursor_tree: &[Node],
    edit_document: &str,
) -> Result<Document, Error> {
    let cursor_document = encode(cursor_tree);
    let cursor_origin = if cursor_tree.is_empty() {
        "encoder_only_empty_root"
    } else {
        "cursor_reconstruction"
    };
    let edit_tree = decode(&serde_json::from_str::<Document>(edit_document)?)?;
    if contains_hole(cursor_tree)? != contains_hole(&edit_tree)? {
        return Err(std::format!("{cell}: the two documents disagree about holes").into());
    }

    let analyze_cursor = editor.run("analyze", &cursor_document, false)?;
    let analyze_edit = editor.run("analyze", edit_document, false)?;
    let admit_cursor = editor.run("admit", &cursor_document, false)?;
    let admit_edit = editor.run("admit", edit_document, false)?;
    same_outcome(cell, &analyze_cursor, &analyze_edit)?;
    same_outcome(cell, &admit_cursor, &admit_edit)?;

    let schemas = report_schemas(cell, &analyze_cursor, &admit_cursor)?;
    let admission = if admit_cursor.exit == 0 {
        admitted(
            cell,
            editor,
            (&cursor_document, edit_document),
            (&admit_cursor, &admit_edit),
        )?
    } else if contains_hole(cursor_tree)? {
        hole_admission(cell, &admit_cursor, cursor_tree)?
    } else {
        Document::Null
    };

    Ok(serde_json::json!({
        "cell": cell,
        "cursor_document": cursor_origin,
        "schemas": schemas,
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

/// The exit code and the whole parsed report must agree.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: both arms read the parsed reports, and the refusal formats
    // the two exit codes.
)]
fn same_outcome(cell: &str, cursor: &Outcome, edit: &Outcome) -> Result<(), Error> {
    if cursor.exit != edit.exit {
        return Err(std::format!(
            "{cell}: exit codes differ: cursor {} edit {}",
            cursor.exit,
            edit.exit
        )
        .into());
    }
    same_document(&cursor.report, &edit.report, cell)
}

/// C6: the report contract itself is unchanged. An analysis answers with the
/// editor report; an admission answers with the core report or the editor
/// refusal, and never with a third shape.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the check clones parsed report fields and formats a
    // refusal, which allocates.
)]
fn report_schemas(cell: &str, analyze: &Outcome, admit: &Outcome) -> Result<Document, Error> {
    let analyze_schema = field(&analyze.report, "schema");
    let admit_schema = field(&admit.report, "schema");
    if analyze_schema != Document::String("noble-editor-report/v1".into()) {
        return Err(std::format!("{cell}: unexpected analyze schema {analyze_schema}").into());
    }
    if admit_schema != Document::String("noble-core-report/v1".into())
        && admit_schema != Document::String("noble-editor-report/v1".into())
    {
        return Err(std::format!("{cell}: unexpected admit schema {admit_schema}").into());
    }
    Ok(serde_json::json!({"analyze": analyze_schema, "admit": admit_schema}))
}

/// Both documents admit: the emitted source bytes and the module digests must
/// agree, because the route passed those same bytes to submission.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the comparison reads two emitted artifacts and reports
    // their digests, which allocates.
)]
fn admitted(
    cell: &str,
    editor: &Editor,
    documents: DocumentPair<'_>,
    outcomes: OutcomePair<'_>,
) -> Result<Document, Error> {
    let (cursor_document, edit_document) = documents;
    let (cursor, edit) = outcomes;
    let emitted_cursor = editor.run("admit", cursor_document, true)?;
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
    same_document(
        &digest(&cursor.report, "source_sha256"),
        &digest(&edit.report, "source_sha256"),
        cell,
    )?;
    same_document(
        &digest(&cursor.report, "wasm_sha256"),
        &digest(&edit.report, "wasm_sha256"),
        cell,
    )?;
    Ok(serde_json::json!({
        "bytes_equal": true,
        "bytes": bytes_cursor.len(),
        "source_sha256": digest(&cursor.report, "source_sha256"),
        "edit_source_sha256": digest(&edit.report, "source_sha256"),
        "wasm_sha256": digest(&cursor.report, "wasm_sha256"),
        "edit_wasm_sha256": digest(&edit.report, "wasm_sha256"),
    }))
}

/// A hole-bearing tree refused before any submission. When its own syntax
/// builds, the refusal is the hole refusal and must name the first hole the
/// cursor itself would address; a tree whose syntax refuses earlier never
/// reaches hole acceptance, and the recorded stage says which happened.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the check reads a parsed report and formats diagnostics.
)]
fn hole_admission(cell: &str, outcome: &Outcome, tree: &[Node]) -> Result<Document, Error> {
    if outcome.exit != 2
        || field(&outcome.report, "accepted_program_created") != Document::Bool(false)
        || field(&outcome.report, "guest_requests") != serde_json::json!(0)
    {
        return Err(std::format!(
            "{cell}: a hole-bearing tree was not refused as before: {}",
            outcome.report
        )
        .into());
    }
    let stage = failure_field(&outcome.report, "source_stage");
    if stage == Document::String("acceptance".into()) {
        check_hole_address(cell, outcome, tree)?;
    }
    Ok(serde_json::json!({
        "hole_refusal": stage,
        "node_path": failure_field(&outcome.report, "node_path"),
        "guest_requests": 0,
    }))
}

/// The hole refusal must map its generated span to the first hole's own
/// address, which is the address the cursor reports for that hole.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the check allocates the expected address list and formats
    // a refusal.
)]
fn check_hole_address(cell: &str, outcome: &Outcome, tree: &[Node]) -> Result<(), Error> {
    let status = failure_field(&outcome.report, "node_path_status");
    if status != Document::String("mapped".into()) {
        return Err(std::format!(
            "{cell}: the hole refusal does not map its node path: {}",
            outcome.report
        )
        .into());
    }
    let address: Vec<Document> = first_hole_address(tree)?
        .iter()
        .flatten()
        .map(|index| serde_json::json!(index))
        .collect();
    if failure_field(&outcome.report, "node_path") == Document::Array(address) {
        return Ok(());
    }
    Err(std::format!(
        "{cell}: the hole refusal does not name the first hole: {}",
        outcome.report
    )
    .into())
}
