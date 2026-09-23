//! The fixed `noble-mc2-build/v1` report serializer.
//!
//! One JSON document is written to stdout; the shared outcome enum owns the
//! exit mapping and the shared `Failure` carrier owns diagnostic rendering.

mod release;

/// The consumer-selected policy revision this build checks evidence against.
pub(super) const POLICY_REVISION: u32 = 1;

pub(super) struct ReleaseDecision {
    pub(super) allowed: bool,
    pub(super) reason: std::string::String,
    pub(super) refusals: std::vec::Vec<&'static str>,
    pub(super) core_subject: Option<u64>,
    pub(super) core_statement: Option<u64>,
    pub(super) core_contract: Option<u32>,
    pub(super) core_evidence: Option<u32>,
}

const fn number_or_null(value: Option<u64>) -> crate::workflow::encoding::Json {
    match value {
        Some(value) => crate::workflow::encoding::Json::Number(value),
        None => crate::workflow::encoding::Json::Null,
    }
}

pub(super) fn hex_u64(value: Option<u64>) -> crate::workflow::encoding::Json {
    match value {
        Some(value) => crate::workflow::encoding::string(std::format!("{value:016x}")),
        None => crate::workflow::encoding::Json::Null,
    }
}

/// The artifact-correspondence record (PO-17/18), always emitted so the report
/// never silently omits that the emitted Wasm is build evidence only.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; artifact_correspondence exhaustively encodes optional artifact byte counts and the absence of a verified backend claim. Unbuilt and partially built artifacts are valid failure-report states; reporting them must not assert successful compilation or loading."
)]
pub(super) fn artifact_correspondence(
    wat_bytes: Option<u64>,
    wasm_bytes: Option<u64>,
) -> crate::workflow::encoding::Json {
    let status = match (wat_bytes, wasm_bytes) {
        (Some(_), Some(_)) => "recorded-trusted-build",
        (Some(_), None) => "recorded-trusted-build-without-wasm",
        _ => "not-recorded",
    };
    crate::workflow::encoding::object([
        ("status", crate::workflow::encoding::string(status)),
        (
            "build_evidence",
            crate::workflow::encoding::string(
                "PO-17: artifact.wat is the independently accepted noble-wasm lowering of SOURCE; artifact.wasm is assembled and validated by the selected wasm-tools path.",
            ),
        ),
        (
            "loading_evidence",
            crate::workflow::encoding::string(
                "PO-18: the assembly path instantiates the module in the selected Node runtime; this command records that loading ran, not a refinement proof.",
            ),
        ),
        (
            "verified_backend_claimed",
            crate::workflow::encoding::Json::Bool(false),
        ),
        (
            "scope",
            crate::workflow::encoding::string(
                "Build/loading evidence for the declared fragment only. It never establishes that emitted Wasm refines the certified semantics.",
            ),
        ),
        ("wat_bytes", number_or_null(wat_bytes)),
        ("wasm_bytes", number_or_null(wasm_bytes)),
    ])
}

pub(super) struct Document {
    pub(super) outcome: crate::workflow::output::Outcome,
    pub(super) require_proof: bool,
    pub(super) source_path: std::string::String,
    pub(super) contract_path: std::string::String,
    pub(super) release: ReleaseDecision,
    pub(super) subject: crate::workflow::encoding::Json,
    pub(super) statement_digest: Option<u64>,
    pub(super) candidate_digest: Option<u64>,
    pub(super) evidence_class: Option<&'static str>,
    pub(super) evidence_supplied: bool,
    pub(super) independent_recheck: bool,
    pub(super) accepted_axioms: Option<std::vec::Vec<std::string::String>>,
    pub(super) declaration_bytes: u64,
    pub(super) tools: crate::workflow::encoding::Json,
    pub(super) artifacts: crate::workflow::encoding::Json,
    pub(super) diagnostics: std::vec::Vec<crate::workflow::encoding::Json>,
    pub(super) timeout_ms: u64,
    pub(super) emitted: Option<std::string::String>,
}

impl Document {
    pub(super) fn record_application(
        &mut self,
        contract: &crate::workflow::admission::Contract,
        source: &[u8],
        evidence: Option<crate::workflow::admission::Evidence>,
    ) {
        self.subject = crate::workflow::subject_json(&contract.prepared, source);
        self.statement_digest = Some(super::statement_fold(contract.statement.as_bytes()));
        self.candidate_digest = Some(noble_contracts::companion::statement_digest(
            &contract.prepared,
        ));
        if let Some(checked) = evidence {
            self.independent_recheck = true;
            self.tools = checked.tools;
            self.accepted_axioms = Some(checked.axioms);
        }
    }

    pub(super) fn new() -> Self {
        Self {
            outcome: crate::workflow::output::Outcome::NotRun,
            require_proof: false,
            source_path: std::string::String::new(),
            contract_path: std::string::String::new(),
            release: ReleaseDecision::blocked("no evidence was offered"),
            subject: crate::workflow::encoding::Json::Null,
            statement_digest: None,
            candidate_digest: None,
            evidence_class: None,
            evidence_supplied: false,
            independent_recheck: false,
            accepted_axioms: None,
            declaration_bytes: 0,
            tools: crate::workflow::encoding::Json::Null,
            artifacts: artifact_correspondence(None, None),
            diagnostics: std::vec::Vec::new(),
            timeout_ms: crate::workflow::DEFAULT_TIMEOUT,
            emitted: None,
        }
    }

    pub(super) fn fail(&mut self, error: crate::workflow::output::Failure) {
        self.outcome = error.outcome;
        self.release = ReleaseDecision::blocked("the build failed");
        self.diagnostics.push(error.json());
    }

    pub(super) fn json(&self) -> crate::workflow::encoding::Json {
        crate::workflow::encoding::object([
            (
                "schema",
                crate::workflow::encoding::string("noble-mc2-build/v1"),
            ),
            ("command", crate::workflow::encoding::string("build")),
            // Assembly loads the inert module; no guest body or host import
            // is invoked by this command, even when the build is released.
            ("candidate_body_started", crate::workflow::encoding::Json::Bool(false)),
            ("guest_requests", crate::workflow::encoding::Json::Number(0)),
            ("protected_operations", crate::workflow::encoding::Json::Number(0)),
            (
                "outcome",
                crate::workflow::encoding::string(self.outcome.name()),
            ),
            (
                "proof_required",
                crate::workflow::encoding::Json::Bool(self.require_proof),
            ),
            (
                "source_path",
                crate::workflow::encoding::string(&self.source_path),
            ),
            (
                "contract_path",
                crate::workflow::encoding::string(&self.contract_path),
            ),
            ("release", self.release.json()),
            (
                "subject_identity",
                crate::workflow::encoding::object([
                    (
                        "digest",
                        hex_u64(self.candidate_digest),
                    ),
                    (
                        "basis",
                        crate::workflow::encoding::string(
                            "exact statement/subject digest recomputed by the deterministic core from the retained preparation; release.core_subject_identity repeats it for a released claim",
                        ),
                    ),
                ]),
            ),
            (
                "claim_statement_digest",
                crate::workflow::encoding::object([
                    ("digest", hex_u64(self.statement_digest)),
                    (
                        "basis",
                        crate::workflow::encoding::string(
                            "fold over the exported Lean claim statement bytes only",
                        ),
                    ),
                ]),
            ),
            (
                "evidence",
                crate::workflow::encoding::object([
                    (
                        "supplied",
                        crate::workflow::encoding::Json::Bool(self.evidence_supplied),
                    ),
                    (
                        "class",
                        crate::workflow::encoding::optional_string(self.evidence_class),
                    ),
                    (
                        "independent_recheck",
                        crate::workflow::encoding::Json::Bool(self.independent_recheck),
                    ),
                    (
                        "declaration_bytes",
                        crate::workflow::encoding::Json::Number(self.declaration_bytes),
                    ),
                    (
                        "accepted_transitive_axioms",
                        match &self.accepted_axioms {
                            Some(axioms) => crate::workflow::encoding::Json::Array(
                                axioms
                                    .iter()
                                    .map(crate::workflow::encoding::string)
                                    .collect(),
                            ),
                            None => crate::workflow::encoding::Json::Null,
                        },
                    ),
                    (
                        "checks",
                        crate::workflow::encoding::string(
                            "isolated Lean producer plus independent consumer declaration replay; producer status text never establishes acceptance",
                        ),
                    ),
                ]),
            ),
            ("subject", self.subject.clone()),
            (
                "context",
                crate::workflow::encoding::object([
                    (
                        "policy_revision",
                        crate::workflow::encoding::Json::Number(u64::from(POLICY_REVISION)),
                    ),
                    (
                        "ruleset",
                        crate::workflow::encoding::Json::Number(u64::from(
                            noble_contracts::companion::RULESET_V1,
                        )),
                    ),
                ]),
            ),
            ("toolchain", self.tools.clone()),
            ("artifact_correspondence", self.artifacts.clone()),
            (
                "outstanding_obligations",
                crate::workflow::encoding::strings(&[
                    "implementation refinement of the frontend and contracts core (separate Charon/Aeneas lane)",
                    "backend correspondence between emitted Wasm and the certified fragment (not claimed here)",
                    "runtime certified invocation and companion session operations (noble companions)",
                ]),
            ),
            ("non_claims", crate::workflow::encoding::strings(&NON_CLAIMS)),
            (
                "diagnostics",
                crate::workflow::encoding::Json::Array(self.diagnostics.clone()),
            ),
            (
                "emitted",
                crate::workflow::encoding::optional_string(self.emitted.as_deref()),
            ),
        ])
    }
}

const NON_CLAIMS: [&str; 8] = [
    "host/resource protocols remain open",
    "native async remains open",
    "components remain open",
    "general refinement inference remains open",
    "proof markets remain open",
    "portable canonical encodings remain open",
    "verified backend correspondence is not claimed",
    "general termination and whole-language preservation remain open",
];
