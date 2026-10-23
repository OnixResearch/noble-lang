//! Host-typed execution of one Worker Program v2 transition.
//!
//! WIT transports canonical `noble-worker-value/v1` frames as strings and an
//! opaque move-only owner. Only this host knows the selected WorkerData@1
//! catalog. It validates both frames, constructs runtime cells, invokes the
//! already bound installed Program, observes exactly `State, List<Action>` and
//! validates those values again before publishing a canonical output frame.

use anyhow::{ensure, Context, Result};
use noble_kernel::types::{NominalShape, Ty};
use serde_json::{json, Value};
use wasmtime::{Instance, Memory, StoreContextMut, TypedFunc};

use super::worker_program::SelectedSchema;
use super::worker_program_guest::{Host, ProgramCell};

const TEXT_START: usize = 983_040;
const MEMORY_END: usize = 1_048_576;
const NODE_LIMIT: usize = 4_096;
const DEPTH_LIMIT: usize = 32;
const OBSERVATION_BYTES: usize = 1_048_576;
const DEADLINE_MS: u64 = 60_000;

#[derive(Clone)]
pub(super) struct TransitionOutput {
    pub(super) encoded: String,
    pub(super) state: Value,
    pub(super) actions: Value,
}

pub(super) struct TransitionState {
    pub(super) schema: SelectedSchema,
    pub(super) output: Option<TransitionOutput>,
    pub(super) outcome: Option<&'static str>,
    pub(super) quota_reason: Option<i64>,
    pub(super) interruption: Interruption,
}

pub(super) struct Interruption {
    pub(super) checks: u32,
    pub(super) last: u32,
    pub(super) max: u32,
    deadline: std::time::Instant,
    cancel_at_check: Option<u32>,
    trap_at_check: Option<u32>,
}

#[derive(Clone, Copy)]
pub(super) struct TransitionControl {
    pub(super) cancel_at_check: Option<u32>,
    pub(super) trap_at_check: Option<u32>,
    pub(super) deadline_ms: u64,
}

impl TransitionControl {
    pub(super) const PRODUCTION: Self = Self {
        cancel_at_check: None,
        trap_at_check: None,
        deadline_ms: DEADLINE_MS,
    };
}

impl TransitionState {
    pub(super) fn new(schema: SelectedSchema) -> Self {
        Self {
            schema,
            output: None,
            outcome: None,
            quota_reason: None,
            interruption: Interruption {
                checks: 0,
                last: 0,
                max: 0,
                deadline: std::time::Instant::now()
                    .checked_add(std::time::Duration::from_millis(DEADLINE_MS))
                    .unwrap_or_else(std::time::Instant::now),
                cancel_at_check: None,
                trap_at_check: None,
            },
        }
    }

    pub(super) fn select_control(&mut self, control: TransitionControl) {
        self.output = None;
        self.outcome = None;
        self.quota_reason = None;
        self.interruption.checks = 0;
        self.interruption.last = 0;
        self.interruption.max = 0;
        self.interruption.cancel_at_check = control.cancel_at_check;
        self.interruption.trap_at_check = control.trap_at_check;
        self.interruption.deadline = std::time::Instant::now()
            .checked_add(std::time::Duration::from_millis(control.deadline_ms))
            .unwrap_or_else(std::time::Instant::now);
    }

    /// 0 continues, 1 acknowledges cancellation and 2 is the host deadline.
    pub(super) fn interrupt(&mut self, total: u32, cancelled: bool) -> i32 {
        let interval = total.saturating_sub(self.interruption.last);
        self.interruption.max = self.interruption.max.max(interval);
        self.interruption.last = total;
        self.interruption.checks = self.interruption.checks.saturating_add(1);
        if self.interruption.trap_at_check == Some(self.interruption.checks) {
            self.outcome = Some("trap");
            3
        } else if cancelled || self.interruption.cancel_at_check == Some(self.interruption.checks) {
            1
        } else if std::time::Instant::now() >= self.interruption.deadline {
            2
        } else {
            0
        }
    }
}

struct Build<'store, 'context> {
    store: &'store mut StoreContextMut<'context, Host>,
    instance: Instance,
    memory: Memory,
    cursor: usize,
    nodes: usize,
}

impl Build<'_, '_> {
    fn typed<P, R>(&mut self, name: &str) -> Result<TypedFunc<P, R>>
    where
        P: wasmtime::WasmParams,
        R: wasmtime::WasmResults,
    {
        self.instance
            .get_typed_func::<P, R>(&mut *self.store, name)
            .with_context(|| format!("transition runtime export {name}"))
    }

    fn node(&mut self, depth: usize) -> Result<()> {
        ensure!(depth < DEPTH_LIMIT, "transition value depth exhausted");
        self.nodes = self
            .nodes
            .checked_add(1)
            .context("transition node count overflow")?;
        ensure!(self.nodes <= NODE_LIMIT, "transition node count exhausted");
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<i32> {
        let bytes = text.as_bytes();
        ensure!(bytes.len() <= 65_536, "transition Text exceeds bound");
        let end = self
            .cursor
            .checked_add(bytes.len())
            .context("transition Text address overflow")?;
        ensure!(end <= MEMORY_END, "transition Text scratch exhausted");
        self.memory.write(&mut *self.store, self.cursor, bytes)?;
        let offset = i32::try_from(self.cursor)?;
        self.cursor = end;
        self.typed::<(i32, i32), i32>("host_text")?
            .call(&mut *self.store, (offset, i32::try_from(bytes.len())?))
            .context("transition host_text")
    }

    fn value(&mut self, value: &Value, ty: &Ty, depth: usize) -> Result<i32> {
        self.node(depth)?;
        match (ty, value["tag"].as_str()) {
            (Ty::I64, Some("i64")) => {
                let number = value["value"]
                    .as_str()
                    .context("transition I64 is not decimal")?
                    .parse::<i64>()?;
                self.typed::<i64, i32>("host_i64")?
                    .call(&mut *self.store, number)
                    .context("transition host_i64")
            }
            (Ty::Bool, Some("bool")) => {
                let boolean = value["value"]
                    .as_bool()
                    .context("transition Bool is not boolean")?;
                self.typed::<i32, i32>("host_bool")?
                    .call(&mut *self.store, i32::from(boolean))
                    .context("transition host_bool")
            }
            (Ty::Unit, Some("unit")) if value["value"].is_null() => self
                .typed::<(), i32>("host_unit")?
                .call(&mut *self.store, ())
                .context("transition host_unit"),
            (Ty::Text, Some("text")) => self.text(
                value["value"]
                    .as_str()
                    .context("transition Text is not string")?,
            ),
            (Ty::Pair(left, right), Some("pair")) => {
                let left = self.value(&value["left"], left, depth + 1)?;
                let right = self.value(&value["right"], right, depth + 1)?;
                self.typed::<(i32, i32), i32>("host_pair")?
                    .call(&mut *self.store, (left, right))
                    .context("transition host_pair")
            }
            (Ty::Sum(left, right), Some("sum")) => {
                let (arm, inner) = match value["arm"].as_str() {
                    Some("left") => (0, left.as_ref()),
                    Some("right") => (1, right.as_ref()),
                    _ => anyhow::bail!("transition Sum arm is invalid"),
                };
                let payload = self.value(&value["value"], inner, depth + 1)?;
                self.typed::<(i32, i32), i32>("host_sum")?
                    .call(&mut *self.store, (arm, payload))
                    .context("transition host_sum")
            }
            (Ty::List(item), Some("list")) => {
                let items = value["items"]
                    .as_array()
                    .context("transition List items are absent")?;
                ensure!(items.len() <= NODE_LIMIT, "transition List exceeds bound");
                let mut tail = self
                    .typed::<(), i32>("host_list_nil")?
                    .call(&mut *self.store, ())?;
                for item_value in items.iter().rev() {
                    let head = self.value(item_value, item, depth + 1)?;
                    tail = self
                        .typed::<(i32, i32), i32>("host_list_cons")?
                        .call(&mut *self.store, (head, tail))?;
                }
                Ok(tail)
            }
            (Ty::Nominal(id, shape), Some(tag @ ("opaque" | "variant"))) => {
                let (arm, inner) = match (tag, shape.as_ref(), value["arm"].as_str()) {
                    ("opaque", NominalShape::Opaque(inner), None) => (0, inner.as_ref()),
                    ("variant", NominalShape::Variant(left, _), Some("left")) => (1, left.as_ref()),
                    ("variant", NominalShape::Variant(_, right), Some("right")) => {
                        (2, right.as_ref())
                    }
                    _ => anyhow::bail!("transition nominal arm is invalid"),
                };
                let payload = self.value(&value["value"], inner, depth + 1)?;
                self.typed::<(i64, i32, i32, i32), i32>("host_nominal")?
                    .call(
                        &mut *self.store,
                        (
                            i64::from_le_bytes(id.module.to_le_bytes()),
                            i32::try_from(id.ordinal)?,
                            arm,
                            payload,
                        ),
                    )
                    .context("transition host_nominal")
            }
            _ => anyhow::bail!("transition value differs from its checked type"),
        }
    }
}

struct Observe<'store, 'context> {
    store: &'store mut StoreContextMut<'context, Host>,
    instance: Instance,
    memory: Memory,
    nodes: usize,
    bytes: usize,
}

impl Observe<'_, '_> {
    fn typed<P, R>(&mut self, name: &str) -> Result<TypedFunc<P, R>>
    where
        P: wasmtime::WasmParams,
        R: wasmtime::WasmResults,
    {
        self.instance
            .get_typed_func::<P, R>(&mut *self.store, name)
            .with_context(|| format!("transition runtime export {name}"))
    }

    fn node(&mut self, depth: usize) -> Result<()> {
        ensure!(
            depth <= DEPTH_LIMIT,
            "transition observation depth exhausted"
        );
        self.nodes = self
            .nodes
            .checked_add(1)
            .context("observation node overflow")?;
        ensure!(
            self.nodes <= NODE_LIMIT,
            "transition observation work exhausted"
        );
        self.bytes = self
            .bytes
            .checked_add(256)
            .context("observation byte overflow")?;
        ensure!(
            self.bytes <= OBSERVATION_BYTES,
            "transition observation bytes exhausted"
        );
        Ok(())
    }

    fn field(&mut self, name: &str, handle: i32) -> Result<i32> {
        self.typed::<i32, i32>(name)?
            .call(&mut *self.store, handle)
            .with_context(|| format!("transition {name}"))
    }

    fn payload(&mut self, handle: i32) -> Result<i64> {
        self.typed::<i32, i64>("cell_payload")?
            .call(&mut *self.store, handle)
            .context("transition cell_payload")
    }

    fn text(&mut self, handle: i32) -> Result<Value> {
        let start = usize::try_from(self.field("cell_x", handle)?)?;
        let length = usize::try_from(self.field("cell_y", handle)?)?;
        let end = start
            .checked_add(length)
            .context("observed Text overflow")?;
        let bytes = self
            .memory
            .data(&*self.store)
            .get(start..end)
            .context("observed Text outside memory")?;
        self.bytes = self
            .bytes
            .checked_add(length)
            .context("Text charge overflow")?;
        ensure!(
            self.bytes <= OBSERVATION_BYTES,
            "transition Text observation exhausted"
        );
        Ok(json!({"type":"Text","value":std::str::from_utf8(bytes)?}))
    }

    fn handle(&mut self, handle: i32, ty: &Ty, depth: usize) -> Result<Value> {
        self.node(depth)?;
        let kind = self.field("cell_kind", handle)?;
        match (ty, kind) {
            (Ty::I64, 1) => Ok(json!({"type":"I64","value":self.payload(handle)?.to_string()})),
            (Ty::Bool, 2) => {
                let value = self.payload(handle)?;
                ensure!(value == 0 || value == 1, "invalid observed Bool");
                Ok(json!({"type":"Bool","value":value == 1}))
            }
            (Ty::Unit, 3) => Ok(json!({"type":"Unit","value":null})),
            (Ty::Text, 11) => self.text(handle),
            (Ty::Pair(left, right), 5) => {
                let left_handle = self.field("cell_a", handle)?;
                let right_handle = self.field("cell_b", handle)?;
                let left = self.handle(left_handle, left, depth + 1)?;
                let right = self.handle(right_handle, right, depth + 1)?;
                Ok(json!({"type":"Pair","value":[left,right]}))
            }
            (Ty::Sum(left, _), 12) if self.field("cell_w", handle)? == 0 => {
                let payload = self.field("cell_a", handle)?;
                let value = self.handle(payload, left, depth + 1)?;
                Ok(json!({"type":"Sum","variant":"left","value":value}))
            }
            (Ty::Sum(_, right), 13) if self.field("cell_w", handle)? == 0 => {
                let payload = self.field("cell_a", handle)?;
                let value = self.handle(payload, right, depth + 1)?;
                Ok(json!({"type":"Sum","variant":"right","value":value}))
            }
            (Ty::List(item), 6 | 7) => self.list(handle, item, depth),
            (Ty::Nominal(id, shape), 12 | 13) => self.nominal(handle, id, shape, kind, depth),
            _ => anyhow::bail!("observed transition value differs from its checked type"),
        }
    }

    fn list(&mut self, mut tail: i32, item: &Ty, depth: usize) -> Result<Value> {
        let mut items = Vec::new();
        loop {
            let kind = self.field("cell_kind", tail)?;
            if kind == 7 {
                break;
            }
            ensure!(
                kind == 6 && items.len() < NODE_LIMIT,
                "invalid observed List"
            );
            let head = self.field("cell_a", tail)?;
            items.push(self.handle(head, item, depth + 1)?);
            let next = self.field("cell_b", tail)?;
            ensure!(next > 0 && next < tail, "cyclic/non-backward observed List");
            tail = next;
        }
        Ok(json!({"type":"List","value":items}))
    }

    fn nominal(
        &mut self,
        handle: i32,
        id: &noble_kernel::types::NominalTypeId,
        shape: &NominalShape,
        kind: i32,
        depth: usize,
    ) -> Result<Value> {
        let low = u64::from(u32::from_le_bytes(
            self.field("cell_x", handle)?.to_le_bytes(),
        ));
        let high = u64::from(u32::from_le_bytes(
            self.field("cell_y", handle)?.to_le_bytes(),
        ));
        let module = low | (high << 32);
        let ordinal = u32::try_from(self.field("cell_z", handle)?)?;
        ensure!(
            module == id.module && ordinal == id.ordinal,
            "wrong observed nominal identity"
        );
        let marker = self.field("cell_w", handle)?;
        let (shape_name, variant, inner) = match (shape, marker, kind) {
            (NominalShape::Opaque(inner), 1, 12) => ("opaque", Value::Null, inner.as_ref()),
            (NominalShape::Variant(left, _), 2, 12) => ("variant", json!("left"), left.as_ref()),
            (NominalShape::Variant(_, right), 2, 13) => ("variant", json!("right"), right.as_ref()),
            _ => anyhow::bail!("invalid observed nominal representation"),
        };
        let payload_handle = self.field("cell_a", handle)?;
        let payload = self.handle(payload_handle, inner, depth + 1)?;
        Ok(
            json!({"type":"Nominal","module":module.to_string(),"ordinal":ordinal,
            "shape":shape_name,"variant":variant,"value":payload}),
        )
    }

    fn slot(&mut self, index: i32, ty: &Ty) -> Result<Value> {
        let kind = self
            .typed::<i32, i32>("stack_kind")?
            .call(&mut *self.store, index)?;
        let raw = self
            .typed::<i32, i64>("stack_value")?
            .call(&mut *self.store, index)?;
        if kind < 4 {
            self.node(0)?;
            return match (ty, kind) {
                (Ty::I64, 1) => Ok(json!({"type":"I64","value":raw.to_string()})),
                (Ty::Bool, 2) if raw == 0 || raw == 1 => {
                    Ok(json!({"type":"Bool","value":raw == 1}))
                }
                (Ty::Unit, 3) => Ok(json!({"type":"Unit","value":null})),
                _ => anyhow::bail!("observed immediate differs from checked type"),
            };
        }
        self.handle(i32::try_from(raw)?, ty, 0)
    }
}

/// Validate, inject, invoke and observe one transition from the component host
/// callback. The returned string is also retained in `Host.transition` so the
/// outer embedder can require byte-for-byte agreement after `post_return`.
pub(super) fn invoke(
    store: &mut StoreContextMut<'_, Host>,
    instance: Instance,
    cell: ProgramCell,
    state_frame: &str,
    event_frame: &str,
) -> Result<String> {
    ensure!(
        state_frame.len() <= 65_536 && event_frame.len() <= 65_536,
        "frame exceeds bound"
    );
    let schema = store
        .data()
        .transition
        .as_ref()
        .context("transition schema is absent")?
        .schema
        .clone();
    let [state_ty, event_ty, action_ty, _] = &schema.types;
    let [state, event] = super::worker_value::admit(
        [state_frame.as_bytes(), event_frame.as_bytes()],
        [state_ty, event_ty],
        &schema.catalog,
        0,
        1_000_000,
    )
    .map_err(|problem| anyhow::anyhow!("transition frame admission: {problem:?}"))?;
    let memory = instance
        .get_memory(&mut *store, "memory")
        .context("transition runtime memory missing")?;
    let begin =
        instance.get_typed_func::<(i32, i32, i32, i32, i32, i32), i32>(&mut *store, "begin")?;
    ensure!(
        begin.call(&mut *store, (196_608, 256, 256, 2_048, 4_096, 4_096))? == 0,
        "transition runtime budget refused"
    );
    let mut build = Build {
        store,
        instance,
        memory,
        cursor: TEXT_START,
        nodes: 0,
    };
    for (value, ty) in [(&state, state_ty), (&event, event_ty)] {
        let handle = build.value(value, ty, 0)?;
        ensure!(
            build
                .typed::<i32, i32>("host_push")?
                .call(&mut *build.store, handle)?
                == 0,
            "transition value push refused"
        );
    }
    let status = build
        .typed::<(i32, i32, i32), i32>("worker_invoke_stack")?
        .call(&mut *build.store, (i32::try_from(cell.address)?, 2, 2))?;
    if status != 0 {
        let metric = build.typed::<i32, i64>("metric")?;
        let retained = i32::try_from(metric.call(&mut *build.store, 0)?)?;
        ensure!(retained == status, "transition status differs from metric");
        let quota_reason = metric.call(&mut *build.store, 1)?;
        let outcome = match status {
            1 | 2 => "runtime-exhausted",
            6 => "trap",
            7 => "cancelled",
            8 => "host-deadline",
            _ => "internal-failure",
        };
        let state = build
            .store
            .data_mut()
            .transition
            .as_mut()
            .context("transition state retired")?;
        state.outcome = Some(outcome);
        state.quota_reason = (quota_reason != 0).then_some(quota_reason);
        anyhow::bail!("transition source outcome {outcome}");
    }
    let count = build
        .typed::<(), i32>("stack_length")?
        .call(&mut *build.store, ())?;
    ensure!(count == 2, "transition returned the wrong stack height");
    let mut observe = Observe {
        store: build.store,
        instance,
        memory,
        nodes: 0,
        bytes: 0,
    };
    let observed_state = observe.slot(0, state_ty)?;
    let actions_ty = Ty::List(Box::new(action_ty.clone()));
    let observed_actions = observe.slot(1, &actions_ty)?;
    let [state, actions] = super::worker_value::returned(
        [&observed_state, &observed_actions],
        [state_ty, &actions_ty],
        &schema.catalog,
    )
    .map_err(|problem| anyhow::anyhow!("transition output observation: {problem:?}"))?;
    let encoded = serde_json::to_string(&json!({
        "actions":actions,"format":"noble-worker-transition-output/v1","state":state
    }))?;
    ensure!(
        encoded.len() <= 262_144,
        "transition output frame exceeds bound"
    );
    let output = TransitionOutput {
        encoded: encoded.clone(),
        state,
        actions,
    };
    observe
        .store
        .data_mut()
        .transition
        .as_mut()
        .context("transition state retired")?
        .output = Some(output);
    observe
        .store
        .data_mut()
        .transition
        .as_mut()
        .context("transition state retired")?
        .outcome = Some("normal");
    Ok(encoded)
}
