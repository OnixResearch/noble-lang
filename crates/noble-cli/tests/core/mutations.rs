fn add_witness(
    submission: &mut noble_kernel::execution::Submission,
) -> Result<&mut noble_kernel::words::Inst, String> {
    for node in &mut submission.body.candidate.nodes {
        if let noble_kernel::untrusted::Node::Invocation {
            def: noble_kernel::contracts::Definition(4),
            inst,
        } = node
        {
            return Ok(inst);
        }
    }
    Err("real-source arithmetic candidate has no add node".into())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; rejects invokes the real allocating source compiler and formats a mutation-specific error for every outcome other than Invalid. Compilation and diagnostic formatting are not const."
)]
fn rejects(submission: &noble_kernel::execution::Submission, mutation: &str) -> Result<(), String> {
    match noble_wasm::source::Compiler::new().prepare(submission) {
        Err(noble_wasm::Diagnostic::Invalid) => Ok(()),
        Err(_) => Err(format!("{mutation}: not rejected as invalid")),
        Ok(_) => Err(format!("{mutation}: hostile candidate was compiled")),
    }
}

#[test]
fn core_16_candidate_mutation_matrix() -> Result<(), String> {
    arithmetic()?;
    text()?;
    definitions()?;
    println!("{{\"id\":\"CORE-16\",\"stage\":\"acceptance\",\"outcome\":\"reject\",\"variants\":12,\"guest_requests\":0,\"protected_operations\":0}}");
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; arithmetic requires the unmodified source candidate to compile and rejects checks Invalid for each forged witness, cycle, node reference and contract. These Result-based behavior checks must not be padded with fixture assertions."
)]
fn arithmetic() -> Result<(), String> {
    let valid = super::candidate(b"1 2 +")?;
    if noble_wasm::source::Compiler::new().prepare(&valid).is_err() {
        return Err("unmodified real-source candidate must compile".into());
    }
    let mut wrong_type = valid.clone();
    add_witness(&mut wrong_type)?.bindings[0] =
        noble_kernel::words::Binding::Stack(vec![noble_kernel::types::Ty::Text]);
    rejects(&wrong_type, "replace-add-input-witness-with-Text")?;
    let mut cyclic = valid.clone();
    add_witness(&mut cyclic)?.bindings[0] =
        noble_kernel::words::Binding::Ref(noble_kernel::words::Variable(0));
    rejects(&cyclic, "cyclic-stack-substitution")?;
    let mut unknown_node = valid.clone();
    unknown_node
        .body
        .candidate
        .body
        .push(noble_kernel::untrusted::NodeId(u32::MAX));
    rejects(&unknown_node, "unknown-node-reference")?;
    let mut forged_contract = valid;
    forged_contract.environment.defs[4].stack_out =
        forged_contract.environment.defs[4].stack_in.clone();
    rejects(&forged_contract, "forged-definition-contract")
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; text uses rejects to require Invalid for missing, duplicate, misaddressed and non-UTF8 payloads. Each helper failure propagates to the matrix test; assertions about the mutations themselves would not strengthen that contract."
)]
fn text() -> Result<(), String> {
    let valid = super::candidate(b"\"payload\" test.emit")?;
    let payload = valid
        .body
        .texts
        .first()
        .ok_or("missing source text payload")?
        .clone();
    let mut missing = valid.clone();
    missing.body.texts.clear();
    rejects(&missing, "missing-text-payload")?;
    let mut duplicate = valid.clone();
    duplicate.body.texts.push(payload);
    rejects(&duplicate, "duplicate-text-payload")?;
    let mut misaddressed = valid.clone();
    misaddressed.body.texts[0].node = noble_kernel::untrusted::NodeId(u32::MAX);
    rejects(&misaddressed, "unknown-text-payload-node")?;
    let mut invalid_utf8 = valid;
    invalid_utf8.body.texts[0].bytes = vec![0xff];
    rejects(&invalid_utf8, "invalid-utf8-text-payload")
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; definitions requires Invalid through rejects for forged implementations, interfaces and semantic identities, then checks a committed identity collision. Fixture setup errors and unexpected compiler outcomes fail the matrix through Result."
)]
fn definitions() -> Result<(), String> {
    let mut source = noble_contracts::source::Session::new();
    for declaration in [
        b"def one [ 1 ]".as_slice(),
        b"def two [ 2 ]",
        b"def copy [ dup ]",
    ] {
        let prepared = source
            .prepare(declaration, &[], super::source_limits())
            .map_err(|error| error.diagnostic().message.clone())?;
        source
            .commit(prepared)
            .map_err(|error| error.diagnostic().message.clone())?;
    }
    let copy = source
        .prepare(b"1 copy", &[], super::source_limits())
        .map_err(|error| error.diagnostic().message.clone())?;
    let mut wrong_body = copy
        .submission()
        .ok_or("missing definition candidate")?
        .clone();
    wrong_body.definitions[0].body.candidate.body.clear();
    rejects(&wrong_body, "forged-named-implementation")?;
    wrong_body.definitions[0].expected.stack_out =
        wrong_body.definitions[0].expected.stack_in.clone();
    rejects(&wrong_body, "forged-named-body-interface")?;

    let pair = source
        .prepare(b"one two", &[], super::source_limits())
        .map_err(|error| error.diagnostic().message.clone())?;
    let mut collision = pair.submission().ok_or("missing alias candidate")?.clone();
    collision.definitions[1].identity = collision.definitions[0].identity;
    rejects(&collision, "conflicting-semantic-identity")?;
    committed_identity(&source)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; committed_identity requires a valid body to prepare and commit, then accepts only Invalid for a later forged identity. The exhaustive outcome match is the observable stateful rejection check; additional fixture assertions would be padding."
)]
fn committed_identity(source: &noble_contracts::source::Session) -> Result<(), String> {
    let first = source
        .prepare(b"one", &[], super::source_limits())
        .map_err(|error| error.diagnostic().message.clone())?;
    let first_body = first
        .submission()
        .ok_or("missing first identity candidate")?;
    let identity = first_body.definitions[0].identity;
    let mut compiler = noble_wasm::source::Compiler::new();
    let installed = compiler
        .prepare(first_body)
        .map_err(|_| "valid identity candidate did not compile")?;
    compiler
        .commit(installed)
        .map_err(|_| "valid identity candidate did not commit")?;
    let second = source
        .prepare(
            b"two",
            &[noble_kernel::types::Ty::I64],
            super::source_limits(),
        )
        .map_err(|error| error.diagnostic().message.clone())?;
    let mut later_collision = second
        .submission()
        .ok_or("missing second identity candidate")?
        .clone();
    later_collision.definitions[0].identity = identity;
    match compiler.prepare(&later_collision) {
        Err(noble_wasm::Diagnostic::Invalid) => Ok(()),
        Ok(_)
        | Err(
            noble_wasm::Diagnostic::Unsupported
            | noble_wasm::Diagnostic::Exhausted
            | noble_wasm::Diagnostic::Defective,
        ) => Err("committed semantic identity collision was not rejected as invalid".into()),
    }
}
