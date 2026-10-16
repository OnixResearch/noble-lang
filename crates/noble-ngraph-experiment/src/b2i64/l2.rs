//! L2 driver: runs `oracle.mjs` under a pinned Node over an extracted graph.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::ir::{Graph, Literal, Op};
use crate::sem::Value;

/// The oracle source (no imports; run with `node -e`).
pub const ORACLE_JS: &str = include_str!("oracle.mjs");

/// A pinned Node binary.
#[derive(Clone, Debug)]
pub struct Oracle {
    /// Absolute `node` path.
    pub node: PathBuf,
}

/// The oracle's cell encoding of a value (`i:<decimal>`, `b:0|1`).
#[must_use]
pub fn cell(v: Value) -> String {
    match v {
        Value::I64(x) => format!("i:{x}"),
        Value::Bool(b) => format!("b:{}", u8::from(b)),
        Value::F32(bits) => format!("f:{bits:08x}"),
    }
}

fn node_json(op: &Op) -> Result<serde_json::Value, String> {
    use serde_json::json;
    Ok(match op {
        Op::Input(k) => json!(["input", k]),
        Op::Const(Literal::I64(v)) => json!(["i64", v.to_string()]),
        Op::Const(Literal::Bool(b)) => json!(["bool", u8::from(*b)]),
        Op::I64Add(a, b) => json!(["add", a.0, b.0]),
        Op::I64Sub(a, b) => json!(["sub", a.0, b.0]),
        Op::I64Mul(a, b) => json!(["mul", a.0, b.0]),
        Op::I64Eq(a, b) => json!(["eq", a.0, b.0]),
        other => return Err(format!("not an ngraph-i64/0 node: {other:?}")),
    })
}

impl Oracle {
    /// Evaluates `graph` on every input vector; returns encoded outputs.
    ///
    /// # Errors
    /// A non-`I64` node, a Node failure or unparsable output.
    pub fn evaluate(
        &self,
        graph: &Graph,
        vectors: &[Vec<Value>],
    ) -> Result<Vec<Vec<String>>, String> {
        let nodes = graph
            .nodes
            .iter()
            .map(|n| node_json(&n.op))
            .collect::<Result<Vec<_>, _>>()?;
        let job = serde_json::json!({
            "nodes": nodes,
            "outputs": graph.outputs.iter().map(|o| o.0).collect::<Vec<_>>(),
            "vectors": vectors
                .iter()
                .map(|v| v.iter().map(|x| cell(*x)).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
        });
        let mut child = Command::new(&self.node)
            .args(["-e", ORACLE_JS])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn {}: {e}", self.node.display()))?;
        let input = job.to_string();
        let mut stdin = child.stdin.take().ok_or("no stdin")?;
        let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        writer
            .join()
            .map_err(|_| "writer panicked".to_owned())?
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!(
                "node failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        serde_json::from_slice(&out.stdout).map_err(|e| format!("oracle output: {e}"))
    }

    /// `node --version`.
    ///
    /// # Errors
    /// A Node failure.
    pub fn version(&self) -> Result<String, String> {
        let out = Command::new(&self.node)
            .arg("--version")
            .output()
            .map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    }
}
