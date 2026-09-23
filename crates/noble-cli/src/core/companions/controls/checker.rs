/// Trusted test adapter delegates every positive admission to the actual MC1
/// consumer. It validates structured checker output and exact generated text,
/// rather than treating process success or producer text as acceptance.
pub(super) struct LeanConsumer {
    pub(super) binary: std::path::PathBuf,
    pub(super) directory: std::path::PathBuf,
    pub(super) source: Vec<u8>,
    pub(super) sequence: usize,
    pub(super) reports: Vec<crate::workflow::encoding::Json>,
    pub(super) last_report: Option<crate::core::report::Value>,
}

impl LeanConsumer {
    pub(super) fn admit(
        &mut self,
        core: &mut noble_contracts::companion::Core,
        expected: &noble_contracts::Prepared,
        offered: noble_contracts::companion::EvidenceOffer,
    ) -> noble_contracts::companion::Admission {
        let request = match core.begin_admission(expected, offered) {
            Ok(request) => request,
            Err(admission) => return admission,
        };
        let observation = self.check(&request);
        core.complete_admission(request, observation)
    }

    pub(super) fn check(
        &mut self,
        request: &noble_contracts::companion::AdmissionRequest<'_>,
    ) -> noble_contracts::companion::CheckObservation {
        let offered = request.offer();
        let bytes = match &offered.payload {
            noble_contracts::companion::EvidencePayload::Declaration(bytes) => bytes,
            noble_contracts::companion::EvidencePayload::Resource(_)
            | noble_contracts::companion::EvidencePayload::ServiceCapability(_) => {
                unreachable!("begin_admission refuses live capabilities")
            }
        };
        let expected_statement = noble_contracts::export_lean(request.expected());
        if noble_contracts::export_lean(&super::prepared(&self.source)) != expected_statement {
            return noble_contracts::companion::CheckObservation::new(
                String::new(),
                bytes.clone(),
                Err(noble_contracts::companion::Refusal::MismatchedClaim),
            );
        }
        let output = self.run(bytes, offered.refutation);
        // Report proof_declarations is checked wire JSON, not submitted Lean source.
        self.observation(
            &expected_statement,
            offered.refutation,
            bytes.clone(),
            output,
        )
    }

    fn run(&mut self, bytes: &[u8], refutation: bool) -> std::process::Output {
        self.sequence += 1;
        let directory = self.directory.join(self.sequence.to_string());
        std::fs::create_dir_all(&directory).expect("checker workspace");
        let contract = directory.join("contract.noble");
        let proof = directory.join("proof.lean");
        std::fs::write(&contract, &self.source).expect("contract source");
        std::fs::write(&proof, bytes).expect("proof source");
        std::process::Command::new(&self.binary)
            .arg("verify")
            .arg(&contract)
            .arg(if refutation {
                "--refutation"
            } else {
                "--proof"
            })
            .arg(&proof)
            .args(["--timeout-ms", "600000"])
            .output()
            .expect("launch real independent checker")
    }

    fn observation(
        &mut self,
        expected_statement: &str,
        refutation: bool,
        checked_source: Vec<u8>,
        output: std::process::Output,
    ) -> noble_contracts::companion::CheckObservation {
        let report_json = std::str::from_utf8(&output.stdout).expect("checker UTF-8 output");
        let report = crate::core::report::parse(report_json).unwrap_or_else(|error| {
            panic!(
                "checker returned no structured report: {error}; stderr={}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
        let result = verdict(
            expected_statement,
            refutation,
            &report,
            output.status.code(),
        );
        let checked_statement = match super::report_text(&report, "generated_statement") {
            Some(statement) => statement.to_owned(),
            None => String::new(),
        };
        let exit = match output.status.code() {
            Some(code) => crate::workflow::encoding::Json::Number(
                u64::try_from(code).expect("nonnegative checker status"),
            ),
            None => crate::workflow::encoding::Json::Null,
        };
        self.reports.push(crate::workflow::encoding::object([
            (
                "report_json",
                crate::workflow::encoding::string(report_json),
            ),
            ("exit", exit),
            (
                "stderr",
                crate::workflow::encoding::string(String::from_utf8_lossy(&output.stderr)),
            ),
        ]));
        self.last_report = Some(report);
        noble_contracts::companion::CheckObservation::new(checked_statement, checked_source, result)
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; verdict checks report schema, outcome, exact statement and theorem, independent recheck, declaration wire presence and the complete axiom list; invalid checker output returns ForgedStatus rather than asserting acceptance."
)]
fn verdict(
    expected_statement: &str,
    refutation: bool,
    report: &crate::core::report::Value,
    exit: Option<i32>,
) -> Result<noble_contracts::companion::EvidenceClass, noble_contracts::companion::Refusal> {
    let (theorem, outcome, status, class) = if refutation {
        (
            "MC1Proof.refutation",
            "disproved",
            1,
            noble_contracts::companion::EvidenceClass::LeanRefutation,
        )
    } else {
        (
            "MC1Proof.proof",
            "proved",
            0,
            noble_contracts::companion::EvidenceClass::LeanExact,
        )
    };
    let is_accepted = exit == Some(status)
        && super::report_text(report, "schema") == Some("noble-mc1-report/v1")
        && super::report_text(report, "outcome") == Some(outcome)
        && report.member("independent_recheck") == Some(&crate::core::report::Value::Bool(true))
        && super::report_text(report, "theorem") == Some(theorem)
        && super::report_text(report, "generated_statement") == Some(expected_statement)
        && super::report_text(report, "proof_declarations").is_some()
        && report
            .member("assumptions")
            .and_then(|value| value.member("accepted_transitive_axioms"))
            .and_then(crate::core::report::Value::items)
            .is_some_and(|axioms| {
                axioms.iter().all(|axiom| {
                    matches!(
                        axiom.text(),
                        Some("propext" | "Classical.choice" | "Quot.sound")
                    )
                })
            });
    if is_accepted {
        Ok(class)
    } else {
        Err(noble_contracts::companion::Refusal::ForgedStatus)
    }
}

pub(super) fn checked(
    core: &mut noble_contracts::companion::Core,
    checker: &mut LeanConsumer,
    source: &[u8],
    proof: &[u8],
) -> (
    noble_contracts::Prepared,
    noble_contracts::companion::Admission,
    crate::workflow::encoding::Json,
) {
    checker.source = source.to_vec();
    let contract = super::prepared(source);
    let result = checker.admit(core, &contract, super::offer(proof));
    assert_eq!(
        result.outcome,
        noble_contracts::companion::Outcome::Proved,
        "independent proof rejected: {:?}",
        checker.last_report
    );
    let evidence = result.evidence.expect("checked evidence");
    let subject = super::retained_subject(core, &result, &contract);
    assert!(core.bind(evidence, &subject).is_ok());
    let baseline = baseline(checker, &result);
    (contract, result, baseline)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; baseline only encodes the fixed fields of an admission already asserted proved and bindable by checked; checked report indexing and required evidence/contract lookups fail explicitly if that provenance is absent."
)]
fn baseline(
    checker: &LeanConsumer,
    result: &noble_contracts::companion::Admission,
) -> crate::workflow::encoding::Json {
    let is_rechecked = checker
        .last_report
        .as_ref()
        .and_then(|report| report.member("independent_recheck"))
        == Some(&crate::core::report::Value::Bool(true));
    crate::workflow::encoding::object([
        (
            "accepted",
            crate::workflow::encoding::Json::Bool(
                result.outcome == noble_contracts::companion::Outcome::Proved,
            ),
        ),
        (
            "independent_recheck",
            crate::workflow::encoding::Json::Bool(is_rechecked),
        ),
        (
            "evidence",
            crate::workflow::encoding::Json::Number(u64::from(
                result.evidence.expect("evidence").0,
            )),
        ),
        (
            "contract",
            crate::workflow::encoding::Json::Number(u64::from(
                result.contract.expect("contract").0,
            )),
        ),
        (
            "statement",
            crate::workflow::encoding::string(result.statement_digest.to_string()),
        ),
        (
            "checker_report",
            crate::workflow::encoding::Json::Number(
                u64::try_from(
                    checker
                        .reports
                        .len()
                        .checked_sub(1)
                        .expect("actual checker report"),
                )
                .expect("checker report index"),
            ),
        ),
    ])
}
