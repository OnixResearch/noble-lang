use noble_ngraph_experiment::opt::{Candidate, Refusal, RefusalClass};

fn refused(source: &str) -> Refusal {
    Candidate::parse(source.as_bytes()).expect_err("candidate refuses")
}

#[test]
fn controls_refuse_at_the_first_offending_field() {
    let effectful = super::SGD_ENVELOPE.replace("\"effects\":[]", "\"effects\":[\"read\"]");
    let observed = refused(&effectful);
    assert_eq!(observed.class, RefusalClass::Unsupported);
    assert_eq!(observed.location, "$.effects[0]");

    let opcode = super::SGD_ENVELOPE.replace("Binary32.mul", "Binary32.add");
    let observed = refused(&opcode);
    assert_eq!(observed.class, RefusalClass::Unsupported);
    assert_eq!(observed.location, "$.nodes[1].op");

    let step_metadata = super::SGD_ENVELOPE.replace("[\"$eta\"]", "[\"$step\"]");
    let observed = refused(&step_metadata);
    assert_eq!(observed.class, RefusalClass::Unsupported);
    assert_eq!(observed.location, "$.nodes[0].operands[0]");

    let state_metadata = super::SGD_ENVELOPE.replace("[\"$eta\"]", "[\"$state\"]");
    let observed = refused(&state_metadata);
    assert_eq!(observed.class, RefusalClass::Unsupported);
    assert_eq!(observed.location, "$.nodes[0].operands[0]");

    let duplicate = super::SGD_ENVELOPE.replace(
        "\"id\":1,\"op\":\"Binary32.mul\"",
        "\"id\":0,\"op\":\"Binary32.mul\"",
    );
    let observed = refused(&duplicate);
    assert_eq!(observed.class, RefusalClass::Malformed);
    assert_eq!(observed.location, "$.nodes[1].id");

    let forward = super::SGD_ENVELOPE.replace("[\"$p\",1]", "[\"$p\",9]");
    let observed = refused(&forward);
    assert_eq!(observed.class, RefusalClass::Malformed);
    assert_eq!(observed.location, "$.nodes[2].operands[1]");

    let widened_budget = super::SGD_ENVELOPE.replace("\"work\":128", "\"work\":129");
    let observed = refused(&widened_budget);
    assert_eq!(observed.class, RefusalClass::Malformed);
    assert_eq!(observed.location, "$.budgets.work");

    let duplicate_field = super::SGD_ENVELOPE.replace(
        "\"format\":\"ngraph-opt/1\"",
        "\"format\":\"ngraph-opt/1\",\"format\":\"ngraph-opt/1\"",
    );
    let observed = refused(&duplicate_field);
    assert_eq!(observed.class, RefusalClass::Malformed);
    assert_eq!(observed.location, "$.format");

    let foreign_capability = super::SGD_ENVELOPE.replace(
        "\"outputs\":{",
        "\"capability\":\"filesystem\",\"outputs\":{",
    );
    let observed = refused(&foreign_capability);
    assert_eq!(observed.class, RefusalClass::Unsupported);
    assert_eq!(observed.location, "$.capability");

    let malformed = refused("{");
    assert_eq!(malformed.class, RefusalClass::Malformed);
    assert_eq!(malformed.location, "$.envelope");
}

#[test]
fn fixed_node_work_and_output_budgets_are_enforced() {
    let too_many_nodes = (0..17)
        .map(|id| {
            format!(
                "{{\"id\":{id},\"op\":\"constant\",\"operands\":[],\"type\":\"F32\",\"bits\":\"0x3F800000\"}}"
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let over_nodes = envelope(2, &too_many_nodes, 0);
    let observed = refused(&over_nodes);
    assert_eq!(observed.class, RefusalClass::Exhausted);
    assert_eq!(observed.location, "$.nodes");

    let work_nodes = r#"{"id":0,"op":"Binary32.mul","operands":["$p","$g"],"type":"F32[n]"},{"id":1,"op":"Binary32.sub","operands":["$p",0],"type":"F32[n]"},{"id":2,"op":"Binary32.mul","operands":["$p","$g"],"type":"F32[n]"}"#;
    let over_work = envelope(64, work_nodes, 2);
    let observed = refused(&over_work);
    assert_eq!(observed.class, RefusalClass::Exhausted);
    assert_eq!(observed.location, "$.work");

    let over_lanes = super::SGD_ENVELOPE.replace("\"n\":2", "\"n\":65");
    let observed = refused(&over_lanes);
    assert_eq!(observed.class, RefusalClass::Exhausted);
    assert_eq!(observed.location, "$.outputs");
}

fn envelope(lanes: usize, nodes: &str, output: u16) -> String {
    format!(
        r#"{{"format":"ngraph-opt/1","n":{lanes},"inputs":["p","g","eta","step","state"],"state":"State0","effects":[],"budgets":{{"nodes":16,"state_bytes":0,"work":128,"output_lanes":64}},"nodes":[{nodes}],"outputs":{{"parameters":{output},"state":"State0"}}}}"#
    )
}
