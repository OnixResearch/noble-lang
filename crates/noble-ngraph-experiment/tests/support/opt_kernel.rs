//! Rust candidate observations exported to the Lean `checkStep` corpus.

const SGD_ENVELOPE: &str = r#"{
  "format":"ngraph-opt/1","n":2,
  "inputs":["p","g","eta","step","state"],"state":"State0","effects":[],
  "budgets":{"nodes":16,"state_bytes":0,"work":128,"output_lanes":64},
  "nodes":[
    {"id":0,"op":"broadcast","operands":["$eta"],"type":"F32[n]"},
    {"id":1,"op":"Binary32.mul","operands":[0,"$g"],"type":"F32[n]"},
    {"id":2,"op":"Binary32.sub","operands":["$p",1],"type":"F32[n]"}
  ],"outputs":{"parameters":2,"state":"State0"}
}"#;

#[path = "opt_kernel/corpus.rs"]
mod corpus;
#[path = "opt_kernel/evaluation.rs"]
mod evaluation;
#[path = "opt_kernel/isolation.rs"]
mod isolation;
#[path = "opt_kernel/parser.rs"]
mod parser;
