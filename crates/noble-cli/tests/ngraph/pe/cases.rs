//! Canonical PE case executions through the existing ngraph integration target.
//!
//! Each case makes its actual assertions first, then prints one machine-readable
//! observation. The external case gate retains the full raw test output and
//! compares the observation with the separately registered case expectations.

use super::*;

#[path = "cases/admitted.rs"]
mod admitted;
#[path = "cases/graph.rs"]
mod graph;
#[path = "cases/program.rs"]
mod program;
#[path = "cases/refusals.rs"]
mod refusals;

struct TestHasher(blake3::Hasher);

impl noble_ngraph::IdentityHasher for TestHasher {
    fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    fn finish(self: Box<Self>) -> noble_ngraph::ir::Digest {
        *self.0.finalize().as_bytes()
    }
}

struct TestIds;

impl noble_ngraph::IdentityScheme for TestIds {
    fn name(&self) -> &'static str {
        "blake3"
    }

    fn start(&self) -> Box<dyn noble_ngraph::IdentityHasher + '_> {
        Box::new(TestHasher(blake3::Hasher::new()))
    }
}

fn case_row(id: u8) -> Result<Value, Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../specs/conformance/numeric-graph-pe-cases.json");
    let registry: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let case_id = format!("NGRAPH-PE-CASE-{id:02}");
    registry["cases"]
        .as_array()
        .and_then(|cases| cases.iter().find(|case| case["id"] == case_id))
        .cloned()
        .ok_or_else(|| format!("missing canonical {case_id}").into())
}

fn case_variants(row: &Value) -> Value {
    Value::Object(
        row["input"]
            .as_object()
            .into_iter()
            .flat_map(|fields| fields.iter())
            .filter(|(_, value)| value.is_array())
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    )
}

fn cli_observation(
    name: &str,
    run: &Run,
    output: &Path,
) -> Result<Value, Box<dyn std::error::Error>> {
    let report = single_report(run, "specialize")?;
    assert_eq!(report["guest_requests"], 0, "{name}: {report}");
    Ok(serde_json::json!({
        "kind": "cli",
        "name": name,
        "exit": run.exit,
        "stage": report["stage"],
        "outcome": report["outcome"],
        "code": report["code"],
        "guest_requests": report["guest_requests"],
        "protected_operations": 0,
        "published_output": output.exists(),
    }))
}

fn case_observation(
    id: u8,
    runs: Vec<Value>,
    checks: Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let row = case_row(id)?;
    assert!(!runs.is_empty(), "each case executes a real check");
    let mut names = std::collections::BTreeSet::new();
    for run in &runs {
        assert!(names.insert(run["name"].as_str().ok_or("run has name")?));
    }
    let id = row["id"].as_str().ok_or("canonical case id")?;
    let harness = row["input"]["harness"]
        .as_str()
        .ok_or("canonical harness")?;
    let mut observed = serde_json::json!({"runs": runs, "checks": checks});
    if id == "NGRAPH-PE-CASE-10" {
        observed["measurement_file"] = observed["checks"]["measurement_file"].clone();
        observed["measurement_records"] = observed["checks"]["measurement_records"].clone();
    }
    println!(
        "PE_CASE_OBSERVATION:{}",
        serde_json::json!({
            "id": id,
            "harness": harness,
            "variants": case_variants(&row),
            "observed": observed,
        })
    );
    Ok(())
}

fn checked_case(
    scratch: &Scratch,
    name: &str,
    command: &Ngraph,
    output: &Path,
    expected_code: Option<&str>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let run = scratch.run(command)?;
    match expected_code {
        Some(code) => {
            expect_code(&run, "specialize", code)?;
            assert_no_publication(output);
        }
        None => {
            expect_normal(&run, "specialize")?;
            assert_published(output);
        }
    }
    cli_observation(name, &run, output)
}

#[test]
fn case_01() -> TestResult {
    admitted::case_01()
}

#[test]
fn case_04() -> TestResult {
    let graph = noble_ngraph::check::check(
        &TestIds,
        noble_ngraph::format::graph::parse(&TestIds, PE_GRAPH)?,
    )?;
    let mut runs = Vec::new();
    for (name, map) in [
        ("input-leaf-count-minus-one", vec![Some(5)]),
        ("input-leaf-count-plus-one", vec![Some(5), Some(6), Some(7)]),
    ] {
        let diagnostic = match noble_ngraph::specialize::specialize(
            &TestIds,
            &graph,
            &noble_ngraph::specialize::Request {
                static_inputs: &map,
                budgets: &noble_ngraph::check::Budgets::default(),
            },
        ) {
            Ok(_) => return Err("an invalid arity created a residual".into()),
            Err(diagnostic) => diagnostic,
        };
        assert_eq!(diagnostic.code_string(), "E-INTERFACE");
        runs.push(serde_json::json!({
            "kind": "library", "name": name, "exit": null,
            "stage": "static-map-admission", "outcome": "reject",
            "code": diagnostic.code_string(),
            "guest_requests": 0, "protected_operations": 0, "published_output": false,
        }));
    }
    case_observation(
        4,
        runs,
        serde_json::json!({
            "library_arity_code": "E-INTERFACE",
            "residual_created": false,
        }),
    )
}

#[test]
fn case_02() -> TestResult {
    refusals::case_02()
}

#[test]
fn case_03() -> TestResult {
    graph::case_03()
}

#[test]
fn case_05() -> TestResult {
    graph::case_05()
}

#[test]
fn case_06() -> TestResult {
    graph::case_06()
}

#[test]
fn case_07() -> TestResult {
    graph::case_07()
}

#[test]
fn case_08() -> TestResult {
    graph::case_08()
}

#[test]
fn case_09() -> TestResult {
    refusals::case_09()
}

#[test]
fn case_10() -> TestResult {
    graph::case_10()
}

#[test]
fn case_11() -> TestResult {
    graph::case_11()
}

#[test]
fn case_12() -> TestResult {
    refusals::case_12()
}

#[test]
fn case_13() -> TestResult {
    graph::case_13()
}

#[test]
fn case_14() -> TestResult {
    graph::case_14()
}

#[test]
fn case_15() -> TestResult {
    program::case_15()
}

#[test]
fn case_16() -> TestResult {
    program::case_16()
}

#[test]
fn case_17() -> TestResult {
    refusals::case_17()
}

#[test]
fn case_18() -> TestResult {
    refusals::case_18()
}
