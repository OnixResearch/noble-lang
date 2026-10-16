//! `noble-ngraph-report/v1` lines (design D8): a fixed key prefix, then the
//! command's fields. Reports carry no file path and no timing (D11).

use crate::workflow::encoding::{string, Json};

pub(super) const SCHEMA: &str = "noble-ngraph-report/v1";

/// One report line (boxed: refusals travel through `Result` errors).
pub(super) struct Line(std::boxed::Box<Fields>);

struct Fields {
    command: &'static str,
    stage: &'static str,
    outcome: noble_ngraph::Outcome,
    code: Option<std::string::String>,
    site: Option<std::string::String>,
    detail: Option<std::string::String>,
    fields: std::vec::Vec<(&'static str, Json)>,
}

impl Line {
    /// A `normal` line.
    pub(super) fn normal(command: &'static str, stage: &'static str) -> Self {
        Self(std::boxed::Box::new(Fields {
            command,
            stage,
            outcome: noble_ngraph::Outcome::Normal,
            code: None,
            site: None,
            detail: None,
            fields: std::vec::Vec::new(),
        }))
    }

    /// A coded refusal or observation: the outcome class follows the code.
    pub(super) fn diagnostic(
        command: &'static str,
        stage: &'static str,
        d: &noble_ngraph::Diagnostic,
    ) -> Self {
        Self(std::boxed::Box::new(Fields {
            command,
            stage,
            outcome: d.code.outcome(),
            code: Some(d.code_string()),
            site: Some(d.site.render()),
            detail: Some(d.detail.clone()),
            fields: std::vec::Vec::new(),
        }))
    }

    /// A host condition (I/O failure, exceeded input bound, existing
    /// output): `reject` with a null code.
    pub(super) fn host(
        command: &'static str,
        stage: &'static str,
        detail: impl Into<std::string::String>,
    ) -> Self {
        Self(std::boxed::Box::new(Fields {
            command,
            stage,
            outcome: noble_ngraph::Outcome::Reject,
            code: None,
            site: None,
            detail: Some(detail.into()),
            fields: std::vec::Vec::new(),
        }))
    }

    /// An evaluation observation that is not normal.
    pub(super) fn observation(command: &'static str, obs: &noble_ngraph::Obs) -> Self {
        match obs {
            noble_ngraph::Obs::Normal(_) => Self::normal(command, "evaluation"),
            noble_ngraph::Obs::Refused(d) => Self::diagnostic(command, "evaluation", d),
            noble_ngraph::Obs::GuardFailed { node, fact } => Self(std::boxed::Box::new(Fields {
                command,
                stage: "evaluation",
                outcome: noble_ngraph::Outcome::Trap,
                code: Some("G-FACT-FALSE".into()),
                site: Some(noble_ngraph::Site::Node(*node).render()),
                detail: Some(std::format!(
                    "%{} fails its {} fact",
                    node.0,
                    fact.kind_name()
                )),
                fields: std::vec::Vec::new(),
            })),
            noble_ngraph::Obs::Exhausted(kind) => Self(std::boxed::Box::new(Fields {
                command,
                stage: "evaluation",
                outcome: noble_ngraph::Outcome::Exhausted,
                code: Some(noble_ngraph::Code::Budget(*kind).as_string()),
                site: None,
                detail: Some("host evaluation fuel exhausted".into()),
                fields: std::vec::Vec::new(),
            })),
            noble_ngraph::Obs::Cancelled => Self(std::boxed::Box::new(Fields {
                command,
                stage: "evaluation",
                outcome: noble_ngraph::Outcome::Trap,
                code: Some("C-CANCELLED".into()),
                site: None,
                detail: Some("cancelled by the host".into()),
                fields: std::vec::Vec::new(),
            })),
        }
    }

    /// Appends a command field.
    pub(super) fn with(mut self, key: &'static str, value: Json) -> Self {
        self.0.fields.push((key, value));
        self
    }

    /// Whether the line is `normal`.
    pub(super) fn is_normal(&self) -> bool {
        self.0.outcome == noble_ngraph::Outcome::Normal
    }

    /// The process exit status of this line's outcome class.
    pub(super) fn exit(&self) -> u8 {
        self.0.outcome.exit()
    }

    /// The JSON text of the line.
    pub(super) fn encode(&self) -> std::string::String {
        let line = &self.0;
        let optional = |v: &Option<std::string::String>| match v {
            Some(s) => string(s),
            None => Json::Null,
        };
        let mut entries = std::vec![
            ("schema", string(SCHEMA)),
            ("command", string(line.command)),
            ("stage", string(line.stage)),
            ("outcome", string(line.outcome.name())),
            ("code", optional(&line.code)),
            ("site", optional(&line.site)),
        ];
        if let Some(d) = &line.detail {
            entries.push(("detail", string(d)));
        }
        entries.extend(line.fields.iter().cloned());
        Json::Object(entries).encode()
    }
}

/// `"0x%016x"` of a binary64 value.
pub(super) fn bits64(v: f64) -> Json {
    string(std::format!("0x{:016x}", v.to_bits()))
}

/// Lowercase hex of a digest.
pub(super) fn digest(d: &[u8; 32]) -> Json {
    string(noble_ngraph::ir::hex(d))
}

/// A count.
pub(super) fn count(n: usize) -> Json {
    Json::Number(u64::try_from(n).unwrap_or(u64::MAX))
}
