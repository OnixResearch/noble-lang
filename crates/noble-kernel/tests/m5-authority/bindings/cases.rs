#[derive(Clone, Copy, Debug)]
#[tigerstyle::sealed_enum]
pub(super) enum Mutation {
    Operation,
    Effect,
    Target,
    Amount,
    Actor,
    Owner,
    Policy,
    Expired,
    Revoked,
    Quota,
    MissingGrant,
    MissingCredential,
    MissingRevocation,
    MissingPolicy,
    MissingQuota,
    Kind,
    Generation,
    Rights,
    Scope,
    Source,
    StaleFacts,
}

pub(super) const ALL: [Mutation; 21] = [
    Mutation::Operation,
    Mutation::Effect,
    Mutation::Target,
    Mutation::Amount,
    Mutation::Actor,
    Mutation::Owner,
    Mutation::Policy,
    Mutation::Expired,
    Mutation::Revoked,
    Mutation::Quota,
    Mutation::MissingGrant,
    Mutation::MissingCredential,
    Mutation::MissingRevocation,
    Mutation::MissingPolicy,
    Mutation::MissingQuota,
    Mutation::Kind,
    Mutation::Generation,
    Mutation::Rights,
    Mutation::Scope,
    Mutation::Source,
    Mutation::StaleFacts,
];

pub(super) fn apply(
    case: Mutation,
    fixture: &mut crate::support::Fixture,
    request: &mut noble_kernel::authority::AdmissionRequest,
) -> Result<noble_kernel::authority::Denial, String> {
    match case {
        Mutation::Operation
        | Mutation::Effect
        | Mutation::Target
        | Mutation::Amount
        | Mutation::Actor => mutate_plan(case, request),
        Mutation::Owner => {
            request.claim.owner.invocation += 1;
            Ok(noble_kernel::authority::Denial::WrongOwnerContext)
        }
        Mutation::Expired => {
            let checkpoint = noble_kernel::authority::Checkpoint {
                sequence: 4,
                now: 100,
            };
            match fixture.table.advance_from_trusted_host(checkpoint) {
                Ok(()) => fixture.facts.checkpoint = checkpoint,
                Err(error) => return Err(format!("checkpoint: {error:?}")),
            }
            Ok(noble_kernel::authority::Denial::Expired)
        }
        Mutation::Kind => {
            request.claim.kind = noble_kernel::types::ResourceKind(99);
            Ok(noble_kernel::authority::Denial::WrongResourceKind)
        }
        Mutation::Generation => {
            request.claim.generation += 1;
            Ok(noble_kernel::authority::Denial::StaleGeneration)
        }
        Mutation::Rights => {
            request.claim.rights = 0;
            Ok(noble_kernel::authority::Denial::MissingRights)
        }
        Mutation::Scope => {
            request.claim.scope += 1;
            Ok(noble_kernel::authority::Denial::WrongScope)
        }
        facts @ (Mutation::Policy
        | Mutation::Revoked
        | Mutation::Quota
        | Mutation::MissingGrant
        | Mutation::MissingCredential
        | Mutation::MissingRevocation
        | Mutation::MissingPolicy
        | Mutation::MissingQuota
        | Mutation::Source
        | Mutation::StaleFacts) => mutate_facts(facts, fixture),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; mutate_facts drops the grant's owned Plan and allocates String diagnostics for non-fact cases; those runtime ownership operations cannot be const."
)]
fn mutate_facts(
    case: Mutation,
    fixture: &mut crate::support::Fixture,
) -> Result<noble_kernel::authority::Denial, String> {
    match case {
        Mutation::Policy => {
            fixture.facts.policy = Some(noble_kernel::authority::PolicyRevision(12));
            Ok(noble_kernel::authority::Denial::ChangedPolicy)
        }
        Mutation::Revoked => {
            fixture.facts.revoked = Some(true);
            Ok(noble_kernel::authority::Denial::Revoked)
        }
        Mutation::Quota => {
            fixture.facts.quota_remaining = Some(6);
            Ok(noble_kernel::authority::Denial::QuotaExhausted)
        }
        Mutation::MissingGrant => {
            fixture.facts.grant = None;
            Ok(noble_kernel::authority::Denial::MissingFacts)
        }
        Mutation::MissingCredential => {
            fixture.facts.credentials_valid = None;
            Ok(noble_kernel::authority::Denial::MissingFacts)
        }
        Mutation::MissingRevocation => {
            fixture.facts.revoked = None;
            Ok(noble_kernel::authority::Denial::MissingFacts)
        }
        Mutation::MissingPolicy => {
            fixture.facts.policy = None;
            Ok(noble_kernel::authority::Denial::MissingFacts)
        }
        Mutation::MissingQuota => {
            fixture.facts.quota_remaining = None;
            Ok(noble_kernel::authority::Denial::MissingFacts)
        }
        Mutation::Source => {
            fixture.facts.source = noble_kernel::authority::SourceId(99);
            Ok(noble_kernel::authority::Denial::WrongAuthoritySource)
        }
        Mutation::StaleFacts => {
            fixture.facts.checkpoint.sequence -= 1;
            Ok(noble_kernel::authority::Denial::StaleFacts)
        }
        Mutation::Operation
        | Mutation::Effect
        | Mutation::Target
        | Mutation::Amount
        | Mutation::Actor
        | Mutation::Owner
        | Mutation::Expired
        | Mutation::Kind
        | Mutation::Generation
        | Mutation::Rights
        | Mutation::Scope => Err("not a fact mutation".to_string()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; mutate_plan constructs one deliberately mismatched owned plan; rejects_mutation checks ChangedPlan, witness retention and absence of protected execution."
)]
fn mutate_plan(
    case: Mutation,
    request: &mut noble_kernel::authority::AdmissionRequest,
) -> Result<noble_kernel::authority::Denial, String> {
    let mut description = request.plan.description().clone();
    match case {
        Mutation::Operation => {
            description.operation.name = b"noble-test:authority/operations@2.0.0#send".to_vec()
        }
        Mutation::Effect => description.operation.effect = noble_kernel::types::EffId(92),
        Mutation::Target => description.arguments = b"target=account-b;amount=7".to_vec(),
        Mutation::Amount => description.arguments = b"target=account-a;amount=8".to_vec(),
        Mutation::Actor => description.actor = noble_kernel::authority::ActorId(6),
        Mutation::Owner
        | Mutation::Policy
        | Mutation::Expired
        | Mutation::Revoked
        | Mutation::Quota
        | Mutation::MissingGrant
        | Mutation::MissingCredential
        | Mutation::MissingRevocation
        | Mutation::MissingPolicy
        | Mutation::MissingQuota
        | Mutation::Kind
        | Mutation::Generation
        | Mutation::Rights
        | Mutation::Scope
        | Mutation::Source
        | Mutation::StaleFacts => return Err("not a plan mutation".to_string()),
    }
    request.plan = match noble_kernel::authority::Plan::new(description) {
        Ok(plan) => plan,
        Err(error) => return Err(format!("mutated plan: {error:?}")),
    };
    Ok(noble_kernel::authority::Denial::ChangedPlan)
}
