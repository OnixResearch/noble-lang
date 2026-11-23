//! Read-only obligation map and goal view, and the `verify-module` report
//! revision that replaced the non-standard `unproved` outcome.
//!
//! None of these cases needs a Lean toolchain, a sandbox, or a library root:
//! they exercise preparation, the source checker's refusal path and the two
//! read-only projections only. The commands are run with the consumer
//! environment removed to prove that property.

// Justification: this harness reads JSON reports with panic-on-shape
// assertions, so unwraps, long setup legs and the repeated fixture path
// segment are the readable form, exactly as the enclosing target states.
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::function_length,
    tigerstyle::path_segment_repetition,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so unwraps and long setup legs are the readable form."
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const NO_EVIDENCE: &str = "module arithmetic@1 [ export increment def increment [ 1 + ] contract 1 increment-law [ subject increment input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] ]";

const WITH_PROOF: &str = "module arithmetic@1 [ export increment def increment [ 1 + ] contract 1 increment-law [ subject increment input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] proof 1 increment-correct for increment-law [ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ] ]";

const ILL_FORMED_PROOF: &str = "module broken@1 [ export value def value [ 1 ] proof 1 bad : [ (Pi (x I64) (Eq I64 x x)) ] [ (bogus x) ] ]";

const INCREMENT_CONTRACT: &str = "(contract 1 increment (input (x I64)) (output (y I64)) (program [ 1 + ]) (requires true) (ensures (eq (out y) (add (in x) 1))))";

const DEFINITIONS: &str = "module Definitions@5 [ export step def step [ 1 + ] ]";
const IMPORT: &str = "import Definitions@5 as d";
const NAMED_SUBJECT: &str = "module Subject@3 [ def twice [ d.step d.step ] contract 2 twice-law [ subject twice input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 2)) ] ] ]";

const SANDBOX_TOOLS: [&str; 6] = [
    "NOBLE_LEAN",
    "NOBLE_BWRAP",
    "NOBLE_PRLIMIT",
    "NOBLE_SYSTEMD_RUN",
    "NOBLE_NIX_STORE",
    "NOBLE_CONTRACT_LIBRARY",
];

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str, files: &[(&str, &str)]) -> std::io::Result<Self> {
        let base = std::env::temp_dir().join(std::format!(
            "noble-obligation-map-{label}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&base)?;
        for (name, source) in files {
            std::fs::write(base.join(name), source)?;
        }
        Ok(Self(base))
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn noble(arguments: &[&std::ffi::OsStr]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noble"));
    command.args(arguments);
    for key in SANDBOX_TOOLS {
        command.env_remove(key);
    }
    command
}

fn run(mut command: Command) -> Result<Output, Box<dyn std::error::Error>> {
    Ok(command.output()?)
}

fn report(output: &Output) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn digest_of(value: &Value) -> &str {
    value
        .as_str()
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .expect("a lowercase SHA-256 digest")
}

fn source(path: &Path) -> std::ffi::OsString {
    path.as_os_str().to_os_string()
}

#[test]
fn verify_module_reports_unknown_with_exit_three_under_v2() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::new("v2-no-evidence", &[("subject.noble", NO_EVIDENCE)])?;
    let subject = source(&fixture.path("subject.noble"));
    let output = run(noble(&[std::ffi::OsStr::new("verify-module"), &subject]))?;
    let report = report(&output)?;
    assert_eq!(
        output.status.code(),
        Some(3),
        "missing evidence is the VC-TOOL-02 unknown outcome: {}",
        text(&output)
    );
    assert_eq!(report["schema"], "noble-intrinsic-module-report/v2");
    assert_eq!(report["outcome"], "unknown");
    assert_eq!(report["category"], "no-evidence");
    assert_eq!(report["remediation"], "supply-a-proof-declaration");
    assert_eq!(report["stage"], "source-contract");
    assert_eq!(report["profile"], "Intrinsic-Proofs-Draft");
    digest_of(&report["source_sha256"]);
    Ok(())
}

#[test]
fn v1_keeps_the_published_field_set_and_success_exit() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("v1-no-evidence", &[("subject.noble", NO_EVIDENCE)])?;
    let subject = source(&fixture.path("subject.noble"));
    let output = run(noble(&[
        std::ffi::OsStr::new("verify-module"),
        &subject,
        std::ffi::OsStr::new("--report-schema"),
        std::ffi::OsStr::new("v1"),
    ]))?;
    let report = report(&output)?;
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(report["schema"], "noble-intrinsic-module-report/v1");
    assert_eq!(report["outcome"], "unproved");
    assert_eq!(report["stage"], "source-contract");
    assert!(
        report.get("category").is_none(),
        "v1 must not gain fields: {}",
        text(&output)
    );
    assert!(
        report.get("remediation").is_none(),
        "v1 must not gain fields: {}",
        text(&output)
    );
    let mut keys: Vec<&str> = report
        .as_object()
        .expect("a JSON object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "candidate_executions",
            "command",
            "guest_requests",
            "host_requests",
            "independent_recheck",
            "outcome",
            "profile",
            "proof_grants_host_authority",
            "protected_operations",
            "runtime_prover_calls",
            "schema",
            "source_path",
            "source_sha256",
            "stage",
        ],
        "the v1 field set is pinned"
    );
    let rendered = text(&output);
    assert!(
        rendered.starts_with(
            "{\"schema\":\"noble-intrinsic-module-report/v1\",\"profile\":\"Intrinsic-Proofs-Draft\",\"command\":\"verify-module\",\"outcome\":\"unproved\",\"stage\":\"source-contract\",\"source_path\":"
        ),
        "v1 keeps its published field order: {rendered}"
    );
    Ok(())
}

#[test]
fn a_repeated_or_unknown_report_schema_is_a_usage_refusal() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::new("schema-usage", &[("subject.noble", NO_EVIDENCE)])?;
    let subject = source(&fixture.path("subject.noble"));
    for (label, arguments) in [
        (
            "repeated",
            vec!["--report-schema", "v2", "--report-schema", "v2"],
        ),
        ("unknown", vec!["--report-schema", "v3"]),
    ] {
        let mut command = noble(&[
            std::ffi::OsStr::new("verify-module"),
            &subject,
            std::ffi::OsStr::new(arguments[0]),
            std::ffi::OsStr::new(arguments[1]),
        ]);
        if arguments.len() > 2 {
            command.arg(arguments[2]).arg(arguments[3]);
        }
        let output = run(command)?;
        let report = report(&output)?;
        assert_eq!(output.status.code(), Some(2), "{label}: {}", text(&output));
        assert_eq!(report["outcome"], "error", "{label}");
        assert_eq!(report["category"], "usage", "{label}");
        assert_eq!(report["diagnostic"]["code"], "usage", "{label}");
    }
    Ok(())
}

#[test]
fn a_refused_proof_term_names_its_category() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("refused-term", &[("subject.noble", ILL_FORMED_PROOF)])?;
    let subject = source(&fixture.path("subject.noble"));
    let output = run(noble(&[std::ffi::OsStr::new("verify-module"), &subject]))?;
    let report = report(&output)?;
    assert_eq!(output.status.code(), Some(2), "{}", text(&output));
    assert_eq!(report["outcome"], "error");
    assert_eq!(report["category"], "rejected-term");
    assert_eq!(report["remediation"], "fix-the-reported-proof-constructor");
    // The source checker refuses the term at its acceptance stage.
    assert_eq!(report["stage"], "acceptance");
    Ok(())
}

#[test]
fn obligation_map_names_every_declared_site_and_digest() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("map", &[("subject.noble", WITH_PROOF)])?;
    let subject = source(&fixture.path("subject.noble"));
    let output = run(noble(&[std::ffi::OsStr::new("obligations"), &subject]))?;
    let report = report(&output)?;
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(report["schema"], "noble-obligation-map/v1");
    assert_eq!(report["role"], "read-only-diagnostic");
    assert_eq!(report["acceptance"], false);
    assert_eq!(report["profile"], "Intrinsic-Proofs-Draft");
    assert_eq!(
        report["module"]["name"], "arithmetic",
        "the prepared module identity is reported"
    );
    let obligations = report["obligations"]
        .as_array()
        .expect("an obligations array");
    assert_eq!(obligations.len(), 1);
    let entry = &obligations[0];
    assert_eq!(entry["key"], "arithmetic@1/increment-law/1");
    assert_eq!(entry["kind"], "contract");
    assert_eq!(entry["declaration"], "increment-law");
    assert_eq!(entry["revision"], 1);
    assert_eq!(entry["semantics"], "partial-correctness");
    assert_eq!(entry["statement_source"], "export_lean");
    assert_eq!(entry["statement_ref"], "MC1Obligation.claim");
    assert_eq!(entry["proof"], "increment-correct");
    assert_eq!(entry["vacuity"], "none");
    assert_eq!(entry["acceptance"], false);
    digest_of(&entry["statement_digest"]);
    let statement = entry["statement"].as_str().expect("the exact statement");
    assert!(statement.starts_with("import NobleContracts"));
    assert!(statement.contains("def claim : Prop :="), "{statement}");
    digest_of(&entry["staleness"]["checker_revision"]);
    digest_of(&entry["staleness"]["model_revision"]);
    assert_eq!(entry["staleness"]["contract_revision"], 1);
    assert_eq!(
        entry["staleness"]["statement_digest"],
        entry["statement_digest"]
    );
    assert_eq!(entry["predicates"]["pre"]["source_text"], "true");
    assert_eq!(
        entry["predicates"]["post"]["source_text"],
        "(eq (out y) (add (in x) 1))"
    );
    assert_eq!(
        entry["predicates"]["pre"]["statement_ref"],
        "MC1Obligation.precondition"
    );
    assert_eq!(
        entry["predicates"]["post"]["statement_ref"],
        "MC1Obligation.postcondition"
    );
    let slices = entry["stacks"]["reviewed_slices"]
        .as_array()
        .expect("the declared slices");
    assert_eq!(slices.len(), 3);
    assert_eq!(slices[0]["text"], "tail ++ [x]");
    assert_eq!(slices[1]["text"], "tail ++ [x, 1]");
    assert_eq!(slices[2]["text"], "tail ++ [wrap64(x + 1)]");
    let sites = entry["sites"].as_array().expect("site rows");
    assert!(
        sites.iter().any(|site| site["site"]
            == "export-unary-I64.0.intro.1.intro.1.pc-sequence.2.bridge"
            && site["constructor"] == "bridge"
            && site["discharged_by"] == "by-exact-append-assoc"),
        "the declared bridge site is source-located: {}",
        entry["sites"]
    );
    assert!(
        sites
            .iter()
            .any(|site| site["constructor"] == "join"
                && site["discharged_by"] == "by-exact-result"),
        "the declared join names its rule: {}",
        entry["sites"]
    );
    assert!(
        sites
            .iter()
            .any(|site| site["statement_ref"] == "MC1Obligation.postcondition"),
        "the postcondition site keeps its source span"
    );
    let premises = entry["premises"].as_array().expect("premise rows");
    assert_eq!(premises.len(), 2);
    assert_eq!(premises[0]["name"], "tail");
    assert_eq!(premises[0]["sort"], "Stack");
    assert_eq!(premises[1]["name"], "x");
    assert_eq!(premises[1]["sort"], "I64");
    Ok(())
}

#[test]
fn obligation_map_is_deterministic_and_statement_sensitive(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("determinism", &[("subject.noble", WITH_PROOF)])?;
    let subject = source(&fixture.path("subject.noble"));
    let first = run(noble(&[std::ffi::OsStr::new("obligations"), &subject]))?;
    let second = run(noble(&[std::ffi::OsStr::new("obligations"), &subject]))?;
    assert_eq!(first.stdout, second.stdout, "the map is deterministic");

    let changed = Fixture::new(
        "changed",
        &[(
            "subject.noble",
            WITH_PROOF
                .replace("(add (in x) 1)", "(add (in x) 2)")
                .as_str(),
        )],
    )?;
    let changed_subject = source(&changed.path("subject.noble"));
    let other = run(noble(&[
        std::ffi::OsStr::new("obligations"),
        &changed_subject,
    ]))?;
    let digest = |output: &Output| -> String {
        report(output)
            .expect("a report")
            .pointer("/obligations/0/statement_digest")
            .and_then(Value::as_str)
            .expect("a digest")
            .to_string()
    };
    assert_ne!(
        digest(&first),
        digest(&other),
        "a changed postcondition changes the statement digest"
    );
    Ok(())
}

#[test]
fn goal_view_projects_the_map_without_claiming_a_checker_result(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("view", &[("subject.noble", WITH_PROOF)])?;
    let subject = source(&fixture.path("subject.noble"));
    let map = run(noble(&[std::ffi::OsStr::new("obligations"), &subject]))?;
    let view = run(noble(&[std::ffi::OsStr::new("goal-view"), &subject]))?;
    let map = report(&map)?;
    let view = report(&view)?;
    assert_eq!(view["schema"], "noble-goal-view/v1");
    assert_eq!(view["role"], "read-only-projection");
    assert_eq!(view["acceptance"], false);
    assert_eq!(view["checker_result"], Value::Null);
    assert!(
        view["notice"]
            .as_str()
            .is_some_and(|notice| !notice.is_empty()),
        "the view must state why no chip is checked"
    );
    let sites = view["sites"].as_array().expect("goal-view rows");
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0]["key"], map["obligations"][0]["key"]);
    assert_eq!(
        sites[0]["statement_digest"],
        map["obligations"][0]["statement_digest"]
    );
    assert_eq!(sites[0]["statement"], map["obligations"][0]["statement"]);
    assert_eq!(sites[0]["chip"], "incomplete");
    assert_eq!(sites[0]["outcome"], "not-run");
    assert_eq!(sites[0]["category"], "explanation-only");
    assert_eq!(sites[0]["checker_result"], Value::Null);
    assert_eq!(sites[0]["acceptance"], false);
    Ok(())
}

#[test]
fn named_contracts_report_their_generated_obligation_text() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::new(
        "named",
        &[
            ("definitions.noble", DEFINITIONS),
            ("import.noble", IMPORT),
            ("subject.noble", NAMED_SUBJECT),
        ],
    )?;
    let subject = source(&fixture.path("subject.noble"));
    let definitions = source(&fixture.path("definitions.noble"));
    let import = source(&fixture.path("import.noble"));
    let output = run(noble(&[
        std::ffi::OsStr::new("obligations"),
        &subject,
        std::ffi::OsStr::new("--module"),
        &definitions,
        std::ffi::OsStr::new("--module"),
        &import,
    ]))?;
    let report = report(&output)?;
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(report["profile"], "Named-Contracts-v2");
    digest_of(&report["revisions"]["named_model"]);
    digest_of(&report["revisions"]["named_checker"]);
    let entry = &report["obligations"][0];
    assert_eq!(entry["key"], "Subject@3/twice-law/2");
    assert_eq!(entry["revision"], 2);
    assert_eq!(entry["statement_source"], "prepare_named_contract");
    assert_eq!(entry["statement_ref"], "NamedV2Obligation.claim");
    let statement = entry["statement"].as_str().expect("the obligation text");
    assert!(
        statement.contains("namespace NamedV2Obligation"),
        "{statement}"
    );
    let sites = entry["sites"].as_array().expect("site rows");
    assert!(sites.iter().any(|site| site["site"] == "useA"));
    assert!(sites.iter().any(|site| site["site"] == "useB"));
    assert_eq!(entry["vacuity"], "none");
    assert_eq!(entry["predicates"]["pre"]["source_text"], "true");
    assert_eq!(
        entry["predicates"]["post"]["statement_ref"],
        "NamedV2Obligation.claim"
    );
    assert_eq!(entry["stacks"]["reviewed_slices"], Value::Null);
    Ok(())
}

#[test]
fn a_contract_file_maps_its_exported_claim() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(
        "contract",
        &[("increment.noble-contract", INCREMENT_CONTRACT)],
    )?;
    let contract = source(&fixture.path("increment.noble-contract"));
    let output = run(noble(&[std::ffi::OsStr::new("obligations"), &contract]))?;
    let report = report(&output)?;
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(report["schema"], "noble-obligation-map/v1");
    assert_eq!(report["profile"], "MC1");
    let entry = &report["obligations"][0];
    // The exported semantic revision of this contract text is 0.
    assert_eq!(entry["key"], "increment@0/claim/1");
    assert_eq!(entry["statement_source"], "export_lean");
    assert_eq!(entry["statement_ref"], "MC1Obligation.claim");
    assert_eq!(entry["kind"], "contract");
    let statement = entry["statement"].as_str().expect("the exported statement");
    assert!(statement.contains("def claim : Prop :="), "{statement}");
    digest_of(&entry["statement_digest"]);
    let sites = entry["sites"].as_array().expect("site rows");
    assert!(sites
        .iter()
        .any(|site| site["statement_ref"] == "MC1Obligation.precondition"));
    Ok(())
}

#[test]
fn a_module_without_contracts_reports_no_obligations() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(
        "empty",
        &[(
            "subject.noble",
            "module empty@1 [ export value def value [ 1 ] ]",
        )],
    )?;
    let subject = source(&fixture.path("subject.noble"));
    let output = run(noble(&[std::ffi::OsStr::new("obligations"), &subject]))?;
    let report = report(&output)?;
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(
        report["obligations"]
            .as_array()
            .expect("an obligations array")
            .len(),
        0
    );
    Ok(())
}

struct HoleReplacement<'a> {
    needle: &'a str,
    label: &'a str,
}

fn proof_hole_json(replacement: HoleReplacement<'_>) -> String {
    let at = WITH_PROOF
        .find(replacement.needle)
        .expect("hole replacement target");
    serde_json::json!({
        "format": 1,
        "module": [
            &WITH_PROOF[..at],
            {"kind": "proof-hole", "label": replacement.label},
            &WITH_PROOF[at.saturating_add(replacement.needle.len())..],
        ],
    })
    .to_string()
}

#[test]
fn proof_hole_transport_reports_incomplete_and_ordinary_commands_refuse_holes(
) -> Result<(), Box<dyn std::error::Error>> {
    let bridge_doc = proof_hole_json(HoleReplacement {
        needle: "(bridge (by-exact-append-assoc))",
        label: "bridge",
    });
    let ensures_doc = proof_hole_json(HoleReplacement {
        needle: "(add (in x) 1)",
        label: "pred",
    });
    let exec_doc = proof_hole_json(HoleReplacement {
        needle: "1 +",
        label: "body",
    });
    let raw_hole = WITH_PROOF.replacen(
        "(bridge (by-exact-append-assoc))",
        "(editor-hole bridge)",
        1,
    );
    let fixture = Fixture::new(
        "proof-holes",
        &[
            ("bridge-hole.json", &bridge_doc),
            ("ensures-hole.json", &ensures_doc),
            ("exec-hole.json", &exec_doc),
            ("raw-hole.noble", &raw_hole),
        ],
    )?;

    let bridge = source(&fixture.path("bridge-hole.json"));
    let output = run(noble(&[
        std::ffi::OsStr::new("goal-view"),
        std::ffi::OsStr::new("--proof-holes"),
        &bridge,
    ]))?;
    let view = report(&output)?;
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert_eq!(view["schema"], "noble-goal-view/v1");
    assert_eq!(view["outcome"], "unknown");
    assert_eq!(view["category"], "incomplete-hole");
    assert_eq!(view["acceptance"], false);
    assert_eq!(view["transport"]["schema"], "noble-proof-editor/v1");
    assert_eq!(view["transport"]["holes"][0]["label"], "bridge");
    let site = &view["sites"][0];
    assert_eq!(site["chip"], "incomplete");
    assert_eq!(site["outcome"], "unknown");
    assert_eq!(site["category"], "incomplete-hole");
    assert_eq!(site["holes"], serde_json::json!(["bridge"]));
    let hole_row = site["sites"]
        .as_array()
        .expect("site rows")
        .iter()
        .find(|row| row["constructor"] == "editor-hole")
        .expect("editor-hole site");
    assert_eq!(hole_row["chip"], "incomplete");
    assert_eq!(hole_row["label"], "bridge");
    assert_eq!(hole_row["parent_constructor"], "pc-sequence");
    assert_eq!(hole_row["goal"]["declaration"], "increment-law");
    assert_eq!(
        hole_row["goal"]["statement_digest"],
        site["statement_digest"]
    );

    let raw = source(&fixture.path("raw-hole.noble"));
    for command in ["verify-module", "obligations", "goal-view"] {
        let refused = run(noble(&[std::ffi::OsStr::new(command), &raw]))?;
        let parsed = report(&refused)?;
        assert_eq!(
            refused.status.code(),
            Some(2),
            "{command}: {}",
            text(&refused)
        );
        assert_eq!(parsed["outcome"], "error", "{command}");
        assert!(
            parsed["diagnostic"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("editor-only")),
            "{command}: {}",
            text(&refused)
        );
    }

    for (label, file) in [("pred", "ensures-hole.json"), ("body", "exec-hole.json")] {
        let path = source(&fixture.path(file));
        let refused = run(noble(&[
            std::ffi::OsStr::new("goal-view"),
            std::ffi::OsStr::new("--proof-holes"),
            &path,
        ]))?;
        let parsed = report(&refused)?;
        assert_eq!(
            refused.status.code(),
            Some(2),
            "{label}: {}",
            text(&refused)
        );
        assert_eq!(
            parsed["diagnostic"]["code"],
            "proof-hole-transport",
            "{label}: {}",
            text(&refused)
        );
        assert!(
            parsed["diagnostic"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("not in a proof-term position")),
            "{label}: {}",
            text(&refused)
        );
    }
    Ok(())
}

#[test]
fn retained_evidence_marks_stale_without_rewriting_and_never_grants_checked(
) -> Result<(), Box<dyn std::error::Error>> {
    let changed_subject = WITH_PROOF.replacen("def increment [ 1 + ]", "def increment [ 2 + ]", 1);
    let bridge_doc = proof_hole_json(HoleReplacement {
        needle: "(bridge (by-exact-append-assoc))",
        label: "bridge",
    });
    let fixture = Fixture::new(
        "retained-evidence",
        &[
            ("v1.noble", WITH_PROOF),
            ("v2.noble", &changed_subject),
            ("bridge-hole.json", &bridge_doc),
        ],
    )?;
    let v1 = source(&fixture.path("v1.noble"));
    let v2 = source(&fixture.path("v2.noble"));
    let bridge = source(&fixture.path("bridge-hole.json"));

    let map_output = run(noble(&[std::ffi::OsStr::new("obligations"), &v1]))?;
    assert_eq!(map_output.status.code(), Some(0), "{}", text(&map_output));
    let map = report(&map_output)?;
    let entry = &map["obligations"][0];
    let evidence_dir = fixture.path("retained");
    std::fs::create_dir(&evidence_dir)?;
    std::fs::write(evidence_dir.join("source.noble"), WITH_PROOF)?;
    let evidence_json = serde_json::json!({
        "schema": "noble-intrinsic-evidence/v1",
        "command": "verify-module",
        "outcome": "proved",
        "independent_recheck": true,
        "source_sha256": map["source_sha256"],
        "module": {"name": "arithmetic", "version": 1},
        "revisions": {
            "model": map["revisions"]["model"],
            "checker": map["revisions"]["checker"],
            "named_model": Value::Null,
            "named_checker": Value::Null
        },
        "dependencies": [],
        "proofs": [{
            "name": "increment-correct",
            "kind": "contract",
            "statement_digest": entry["statement_digest"],
            "statement_basis": "generated-statement",
            "model_revision": map["revisions"]["model"],
            "checker_revision": map["revisions"]["checker"],
            "subject": {
                "module": "arithmetic",
                "version": 1,
                "definition": "increment",
                "module_source_sha256": entry["subject"]["module_source_sha256"],
                "definition_source_sha256": entry["subject"]["definition_source_sha256"],
                "generated_statement_sha256": entry["statement_digest"]
            }
        }]
    });
    let evidence_bytes = std::format!("{evidence_json}\n");
    std::fs::write(evidence_dir.join("evidence.json"), &evidence_bytes)?;
    let retained = source(&evidence_dir);

    let matching_view = run(noble(&[
        std::ffi::OsStr::new("goal-view"),
        &v1,
        std::ffi::OsStr::new("--evidence"),
        &retained,
    ]))?;
    let matching = report(&matching_view)?;
    assert_eq!(
        matching_view.status.code(),
        Some(0),
        "{}",
        text(&matching_view)
    );
    assert_eq!(matching["evidence"]["records"][0]["matches"], true);
    assert_eq!(
        matching["sites"][0]["chip"], "incomplete",
        "retained evidence alone must never grant checked"
    );

    let hole_view = run(noble(&[
        std::ffi::OsStr::new("goal-view"),
        std::ffi::OsStr::new("--proof-holes"),
        &bridge,
        std::ffi::OsStr::new("--evidence"),
        &retained,
    ]))?;
    let hole_parsed = report(&hole_view)?;
    assert_eq!(hole_view.status.code(), Some(3), "{}", text(&hole_view));
    assert_eq!(hole_parsed["outcome"], "unknown");
    assert_eq!(hole_parsed["category"], "incomplete-hole");
    assert_eq!(hole_parsed["sites"][0]["chip"], "incomplete");

    let stale_view = run(noble(&[
        std::ffi::OsStr::new("goal-view"),
        &v2,
        std::ffi::OsStr::new("--evidence"),
        &retained,
    ]))?;
    let stale = report(&stale_view)?;
    assert_eq!(stale_view.status.code(), Some(0), "{}", text(&stale_view));
    assert_eq!(stale["evidence"]["records"][0]["matches"], false);
    assert_eq!(stale["sites"][0]["chip"], "stale");
    assert_eq!(stale["sites"][0]["outcome"], "unsupported");
    assert_eq!(stale["sites"][0]["category"], "stale-dependency");
    assert_eq!(
        stale["sites"][0]["retained_evidence"][0]["recorded"]["subject"]["definition"],
        "increment"
    );

    let stale_verify = run(noble(&[
        std::ffi::OsStr::new("verify-module"),
        &v2,
        std::ffi::OsStr::new("--evidence"),
        &retained,
    ]))?;
    let stale_report = report(&stale_verify)?;
    assert_eq!(
        stale_verify.status.code(),
        Some(4),
        "{}",
        text(&stale_verify)
    );
    assert_eq!(stale_report["outcome"], "unsupported");
    assert_eq!(stale_report["category"], "stale-dependency");
    assert_eq!(stale_report["diagnostic"]["code"], "intrinsic-stale-proof");
    let mismatches = stale_report["diagnostic"]["details"]["mismatches"]
        .as_array()
        .expect("mismatch list");
    assert!(mismatches.iter().any(|item| item == "source_sha256"));
    assert!(mismatches
        .iter()
        .any(|item| item == "proofs.increment-correct.statement_digest"));
    assert_eq!(
        std::fs::read_to_string(evidence_dir.join("evidence.json"))?,
        evidence_bytes,
        "retained evidence must never be rewritten"
    );

    let forged_dir = fixture.path("forged");
    std::fs::create_dir(&forged_dir)?;
    std::fs::write(forged_dir.join("source.noble"), "module forged@1 [ ]")?;
    std::fs::write(forged_dir.join("evidence.json"), &evidence_bytes)?;
    let forged = source(&forged_dir);
    let forged_verify = run(noble(&[
        std::ffi::OsStr::new("verify-module"),
        &v1,
        std::ffi::OsStr::new("--evidence"),
        &forged,
    ]))?;
    let forged_report = report(&forged_verify)?;
    assert_eq!(
        forged_verify.status.code(),
        Some(2),
        "{}",
        text(&forged_verify)
    );
    assert_eq!(forged_report["outcome"], "error");
    assert_eq!(forged_report["category"], "forged-identity");
    assert_eq!(forged_report["diagnostic"]["code"], "evidence-invalid");

    let existing_emit = run(noble(&[
        std::ffi::OsStr::new("verify-module"),
        &v1,
        std::ffi::OsStr::new("--emit"),
        &retained,
    ]))?;
    let existing_report = report(&existing_emit)?;
    assert_eq!(
        existing_emit.status.code(),
        Some(2),
        "{}",
        text(&existing_emit)
    );
    assert_eq!(existing_report["outcome"], "error");
    assert_eq!(existing_report["category"], "unavailable");
    assert_eq!(existing_report["diagnostic"]["code"], "emit-destination");

    let unused_emit = fixture.path("never-created-emit");
    let unused_emit_two = fixture.path("never-created-emit-two");
    let repeated_emit = run(noble(&[
        std::ffi::OsStr::new("verify-module"),
        &v1,
        std::ffi::OsStr::new("--emit"),
        &source(&unused_emit),
        std::ffi::OsStr::new("--emit"),
        &source(&unused_emit_two),
    ]))?;
    let repeated_report = report(&repeated_emit)?;
    assert_eq!(
        repeated_emit.status.code(),
        Some(2),
        "{}",
        text(&repeated_emit)
    );
    assert_eq!(repeated_report["category"], "usage");
    assert!(
        !unused_emit.exists(),
        "a usage failure must not create the --emit directory"
    );
    Ok(())
}
