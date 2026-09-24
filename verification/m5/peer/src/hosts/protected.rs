use super::{checked, Authorization, Host};
use anyhow::{bail, Context, Result};
use noble_kernel::{authority as auth, types::EffId};
use serde_json::json;
use wasmtime::{
    component::{Resource, ResourceAny, Val},
    AsContextMut, StoreContextMut,
};

const OWNER: auth::OwnerContext = auth::OwnerContext {
    instance: 1,
    invocation: 1,
    generation: 1,
};
const CHECKPOINT: auth::Checkpoint = auth::Checkpoint {
    sequence: 1,
    now: 10,
};
const SOURCE: auth::SourceId = auth::SourceId(1);
const OBSERVER: auth::SourceId = auth::SourceId(2);
const POLICY: auth::PolicyRevision = auth::PolicyRevision(1);

pub fn authority() -> Result<auth::Authority> {
    checked(auth::Authority::new(auth::Profile {
        owner: OWNER,
        witness_kind: auth::WITNESS_KIND,
        authority_source: SOURCE,
        observation_source: OBSERVER,
        invocation_source: auth::SourceId(3),
        checkpoint: CHECKPOINT,
        limits: auth::Limits {
            witnesses: 64,
            attempts: 64,
            requests: 256,
        },
    }))
}

fn plan(argument: i64) -> Result<auth::Plan> {
    checked(auth::Plan::new(auth::PlanDescription {
        operation: auth::OperationContract {
            name: b"noble-test:sync/authorization@1.0.0#protected".to_vec(),
            effect: EffId(8),
        },
        arguments: argument.to_le_bytes().to_vec(),
        actor: auth::ActorId(1),
        owner: OWNER,
        policy: POLICY,
        constraints: auth::Constraints {
            not_before: 1,
            expires_at: 20,
            quota: auth::QuotaId(1),
            units: 1,
            scope: 1,
        },
    }))
}

fn facts(plan: &auth::Plan, host: &Host) -> auth::CurrentFacts {
    // Independent local policy configuration, never a guest-supplied allow bit.
    // This fixture's authority scope is one synchronous, serialized invocation;
    // it grants no remote permissions and assumes no remote revocation service.
    auth::CurrentFacts {
        source: SOURCE,
        checkpoint: CHECKPOINT,
        grant: Some(auth::Grant {
            plan: plan.clone(),
            kind: auth::WITNESS_KIND,
            rights: auth::EXECUTE_RIGHT,
        }),
        credentials_valid: Some(host.allow_protected),
        revoked: Some(false),
        policy: Some(POLICY),
        quota_remaining: Some(64),
    }
}

pub fn call(mut store: StoreContextMut<'_, Host>, name: &str, params: &[Val]) -> Result<Val> {
    match (name, params) {
        ("prepare", [Val::S64(argument)]) => {
            let plan = plan(*argument)?;
            let facts = facts(&plan, store.data());
            let witness = match store
                .data_mut()
                .authority
                .authorize_from_trusted_host(plan, &facts)
            {
                auth::Authorization::Authorized(witness) => witness,
                auth::Authorization::Denied(rejection)
                | auth::Authorization::Preflight(rejection) => {
                    retain_receipt(store.data_mut(), rejection.receipt());
                    bail!("authorization refused");
                }
            };
            let rep = store.data_mut().rep()?;
            store.data_mut().witnesses.insert(rep, witness);
            Ok(Val::Resource(ResourceAny::try_from_resource(
                Resource::<Authorization>::new_own(rep),
                store,
            )?))
        }
        ("protected", [Val::Resource(handle), Val::S64(argument)]) => {
            let witness = handle.try_into_resource::<Authorization>(store.as_context_mut())?;
            anyhow::ensure!(
                witness.owned(),
                "protected call requires owned one-shot witness"
            );
            execute(store.data_mut(), witness.rep(), *argument)
        }
        _ => bail!("unsupported protected boundary call {name}"),
    }
}

fn execute(host: &mut Host, rep: u32, argument: i64) -> Result<Val> {
    let witness = host
        .witnesses
        .remove(&rep)
        .context("missing live one-shot witness")?;
    let plan = plan(argument)?;
    let facts = facts(&plan, host);
    let request = auth::AdmissionRequest {
        claim: witness.claim(),
        plan,
    };
    let execution = match host.authority.admit(Some(witness), &request, &facts) {
        auth::Admission::Committed(execution) => execution,
        auth::Admission::Denied { witness, rejection }
        | auth::Admission::Preflight { witness, rejection } => {
            if let Some(witness) = witness {
                host.witnesses.insert(rep, witness);
            }
            retain_receipt(host, rejection.receipt());
            bail!("protected admission refused");
        }
    };
    // No protected action precedes this committed, recorded attempt.
    let started = checked(host.authority.start_execution(execution))?;
    let total = host.total.checked_add(argument);
    let outcome = if let Some(total) = total {
        host.total = total;
        auth::ObservedOutcome::OperationSuccess
    } else {
        auth::ObservedOutcome::OperationFailure
    };
    let observation = checked(host.authority.observe_from_trusted_host(
        auth::ObservationDescription {
            attempt: started.attempt(),
            plan: started.plan().clone(),
            source: OBSERVER,
            checkpoint: CHECKPOINT,
            outcome,
        },
    ))?;
    let claim = if total.is_some() {
        auth::ReceiptClaim::OperationSuccess
    } else {
        auth::ReceiptClaim::OperationFailure
    };
    let receipt = checked(
        host.authority
            .receipt(started.attempt(), claim, Some(&observation)),
    )?;
    retain_receipt(host, &receipt);
    match total {
        Some(value) => Ok(Val::S64(value)),
        None => bail!("protected local counter overflow"),
    }
}

fn retain_receipt(host: &mut Host, receipt: &auth::Receipt) {
    let description = receipt.description();
    host.receipts.push(json!({"claim":format!("{:?}",description.claim),
        "scope":format!("{:?}",description.scope),"invocation":format!("{:?}",description.invocation),
        "attempt":description.attempt.map(|attempt| attempt.slot),"source":description.source.0,
        "operation":String::from_utf8_lossy(&description.plan.description().operation.name),
        "arguments":description.plan.description().arguments}));
}
