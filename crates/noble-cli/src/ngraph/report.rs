//! `noble-ngraph-report/v1` lines (design D8): a fixed key prefix, then the
//! command's fields. `guest_requests` is always zero: the `ngraph` command
//! family has no guest-runtime path. Reports carry no file path and no timing
//! (D11).

use crate::workflow::encoding::{decimal_len, quoted_len, string, Json};

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

    /// A backend-specific refusal or exhaustion that has no shared graph
    /// diagnostic code.
    pub(super) fn backend_failure(
        command: &'static str,
        stage: &'static str,
        outcome: noble_ngraph::Outcome,
        code: &str,
        detail: impl Into<std::string::String>,
    ) -> Self {
        Self(std::boxed::Box::new(Fields {
            command,
            stage,
            outcome,
            code: Some(code.into()),
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
            ("guest_requests", Json::Number(0)),
        ];
        if let Some(d) = &line.detail {
            entries.push(("detail", string(d)));
        }
        entries.extend(line.fields.iter().cloned());
        Json::Object(entries).encode()
    }

    /// The exact byte length of [`Line::encode`], computed without encoding,
    /// so an output ceiling can be charged before the text is built.
    pub(super) fn encoded_len(&self) -> usize {
        let line = &self.0;
        let optional = |v: &Option<std::string::String>| v.as_deref().map_or(4, quoted_len);
        let envelope = [
            ("schema", quoted_len(SCHEMA)),
            ("command", quoted_len(line.command)),
            ("stage", quoted_len(line.stage)),
            ("outcome", quoted_len(line.outcome.name())),
            ("code", optional(&line.code)),
            ("site", optional(&line.site)),
            ("guest_requests", decimal_len(0)),
        ];
        let detail = line.detail.as_deref().map(|d| ("detail", quoted_len(d)));
        let fields = line
            .fields
            .iter()
            .map(|(key, value)| (*key, value.encoded_len()));
        let (members, length) = envelope.into_iter().chain(detail).chain(fields).fold(
            (0usize, 0usize),
            |(members, length), (key, value)| {
                (
                    members.saturating_add(1),
                    length
                        .saturating_add(quoted_len(key))
                        .saturating_add(1)
                        .saturating_add(value),
                )
            },
        );
        length
            .saturating_add(members.saturating_sub(1))
            .saturating_add(2)
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

#[cfg(test)]
mod tests {
    use super::{string, Json, Line};

    #[test]
    fn encoded_len_is_the_length_of_every_line_shape() {
        let lines = [
            Line::normal("eval", "evaluation"),
            Line::host("eval", "input", "a \"quoted\"\tdetail"),
            Line::backend_failure(
                "eval",
                "output",
                noble_ngraph::Outcome::Exhausted,
                "E-NET-BUDGET-output-bytes",
                "net report needs more bytes",
            )
            .with("cap", Json::Number(0))
            .with("observed", Json::Number(u64::MAX)),
            Line::normal("eval", "evaluation")
                .with("vector", Json::Number(7))
                .with(
                    "outputs",
                    Json::Array(std::vec![string("i64:-1"), string("bool:true")]),
                )
                .with("nested", Json::Object(std::vec![("a", Json::Null)])),
        ];
        for line in &lines {
            assert_eq!(line.encoded_len(), line.encode().len(), "{}", line.encode());
        }
    }
}
