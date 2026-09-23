//! Shared consumer-side contract admission.
//!
//! `noble verify` owns the report-shaped explanation path; proof-required
//! builds and the companion session need the same admission decision without a
//! report. Both call this module so there is exactly one place that decides
//! whether evidence is accepted for an exact claim, and so both inherit the
//! same seven-outcome spelling.

/// A contract the consumer prepared and elaborated itself.
pub(crate) struct Contract {
    pub(crate) prepared: noble_contracts::Prepared,
    /// The exact exported statement text the consumer generated. Producer text
    /// never supplies this: it is regenerated from the retained preparation.
    pub(crate) statement: std::string::String,
}

/// Evidence the consumer's own independent recheck accepted.
pub(crate) struct Evidence {
    pub(crate) axioms: std::vec::Vec<std::string::String>,
    pub(crate) tools: crate::workflow::encoding::Json,
}

/// Why admission did not accept evidence.
pub(crate) struct Refusal {
    /// One of the protocol's seven closed outcomes. A missing proof maps to
    /// `unknown`, a rejected proof term to `error`, an unavailable sandbox to
    /// `unsupported`, and an expired budget to `timeout`; none of them is a
    /// semantic claim about the proposition.
    pub(crate) outcome: crate::workflow::output::Outcome,
    pub(crate) code: &'static str,
    pub(crate) message: std::string::String,
    pub(crate) details: crate::workflow::encoding::Json,
    pub(crate) verification_ran: bool,
    /// True when the ordinary subject still accepted despite the refusal.
    pub(crate) ordinary_typing: bool,
}

impl Refusal {
    fn new(
        outcome: crate::workflow::output::Outcome,
        code: &'static str,
        message: std::string::String,
    ) -> Self {
        Self {
            outcome,
            code,
            message,
            details: crate::workflow::encoding::Json::Null,
            verification_ran: false,
            ordinary_typing: false,
        }
    }

    /// Preserve the verification pipeline's own classification, code, and
    /// message: a sandbox that could not start and an expired deadline are not
    /// the same outcome as a rejected proof term.
    fn verification(failure: crate::workflow::output::Failure) -> Self {
        Self {
            outcome: failure.outcome,
            code: failure.code,
            message: failure.message,
            details: failure.details,
            verification_ran: true,
            ordinary_typing: true,
        }
    }

    /// The seven-outcome spelling of this refusal.
    pub(crate) const fn claim_outcome(&self) -> &'static str {
        self.outcome.name()
    }
}

/// Exact contract, host check evidence, and the core's bounded completion.
pub(crate) struct Completion {
    pub(crate) contract: Contract,
    pub(crate) evidence: Option<Evidence>,
    pub(crate) decision: noble_contracts::companion::Admission,
}

/// External checking belongs to the shell, never to the deterministic core.
/// This observation is constructed only after the real isolated producer and
/// independent consumer have accepted the exact regenerated statement.
#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; check reads the consumer library, regenerates and verifies Lean through isolated processes, and allocates an exact observation. These non-const operations preserve malformed evidence and host failures as typed Failure instead of assertion panics."
)]
fn check(
    request: &noble_contracts::companion::AdmissionRequest<'_>,
    timeout_ms: u64,
) -> Result<
    (noble_contracts::companion::CheckObservation, Evidence),
    crate::workflow::output::Failure,
> {
    let offer = request.offer();
    let bytes = match &offer.payload {
        noble_contracts::companion::EvidencePayload::Declaration(bytes) => bytes,
        noble_contracts::companion::EvidencePayload::Resource(_)
        | noble_contracts::companion::EvidencePayload::ServiceCapability(_) => {
            return Err(crate::workflow::output::Failure::error(
                "live-capability-in-evidence",
                "evidence must contain only inert proof data".into(),
            ));
        }
    };
    let proof = attempt!(std::str::from_utf8(bytes).map_err(|_| {
        crate::workflow::output::Failure::error(
            "proof-encoding",
            "proof source must be UTF-8".into(),
        )
    }));
    let library = attempt!(super::rules::read_library());
    let statement = noble_contracts::export_lean(request.expected());
    let acceptance = attempt!(super::verification::verify(
        &statement,
        proof,
        &library,
        offer.refutation,
        timeout_ms
    ));
    let class = if offer.refutation {
        noble_contracts::companion::EvidenceClass::LeanRefutation
    } else {
        noble_contracts::companion::EvidenceClass::LeanExact
    };
    let observation =
        noble_contracts::companion::CheckObservation::new(statement, bytes.clone(), Ok(class));
    Ok((
        observation,
        Evidence {
            axioms: acceptance.axioms,
            tools: acceptance.tools,
        },
    ))
}

/// Raw offers grant nothing. A current, exact host observation is required to
/// complete a nonempty offer; host failures preserve the actual diagnostics.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; admit preserves preparation refusals, bounded core admission decisions and isolated verification failures before completing the exact request with its host observation. Rejected or unavailable evidence must remain a decision or typed Refusal, not an asserted success assumption."
)]
pub(crate) fn admit(
    core: &mut noble_contracts::companion::Core,
    source: &[u8],
    limits: noble_contracts::Limits,
    proof: Option<(&[u8], bool)>,
    timeout_ms: u64,
) -> Result<Completion, Refusal> {
    let contract = attempt!(prepare(source, limits));
    let request = match core.begin_admission(&contract.prepared, offer(proof)) {
        Ok(request) => request,
        Err(decision) => {
            return Ok(Completion {
                contract,
                evidence: None,
                decision,
            })
        }
    };
    let (observation, evidence) =
        attempt!(check(&request, timeout_ms).map_err(Refusal::verification));
    let decision = core.complete_admission(request, observation);
    Ok(Completion {
        contract,
        evidence: Some(evidence),
        decision,
    })
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; prepare invokes the runtime frontend and Lean exporter, allocating retained compiler state and statement text. Invalid, unsupported, exhausted and internal diagnostics are exhaustively preserved as typed Refusal; hostile contract source must not trigger assertion panics."
)]
fn prepare(source: &[u8], limits: noble_contracts::Limits) -> Result<Contract, Refusal> {
    let prepared = match noble_contracts::prepare(source, limits) {
        Ok(prepared) => prepared,
        Err(error) => {
            let has_ordinary_typing = error.ordinary_typing().is_some();
            let mut refusal = match error.kind {
                noble_contracts::DiagnosticKind::Unsupported => Refusal::new(
                    crate::workflow::output::Outcome::Unsupported,
                    "contract-unsupported",
                    error.message,
                ),
                noble_contracts::DiagnosticKind::Invalid => Refusal::new(
                    crate::workflow::output::Outcome::Error,
                    "contract-invalid",
                    error.message,
                ),
                noble_contracts::DiagnosticKind::Exhausted => Refusal::new(
                    crate::workflow::output::Outcome::Error,
                    "preparation-limit",
                    error.message,
                ),
                noble_contracts::DiagnosticKind::Internal => Refusal::new(
                    crate::workflow::output::Outcome::Error,
                    "preparation-internal",
                    error.message,
                ),
            };
            refusal.ordinary_typing = has_ordinary_typing;
            return Err(refusal);
        }
    };

    let statement = noble_contracts::export_lean(&prepared);
    Ok(Contract {
        prepared,
        statement,
    })
}

fn offer(proof: Option<(&[u8], bool)>) -> noble_contracts::companion::EvidenceOffer {
    let (declaration, is_refutation) = match proof {
        Some((bytes, is_refutation)) => (bytes.to_vec(), is_refutation),
        None => (std::vec::Vec::new(), false),
    };
    noble_contracts::companion::EvidenceOffer {
        class: if is_refutation {
            noble_contracts::companion::EvidenceClass::LeanRefutation
        } else {
            noble_contracts::companion::EvidenceClass::LeanExact
        },
        refutation: is_refutation,
        payload: noble_contracts::companion::EvidencePayload::Declaration(declaration),
    }
}

/// The toolchain identities a build report must retain when no proof ran.
pub(crate) const fn no_tools() -> crate::workflow::encoding::Json {
    crate::workflow::encoding::Json::Null
}

/// One contract's statement digest as a decimal string, for report fields.
pub(crate) fn digest_text(digest: u64) -> std::string::String {
    std::format!("{digest:016x}")
}

/// Encode the accepted toolchain identities for a report field.
pub(crate) fn tools_of(evidence: &Evidence) -> crate::workflow::encoding::Json {
    evidence.tools.clone()
}

/// Encode a refusal as a diagnostic entry.
pub(crate) fn refusal_json(refusal: &Refusal) -> crate::workflow::encoding::Json {
    crate::workflow::encoding::object([
        ("code", crate::workflow::encoding::string(refusal.code)),
        (
            "message",
            crate::workflow::encoding::string(&refusal.message),
        ),
        ("details", refusal.details.clone()),
    ])
}
