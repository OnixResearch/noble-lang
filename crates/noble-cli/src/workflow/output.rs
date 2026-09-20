// The protocol has seven closed outcomes; additions must select a name and exit code.
#[derive(Clone, Copy)]
#[octet::sealed_enum]
pub(super) enum Outcome {
    Proved,
    Disproved,
    Unknown,
    Timeout,
    Unsupported,
    Error,
    NotRun,
}

impl Outcome {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Proved => "proved",
            Self::Disproved => "disproved",
            Self::Unknown => "unknown",
            Self::Timeout => "timeout",
            Self::Unsupported => "unsupported",
            Self::Error => "error",
            Self::NotRun => "not-run",
        }
    }

    pub(super) const fn exit(self) -> u8 {
        match self {
            Self::Proved | Self::NotRun => 0,
            Self::Disproved => 1,
            Self::Error => 2,
            Self::Unknown => 3,
            Self::Unsupported => 4,
            Self::Timeout => 124,
        }
    }
}

pub struct Failure {
    pub(super) outcome: Outcome,
    pub(super) code: &'static str,
    pub(super) message: std::string::String,
    pub(super) details: super::encoding::Json,
}

impl Failure {
    pub fn error(code: &'static str, message: std::string::String) -> Self {
        Self {
            outcome: Outcome::Error,
            code,
            message,
            details: super::encoding::Json::Null,
        }
    }

    pub fn unsupported(code: &'static str, message: std::string::String) -> Self {
        Self {
            outcome: Outcome::Unsupported,
            code,
            message,
            details: super::encoding::Json::Null,
        }
    }

    pub fn timeout(message: &str) -> Self {
        Self {
            outcome: Outcome::Timeout,
            code: "timeout",
            message: message.into(),
            details: super::encoding::Json::Null,
        }
    }

    pub(super) fn json(&self) -> super::encoding::Json {
        super::encoding::object([
            ("code", super::encoding::string(self.code)),
            ("message", super::encoding::string(&self.message)),
            ("details", self.details.clone()),
        ])
    }
}

pub(super) struct Report {
    pub(super) outcome: Outcome,
    pub(super) command: &'static str,
    pub(super) source_path: std::string::String,
    pub(super) source: Option<std::string::String>,
    pub(super) invalid_source_bytes: Option<std::string::String>,
    pub(super) generated: Option<std::string::String>,
    pub(super) proof_wire: Option<std::string::String>,
    pub(super) subject: super::encoding::Json,
    pub(super) ordinary_typing: super::encoding::Json,
    pub(super) library: super::encoding::Json,
    pub(super) proof_request: super::encoding::Json,
    pub(super) theorem: Option<&'static str>,
    pub(super) axioms: Option<std::vec::Vec<std::string::String>>,
    pub(super) rechecked: bool,
    pub(super) diagnostics: std::vec::Vec<super::encoding::Json>,
    pub(super) timeout_ms: u64,
    pub(super) emitted: Option<std::string::String>,
    pub(super) tools: super::encoding::Json,
}

impl Report {
    pub(super) fn new() -> Self {
        Self {
            outcome: Outcome::NotRun,
            command: "verify",
            source_path: std::string::String::new(),
            source: None,
            invalid_source_bytes: None,
            generated: None,
            proof_wire: None,
            subject: super::encoding::Json::Null,
            ordinary_typing: super::encoding::string("not-run"),
            library: super::encoding::Json::Null,
            proof_request: super::encoding::Json::Null,
            theorem: None,
            axioms: None,
            rechecked: false,
            diagnostics: std::vec::Vec::new(),
            timeout_ms: super::DEFAULT_TIMEOUT,
            emitted: None,
            tools: super::encoding::Json::Null,
        }
    }

    pub(super) fn fail(&mut self, error: Failure) {
        self.outcome = error.outcome;
        self.diagnostics.push(error.json());
    }

    pub(super) fn json(&self) -> super::encoding::Json {
        super::encoding::object([
            ("schema", super::encoding::string("noble-mc1-report/v1")),
            ("command", super::encoding::string(self.command)),
            ("outcome", super::encoding::string(self.outcome.name())),
            ("source_path", super::encoding::string(&self.source_path)),
            ("source", super::encoding::optional_string(self.source.as_deref())),
            ("invalid_source_bytes_hex", super::encoding::optional_string(self.invalid_source_bytes.as_deref())),
            ("generated_statement", super::encoding::optional_string(self.generated.as_deref())),
            ("proof_declarations", super::encoding::optional_string(self.proof_wire.as_deref())),
            ("claim_kind", super::encoding::string("partial-correctness")),
            ("subject", self.subject.clone()),
            ("ordinary_typing", self.ordinary_typing.clone()),
            ("assumptions", super::encoding::object([
                ("allowed_axioms", super::encoding::strings(&["propext", "Classical.choice", "Quot.sound"])),
                ("accepted_transitive_axioms", match &self.axioms {
                    Some(axioms) => super::encoding::Json::Array(axioms.iter().map(super::encoding::string).collect()),
                    None => super::encoding::Json::Null,
                }),
                ("semantic_premises", super::encoding::strings(&[
                    "universally quantified typed input stack and ghost parameters",
                    "the exact exported requires predicate",
                    "normal-return Exec of the exact accepted program",
                ])),
                ("scope", super::encoding::string("Pure normal-return partial correctness with allocation/quota abstraction; no termination, abnormal-outcome safety, runtime applicability, host-state, or Wasm claim.")),
            ])),
            ("rule_library_revision", self.library.clone()),
            ("proof_request", self.proof_request.clone()),
            ("theorem", super::encoding::optional_string(self.theorem)),
            ("independent_recheck", super::encoding::Json::Bool(self.rechecked)),
            ("recheck_method", super::encoding::string("Bounded JSON declaration decoding into fresh kernel constructors, consumer-selected definitions, exact theorem type and transitive axiom traversal; no producer binary artifacts loaded by the consumer.")),
            ("implementation_refinement", super::encoding::object([
                ("status", super::encoding::string("not-checked-by-this-command")),
                ("scope", super::encoding::string("Rust frontend/exporter and ordinary kernel refinement are separate Aeneas obligations; application acceptance does not establish them.")),
            ])),
            ("backend_correspondence", super::encoding::object([
                ("status", super::encoding::string("not-claimed")),
                ("scope", super::encoding::string("No Wasm, compilation, loading, runtime replay, or MC2 evidence is accepted here.")),
            ])),
            ("outstanding_obligations", if self.rechecked {
                super::encoding::strings(&["implementation refinement (separate verification lane)", "backend correspondence (outside MC1)"])
            } else {
                super::encoding::strings(&["application claim or refutation", "implementation refinement (separate verification lane)", "backend correspondence (outside MC1)"])
            }),
            ("consumer_policy", super::encoding::object([
                ("lean_toolchain", super::encoding::string(crate::sandbox::PIN)),
                ("lean_commit", super::encoding::string(crate::sandbox::COMMIT)),
                ("timeout_ms", super::encoding::Json::Number(self.timeout_ms)),
                ("proof_source_bytes", super::encoding::Json::Number(super::PROOF_LIMIT as u64)),
                ("process_output_bytes_per_stream", super::encoding::Json::Number(crate::sandbox::OUTPUT_LIMIT as u64)),
                ("artifact_bytes_per_file", super::encoding::Json::Number(crate::sandbox::ARTIFACT_LIMIT)),
                ("sandbox", super::encoding::string("bubblewrap; all namespaces; no network; read-only inputs; fixed writable artifact files; memory/CPU/file/output limits")),
                ("trusted_tools", self.tools.clone()),
            ])),
            ("diagnostics", super::encoding::Json::Array(self.diagnostics.clone())),
            ("emitted", super::encoding::optional_string(self.emitted.as_deref())),
        ])
    }
}
