mod protected;
mod resources;

use anyhow::{bail, Context, Result};
use noble_kernel::{authority as auth, resources as owner};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use wasmtime::{
    component::{types::ComponentItem, Component, Linker, Val},
    Engine, Store, StoreContextMut, StoreLimits,
};

pub struct Counter;
pub struct Authorization;

pub struct Host {
    pub limits: StoreLimits,
    owners: owner::Table,
    counters: BTreeMap<u32, (owner::Owner, i64)>,
    authority: auth::Authority,
    witnesses: BTreeMap<u32, auth::Witness>,
    next_rep: u32,
    requests: Vec<String>,
    releases: usize,
    native_releases: usize,
    returned: usize,
    receipts: Vec<Value>,
    pub allow_protected: bool,
    pub counter_error: bool,
    total: i64,
}

fn checked<T, E: std::fmt::Debug>(value: Result<T, E>) -> Result<T> {
    value.map_err(|error| anyhow::anyhow!("{error:?}"))
}

impl Host {
    pub fn new(limits: StoreLimits) -> Result<Self> {
        Ok(Self {
            limits,
            owners: checked(owner::Table::new(
                owner::TableId(1),
                owner::Limits {
                    slots: 64,
                    pins: 64,
                    owners_per_context: 64,
                    generations: 1_000_000,
                    scopes: 1_000_000,
                },
            ))?,
            counters: BTreeMap::new(),
            authority: protected::authority()?,
            witnesses: BTreeMap::new(),
            next_rep: 0,
            requests: Vec::new(),
            releases: 0,
            native_releases: 0,
            returned: 0,
            receipts: Vec::new(),
            allow_protected: true,
            counter_error: false,
            total: 0,
        })
    }

    fn rep(&mut self) -> Result<u32> {
        self.next_rep = self
            .next_rep
            .checked_add(1)
            .context("peer representation budget")?;
        Ok(self.next_rep)
    }

    pub fn report(&self) -> Value {
        let state = self.owners.observation();
        let counts = self.authority.counters();
        json!({"requests":self.requests, "guest_requests":self.requests.len(),
            "protected_operations":counts.protected_operations,
            "witnesses_created":counts.witnesses_created,
            "witness_consumptions":counts.witness_consumptions,
            "attempts_admitted":counts.attempts_admitted,
            "receipts":self.receipts,
            "resources":{"live":state.live,"busy":state.busy,"retiring":state.retiring,
                "retired":state.retired,"native_pins":state.native_pins,"local_releases":self.releases,
                "native_releases":self.native_releases,"returned_owners":self.returned},
            "invocation":format!("{:?}",self.authority.invocation_outcome()),
            "protected_total":self.total})
    }

    pub fn cleanup(&mut self, failed: bool) -> Result<()> {
        for (_, witness) in std::mem::take(&mut self.witnesses) {
            checked(self.authority.retire_witness(witness))?;
        }
        if failed {
            checked(self.authority.fail_invocation())?;
        }
        for context in [owner::Context(1), owner::Context(2)] {
            let decisions = checked(
                self.owners
                    .retire_context(context, owner::Retirement::HostCleanup),
            )?;
            for decision in decisions {
                self.releases += decision.accounting().local_releases;
            }
        }
        self.native_releases += self.counters.len();
        self.counters.clear();
        Ok(())
    }
}

pub fn decode_input(store: &mut Store<Host>, value: &Value) -> Result<Val> {
    if value["type"] == "host-counter" {
        let number = value["value"]
            .as_str()
            .context("counter value must be decimal text")?
            .parse()?;
        resources::inject(store, number)
    } else {
        crate::values::decode(value)
    }
}

pub fn encode_output(store: &mut Store<Host>, value: &Val) -> Result<Value> {
    match value {
        Val::Resource(handle) => {
            resources::finish_returned(store, *handle)?;
            Ok(json!({"type":"own<counter>","disposition":"released-by-peer"}))
        }
        _ => crate::values::encode(value),
    }
}

pub fn link(engine: &Engine, component: &Component, linker: &mut Linker<Host>) -> Result<()> {
    for (name, item) in component.component_type().imports(engine) {
        let interface = match item {
            ComponentItem::ComponentInstance(interface) => interface,
            ComponentItem::Resource(_) => {
                resources::define(&mut linker.root(), name)?;
                continue;
            }
            ComponentItem::Type(_) => continue,
            _ => bail!("unapproved root import {name}: {item:?}"),
        };
        anyhow::ensure!(
            matches!(
                name,
                "noble-test:sync/arithmetic@1.0.0"
                    | "noble-test:math/arithmetic@1.0.0"
                    | "noble-test:sync/echo@1.0.0"
                    | "noble-test:sync/counters@1.0.0"
                    | "noble-test:counter/counters@1.0.0"
                    | "noble-test:sync/authorization@1.0.0"
                    | "noble-test:store/api@1.0.0"
            ),
            "unapproved import interface {name}"
        );
        let mut instance = linker.instance(name)?;
        for (export, kind) in interface.exports(engine) {
            match kind {
                ComponentItem::Resource(_) => resources::define(&mut instance, export)?,
                ComponentItem::Type(_) => {}
                ComponentItem::ComponentFunc(_) => {
                    let operation = format!("{name}#{export}");
                    let interface_name = name.to_owned();
                    let function = export.to_owned();
                    instance.func_new(export, move |store, _ty, params, results| {
                        call(
                            store,
                            &interface_name,
                            &function,
                            &operation,
                            params,
                            results,
                        )
                    })?;
                }
                _ => bail!("unsupported imported component item {name}#{export}"),
            }
        }
    }
    Ok(())
}

fn call(
    mut store: StoreContextMut<'_, Host>,
    interface: &str,
    name: &str,
    operation: &str,
    params: &[Val],
    results: &mut [Val],
) -> Result<()> {
    store.data_mut().requests.push(operation.to_owned());
    let result = if interface == "noble-test:store/api@1.0.0" && name == "read" {
        store.data_mut().receipts.push(json!({"operation":operation,
            "admission":"denied","reason":"host-grant-absent","protected_operations":0}));
        bail!("authorization denied: host grant absent for {operation}");
    } else if interface.contains("/arithmetic@") && name == "inc" {
        let [Val::S64(value)] = params else {
            bail!("invalid arithmetic input")
        };
        Val::S64(value.wrapping_add(1))
    } else if interface == "noble-test:sync/echo@1.0.0" && matches!(name, "text" | "bytes") {
        let [value] = params else {
            bail!("invalid echo input")
        };
        match (name, value) {
            ("text", Val::String(_)) | ("bytes", Val::List(_)) => value.clone(),
            _ => bail!("invalid echo type"),
        }
    } else if interface.contains("/counters@") {
        resources::call(store, name, params)?
    } else if interface == "noble-test:sync/authorization@1.0.0" {
        protected::call(store, name, params)?
    } else {
        bail!("unapproved function {operation}")
    };
    anyhow::ensure!(results.len() == 1, "wrong host result count");
    results[0] = result;
    Ok(())
}
