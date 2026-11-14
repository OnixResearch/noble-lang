//! Versioned, checked byte plans for revision-3 local interactions.
//!
//! The compiler and decoder/checker are intentionally separate: the checker
//! decodes bytes and reconstructs the selected rule from the live arena.

use super::{
    ActiveWorklist, Endpoint, Invalid, Op, Pair, PortIndex, PortRef, RuntimeAgent, Slot,
    StoragePolicy, Symbol,
};

const MAGIC: [u8; 4] = *b"NPLN";
const VERSION: u8 = 1;
const COMMIT: u8 = 0xc7;
pub(super) const MAX_PLAN_BYTES: usize = 256;
const MAX_BOUNDARIES: usize = 2;
const MAX_RESULTS: usize = 2;
const MAX_WRITES: usize = 2;

/// Fixed-size storage means producing or decoding a plan does not allocate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanBytes {
    bytes: [u8; MAX_PLAN_BYTES],
    len: usize,
}

impl PlanBytes {
    /// Exact emitted bytes, excluding unused fixed-buffer capacity.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    pub(super) fn as_mut_bytes(&mut self) -> &mut [u8] {
        &mut self.bytes[..self.len]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Boundary {
    source: PortRef,
    external: Endpoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResultAgent {
    slot: u32,
    fresh: bool,
    symbol: Symbol,
    donor_slot: u32,
    ports: [Option<u8>; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WireWrite {
    result: u8,
    port: u8,
    boundary: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Active {
    first: u32,
    first_generation: u64,
    second: u32,
    second_generation: u64,
}

#[derive(Clone, Copy)]
struct RuleInstance {
    opcode: u8,
    action: [u8; 2],
    boundaries: [Option<Boundary>; MAX_BOUNDARIES],
    boundary_count: u8,
    results: [Option<ResultAgent>; MAX_RESULTS],
    result_count: u8,
    writes: [Option<WireWrite>; MAX_WRITES],
    write_count: u8,
    publications: [Option<Active>; MAX_RESULTS],
    publication_count: u8,
}

#[derive(Clone, Copy)]
struct Image {
    mode: StoragePolicy,
    pair: Pair,
    opcode: u8,
    action: [u8; 2],
    boundaries: [Option<Boundary>; MAX_BOUNDARIES],
    boundary_count: u8,
    results: [Option<ResultAgent>; MAX_RESULTS],
    result_count: u8,
    writes: [Option<WireWrite>; MAX_WRITES],
    write_count: u8,
    external_writes: [Option<WireWrite>; MAX_WRITES],
    external_write_count: u8,
    arena_reservation: u32,
    scratch_reservation: u32,
    queue_reservation: u32,
    publications: [Option<Active>; MAX_RESULTS],
    publication_count: u8,
}

pub(super) struct CheckedPlan {
    image: Image,
    pub(super) arena_reservation: u32,
    pub(super) scratch_reservation: u32,
    pub(super) queue_reservation: u32,
    pub(super) reused_slots: u32,
    pub(super) freed_slots: u32,
    pub(super) live_decrease: u32,
}

pub(super) enum ExecuteError {
    Invalid(Invalid),
    Limited {
        limit: super::Limit,
        cap: u64,
        observed: u64,
    },
}

pub(super) struct Execution {
    pub(super) live_decrease: u32,
    pub(super) transient_live_nodes: u32,
    pub(super) reused_slots: u32,
    pub(super) freed_slots: u32,
    pub(super) allocated_slots: u32,
    pub(super) microsteps: u32,
}

struct Writer {
    bytes: [u8; MAX_PLAN_BYTES],
    len: usize,
}

impl Writer {
    fn new() -> Self {
        Self {
            bytes: [0; MAX_PLAN_BYTES],
            len: 0,
        }
    }

    fn put(&mut self, value: u8) -> Result<(), Invalid> {
        let Some(target) = self.bytes.get_mut(self.len) else {
            return Err(Invalid::PlanEncoding);
        };
        *target = value;
        self.len = self.len.checked_add(1).ok_or(Invalid::CounterOverflow)?;
        Ok(())
    }

    fn put_u32(&mut self, value: u32) -> Result<(), Invalid> {
        for byte in value.to_le_bytes() {
            self.put(byte)?;
        }
        Ok(())
    }

    fn put_u64(&mut self, value: u64) -> Result<(), Invalid> {
        for byte in value.to_le_bytes() {
            self.put(byte)?;
        }
        Ok(())
    }

    fn put_endpoint(&mut self, endpoint: Endpoint) -> Result<(), Invalid> {
        match endpoint {
            Endpoint::Port(port) => {
                self.put(0)?;
                self.put_u32(port.agent)?;
                self.put(u8::from(port.port_index))
            }
            Endpoint::Output(output) => {
                self.put(1)?;
                self.put_u32(output)
            }
        }
    }

    fn finish(self) -> PlanBytes {
        PlanBytes {
            bytes: self.bytes,
            len: self.len,
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn get(&mut self) -> Result<u8, Invalid> {
        let value = *self.bytes.get(self.offset).ok_or(Invalid::PlanEncoding)?;
        self.offset = self.offset.checked_add(1).ok_or(Invalid::CounterOverflow)?;
        Ok(value)
    }

    fn get_u32(&mut self) -> Result<u32, Invalid> {
        let mut bytes = [0; 4];
        for byte in &mut bytes {
            *byte = self.get()?;
        }
        Ok(u32::from_le_bytes(bytes))
    }

    fn get_u64(&mut self) -> Result<u64, Invalid> {
        let mut bytes = [0; 8];
        for byte in &mut bytes {
            *byte = self.get()?;
        }
        Ok(u64::from_le_bytes(bytes))
    }

    fn get_endpoint(&mut self) -> Result<Endpoint, Invalid> {
        match self.get()? {
            0 => Ok(Endpoint::Port(PortRef {
                agent: self.get_u32()?,
                port_index: PortIndex::from(self.get()?),
            })),
            1 => Ok(Endpoint::Output(self.get_u32()?)),
            _ => Err(Invalid::PlanEncoding),
        }
    }
}

fn op_tag(op: Op) -> u8 {
    match op {
        Op::Add => 0,
        Op::Sub => 1,
        Op::Mul => 2,
        Op::Eq => 3,
    }
}

fn decode_op(tag: u8) -> Result<Op, Invalid> {
    match tag {
        0 => Ok(Op::Add),
        1 => Ok(Op::Sub),
        2 => Ok(Op::Mul),
        3 => Ok(Op::Eq),
        _ => Err(Invalid::PlanEncoding),
    }
}

fn symbol_tag(symbol: Symbol) -> (u8, u64) {
    match symbol {
        Symbol::Num(value) => (0, value),
        Symbol::Op2(op) => (1 + op_tag(op), 0),
        Symbol::Op1(op, first) => (5 + op_tag(op), first),
        Symbol::Dup => (9, 0),
        Symbol::Era => (10, 0),
    }
}

fn decode_symbol(tag: u8, payload: u64) -> Result<Symbol, Invalid> {
    match tag {
        0 => Ok(Symbol::Num(payload)),
        1..=4 if payload == 0 => Ok(Symbol::Op2(decode_op(tag - 1)?)),
        5..=8 => Ok(Symbol::Op1(decode_op(tag - 5)?, payload)),
        9 if payload == 0 => Ok(Symbol::Dup),
        10 if payload == 0 => Ok(Symbol::Era),
        _ => Err(Invalid::PlanEncoding),
    }
}

fn slot_agent(slots: &[Slot], index: u32) -> Result<RuntimeAgent, Invalid> {
    slots
        .get(index as usize)
        .and_then(|slot| slot.agent)
        .ok_or(Invalid::StalePlanSlot)
}

fn active_pair(slots: &[Slot], pair: Pair) -> Result<(RuntimeAgent, RuntimeAgent), Invalid> {
    let first = slots
        .get(pair.first as usize)
        .ok_or(Invalid::StalePlanSlot)?;
    let second = slots
        .get(pair.second as usize)
        .ok_or(Invalid::StalePlanSlot)?;
    if first.generation != pair.first_generation || second.generation != pair.second_generation {
        return Err(Invalid::StalePlanSlot);
    }
    let a = first.agent.ok_or(Invalid::StalePlanSlot)?;
    let b = second.agent.ok_or(Invalid::StalePlanSlot)?;
    if a.ports[0]
        != Some(Endpoint::Port(PortRef {
            agent: pair.second,
            port_index: PortIndex::from(0),
        }))
        || b.ports[0]
            != Some(Endpoint::Port(PortRef {
                agent: pair.first,
                port_index: PortIndex::from(0),
            }))
    {
        return Err(Invalid::StalePlanSlot);
    }
    Ok((a, b))
}

fn boundary(
    slots: &[Slot],
    outputs: &[PortRef],
    owner: u32,
    port: u8,
    pair: Pair,
    into: &mut [Option<Boundary>; MAX_BOUNDARIES],
    count: &mut u8,
) -> Result<(), Invalid> {
    let source_agent = slot_agent(slots, owner)?;
    let external = source_agent
        .ports
        .get(usize::from(port))
        .copied()
        .flatten()
        .ok_or(Invalid::PortArity)?;
    match external {
        Endpoint::Port(target) => {
            if target.agent == pair.first || target.agent == pair.second {
                return Err(Invalid::PlanBoundaryAlias);
            }
            let target_agent = slot_agent(slots, target.agent)?;
            if usize::from(target.port_index) >= target_agent.symbol.port_count()
                || target_agent.ports[usize::from(target.port_index)]
                    != Some(Endpoint::Port(PortRef {
                        agent: owner,
                        port_index: PortIndex::from(port),
                    }))
            {
                return Err(Invalid::NonReciprocal);
            }
        }
        Endpoint::Output(index) => {
            if outputs.get(index as usize)
                != Some(&PortRef {
                    agent: owner,
                    port_index: PortIndex::from(port),
                })
            {
                return Err(Invalid::OutputBoundary);
            }
        }
    }
    for prior in into.iter().flatten() {
        if prior.external == external {
            return Err(Invalid::PlanBoundaryAlias);
        }
    }
    let index = usize::from(*count);
    *into.get_mut(index).ok_or(Invalid::PlanEncoding)? = Some(Boundary {
        source: PortRef {
            agent: owner,
            port_index: PortIndex::from(port),
        },
        external,
    });
    *count = (*count).checked_add(1).ok_or(Invalid::CounterOverflow)?;
    Ok(())
}

fn classify(
    slots: &[Slot],
    outputs: &[PortRef],
    pair: Pair,
    policy: StoragePolicy,
) -> Result<RuleInstance, Invalid> {
    let (first, second) = active_pair(slots, pair)?;
    let mut instance = RuleInstance {
        opcode: 0,
        action: [0; 2],
        boundaries: [None; MAX_BOUNDARIES],
        boundary_count: 0,
        results: [None; MAX_RESULTS],
        result_count: 0,
        writes: [None; MAX_WRITES],
        write_count: 0,
        publications: [None; MAX_RESULTS],
        publication_count: 0,
    };
    let (rule_slot, value_slot, opcode, payload) = match (first.symbol, second.symbol) {
        (Symbol::Op2(_), Symbol::Num(value)) => (pair.first, pair.second, 1, value),
        (Symbol::Num(value), Symbol::Op2(_)) => (pair.second, pair.first, 1, value),
        (Symbol::Op1(_, _), Symbol::Num(second_value)) => {
            (pair.first, pair.second, 2, second_value)
        }
        (Symbol::Num(second_value), Symbol::Op1(_, _)) => {
            (pair.second, pair.first, 2, second_value)
        }
        (Symbol::Dup, Symbol::Num(value)) => (pair.first, pair.second, 3, value),
        (Symbol::Num(value), Symbol::Dup) => (pair.second, pair.first, 3, value),
        (Symbol::Era, Symbol::Num(_)) => (pair.first, pair.second, 4, 0),
        (Symbol::Num(_), Symbol::Era) => (pair.second, pair.first, 4, 0),
        _ => return Err(Invalid::StuckPair),
    };
    instance.opcode = opcode;
    let result_slots = match policy {
        StoragePolicy::Reuse => [Some(rule_slot), None],
        StoragePolicy::Allocate => [None, None],
    };
    match opcode {
        1 => {
            boundary(slots, outputs, rule_slot, 1, pair, &mut instance.boundaries, &mut instance.boundary_count)?;
            boundary(slots, outputs, rule_slot, 2, pair, &mut instance.boundaries, &mut instance.boundary_count)?;
            let op = match slot_agent(slots, rule_slot)?.symbol {
                Symbol::Op2(op) => op,
                _ => return Err(Invalid::PlanMismatch),
            };
            add_result(
                &mut instance,
                slots,
                rule_slot,
                result_slots[0],
                policy,
                Symbol::Op1(op, payload),
                [Some(0), Some(1), None],
            )?;
            instance.action = consumed_actions(pair, [rule_slot], policy)?;
        }
        2 => {
            boundary(slots, outputs, rule_slot, 1, pair, &mut instance.boundaries, &mut instance.boundary_count)?;
            let (op, first_value) = match slot_agent(slots, rule_slot)?.symbol {
                Symbol::Op1(op, first_value) => (op, first_value),
                _ => return Err(Invalid::PlanMismatch),
            };
            let second_value = match slot_agent(slots, value_slot)?.symbol {
                Symbol::Num(value) => value,
                _ => return Err(Invalid::PlanMismatch),
            };
            add_result(
                &mut instance,
                slots,
                rule_slot,
                result_slots[0],
                policy,
                Symbol::Num(apply(op, first_value, second_value)),
                [Some(0), None, None],
            )?;
            instance.action = consumed_actions(pair, [rule_slot], policy)?;
        }
        3 => {
            boundary(slots, outputs, rule_slot, 1, pair, &mut instance.boundaries, &mut instance.boundary_count)?;
            boundary(slots, outputs, rule_slot, 2, pair, &mut instance.boundaries, &mut instance.boundary_count)?;
            let second_destination = match policy {
                StoragePolicy::Reuse => Some(value_slot),
                StoragePolicy::Allocate => None,
            };
            add_result(
                &mut instance,
                slots,
                rule_slot,
                result_slots[0],
                policy,
                Symbol::Num(payload),
                [Some(0), None, None],
            )?;
            add_result(
                &mut instance,
                slots,
                value_slot,
                second_destination,
                policy,
                Symbol::Num(payload),
                [Some(1), None, None],
            )?;
            instance.action = consumed_actions(pair, [rule_slot, value_slot], policy)?;
        }
        4 => {
            instance.action = [0; 2];
        }
        _ => return Err(Invalid::PlanEncoding),
    }
    populate_writes_and_publications(slots, pair, &mut instance)?;
    Ok(instance)
}

fn consumed_actions(pair: Pair, reused: [u32; 2], policy: StoragePolicy) -> Result<[u8; 2], Invalid> {
    let mut actions = [0; 2];
    for id in reused {
        let index = if pair.first == id { 0 } else if pair.second == id { 1 } else {
            return Err(Invalid::PlanMismatch);
        };
        actions[index] = u8::from(policy == StoragePolicy::Reuse);
    }
    Ok(actions)
}

fn add_result(
    instance: &mut RuleInstance,
    slots: &[Slot],
    donor_slot: u32,
    reused_slot: Option<u32>,
    policy: StoragePolicy,
    symbol: Symbol,
    ports: [Option<u8>; 3],
) -> Result<(), Invalid> {
    let index = usize::from(instance.result_count);
    let destination = match reused_slot {
        Some(slot) => slot,
        None => {
            let ordinal = u32::from(instance.result_count);
            u32::try_from(slots.len())
                .map_err(|_| Invalid::CounterOverflow)?
                .checked_add(ordinal)
                .ok_or(Invalid::CounterOverflow)?
        }
    };
    let result = ResultAgent {
        slot: destination,
        fresh: policy == StoragePolicy::Allocate,
        symbol,
        donor_slot,
        ports,
    };
    *instance.results.get_mut(index).ok_or(Invalid::PlanEncoding)? = Some(result);
    instance.result_count = instance.result_count.checked_add(1).ok_or(Invalid::CounterOverflow)?;
    Ok(())
}

fn populate_writes_and_publications(
    slots: &[Slot],
    pair: Pair,
    instance: &mut RuleInstance,
) -> Result<(), Invalid> {
    for (result_index, result) in instance.results.iter().take(usize::from(instance.result_count)).enumerate() {
        let result = result.ok_or(Invalid::PlanEncoding)?;
        for (port_index, boundary_index) in result.ports.iter().enumerate() {
            if let Some(boundary_index) = boundary_index {
                let write_index = usize::from(instance.write_count);
                let write = WireWrite {
                    result: u8::try_from(result_index).map_err(|_| Invalid::CounterOverflow)?,
                    port: u8::try_from(port_index).map_err(|_| Invalid::CounterOverflow)?,
                    boundary: *boundary_index,
                };
                *instance.writes.get_mut(write_index).ok_or(Invalid::PlanEncoding)? = Some(write);
                instance.write_count = instance.write_count.checked_add(1).ok_or(Invalid::CounterOverflow)?;
                if port_index == 0 {
                    if let Some(Endpoint::Port(target)) = instance
                        .boundaries
                        .get(usize::from(*boundary_index))
                        .and_then(|boundary| *boundary)
                        .map(|boundary| boundary.external)
                    {
                        if target.port_index == PortIndex::from(0)
                            && target.agent != pair.first
                            && target.agent != pair.second
                        {
                            let target_slot = slots.get(target.agent as usize).ok_or(Invalid::StalePlanSlot)?;
                            let destination_generation = if result.fresh {
                                0
                            } else {
                                slots
                                    .get(result.slot as usize)
                                    .ok_or(Invalid::StalePlanSlot)?
                                    .generation
                                    .checked_add(1)
                                    .ok_or(Invalid::CounterOverflow)?
                            };
                            let (first, first_generation, second, second_generation) = if result.slot < target.agent {
                                (result.slot, destination_generation, target.agent, target_slot.generation)
                            } else {
                                (target.agent, target_slot.generation, result.slot, destination_generation)
                            };
                            let publication_index = usize::from(instance.publication_count);
                            *instance.publications.get_mut(publication_index).ok_or(Invalid::PlanEncoding)? = Some(Active {
                                first,
                                first_generation,
                                second,
                                second_generation,
                            });
                            instance.publication_count = instance.publication_count.checked_add(1).ok_or(Invalid::CounterOverflow)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Compile the selected active pair into actual versioned bytes.
pub(super) fn compile(
    slots: &[Slot],
    outputs: &[PortRef],
    pair: Pair,
    policy: StoragePolicy,
) -> Result<PlanBytes, Invalid> {
    let instance = classify(slots, outputs, pair, policy)?;
    let mut writer = Writer::new();
    for byte in MAGIC {
        writer.put(byte)?;
    }
    writer.put(VERSION)?;
    writer.put(match policy { StoragePolicy::Reuse => 0, StoragePolicy::Allocate => 1 })?;
    writer.put(instance.opcode)?;
    writer.put_u32(pair.first)?;
    writer.put_u64(pair.first_generation)?;
    writer.put_u32(pair.second)?;
    writer.put_u64(pair.second_generation)?;
    writer.put(instance.action[0])?;
    writer.put(instance.action[1])?;
    writer.put(instance.boundary_count)?;
    for boundary in instance.boundaries.iter().take(usize::from(instance.boundary_count)) {
        let boundary = boundary.ok_or(Invalid::PlanEncoding)?;
        writer.put_u32(boundary.source.agent)?;
        writer.put(u8::from(boundary.source.port_index))?;
        writer.put_endpoint(boundary.external)?;
    }
    writer.put(instance.result_count)?;
    let mut fresh = 0u32;
    for result in instance.results.iter().take(usize::from(instance.result_count)) {
        let result = result.ok_or(Invalid::PlanEncoding)?;
        writer.put(u8::from(result.fresh))?;
        writer.put_u32(result.slot)?;
        let (tag, payload) = symbol_tag(result.symbol);
        writer.put(tag)?;
        writer.put_u64(payload)?;
        writer.put_u32(result.donor_slot)?;
        for boundary in result.ports {
            writer.put(boundary.unwrap_or(u8::MAX))?;
        }
        if result.fresh {
            fresh = fresh.checked_add(1).ok_or(Invalid::CounterOverflow)?;
        }
    }
    writer.put(instance.write_count)?;
    for write in instance.writes.iter().take(usize::from(instance.write_count)) {
        let write = write.ok_or(Invalid::PlanEncoding)?;
        writer.put(write.result)?;
        writer.put(write.port)?;
        writer.put(write.boundary)?;
    }
    writer.put(instance.write_count)?;
    for write in instance.writes.iter().take(usize::from(instance.write_count)) {
        let write = write.ok_or(Invalid::PlanEncoding)?;
        writer.put(write.result)?;
        writer.put(write.port)?;
        writer.put(write.boundary)?;
    }
    writer.put_u32(fresh)?;
    writer.put_u32(u32::try_from(MAX_PLAN_BYTES).map_err(|_| Invalid::CounterOverflow)?)?;
    writer.put_u32(u32::from(instance.publication_count))?;
    writer.put(instance.publication_count)?;
    for active in instance.publications.iter().take(usize::from(instance.publication_count)) {
        let active = active.ok_or(Invalid::PlanEncoding)?;
        writer.put_u32(active.first)?;
        writer.put_u64(active.first_generation)?;
        writer.put_u32(active.second)?;
        writer.put_u64(active.second_generation)?;
    }
    writer.put(COMMIT)?;
    Ok(writer.finish())
}

fn decode(bytes: &[u8]) -> Result<Image, Invalid> {
    if bytes.len() > MAX_PLAN_BYTES {
        return Err(Invalid::PlanEncoding);
    }
    let mut reader = Reader::new(bytes);
    for expected in MAGIC {
        if reader.get()? != expected {
            return Err(Invalid::PlanEncoding);
        }
    }
    if reader.get()? != VERSION {
        return Err(Invalid::PlanVersion);
    }
    let mode = match reader.get()? {
        0 => StoragePolicy::Reuse,
        1 => StoragePolicy::Allocate,
        _ => return Err(Invalid::PlanEncoding),
    };
    let opcode = reader.get()?;
    let pair = Pair {
        first: reader.get_u32()?,
        first_generation: reader.get_u64()?,
        second: reader.get_u32()?,
        second_generation: reader.get_u64()?,
    };
    let action = [reader.get()?, reader.get()?];
    if action.iter().any(|value| *value > 1) {
        return Err(Invalid::PlanEncoding);
    }
    let boundary_count = reader.get()?;
    if usize::from(boundary_count) > MAX_BOUNDARIES {
        return Err(Invalid::PlanEncoding);
    }
    let mut boundaries = [None; MAX_BOUNDARIES];
    for boundary in boundaries.iter_mut().take(usize::from(boundary_count)) {
        *boundary = Some(Boundary {
            source: PortRef {
                agent: reader.get_u32()?,
                port_index: PortIndex::from(reader.get()?),
            },
            external: reader.get_endpoint()?,
        });
    }
    let result_count = reader.get()?;
    if usize::from(result_count) > MAX_RESULTS {
        return Err(Invalid::PlanEncoding);
    }
    let mut results = [None; MAX_RESULTS];
    for result in results.iter_mut().take(usize::from(result_count)) {
        let fresh = match reader.get()? {
            0 => false,
            1 => true,
            _ => return Err(Invalid::PlanEncoding),
        };
        let slot = reader.get_u32()?;
        let symbol = decode_symbol(reader.get()?, reader.get_u64()?)?;
        let donor_slot = reader.get_u32()?;
        let mut ports = [None; 3];
        for port in &mut ports {
            let value = reader.get()?;
            *port = if value == u8::MAX { None } else { Some(value) };
        }
        *result = Some(ResultAgent { slot, fresh, symbol, donor_slot, ports });
    }
    let write_count = reader.get()?;
    if usize::from(write_count) > MAX_WRITES {
        return Err(Invalid::PlanEncoding);
    }
    let mut writes = [None; MAX_WRITES];
    for write in writes.iter_mut().take(usize::from(write_count)) {
        *write = Some(WireWrite { result: reader.get()?, port: reader.get()?, boundary: reader.get()? });
    }
    let external_write_count = reader.get()?;
    if usize::from(external_write_count) > MAX_WRITES {
        return Err(Invalid::PlanEncoding);
    }
    let mut external_writes = [None; MAX_WRITES];
    for write in external_writes.iter_mut().take(usize::from(external_write_count)) {
        *write = Some(WireWrite { result: reader.get()?, port: reader.get()?, boundary: reader.get()? });
    }
    let arena_reservation = reader.get_u32()?;
    let scratch_reservation = reader.get_u32()?;
    let queue_reservation = reader.get_u32()?;
    let publication_count = reader.get()?;
    if usize::from(publication_count) > MAX_RESULTS {
        return Err(Invalid::PlanEncoding);
    }
    let mut publications = [None; MAX_RESULTS];
    for active in publications.iter_mut().take(usize::from(publication_count)) {
        *active = Some(Active {
            first: reader.get_u32()?,
            first_generation: reader.get_u64()?,
            second: reader.get_u32()?,
            second_generation: reader.get_u64()?,
        });
    }
    if reader.get()? != COMMIT || reader.offset != bytes.len() {
        return Err(Invalid::PlanEncoding);
    }
    Ok(Image {
        mode,
        pair,
        opcode,
        action,
        boundaries,
        boundary_count,
        results,
        result_count,
        writes,
        write_count,
        external_writes,
        external_write_count,
        arena_reservation,
        scratch_reservation,
        queue_reservation,
        publications,
        publication_count,
    })
}

fn expected_symbol(
    opcode: u8,
    slots: &[Slot],
    pair: Pair,
) -> Result<(u32, u32, Symbol), Invalid> {
    let (left, right) = active_pair(slots, pair)?;
    match (left.symbol, right.symbol) {
        (Symbol::Op2(op), Symbol::Num(value)) => Ok((pair.first, pair.second, Symbol::Op1(op, value))),
        (Symbol::Num(value), Symbol::Op2(op)) => Ok((pair.second, pair.first, Symbol::Op1(op, value))),
        (Symbol::Op1(op, first), Symbol::Num(second)) if opcode == 2 => {
            Ok((pair.first, pair.second, Symbol::Num(apply(op, first, second))))
        }
        (Symbol::Num(second), Symbol::Op1(op, first)) if opcode == 2 => {
            Ok((pair.second, pair.first, Symbol::Num(apply(op, first, second))))
        }
        (Symbol::Dup, Symbol::Num(value)) => Ok((pair.first, pair.second, Symbol::Num(value))),
        (Symbol::Num(value), Symbol::Dup) => Ok((pair.second, pair.first, Symbol::Num(value))),
        (Symbol::Era, Symbol::Num(_)) => Ok((pair.first, pair.second, Symbol::Num(0))),
        (Symbol::Num(_), Symbol::Era) => Ok((pair.second, pair.first, Symbol::Num(0))),
        _ => Err(Invalid::PlanMismatch),
    }
}

fn validate_image(
    image: &Image,
    slots: &[Slot],
    outputs: &[PortRef],
    pair: Pair,
    policy: StoragePolicy,
) -> Result<CheckedPlan, Invalid> {
    if image.pair.first != pair.first
        || image.pair.second != pair.second
        || image.pair.first_generation != pair.first_generation
        || image.pair.second_generation != pair.second_generation
        || image.mode != policy
    {
        return Err(Invalid::StalePlanSlot);
    }
    let (first, second) = active_pair(slots, pair)?;
    let rule = match (first.symbol, second.symbol) {
        (Symbol::Op2(_), Symbol::Num(_)) | (Symbol::Num(_), Symbol::Op2(_)) => 1,
        (Symbol::Op1(_, _), Symbol::Num(_)) | (Symbol::Num(_), Symbol::Op1(_, _)) => 2,
        (Symbol::Dup, Symbol::Num(_)) | (Symbol::Num(_), Symbol::Dup) => 3,
        (Symbol::Era, Symbol::Num(_)) | (Symbol::Num(_), Symbol::Era) => 4,
        _ => return Err(Invalid::StuckPair),
    };
    if image.opcode != rule {
        return Err(Invalid::PlanMismatch);
    }
    let (rule_slot, value_slot, result_symbol) = expected_symbol(rule, slots, pair)?;
    let expected_boundary_ports: &[(u32, u8)] = match rule {
        1 => &[(rule_slot, 1), (rule_slot, 2)],
        2 => &[(rule_slot, 1)],
        3 => &[(rule_slot, 1), (rule_slot, 2)],
        4 => &[],
        _ => return Err(Invalid::PlanMismatch),
    };
    if usize::from(image.boundary_count) != expected_boundary_ports.len() {
        return Err(Invalid::PlanMismatch);
    }
    for (index, (owner, port)) in expected_boundary_ports.iter().enumerate() {
        let expected = Boundary {
            source: PortRef { agent: *owner, port_index: PortIndex::from(*port) },
            external: slot_agent(slots, *owner)?.ports[usize::from(*port)].ok_or(Invalid::PortArity)?,
        };
        if image.boundaries.get(index).copied().flatten() != Some(expected) {
            return Err(Invalid::PlanMismatch);
        }
        validate_boundary_independent(slots, outputs, pair, expected)?;
    }
    for prior_index in 0..usize::from(image.boundary_count) {
        let prior = image.boundaries[prior_index].ok_or(Invalid::PlanEncoding)?;
        for later in image.boundaries.iter().skip(prior_index + 1).take(usize::from(image.boundary_count) - prior_index - 1).flatten() {
            if prior.external == later.external {
                return Err(Invalid::PlanBoundaryAlias);
            }
        }
    }
    let expected_results = match (rule, policy) {
        (1, StoragePolicy::Reuse) | (2, StoragePolicy::Reuse) => 1,
        (1, StoragePolicy::Allocate) | (2, StoragePolicy::Allocate) => 1,
        (3, _) => 2,
        (4, _) => 0,
        _ => return Err(Invalid::PlanMismatch),
    };
    if usize::from(image.result_count) != expected_results {
        return Err(Invalid::PlanMismatch);
    }
    let consumed = [pair.first, pair.second];
    let mut consumed_reused = [false; 2];
    let mut fresh_count = 0u32;
    for (index, result) in image.results.iter().take(expected_results).enumerate() {
        let result = result.ok_or(Invalid::PlanEncoding)?;
        let expected_symbol_for_result = if rule == 3 { result_symbol } else { result_symbol };
        if result.symbol != expected_symbol_for_result {
            return Err(Invalid::PlanMismatch);
        }
        if result.donor_slot != rule_slot {
            return Err(Invalid::PlanMismatch);
        }
        let expected_ports: [Option<u8>; 3] = match (rule, index) {
            (1, 0) => [Some(0), Some(1), None],
            (2, 0) => [Some(0), None, None],
            (3, 0) => [Some(0), None, None],
            (3, 1) => [Some(1), None, None],
            _ => return Err(Invalid::PlanMismatch),
        };
        if result.ports != expected_ports {
            return Err(Invalid::PlanMismatch);
        }
        if result.fresh {
            if policy != StoragePolicy::Allocate
                || result.slot != u32::try_from(slots.len()).map_err(|_| Invalid::CounterOverflow)?
                    .checked_add(u32::try_from(index).map_err(|_| Invalid::CounterOverflow)?)
                    .ok_or(Invalid::CounterOverflow)?
            {
                return Err(Invalid::PlanMismatch);
            }
            fresh_count = fresh_count.checked_add(1).ok_or(Invalid::CounterOverflow)?;
        } else {
            if policy != StoragePolicy::Reuse {
                return Err(Invalid::PlanMismatch);
            }
            let consumed_index = if result.slot == pair.first { 0 } else if result.slot == pair.second { 1 } else {
                return Err(Invalid::PlanMismatch);
            };
            if consumed_reused[consumed_index] {
                return Err(Invalid::DuplicateFree);
            }
            consumed_reused[consumed_index] = true;
            let expected_slot = if rule == 3 && index == 1 {
                value_slot
            } else {
                rule_slot
            };
            if result.slot != expected_slot {
                return Err(Invalid::PlanMismatch);
            }
        }
    }
    for index in 0..2 {
        let expected_action = u8::from(consumed_reused[index]);
        if image.action[index] != expected_action {
            return Err(Invalid::PlanMismatch);
        }
    }
    let mut expected_writes = 0usize;
    for result in image.results.iter().take(expected_results) {
        let result = result.ok_or(Invalid::PlanEncoding)?;
        expected_writes = expected_writes
            .checked_add(result.ports.iter().filter(|port| port.is_some()).count())
            .ok_or(Invalid::CounterOverflow)?;
    }
    if usize::from(image.write_count) != expected_writes
        || usize::from(image.external_write_count) != expected_writes
    {
        return Err(Invalid::PlanMismatch);
    }
    let mut write_index = 0usize;
    for (result_index, result) in image.results.iter().take(expected_results).enumerate() {
        let result = result.ok_or(Invalid::PlanEncoding)?;
        for (port_index, boundary_index) in result.ports.iter().enumerate() {
            let Some(boundary_index) = boundary_index else { continue };
            if usize::from(*boundary_index) >= usize::from(image.boundary_count) {
                return Err(Invalid::PlanMismatch);
            }
            let expected_write = WireWrite {
                result: u8::try_from(result_index).map_err(|_| Invalid::CounterOverflow)?,
                port: u8::try_from(port_index).map_err(|_| Invalid::CounterOverflow)?,
                boundary: *boundary_index,
            };
            if image.writes.get(write_index).copied().flatten() != Some(expected_write)
                || image.external_writes.get(write_index).copied().flatten() != Some(expected_write)
            {
                return Err(Invalid::PlanMismatch);
            }
            write_index = write_index.checked_add(1).ok_or(Invalid::CounterOverflow)?;
        }
    }
    let expected_published = expected_publications(image, slots, pair)?;
    if usize::from(image.publication_count) != expected_published.0
        || image.publications[..expected_published.0] != expected_published.1[..expected_published.0]
    {
        return Err(Invalid::PlanMismatch);
    }
    let expected_arena = fresh_count;
    let expected_queue = u32::try_from(expected_published.0).map_err(|_| Invalid::CounterOverflow)?;
    if image.arena_reservation != expected_arena
        || image.scratch_reservation as usize != MAX_PLAN_BYTES
        || image.queue_reservation != expected_queue
    {
        return Err(Invalid::PlanMismatch);
    }
    let reused_slots = u32::try_from(consumed_reused.iter().filter(|reused| **reused).count())
        .map_err(|_| Invalid::CounterOverflow)?;
    let freed_slots = 2u32.checked_sub(reused_slots).ok_or(Invalid::CounterOverflow)?;
    let live_decrease = 2u32.checked_sub(u32::try_from(expected_results).map_err(|_| Invalid::CounterOverflow)?)
        .ok_or(Invalid::CounterOverflow)?;
    Ok(CheckedPlan {
        image: *image,
        arena_reservation: expected_arena,
        scratch_reservation: image.scratch_reservation,
        queue_reservation: expected_queue,
        reused_slots,
        freed_slots,
        live_decrease,
    })
}

fn validate_boundary_independent(
    slots: &[Slot],
    outputs: &[PortRef],
    pair: Pair,
    boundary: Boundary,
) -> Result<(), Invalid> {
    if boundary.source.agent != pair.first && boundary.source.agent != pair.second {
        return Err(Invalid::PlanMismatch);
    }
    let source = slot_agent(slots, boundary.source.agent)?;
    if source.ports.get(usize::from(boundary.source.port_index)).copied().flatten() != Some(boundary.external) {
        return Err(Invalid::PlanMismatch);
    }
    match boundary.external {
        Endpoint::Port(target) => {
            if target.agent == pair.first || target.agent == pair.second {
                return Err(Invalid::PlanBoundaryAlias);
            }
            let opposite = slot_agent(slots, target.agent)?;
            if usize::from(target.port_index) >= opposite.symbol.port_count()
                || opposite.ports[usize::from(target.port_index)]
                    != Some(Endpoint::Port(boundary.source))
            {
                return Err(Invalid::NonReciprocal);
            }
        }
        Endpoint::Output(index) => {
            if outputs.get(index as usize) != Some(&boundary.source) {
                return Err(Invalid::OutputBoundary);
            }
        }
    }
    Ok(())
}

fn expected_publications(
    image: &Image,
    slots: &[Slot],
    pair: Pair,
) -> Result<(usize, [Option<Active>; MAX_RESULTS]), Invalid> {
    let mut result = [None; MAX_RESULTS];
    let mut count = 0usize;
    for agent in image.results.iter().take(usize::from(image.result_count)).flatten() {
        let Some(boundary_index) = agent.ports[0] else {
            continue;
        };
        let boundary = image.boundaries[usize::from(boundary_index)].ok_or(Invalid::PlanEncoding)?;
        if let Endpoint::Port(target) = boundary.external {
            if target.port_index == PortIndex::from(0)
                && target.agent != pair.first
                && target.agent != pair.second
            {
                let target_slot = slots.get(target.agent as usize).ok_or(Invalid::StalePlanSlot)?;
                let generation = if agent.fresh {
                    0
                } else {
                    slots.get(agent.slot as usize).ok_or(Invalid::StalePlanSlot)?
                        .generation.checked_add(1).ok_or(Invalid::CounterOverflow)?
                };
                let active = if agent.slot < target.agent {
                    Active { first: agent.slot, first_generation: generation, second: target.agent, second_generation: target_slot.generation }
                } else {
                    Active { first: target.agent, first_generation: target_slot.generation, second: agent.slot, second_generation: generation }
                };
                if result.iter().flatten().any(|prior| prior.first == active.first && prior.second == active.second) {
                    return Err(Invalid::DoubleEnqueue);
                }
                result[count] = Some(active);
                count = count.checked_add(1).ok_or(Invalid::CounterOverflow)?;
            }
        }
    }
    Ok((count, result))
}

fn validate_worklist(
    slots: &[Slot],
    worklist: &ActiveWorklist,
    selected: Pair,
) -> Result<(), Invalid> {
    if worklist.entries.len() > slots.len() {
        return Err(Invalid::WorklistBound);
    }
    for (index, queued) in worklist.entries.iter().enumerate() {
        if queued.first >= queued.second {
            return Err(Invalid::PlanEncoding);
        }
        if queued.second as usize >= slots.len() {
            return Err(Invalid::StalePlanSlot);
        }
        if *queued == selected {
            return Err(Invalid::DoubleEnqueue);
        }
        if worklist.entries.iter().take(index).any(|prior| prior == queued) {
            return Err(Invalid::DoubleEnqueue);
        }
        if active_pair(slots, *queued).is_ok() {
            let first = slots.get(queued.first as usize).ok_or(Invalid::StalePlanSlot)?;
            let second = slots.get(queued.second as usize).ok_or(Invalid::StalePlanSlot)?;
            if first.queued_generation != Some(queued.first_generation)
                || second.queued_generation != Some(queued.second_generation)
            {
                return Err(Invalid::WorklistBound);
            }
            for prior in worklist.entries.iter().take(index) {
                if active_pair(slots, *prior).is_ok()
                    && (prior.first == queued.first
                        || prior.first == queued.second
                        || prior.second == queued.first
                        || prior.second == queued.second)
                {
                    return Err(Invalid::DoubleEnqueue);
                }
            }
        }
    }
    for (index, slot) in slots.iter().enumerate() {
        if let Some(generation) = slot.queued_generation {
            if generation != slot.generation {
                return Err(Invalid::WorklistBound);
            }
            let owner = u32::try_from(index).map_err(|_| Invalid::CounterOverflow)?;
            let references = worklist
                .entries
                .iter()
                .filter(|queued| {
                    (queued.first == owner && queued.first_generation == generation)
                        || (queued.second == owner && queued.second_generation == generation)
                })
                .count();
            if references != 1 {
                return Err(Invalid::WorklistBound);
            }
        }
    }
    for (index, slot) in slots.iter().enumerate() {
        let Some(agent) = slot.agent else { continue };
        let Some(Endpoint::Port(target)) = agent.ports[0] else { continue };
        if target.port_index != PortIndex::from(0) || index >= target.agent as usize {
            continue;
        }
        let source = u32::try_from(index).map_err(|_| Invalid::CounterOverflow)?;
        let Some(other) = slots.get(target.agent as usize) else {
            return Err(Invalid::StalePlanSlot);
        };
        if other.agent.is_none_or(|other_agent| {
            other_agent.ports[0]
                != Some(Endpoint::Port(PortRef {
                    agent: source,
                    port_index: PortIndex::from(0),
                }))
        }) {
            continue;
        }
        let pair = Pair {
            first: source,
            first_generation: slot.generation,
            second: target.agent,
            second_generation: other.generation,
        };
        let count = worklist.entries.iter().filter(|queued| **queued == pair).count();
        if pair == selected {
            if count != 0 {
                return Err(Invalid::DoubleEnqueue);
            }
        } else if count != 1 {
            return Err(if count > 1 {
                Invalid::DoubleEnqueue
            } else {
                Invalid::WorklistBound
            });
        }
    }
    Ok(())
}

/// Independently decode and check emitted bytes against the current arena.
#[cfg(test)]
pub(super) fn check(
    bytes: &PlanBytes,
    slots: &[Slot],
    outputs: &[PortRef],
    pair: Pair,
    policy: StoragePolicy,
) -> Result<CheckedPlan, Invalid> {
    let empty = ActiveWorklist {
        entries: alloc::vec::Vec::new(),
    };
    check_with_worklist(bytes, slots, outputs, &empty, pair, policy)
}

/// Independently decode and check emitted bytes against the arena and queue.
pub(super) fn check_with_worklist(
    bytes: &PlanBytes,
    slots: &[Slot],
    outputs: &[PortRef],
    worklist: &ActiveWorklist,
    pair: Pair,
    policy: StoragePolicy,
) -> Result<CheckedPlan, Invalid> {
    validate_worklist(slots, worklist, pair)?;
    let image = decode(bytes.as_bytes())?;
    validate_image(&image, slots, outputs, pair, policy)
}

/// Reserve and execute a checked plan as one unobservable transaction.
pub(super) fn execute(
    checked: CheckedPlan,
    slots: &mut alloc::vec::Vec<Slot>,
    outputs: &mut [PortRef],
    worklist: &mut ActiveWorklist,
    arena_limit: u64,
    scratch_limit: u64,
    queue_limit: u64,
) -> Result<Execution, ExecuteError> {
    execute_with_interrupt(
        checked,
        slots,
        outputs,
        worklist,
        arena_limit,
        scratch_limit,
        queue_limit,
        None,
    )
}

pub(super) fn execute_with_interrupt(
    checked: CheckedPlan,
    slots: &mut alloc::vec::Vec<Slot>,
    outputs: &mut [PortRef],
    worklist: &mut ActiveWorklist,
    arena_limit: u64,
    scratch_limit: u64,
    queue_limit: u64,
    interrupt_after: Option<&dyn Fn(u8) -> bool>,
) -> Result<Execution, ExecuteError> {
    execute_with_hooks(
        checked,
        slots,
        outputs,
        worklist,
        arena_limit,
        scratch_limit,
        queue_limit,
        interrupt_after,
        None,
    )
}

#[cfg(test)]
pub(super) fn execute_with_reservation_failure(
    checked: CheckedPlan,
    slots: &mut alloc::vec::Vec<Slot>,
    outputs: &mut [PortRef],
    worklist: &mut ActiveWorklist,
    arena_limit: u64,
    scratch_limit: u64,
    queue_limit: u64,
    fail_after_reservation: u8,
) -> Result<Execution, ExecuteError> {
    execute_with_hooks(
        checked,
        slots,
        outputs,
        worklist,
        arena_limit,
        scratch_limit,
        queue_limit,
        None,
        Some(fail_after_reservation),
    )
}

fn execute_with_hooks(
    checked: CheckedPlan,
    slots: &mut alloc::vec::Vec<Slot>,
    outputs: &mut [PortRef],
    worklist: &mut ActiveWorklist,
    arena_limit: u64,
    scratch_limit: u64,
    queue_limit: u64,
    interrupt_after: Option<&dyn Fn(u8) -> bool>,
    reservation_failure_after: Option<u8>,
) -> Result<Execution, ExecuteError> {
    if u64::from(checked.scratch_reservation) > scratch_limit {
        return Err(ExecuteError::Limited {
            limit: super::Limit::PlanScratch,
            cap: scratch_limit,
            observed: u64::from(checked.scratch_reservation),
        });
    }
    let required_slots = u64::try_from(slots.len())
        .map_err(|_| ExecuteError::Invalid(Invalid::CounterOverflow))?
        .checked_add(u64::from(checked.arena_reservation))
        .ok_or(ExecuteError::Invalid(Invalid::CounterOverflow))?;
    if required_slots > arena_limit {
        return Err(ExecuteError::Limited {
            limit: super::Limit::ArenaReservation,
            cap: arena_limit,
            observed: required_slots,
        });
    }
    let required_queue = u64::try_from(worklist.entries.len())
        .map_err(|_| ExecuteError::Invalid(Invalid::CounterOverflow))?
        .checked_add(u64::from(checked.queue_reservation))
        .ok_or(ExecuteError::Invalid(Invalid::CounterOverflow))?;
    if required_queue > queue_limit {
        return Err(ExecuteError::Limited {
            limit: super::Limit::WorklistReservation,
            cap: queue_limit,
            observed: required_queue,
        });
    }
    let old_slot_capacity = slots.capacity();
    let old_queue_capacity = worklist.entries.capacity();
    if reservation_failure_after == Some(0) {
        return Err(ExecuteError::Invalid(Invalid::PlanReservationFailure));
    }
    let image = checked.image;
    let old_slot_len = slots.len();
    let old_queue_len = worklist.entries.len();
    let live_before = u32::try_from(slots.iter().filter(|slot| slot.agent.is_some()).count())
        .map_err(|_| ExecuteError::Invalid(Invalid::CounterOverflow))?;
    let transient_live_nodes = live_before
        .checked_add(checked.arena_reservation)
        .ok_or(ExecuteError::Invalid(Invalid::CounterOverflow))?;
    let mut backups: [Option<(usize, Slot)>; 4] = [None; 4];
    let mut backup_count = 0usize;
    let mut output_backups: [Option<(usize, PortRef)>; MAX_BOUNDARIES] = [None; MAX_BOUNDARIES];
    let mut output_backup_count = 0usize;
    for id in [image.pair.first, image.pair.second] {
        save_slot(slots, id as usize, &mut backups, &mut backup_count)?;
    }
    for boundary in image.boundaries.iter().take(usize::from(image.boundary_count)).flatten() {
        match boundary.external {
            Endpoint::Port(target) => save_slot(slots, target.agent as usize, &mut backups, &mut backup_count)?,
            Endpoint::Output(output) => {
                let index = output as usize;
                if !output_backups.iter().take(output_backup_count).any(|saved| saved.is_some_and(|(saved_index, _)| saved_index == index)) {
                    let old = *outputs.get(index).ok_or(ExecuteError::Invalid(Invalid::OutputBoundary))?;
                    output_backups[output_backup_count] = Some((index, old));
                    output_backup_count = output_backup_count.checked_add(1).ok_or(ExecuteError::Invalid(Invalid::CounterOverflow))?;
                }
            }
        }
    }
    if slots
        .try_reserve(usize::try_from(checked.arena_reservation).map_err(|_| {
            ExecuteError::Invalid(Invalid::CounterOverflow)
        })?)
        .is_err()
    {
        return Err(ExecuteError::Invalid(Invalid::PlanReservationFailure));
    }
    if reservation_failure_after == Some(1) {
        slots.shrink_to(old_slot_capacity);
        return Err(ExecuteError::Invalid(Invalid::PlanReservationFailure));
    }
    if worklist
        .entries
        .try_reserve(usize::try_from(checked.queue_reservation).map_err(|_| {
            ExecuteError::Invalid(Invalid::CounterOverflow)
        })?)
        .is_err()
    {
        slots.shrink_to(old_slot_capacity);
        worklist.entries.shrink_to(old_queue_capacity);
        return Err(ExecuteError::Invalid(Invalid::PlanReservationFailure));
    }
    if reservation_failure_after == Some(2) {
        slots.shrink_to(old_slot_capacity);
        worklist.entries.shrink_to(old_queue_capacity);
        return Err(ExecuteError::Invalid(Invalid::PlanReservationFailure));
    }
    if let Err(invalid) = apply_image(slots, outputs, worklist, &image, interrupt_after) {
        rollback(
            slots,
            outputs,
            worklist,
            old_slot_len,
            old_queue_len,
            &backups,
            backup_count,
            &output_backups,
            output_backup_count,
        );
        slots.shrink_to(old_slot_capacity);
        worklist.entries.shrink_to(old_queue_capacity);
        return Err(ExecuteError::Invalid(invalid));
    }
    Ok(Execution {
        live_decrease: checked.live_decrease,
        transient_live_nodes,
        reused_slots: checked.reused_slots,
        freed_slots: checked.freed_slots,
        allocated_slots: checked.arena_reservation,
        microsteps: 6,
    })
}

fn save_slot(
    slots: &[Slot],
    index: usize,
    backups: &mut [Option<(usize, Slot)>; 4],
    count: &mut usize,
) -> Result<(), ExecuteError> {
    if backups.iter().take(*count).any(|saved| saved.is_some_and(|(saved_index, _)| saved_index == index)) {
        return Ok(());
    }
    let slot = *slots.get(index).ok_or(ExecuteError::Invalid(Invalid::StalePlanSlot))?;
    *backups.get_mut(*count).ok_or(ExecuteError::Invalid(Invalid::PlanBoundaryAlias))? = Some((index, slot));
    *count = count.checked_add(1).ok_or(ExecuteError::Invalid(Invalid::CounterOverflow))?;
    Ok(())
}

fn rollback(
    slots: &mut alloc::vec::Vec<Slot>,
    outputs: &mut [PortRef],
    worklist: &mut ActiveWorklist,
    old_slot_len: usize,
    old_queue_len: usize,
    backups: &[Option<(usize, Slot)>; 4],
    backup_count: usize,
    output_backups: &[Option<(usize, PortRef)>; MAX_BOUNDARIES],
    output_backup_count: usize,
) {
    for (index, slot) in backups.iter().take(backup_count).flatten() {
        if let Some(destination) = slots.get_mut(*index) {
            *destination = *slot;
        }
    }
    for (index, output) in output_backups.iter().take(output_backup_count).flatten() {
        if let Some(destination) = outputs.get_mut(*index) {
            *destination = *output;
        }
    }
    slots.truncate(old_slot_len);
    worklist.entries.truncate(old_queue_len);
}

fn apply_image(
    slots: &mut alloc::vec::Vec<Slot>,
    outputs: &mut [PortRef],
    worklist: &mut ActiveWorklist,
    image: &Image,
    interrupt_after: Option<&dyn Fn(u8) -> bool>,
) -> Result<(), Invalid> {
    let mut changed = [None; 2];
    for (index, result) in image.results.iter().take(usize::from(image.result_count)).enumerate() {
        let result = result.ok_or(Invalid::PlanEncoding)?;
        let donor = slot_agent(slots, result.donor_slot)?;
        let replacement = RuntimeAgent { origin: donor.origin, symbol: result.symbol, ports: [None; 3] };
        if result.fresh {
            if result.slot as usize != slots.len() {
                return Err(Invalid::PlanMismatch);
            }
            slots.push(Slot { agent: Some(replacement), generation: 0, queued_generation: None });
        } else {
            let slot = slots.get_mut(result.slot as usize).ok_or(Invalid::StalePlanSlot)?;
            slot.generation = slot.generation.checked_add(1).ok_or(Invalid::CounterOverflow)?;
            slot.queued_generation = None;
            slot.agent = Some(replacement);
        }
        changed[index] = Some(result.slot);
    }
    interrupt(interrupt_after, 1)?;
    for result in image.results.iter().take(usize::from(image.result_count)).flatten() {
        let agent = slots
            .get_mut(result.slot as usize)
            .and_then(|slot| slot.agent.as_mut())
            .ok_or(Invalid::StalePlanSlot)?;
        for (port_index, boundary_index) in result.ports.iter().enumerate() {
            if let Some(boundary_index) = boundary_index {
                agent.ports[port_index] = Some(
                    image.boundaries[usize::from(*boundary_index)]
                        .ok_or(Invalid::PlanEncoding)?
                        .external,
                );
            }
        }
    }
    interrupt(interrupt_after, 2)?;
    for write in image.external_writes.iter().take(usize::from(image.external_write_count)).flatten() {
        let result = image.results[usize::from(write.result)].ok_or(Invalid::PlanEncoding)?;
        let target = image.boundaries[usize::from(write.boundary)].ok_or(Invalid::PlanEncoding)?;
        let new_port = PortRef { agent: result.slot, port_index: PortIndex::from(write.port) };
        match target.external {
            Endpoint::Port(peer) => {
                let slot = slots
                    .get_mut(peer.agent as usize)
                    .ok_or(Invalid::StalePlanSlot)?;
                if peer.port_index == PortIndex::from(0) {
                    slot.queued_generation = None;
                }
                let agent = slot.agent.as_mut().ok_or(Invalid::StalePlanSlot)?;
                let endpoint = agent.ports.get_mut(usize::from(peer.port_index)).ok_or(Invalid::PortArity)?;
                if *endpoint != Some(Endpoint::Port(target.source)) {
                    return Err(Invalid::NonReciprocal);
                }
                *endpoint = Some(Endpoint::Port(new_port));
            }
            Endpoint::Output(output) => {
                let entry = outputs.get_mut(output as usize).ok_or(Invalid::OutputBoundary)?;
                if *entry != target.source {
                    return Err(Invalid::OutputBoundary);
                }
                *entry = new_port;
            }
        }
    }
    interrupt(interrupt_after, 3)?;
    for id in [image.pair.first, image.pair.second] {
        let retained = image.results.iter().take(usize::from(image.result_count)).flatten().any(|result| !result.fresh && result.slot == id);
        if !retained {
            let slot = slots.get_mut(id as usize).ok_or(Invalid::StalePlanSlot)?;
            slot.generation = slot.generation.checked_add(1).ok_or(Invalid::CounterOverflow)?;
            slot.queued_generation = None;
            slot.agent = None;
        }
    }
    interrupt(interrupt_after, 4)?;
    let start_queue_len = worklist.entries.len();
    for changed_slot in changed.into_iter().flatten() {
        super::enqueue_active(slots, worklist, changed_slot as usize)?;
    }
    let published = worklist.entries.len().checked_sub(start_queue_len).ok_or(Invalid::CounterOverflow)?;
    if published != usize::from(image.publication_count) {
        return Err(Invalid::DoubleEnqueue);
    }
    for (actual, expected) in worklist.entries.iter().skip(start_queue_len).zip(image.publications.iter().take(usize::from(image.publication_count))) {
        let expected = expected.ok_or(Invalid::PlanEncoding)?;
        if actual.first != expected.first || actual.second != expected.second
            || actual.first_generation != expected.first_generation
            || actual.second_generation != expected.second_generation
        {
            return Err(Invalid::DoubleEnqueue);
        }
    }
    interrupt(interrupt_after, 5)?;
    interrupt(interrupt_after, 6)?;
    Ok(())
}

fn interrupt(hook: Option<&dyn Fn(u8) -> bool>, step: u8) -> Result<(), Invalid> {
    if hook.is_some_and(|should_interrupt| should_interrupt(step)) {
        Err(Invalid::PlanInterrupted)
    } else {
        Ok(())
    }
}

fn apply(op: Op, first: u64, second: u64) -> u64 {
    match op {
        Op::Add => first.wrapping_add(second),
        Op::Sub => first.wrapping_sub(second),
        Op::Mul => first.wrapping_mul(second),
        Op::Eq => u64::from(first == second),
    }
}

