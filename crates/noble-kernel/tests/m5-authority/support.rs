pub struct Fixture {
    pub table: noble_kernel::authority::Authority,
    pub plan: noble_kernel::authority::Plan,
    pub facts: noble_kernel::authority::CurrentFacts,
}

pub fn fixture() -> Result<Fixture, String> {
    fixture_with_limits(noble_kernel::authority::Limits {
        witnesses: 8,
        attempts: 8,
        requests: 32,
    })
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; fixture construction allocates the bounded plan and authority table and reports typed construction failures as test errors; behavior is asserted by the consuming scenarios, not padded fixture checks."
)]
pub fn fixture_with_limits(limits: noble_kernel::authority::Limits) -> Result<Fixture, String> {
    let owner = noble_kernel::authority::OwnerContext {
        instance: 19,
        invocation: 23,
        generation: 7,
    };
    let checkpoint = noble_kernel::authority::Checkpoint {
        sequence: 3,
        now: 50,
    };
    let plan = match noble_kernel::authority::Plan::new(noble_kernel::authority::PlanDescription {
        operation: noble_kernel::authority::OperationContract {
            name: b"noble-test:authority/operations@1.0.0#send".to_vec(),
            effect: noble_kernel::types::EffId(91),
        },
        // Full ordered target+amount encoding, not a digest of it.
        arguments: b"target=account-a;amount=7".to_vec(),
        actor: noble_kernel::authority::ActorId(5),
        owner,
        policy: noble_kernel::authority::PolicyRevision(11),
        constraints: noble_kernel::authority::Constraints {
            not_before: 40,
            expires_at: 100,
            quota: noble_kernel::authority::QuotaId(17),
            units: 7,
            scope: 29,
        },
    }) {
        Ok(plan) => plan,
        Err(error) => return Err(format!("plan: {error:?}")),
    };
    let table = match noble_kernel::authority::Authority::new(noble_kernel::authority::Profile {
        owner,
        witness_kind: noble_kernel::authority::WITNESS_KIND,
        authority_source: noble_kernel::authority::SourceId(31),
        observation_source: noble_kernel::authority::SourceId(37),
        invocation_source: noble_kernel::authority::SourceId(41),
        checkpoint,
        limits,
    }) {
        Ok(table) => table,
        Err(error) => return Err(format!("table: {error:?}")),
    };
    let facts = facts_for(&plan, checkpoint);
    Ok(Fixture { table, plan, facts })
}

pub fn facts_for(
    plan: &noble_kernel::authority::Plan,
    checkpoint: noble_kernel::authority::Checkpoint,
) -> noble_kernel::authority::CurrentFacts {
    noble_kernel::authority::CurrentFacts {
        source: noble_kernel::authority::SourceId(31),
        checkpoint,
        grant: Some(noble_kernel::authority::Grant {
            plan: plan.clone(),
            kind: noble_kernel::authority::WITNESS_KIND,
            rights: noble_kernel::authority::EXECUTE_RIGHT,
        }),
        credentials_valid: Some(true),
        revoked: Some(false),
        policy: Some(noble_kernel::authority::PolicyRevision(11)),
        quota_remaining: Some(70),
    }
}

pub fn authorized(fixture: &mut Fixture) -> Result<noble_kernel::authority::Witness, String> {
    match fixture
        .table
        .authorize_from_trusted_host(fixture.plan.clone(), &fixture.facts)
    {
        noble_kernel::authority::Authorization::Authorized(witness) => Ok(witness),
        rejected @ (noble_kernel::authority::Authorization::Denied(_)
        | noble_kernel::authority::Authorization::Preflight(_)) => {
            Err(format!("authorization: {rejected:?}"))
        }
    }
}

pub fn admitted(
    fixture: &mut Fixture,
    witness: noble_kernel::authority::Witness,
) -> Result<noble_kernel::authority::Execution, String> {
    let request = noble_kernel::authority::AdmissionRequest {
        claim: witness.claim(),
        plan: fixture.plan.clone(),
    };
    match fixture.table.admit(Some(witness), &request, &fixture.facts) {
        noble_kernel::authority::Admission::Committed(execution) => Ok(execution),
        rejected @ (noble_kernel::authority::Admission::Denied { .. }
        | noble_kernel::authority::Admission::Preflight { .. }) => {
            Err(format!("admission: {rejected:?}"))
        }
    }
}

pub fn started(fixture: &mut Fixture) -> Result<noble_kernel::authority::Started, String> {
    let witness = authorized(fixture)?;
    let execution = admitted(fixture, witness)?;
    match fixture.table.start_execution(execution) {
        Ok(started) => Ok(started),
        Err(error) => Err(format!("execution: {error:?}")),
    }
}

pub fn description(
    fixture: &Fixture,
    attempt: noble_kernel::authority::AttemptKey,
    outcome: noble_kernel::authority::ObservedOutcome,
) -> noble_kernel::authority::ObservationDescription {
    noble_kernel::authority::ObservationDescription {
        attempt,
        plan: fixture.plan.clone(),
        source: noble_kernel::authority::SourceId(37),
        checkpoint: fixture.facts.checkpoint,
        outcome,
    }
}

pub fn observe(
    fixture: &mut Fixture,
    attempt: noble_kernel::authority::AttemptKey,
    outcome: noble_kernel::authority::ObservedOutcome,
) -> Result<noble_kernel::authority::Observation, String> {
    let description = description(fixture, attempt, outcome);
    match fixture.table.observe_from_trusted_host(description) {
        Ok(observation) => Ok(observation),
        Err(error) => Err(format!("observation: {error:?}")),
    }
}
