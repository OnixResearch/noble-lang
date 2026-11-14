use super::*;

pub(crate) fn write_json_lines(path: &std::path::Path, records: &[Value]) -> Result<(), String> {
    let mut contents = String::new();
    for record in records {
        contents.push_str(&serde_json::to_string(record).map_err(|error| error.to_string())?);
        contents.push('\n');
    }
    std::fs::write(path, contents).map_err(|error| error.to_string())
}

pub(crate) fn command_text(program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_string())
        .map_err(|error| format!("{program} output was not UTF-8: {error}"))
}

pub(crate) fn score_graph_counts(graph: &CheckedGraph) -> Result<Value, String> {
    let mut counts = std::collections::BTreeMap::<&'static str, u32>::new();
    for node in &graph.graph().nodes {
        let name = match &node.op {
            noble_ngraph::ir::Op::Input(_) => "input",
            noble_ngraph::ir::Op::Param(_) => "param",
            noble_ngraph::ir::Op::F32Mul(_, _) => "f32.mul",
            noble_ngraph::ir::Op::F32Add(_, _) => "f32.add",
            _ => {
                return Err(format!(
                    "forbidden operation in fixed scorer: {}",
                    node.op.name()
                ))
            }
        };
        let count = counts.entry(name).or_default();
        *count = count.saturating_add(1);
    }
    if counts.get("input") != Some(&64)
        || counts.get("param") != Some(&65)
        || counts.get("f32.mul") != Some(&64)
        || counts.get("f32.add") != Some(&64)
        || graph.graph().outputs.len() != 1
    {
        return Err(format!(
            "scorer graph shape differs from fixed contract: {counts:?}"
        ));
    }
    Ok(json!({
        "input_nodes": counts.get("input"),
        "param_nodes": counts.get("param"),
        "f32_multiply_nodes": counts.get("f32.mul"),
        "ordered_f32_add_nodes": counts.get("f32.add"),
        "output_count": graph.graph().outputs.len(),
        "forbidden_operations": [],
        "profile": graph.graph().profile.name()
    }))
}

pub(crate) fn loss_graph_counts(graph: &CheckedGraph) -> Result<Value, String> {
    let mut inputs = 0u32;
    let mut subtractions = 0u32;
    let mut multiplications = 0u32;
    for node in &graph.graph().nodes {
        match &node.op {
            noble_ngraph::ir::Op::Input(_) => inputs = inputs.saturating_add(1),
            noble_ngraph::ir::Op::F32Sub(_, _) => subtractions = subtractions.saturating_add(1),
            noble_ngraph::ir::Op::F32Mul(_, _) => multiplications = multiplications.saturating_add(1),
            _ => {
                return Err(format!(
                    "forbidden operation in frozen MSE loss graph: {}",
                    node.op.name()
                ))
            }
        }
    }
    if inputs != 2 || subtractions != 1 || multiplications != 1 || graph.graph().outputs.len() != 1
    {
        return Err(String::from(
            "MSE loss graph differs from the two-input stored-order contract",
        ));
    }
    Ok(json!({
        "input_nodes": inputs,
        "f32_subtraction_nodes": subtractions,
        "f32_multiply_nodes": multiplications,
        "output_count": graph.graph().outputs.len(),
        "forbidden_operations": [],
        "profile": graph.graph().profile.name()
    }))
}

#[test]
#[ignore = "materializes the frozen corpus only; not an experiment measurement"]
pub(crate) fn materialize_tchunk_ranking_corpus() -> Result<(), String> {
    let output = std::env::var_os("NOBLE_TCRANK_CORPUS_OUT")
        .ok_or_else(|| String::from("NOBLE_TCRANK_CORPUS_OUT must name an external output file"))?;
    let bytes = serde_json::to_vec_pretty(&corpus_manifest()).map_err(|error| error.to_string())?;
    std::fs::write(output, bytes).map_err(|error| error.to_string())
}
