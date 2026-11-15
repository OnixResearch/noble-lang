use super::*;

fn checked(text: &str) -> Result<noble_ngraph::check::CheckedGraph, Box<dyn std::error::Error>> {
    Ok(noble_ngraph::check::check(
        &TestIds,
        noble_ngraph::format::graph::parse(&TestIds, text)?,
    )?)
}

fn closed_operation(
    name: &str,
    deeper: i64,
    top: i64,
    operation: &str,
    expected: noble_ngraph::ir::Literal,
) -> Result<Value, Box<dyn std::error::Error>> {
    let text = format!(
        "ngraph-ir/0 ngraph-i64/0\nsubject authored-name pe-fold\n\
         %0 = const i64 {deeper}\n%1 = const i64 {top}\n\
         %2 = {operation} %0 %1\nout %2\n"
    );
    let original = checked(&text)?;
    let result = noble_ngraph::specialize::specialize(
        &TestIds,
        &original,
        &noble_ngraph::specialize::Request {
            static_inputs: &[],
            budgets: &noble_ngraph::check::Budgets::default(),
        },
    )?;
    assert_eq!(
        result.report.folded_nodes, 1,
        "{name}: terminal node must fold"
    );
    assert_eq!(
        result.report.residual_nodes, 1,
        "{name}: dead constants prune"
    );
    assert_eq!(
        result.graph().nodes[0].op,
        noble_ngraph::ir::Op::Const(expected)
    );
    let reference = noble_ngraph::eval::reference(
        &original,
        &[],
        None,
        &mut noble_ngraph::eval::Control::default(),
    );
    let residual = noble_ngraph::eval::reference(
        &result.checked,
        &[],
        None,
        &mut noble_ngraph::eval::Control::default(),
    );
    assert!(
        reference.obs.obs_eq(&residual.obs),
        "{name}: reference differs"
    );
    Ok(serde_json::json!({
        "kind": "library", "name": name, "exit": null,
        "stage": "specialization", "outcome": "normal", "code": null,
        "guest_requests": 0, "protected_operations": 0, "published_output": false,
    }))
}

pub(crate) fn case_05() -> TestResult {
    let mut runs = Vec::new();
    for (name, deeper, top, op, value) in [
        (
            "fully-static-add",
            5,
            2,
            "i64.add",
            noble_ngraph::ir::Literal::I64(7),
        ),
        (
            "fully-static-sub",
            5,
            2,
            "i64.sub",
            noble_ngraph::ir::Literal::I64(3),
        ),
        (
            "fully-static-mul",
            5,
            2,
            "i64.mul",
            noble_ngraph::ir::Literal::I64(10),
        ),
        (
            "fully-static-i64-eq",
            5,
            2,
            "i64.eq",
            noble_ngraph::ir::Literal::Bool(false),
        ),
    ] {
        runs.push(closed_operation(name, deeper, top, op, value)?);
    }
    let mixed = checked(
        "ngraph-ir/0 ngraph-i64/0\nsubject authored-name pe-mixed\n\
         slot input i64\nslot input i64\n%0 = input 0\n%1 = input 1\n\
         %2 = i64.sub %0 %1\nout %2\n",
    )?;
    let result = noble_ngraph::specialize::specialize(
        &TestIds,
        &mixed,
        &noble_ngraph::specialize::Request {
            static_inputs: &[None, Some(5)],
            budgets: &noble_ngraph::check::Budgets::default(),
        },
    )?;
    assert_eq!(
        result.report.folded_nodes, 1,
        "only the known input substituted"
    );
    assert_eq!(
        result.graph().nodes[2].op,
        noble_ngraph::ir::Op::I64Sub(noble_ngraph::ir::NodeId(0), noble_ngraph::ir::NodeId(1)),
        "a mixed noncommutative node preserves both operand positions"
    );
    assert_eq!(result.graph().nodes[0].op, noble_ngraph::ir::Op::Input(0));
    assert_eq!(
        result.graph().nodes[1].op,
        noble_ngraph::ir::Op::Const(noble_ngraph::ir::Literal::I64(5))
    );
    runs.push(serde_json::json!({
        "kind":"library","name":"mixed","exit":null,"stage":"specialization",
        "outcome":"normal","code":null,"guest_requests":0,
        "protected_operations":0,"published_output":false,
    }));
    let known = noble_ngraph::specialize::specialize(
        &TestIds,
        &mixed,
        &noble_ngraph::specialize::Request {
            static_inputs: &[Some(2), Some(7)],
            budgets: &noble_ngraph::check::Budgets::default(),
        },
    )?;
    assert_eq!(known.report.folded_nodes, 3);
    assert_eq!(known.report.residual_nodes, 1);
    assert_eq!(
        known.graph().nodes[0].op,
        noble_ngraph::ir::Op::Const(noble_ngraph::ir::Literal::I64(-5)),
        "a known-through-static-input node folds with original operand order"
    );
    runs.push(serde_json::json!({
        "kind":"library","name":"known-through-static-input","exit":null,
        "stage":"specialization","outcome":"normal","code":null,
        "guest_requests":0,"protected_operations":0,"published_output":false,
    }));
    case_observation(
        5,
        runs,
        serde_json::json!({
            "only_static_whitelist_folded":true,
            "other_node_operation_order_and_operand_positions_unchanged":true,
        }),
    )
}

pub(crate) fn case_06() -> TestResult {
    let add = closed_operation(
        "-1 + 1 (mod 2^64)",
        -1,
        1,
        "i64.add",
        noble_ngraph::ir::Literal::I64(0),
    )?;
    let multiply = closed_operation(
        "I64_MIN * -1",
        i64::MIN,
        -1,
        "i64.mul",
        noble_ngraph::ir::Literal::I64(i64::MIN),
    )?;
    assert_eq!((-1i64).wrapping_add(1), 0);
    assert_eq!(i64::MIN.wrapping_mul(-1), i64::MIN);
    case_observation(
        6,
        vec![add, multiply],
        serde_json::json!({
            "add_result":"0", "multiply_result":i64::MIN.to_string(), "reference_agrees":true,
        }),
    )
}
