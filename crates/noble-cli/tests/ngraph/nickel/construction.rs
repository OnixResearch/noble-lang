use super::{expect_code, expect_normal, Ngraph, Scratch, TestResult};
use serde_json::{json, Value};

fn handle(scope: &str, id: &str) -> Value {
    json!({ "scope": scope, "id": id })
}

fn bundle() -> Value {
    json!({
        "schema": "noble-ngraph-nickel/v1",
        "library_version": "noble-ngraph-nickel/1",
        "contract_version": "noble-ngraph-nickel-contracts/1",
        "profile": "ngraph-i64/0",
        "bundle_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "parameters": [{ "name": "offset", "value": "9223372036854775807" }],
        "instances": [{
            "scope": "one",
            "template": "arithmetic/1",
            "inputs": ["x"],
            "outputs": ["sum", "difference"],
            "nodes": [
                { "id": "x", "op": { "kind": "input" } },
                { "id": "offset", "op": { "kind": "const", "value": "9223372036854775807" } },
                { "id": "sum", "op": { "kind": "add", "left": "x", "right": "offset" } },
                { "id": "difference", "op": { "kind": "sub", "left": "x", "right": "offset" } }
            ]
        }],
        "wires": [],
        "inputs": [handle("one", "x")],
        "outputs": [handle("one", "sum"), handle("one", "difference"), handle("one", "sum")]
    })
}

#[test]
fn lowerer_preserves_order_repetition_and_exact_i64_values() -> TestResult {
    let scratch = Scratch::new("nickel-lowering")?;
    let value = bundle();
    let path = scratch.write("constructor.json", &value.to_string())?;
    let graph = scratch.path("admitted.ngraph");
    let run = scratch.run(
        &Ngraph::new("check")
            .arg(&path)
            .arg("--graph-out")
            .arg(&graph),
    )?;
    let report = expect_normal(&run, "check")?;
    assert_eq!(report["profile"], "ngraph-i64/0");
    assert_eq!(report["lowerer_revision"], "noble-ngraph-nickel-lowerer/1");
    assert!(report["subject"]
        .as_str()
        .unwrap_or_default()
        .contains("authored-nickel"));
    let text = std::fs::read_to_string(&graph)?;
    assert!(text.contains("subject authored-nickel aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa closure -"));
    assert!(
        text.contains("%1 = const i64 9223372036854775807"),
        "{text}"
    );
    assert!(text.contains("%2 = i64.add %0 %1"), "{text}");
    assert!(
        text.contains("%3 = i64.sub %0 %1"),
        "subtraction operand order: {text}"
    );
    assert!(
        text.ends_with("out %2\nout %3\nout %2\n"),
        "repeated output order: {text}"
    );
    let reload = scratch.run(&Ngraph::new("check").arg(&graph))?;
    let rechecked = expect_normal(&reload, "check")?;
    assert_eq!(
        rechecked["graph"], report["graph"],
        "canonical Nickel subject round-trips"
    );
    Ok(())
}

#[test]
fn lowerer_supports_input_identity_and_scalar_copy_without_copy_nodes() -> TestResult {
    let scratch = Scratch::new("nickel-identity-copy")?;
    let mut value = bundle();
    value["parameters"] = json!([]);
    value["instances"][0]["nodes"] = json!([
        { "id": "x", "op": { "kind": "input" } }
    ]);
    value["instances"][0]["outputs"] = json!(["x"]);
    value["outputs"] = json!([handle("one", "x"), handle("one", "x")]);
    let path = scratch.write("identity-copy.json", &value.to_string())?;
    let graph = scratch.path("identity-copy.ngraph");
    let run = scratch.run(
        &Ngraph::new("check")
            .arg(&path)
            .arg("--graph-out")
            .arg(&graph),
    )?;
    expect_normal(&run, "check")?;
    let text = std::fs::read_to_string(graph)?;
    assert_eq!(text.matches("out %0").count(), 2, "{text}");
    assert_eq!(
        text.matches(" = ").count(),
        1,
        "no copy node is introduced: {text}"
    );
    Ok(())
}

#[test]
fn lowerer_preserves_intentionally_shared_cross_instance_handles() -> TestResult {
    let scratch = Scratch::new("nickel-sharing")?;
    let mut value = bundle();
    value["instances"] = json!([
        {
            "scope": "left",
            "template": "shared/1",
            "inputs": ["x"],
            "outputs": ["shared"],
            "nodes": [
                { "id": "x", "op": { "kind": "input" } },
                { "id": "one", "op": { "kind": "const", "value": "1" } },
                { "id": "shared", "op": { "kind": "add", "left": "x", "right": "one" } }
            ]
        },
        {
            "scope": "right",
            "template": "shared/1",
            "inputs": ["y"],
            "outputs": ["result"],
            "nodes": [
                { "id": "y", "op": { "kind": "input" } },
                { "id": "two", "op": { "kind": "const", "value": "2" } },
                { "id": "result", "op": { "kind": "add", "left": "y", "right": "two" } }
            ]
        }
    ]);
    value["wires"] = json!([{ "from": handle("left", "shared"), "to": handle("right", "y") }]);
    value["inputs"] = json!([handle("left", "x")]);
    value["outputs"] = json!([handle("left", "shared"), handle("right", "result")]);
    let path = scratch.write("sharing.json", &value.to_string())?;
    let graph = scratch.path("sharing.ngraph");
    let run = scratch.run(
        &Ngraph::new("check")
            .arg(&path)
            .arg("--graph-out")
            .arg(&graph),
    )?;
    expect_normal(&run, "check")?;
    let text = std::fs::read_to_string(graph)?;
    assert_eq!(text.matches("i64.add %0 %1").count(), 1, "{text}");
    assert!(
        text.contains("i64.add %2 %3"),
        "wire retains the shared NodeId: {text}"
    );
    Ok(())
}

#[test]
fn lowerer_refuses_forged_constructor_data_and_exhausted_budgets() -> TestResult {
    let scratch = Scratch::new("nickel-refusals")?;
    let mut cases = Vec::new();

    let mut wrong_type = bundle();
    wrong_type["instances"][0]["nodes"][1]["op"]["value"] = json!(9007199254740993_u64);
    cases.push(("wrong-type", wrong_type, "E-GRAPH-MALFORMED"));

    let mut unknown_op = bundle();
    unknown_op["instances"][0]["nodes"][2]["op"]["kind"] = json!("div");
    cases.push(("unknown-op", unknown_op, "E-GRAPH-MALFORMED"));

    let mut missing = bundle();
    missing["instances"][0]["nodes"][2]["op"]["right"] = json!("missing");
    cases.push(("missing-handle", missing, "E-GRAPH-MALFORMED"));

    let mut cross_scope = bundle();
    cross_scope["instances"][0]["nodes"][2]["op"]["right"] =
        json!({ "scope": "other", "id": "offset" });
    cases.push(("cross-scope-handle", cross_scope, "E-GRAPH-MALFORMED"));

    let mut duplicate = bundle();
    duplicate["instances"][0]["nodes"][2]["id"] = json!("offset");
    cases.push(("duplicate-id", duplicate, "E-GRAPH-MALFORMED"));

    let mut bad_constant = bundle();
    bad_constant["instances"][0]["nodes"][1]["op"]["value"] = json!("9223372036854775808");
    cases.push(("out-of-range", bad_constant, "E-GRAPH-MALFORMED"));

    let mut fractional = bundle();
    fractional["instances"][0]["nodes"][1]["op"]["value"] = json!("1.5");
    cases.push(("fractional", fractional, "E-GRAPH-MALFORMED"));

    for (name, constant) in [
        ("plus", "+1"),
        ("leading-zero", "01"),
        ("negative-zero", "-0"),
    ] {
        let mut invalid = bundle();
        invalid["instances"][0]["nodes"][1]["op"]["value"] = json!(constant);
        cases.push((name, invalid, "E-GRAPH-MALFORMED"));
    }

    let mut wrong_profile = bundle();
    wrong_profile["profile"] = json!("ngraph-f32/0");
    cases.push(("wrong-profile", wrong_profile, "E-PROFILE"));

    let mut bad_digest = bundle();
    bad_digest["bundle_sha256"] = json!("not-a-digest");
    cases.push(("bad-digest", bad_digest, "E-GRAPH-MALFORMED"));

    for (name, value, code) in cases {
        let path = scratch.write(&format!("{name}.json"), &value.to_string())?;
        let run = scratch.run(&Ngraph::new("check").arg(&path))?;
        expect_code(&run, "check", code)?;
    }

    let valid = bundle();
    for (name, flag, limit, expected) in [
        ("work", "--budget-work", "0", "E-BUDGET-WORK"),
        ("expansion", "--budget-expansion", "1", "E-BUDGET-EXPANSION"),
        ("nodes", "--budget-live-nodes", "1", "E-BUDGET-LIVE-NODES"),
        ("inputs", "--budget-inputs", "0", "E-BUDGET-INPUTS"),
        ("outputs", "--budget-outputs", "0", "E-BUDGET-OUTPUTS"),
        ("depth", "--budget-layers", "1", "E-BUDGET-LAYERS"),
        ("bytes", "--budget-graph-bytes", "1", "E-BUDGET-GRAPH-BYTES"),
    ] {
        let path = scratch.write(&format!("budget-{name}.json"), &valid.to_string())?;
        let run = scratch.run(&Ngraph::new("check").arg(&path).arg(flag).arg(limit))?;
        expect_code(&run, "check", expected)?;
    }
    Ok(())
}

#[test]
fn instance_wiring_cycles_and_unlisted_ports_are_refused() -> TestResult {
    let scratch = Scratch::new("nickel-topology")?;
    let mut cycle = bundle();
    cycle["instances"] = json!([
        {
            "scope": "a",
            "template": "cycle/1",
            "inputs": ["x"],
            "outputs": ["y"],
            "nodes": [
                { "id": "x", "op": { "kind": "input" } },
                { "id": "one", "op": { "kind": "const", "value": "1" } },
                { "id": "y", "op": { "kind": "add", "left": "x", "right": "one" } }
            ]
        },
        {
            "scope": "b",
            "template": "cycle/1",
            "inputs": ["x"],
            "outputs": ["y"],
            "nodes": [
                { "id": "x", "op": { "kind": "input" } },
                { "id": "one", "op": { "kind": "const", "value": "1" } },
                { "id": "y", "op": { "kind": "add", "left": "x", "right": "one" } }
            ]
        }
    ]);
    cycle["wires"] = json!([
        { "from": handle("a", "y"), "to": handle("b", "x") },
        { "from": handle("b", "y"), "to": handle("a", "x") }
    ]);
    cycle["inputs"] = json!([]);
    cycle["outputs"] = json!([handle("a", "y")]);
    let path = scratch.write("cycle.json", &cycle.to_string())?;
    let run = scratch.run(&Ngraph::new("check").arg(&path))?;
    expect_code(&run, "check", "E-GRAPH-MALFORMED")?;

    let mut unlisted = bundle();
    unlisted["instances"][0]["inputs"] = json!([]);
    let path = scratch.write("unlisted.json", &unlisted.to_string())?;
    let run = scratch.run(&Ngraph::new("check").arg(&path))?;
    expect_code(&run, "check", "E-GRAPH-MALFORMED")?;

    let mut local_cycle = bundle();
    local_cycle["instances"][0]["nodes"][2]["op"]["right"] = json!("difference");
    local_cycle["instances"][0]["nodes"][3]["op"]["right"] = json!("sum");
    let path = scratch.write("local-cycle.json", &local_cycle.to_string())?;
    let run = scratch.run(&Ngraph::new("check").arg(&path))?;
    expect_code(&run, "check", "E-GRAPH-MALFORMED")?;
    Ok(())
}
