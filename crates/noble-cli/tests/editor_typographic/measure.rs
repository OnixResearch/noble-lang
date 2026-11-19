//! Real-CLI measurement: run `noble editor analyze|admit` on two documents and
//! compare every verdict field the report contract publishes.

use std::path::{Path, PathBuf};
use std::process::Command;

/// One CLI verdict, extracted from the single report document the CLI printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub exit: i32,
    pub schema: String,
    pub stage: String,
    pub outcome: String,
    pub category: Option<String>,
    pub source_stage: Option<String>,
    pub node_path: Option<String>,
    pub accepted_program_created: Option<bool>,
    pub guest_requests: i64,
    pub protected_operations: i64,
    /// The whole printed document, so two runs of equal trees must agree fully.
    pub report: String,
}

impl Verdict {
    pub fn from_report(exit: i32, stdout: &str) -> Result<Self, String> {
        let report = stdout.trim();
        let value: serde_json::Value =
            serde_json::from_str(report).map_err(|error| format!("parse report: {error}"))?;
        let failure = value.get("failure");
        Ok(Verdict {
            exit,
            schema: field(&value, "schema")?,
            stage: field(&value, "stage")?,
            outcome: field(&value, "outcome")?,
            category: optional(failure, "category"),
            source_stage: optional(failure, "source_stage"),
            node_path: failure
                .and_then(|failure| failure.get("node_path"))
                .map(|path| path.to_string()),
            accepted_program_created: value
                .get("accepted_program_created")
                .and_then(serde_json::Value::as_bool),
            guest_requests: value
                .get("guest_requests")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0),
            protected_operations: value
                .get("protected_operations")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0),
            report: report.to_owned(),
        })
    }

    pub fn summary(&self) -> String {
        format!(
            "exit={} stage={} outcome={} category={} node_path={}",
            self.exit,
            self.stage,
            self.outcome,
            self.category.as_deref().unwrap_or("-"),
            self.node_path.as_deref().unwrap_or("-"),
        )
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "exit": self.exit,
            "schema": self.schema,
            "stage": self.stage,
            "outcome": self.outcome,
            "category": self.category,
            "source_stage": self.source_stage,
            "node_path": self.node_path,
            "accepted_program_created": self.accepted_program_created,
            "guest_requests": self.guest_requests,
            "protected_operations": self.protected_operations,
            "report_blake3": blake3::hash(self.report.as_bytes()).to_hex().to_string(),
        })
    }
}

fn field(value: &serde_json::Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("report field {key} must be text"))
}

fn optional(value: Option<&serde_json::Value>, key: &str) -> Option<String> {
    value
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// A bounded driver of the real `noble editor` CLI: every document is written
/// under one scratch directory, submitted once, and removed again.
pub struct Runner {
    directory: PathBuf,
    invocations: usize,
}

impl Runner {
    pub fn new(directory: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("create runner directory: {error}"))?;
        Ok(Runner {
            directory: directory.to_path_buf(),
            invocations: 0,
        })
    }

    pub fn invocations(&self) -> usize {
        self.invocations
    }

    pub fn analyze(&mut self, name: &str, document: &str) -> Result<Verdict, String> {
        self.invoke("analyze", name, document)
    }

    pub fn admit(&mut self, name: &str, document: &str) -> Result<Verdict, String> {
        self.invoke("admit", name, document)
    }

    fn invoke(&mut self, action: &str, name: &str, document: &str) -> Result<Verdict, String> {
        let path = self
            .directory
            .join(format!("{name}-{action}-{}.json", self.invocations));
        std::fs::write(&path, document).map_err(|error| format!("write {name}: {error}"))?;
        let output = Command::new(env!("CARGO_BIN_EXE_noble"))
            .args(["editor", action])
            .arg(&path)
            .output()
            .map_err(|error| format!("launch noble editor {action}: {error}"))?;
        let _ = std::fs::remove_file(&path);
        self.invocations = self.invocations.saturating_add(1);
        let exit = output
            .status
            .code()
            .ok_or_else(|| format!("{name}: the CLI did not exit normally"))?;
        let stdout = String::from_utf8(output.stdout)
            .map_err(|error| format!("{name}: report is not UTF-8: {error}"))?;
        Verdict::from_report(exit, &stdout)
            .map_err(|error| format!("{name}/{action}: {error}; stdout={stdout}"))
    }
}

/// Compare one pair of runs and describe the difference on failure.
pub fn equal(name: &str, what: &str, left: &Verdict, right: &Verdict) -> Result<(), String> {
    if left == right {
        return Ok(());
    }
    Err(format!(
        "{name}: {what} verdicts differ: left[{}] right[{}]",
        left.summary(),
        right.summary()
    ))
}
