pub(super) fn exercise(state: &mut super::Harness) {
    subject(state);
    definition(state);
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; subject changes one operand and one finite interface while retaining actual checked evidence; control asserts MismatchedSubject for each real bind result."
)]
fn subject(state: &mut super::Harness) {
    super::control(
        &mut state.controls,
        ("CONTRACT-02", "program-operand"),
        crate::workflow::encoding::object([
            ("original_ops", super::events_json(&state.observed.events)),
            ("changed_ops", super::events_json(&[(1, 2), (2, 4)])),
        ]),
        state
            .core
            .bind(state.evidence, &super::increment_subject(2))
            .map(|_| ()),
        noble_contracts::companion::Refusal::MismatchedSubject,
        &state.baseline,
    );
    let extended = noble_contracts::companion::interface_signature(&[
        noble_kernel::types::Ty::I64,
        noble_kernel::types::Ty::I64,
    ]);
    let wrong_interface = noble_contracts::companion::observe(
        &state.observed.events,
        noble_contracts::companion::InterfaceSignatures {
            input: extended,
            output: extended,
        },
    );
    super::control(
        &mut state.controls,
        ("CONTRACT-02", "instantiated-interface"),
        crate::workflow::encoding::object([
            (
                "original_input",
                crate::workflow::encoding::strings(&["I64"]),
            ),
            (
                "changed_input",
                crate::workflow::encoding::strings(&["I64", "I64"]),
            ),
            (
                "changed_output",
                crate::workflow::encoding::strings(&["I64", "I64"]),
            ),
        ]),
        state
            .core
            .bind(state.evidence, &wrong_interface)
            .map(|_| ()),
        noble_contracts::companion::Refusal::MismatchedSubject,
        &state.baseline,
    );
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; definition independently checks the fixed declaration through checked, binds its retained program, and delegates the changed-definition MismatchedClaim assertion to control."
)]
fn definition(state: &mut super::Harness) {
    let defined = b"(contract 1 defined (input (x I64)) (output (y I64)) (define delta I64 1) (program [ 1 + ]) (requires true) (ensures (eq (out y) (add (in x) (def delta)))))";
    let proof = std::str::from_utf8(super::PROOF)
        .expect("proof text")
        .replace(
            "(.add (.input 0) (.i64 1))",
            "(.add (.input 0) (.definition 0 (.i64 1)))",
        );
    let (contract, admission, baseline) = super::checker::checked(
        &mut state.core,
        &mut state.checker,
        defined,
        proof.as_bytes(),
    );
    let subject = super::retained_subject(&state.core, &admission, &contract);
    let bound = state
        .core
        .bind(admission.evidence.expect("defined proof"), &subject)
        .expect("defined baseline applies");
    let changed_definition = std::str::from_utf8(defined)
        .expect("text")
        .replace("delta I64 1", "delta I64 2");
    let changed_digest = noble_contracts::companion::statement_digest(&super::prepared(
        changed_definition.as_bytes(),
    ));
    let mut changed = super::derivation(
        noble_contracts::companion::RuleId::ProjectV1,
        &admission,
        &bound,
        vec![admission.evidence.expect("evidence")],
    );
    changed.statement = changed_digest;
    super::control(
        &mut state.controls,
        ("CONTRACT-02", "logical-definition"),
        crate::workflow::encoding::object([
            (
                "original_source",
                crate::workflow::encoding::string(std::str::from_utf8(defined).expect("source")),
            ),
            (
                "changed_source",
                crate::workflow::encoding::string(changed_definition),
            ),
            (
                "offered_statement",
                crate::workflow::encoding::string(changed_digest.to_string()),
            ),
        ]),
        state.core.replay(&changed).map(|_| ()),
        noble_contracts::companion::Refusal::MismatchedClaim,
        &baseline,
    );
}
