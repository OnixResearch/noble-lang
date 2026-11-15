use super::*;

fn library_refusal(
    name: &str,
    original: &noble_kernel::execution::Submission,
    closure: &[u64],
    static_inputs: &[Option<i64>],
    expected: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let result = noble_ngraph::specialize::program::residualize(
        &TestIds,
        &noble_ngraph::specialize::program::Request {
            original,
            claimed_closure: closure,
            expected_original: None,
            preserved_tail: 0,
            static_inputs,
            budgets: &noble_ngraph::check::Budgets::default(),
            source_limits: noble_contracts::Limits::default(),
        },
    );
    let diagnostic = match result {
        Ok(_) => return Err(format!("{name}: a refused subject returned a residual").into()),
        Err(diagnostic) => diagnostic,
    };
    assert_eq!(diagnostic.code_string(), expected, "{name}: {diagnostic}");
    // Library refusal returns no Program or graph. This library has no host
    // execution callback: the zero below is structural, not a process counter.
    Ok(serde_json::json!({
        "kind":"library", "name":name, "exit":null, "stage":"original-admission",
        "outcome":"reject", "code":diagnostic.code_string(),
        "guest_requests":0, "protected_operations":0, "published_output":false,
    }))
}

pub(super) fn case_02() -> TestResult {
    let scratch = Scratch::new("pe-case02")?;
    let out = scratch.path("refused.residual");
    let controls = [
        ("wrong-profile", "ngraph-ir/0 ngraph-f32/0\nsubject authored-name pe-wrong-profile\nslot input f32\n%0 = input 0\nout %0\n", "E-PROFILE"),
        ("param-slot", "ngraph-ir/0 ngraph-f32/0\nsubject authored-name pe-param\nslot param f32\n%0 = param 0\nout %0\n", "E-PROFILE"),
        ("interface-shape-tree", "ngraph-ir/0 ngraph-i64/0\nsubject authored-name pe-shape\nshape input (p s s)\nslot input i64\nslot input i64\n%0 = input 0\n%1 = input 1\n%2 = i64.add %0 %1\nout %2\n", "E-AGGREGATE"),
        ("fact-record", CHECK_GRAPH, "E-PROFILE"),
        ("guard-node", GUARD_GRAPH, "E-PROFILE"),
    ];
    let mut runs = Vec::new();
    for (name, graph, code) in controls {
        let input = scratch.write(&format!("{name}.ngraph"), graph)?;
        runs.push(checked_case(
            &scratch,
            name,
            &specialize(&input, "", &out),
            &out,
            Some(code),
        )?);
    }
    let graph = noble_ngraph::check::check(
        &TestIds,
        noble_ngraph::format::graph::parse(&TestIds, PE_GRAPH)?,
    )?;
    let invalid = noble_ngraph::specialize::specialize(
        &TestIds,
        &graph,
        &noble_ngraph::specialize::Request {
            static_inputs: &[Some(5)],
            budgets: &noble_ngraph::check::Budgets::default(),
        },
    );
    let diagnostic = match invalid {
        Ok(_) => return Err("wrong map arity returned a residual".into()),
        Err(diagnostic) => diagnostic,
    };
    assert_eq!(diagnostic.code_string(), "E-INTERFACE");
    runs.push(serde_json::json!({
        "kind":"library", "name":"wrong-static-map-arity", "exit":null,
        "stage":"static-map-admission", "outcome":"reject", "code":diagnostic.code_string(),
        "guest_requests":0, "protected_operations":0, "published_output":false,
    }));
    case_observation(
        2,
        runs,
        serde_json::json!({
            "residual_nodes_created":0, "published_output":false,
        }),
    )
}

pub(super) fn case_09() -> TestResult {
    let scratch = Scratch::new("pe-case09")?;
    let input = scratch.write("original.ngraph", PE_GRAPH)?;
    let mut runs = Vec::new();
    for (name, cap, code) in [
        ("one-below", "3", Some("E-BUDGET-RESIDUAL-NODES")),
        ("at", "4", None),
        ("one-above", "5", None),
    ] {
        let out = scratch.path(&format!("nodes-{name}.noble"));
        let command = specialize(&input, "0=5", &out)
            .arg("--budget-residual-nodes")
            .arg(cap);
        runs.push(checked_case(&scratch, name, &command, &out, code)?);
    }
    for (name, cap, code) in [
        ("zero", "0", Some("E-BUDGET-VARIANTS")),
        ("default-one", "1", None),
        ("above-default", "2", Some("E-PROFILE")),
    ] {
        let out = scratch.path(&format!("variants-{name}.noble"));
        let command = specialize(&input, "0=5", &out)
            .arg("--budget-variants")
            .arg(cap);
        runs.push(checked_case(&scratch, name, &command, &out, code)?);
    }
    case_observation(9, runs, serde_json::json!({"boundary_specific":true}))
}

pub(super) fn case_12() -> TestResult {
    let effect = noble_ngraph::extract::source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        &[],
        "\"hi\" test.emit 5",
        &[],
    )?;
    let mut runs = vec![library_refusal(
        "effectful",
        &effect.submission,
        &[],
        &[],
        "E-EFFECT",
    )?];
    // Resource is not a Core literal: construct an ordinary declared-module
    // submission with a Resource interface and recheck it independently.
    let retained =
        noble_contracts::source::RetainedSourceLimit::new(noble_contracts::Limits::default().bytes)
            .ok_or("default source limit is not retainable")?;
    let module = noble_contracts::source::ModuleSession::new(&[], retained)
        .map_err(|err| format!("declared-module session: {err:?}"))?;
    let resource = module
        .prepare(
            b"swap",
            &[
                noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE),
                noble_kernel::types::Ty::I64,
            ],
            noble_contracts::Limits::default(),
        )
        .map_err(|err| format!("resource source preparation: {err:?}"))?;
    runs.push(library_refusal(
        "resource-interface",
        resource.submission().ok_or("resource submission missing")?,
        &[],
        &[],
        "E-RESOURCE",
    )?);
    // Recursive definitions are producer-impossible in the source frontend.
    // Mutate a prepared, untrusted submission instead of claiming a live Core
    // source can provide a recursive dependency.
    let mut recursive = noble_ngraph::extract::source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        &["def f [ dup drop ]"],
        "5 f",
        &[],
    )?;
    let target = recursive.submission.definitions[0].definition;
    let body = &mut recursive.submission.definitions[0].body.candidate;
    body.nodes = vec![noble_kernel::untrusted::Node::Invocation {
        def: target,
        inst: noble_kernel::words::Inst { bindings: vec![] },
    }];
    body.body = vec![noble_kernel::untrusted::NodeId(0)];
    runs.push(library_refusal(
        "recursive",
        &recursive.submission,
        &recursive.claimed_closure,
        &[],
        "E-RECURSION",
    )?);
    case_observation(
        12,
        runs,
        serde_json::json!({
            "graph_created":false, "residual_created":false, "published_output":false,
        }),
    )
}

pub(super) fn case_17() -> TestResult {
    let scratch = Scratch::new("pe-case17")?;
    let controls = [
        (
            "operation-touches-opaque-tail",
            "swap",
            "I64 I64 --",
            "1",
            "",
            "E-INTERFACE",
        ),
        (
            "static-map-binds-tail",
            "1 +",
            "Text I64 --",
            "1",
            "0=5",
            "E-INTERFACE",
        ),
        ("oversized-tail", "1 +", "I64 --", "2", "", "E-INTERFACE"),
        (
            "effectful-checked-root",
            "\"audit\" test.emit 1 +",
            "I64 --",
            "0",
            "",
            "E-EFFECT",
        ),
    ];
    let mut runs = Vec::new();
    for (name, source, interface, tail, map, code) in controls {
        let input = scratch.write(&format!("{name}.noble"), source)?;
        let out = scratch.path(&format!("{name}.residual"));
        let graph = scratch.path(&format!("{name}.ngraph"));
        let command = specialize_program_source(&input, interface, tail, map, &out)
            .arg("--graph-out")
            .arg(&graph);
        runs.push(checked_case(&scratch, name, &command, &out, Some(code))?);
        assert_no_publication(&graph);
    }
    case_observation(
        17,
        runs,
        serde_json::json!({
            "residual_source_published":false, "graph_published":false,
        }),
    )
}

pub(super) fn case_18() -> TestResult {
    let root = noble_ngraph::extract::source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        &["def inc [ 1 + ]"],
        "inc",
        &[noble_kernel::types::Ty::I64],
    )?;
    assert_eq!(root.claimed_closure.len(), 1);
    let run = library_refusal(
        "omitted-reached-definition",
        &root.submission,
        &[],
        &[None],
        "E-CLOSURE",
    )?;
    case_observation(
        18,
        vec![run],
        serde_json::json!({
            "extraction_started":false, "published_output":false,
        }),
    )
}
