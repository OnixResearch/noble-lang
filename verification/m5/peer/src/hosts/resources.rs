use super::{checked, Authorization, Counter, Host};
use anyhow::{bail, Context, Result};
use noble_kernel::{resources as owner, types::ResourceKind};
use wasmtime::{
    component::{LinkerInstance, Resource, ResourceAny, ResourceType, Val},
    AsContextMut, Store, StoreContextMut,
};

fn required(context: u64) -> owner::Requirement {
    owner::Requirement {
        context: owner::Context(context),
        kind: ResourceKind(1),
        rights: owner::Rights(1),
    }
}

pub fn define(instance: &mut LinkerInstance<'_, Host>, name: &str) -> Result<()> {
    match name {
        "counter" => instance.resource(name, ResourceType::host::<Counter>(), |mut store, rep| {
            release(store.data_mut(), rep)
        }),
        "authorization" => instance.resource(
            name,
            ResourceType::host::<Authorization>(),
            |mut store, rep| {
                let witness = store
                    .data_mut()
                    .witnesses
                    .remove(&rep)
                    .context("unknown witness destructor")?;
                checked(store.data_mut().authority.retire_witness(witness))
            },
        ),
        _ => bail!("unapproved peer resource kind {name}"),
    }
}

pub fn inject(store: &mut Store<Host>, value: i64) -> Result<Val> {
    let owner = checked(store.data_mut().owners.register(required(1)))?;
    let rep = store.data_mut().rep()?;
    store.data_mut().counters.insert(rep, (owner, value));
    Ok(Val::Resource(ResourceAny::try_from_resource(
        Resource::<Counter>::new_own(rep),
        store,
    )?))
}

pub fn release(host: &mut Host, rep: u32) -> Result<()> {
    let (owner, _) = host
        .counters
        .remove(&rep)
        .context("unknown counter destructor")?;
    let decision = checked(host.owners.release(owner, required(1)))?;
    host.releases += decision.accounting().local_releases;
    host.native_releases += 1;
    Ok(())
}

pub fn finish_returned(store: &mut Store<Host>, handle: ResourceAny) -> Result<()> {
    let returned = handle.try_into_resource::<Counter>(&mut *store)?;
    anyhow::ensure!(returned.owned(), "borrow escaped the component export");
    release(store.data_mut(), returned.rep())
}

pub fn call(mut store: StoreContextMut<'_, Host>, name: &str, params: &[Val]) -> Result<Val> {
    let [Val::Resource(resource)] = params else {
        bail!("invalid counter parameter")
    };
    let counter = resource.try_into_resource::<Counter>(store.as_context_mut())?;
    match name {
        "[method]counter.read" => {
            anyhow::ensure!(
                !counter.owned(),
                "receiver must be borrowed only inside adapter"
            );
            read(store.data_mut(), counter.rep())
        }
        "transfer" => {
            anyhow::ensure!(counter.owned(), "transfer requires an owning resource");
            transfer(store.data_mut(), counter.rep())
        }
        _ => bail!("unsupported counter operation {name}"),
    }
}

fn read(host: &mut Host, rep: u32) -> Result<Val> {
    let (owner, value) = host.counters.remove(&rep).context("unknown counter")?;
    let admitted = match host.owners.begin(owner, required(1)) {
        Ok(admitted) => admitted,
        Err(rejected) => {
            host.counters.insert(rep, (rejected.input, value));
            bail!("counter admission rejected: {:?}", rejected.error);
        }
    };
    checked(host.owners.native_access(&admitted.borrow))?;
    let completion = if host.counter_error {
        owner::Completion::DomainError
    } else {
        owner::Completion::Success
    };
    let completed = checked(host.owners.complete(admitted.borrow.scope(), completion))?;
    host.returned += completed.decision.accounting().owners_returned;
    host.counters.insert(
        rep,
        (
            completed.owner.context("normal completion omitted owner")?,
            value,
        ),
    );
    Ok(if host.counter_error {
        Val::Result(Err(Some(Box::new(Val::String("read-failed".into())))))
    } else {
        Val::Result(Ok(Some(Box::new(Val::S64(value)))))
    })
}

fn transfer(host: &mut Host, rep: u32) -> Result<Val> {
    let (owner, _) = host.counters.remove(&rep).context("unknown counter")?;
    let mut transferred = checked(host.owners.transfer(
        vec![owner],
        &[required(1)],
        owner::Context(2),
    ))?;
    let received = transferred.owners.pop().context("missing transfer owner")?;
    // This host owns cleanup after commit, including a domain error. No sender
    // owner is reconstructed from the error or from Canonical ABI handle bytes.
    let decision = checked(host.owners.release(received, required(2)))?;
    host.releases += decision.accounting().local_releases;
    host.native_releases += 1;
    Ok(Val::Result(Err(Some(Box::new(Val::String(
        "receiver-error".into(),
    ))))))
}
