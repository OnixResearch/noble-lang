//! The checking environment: one scheme per definition, its behavior kind,
//! and the supplied effect-identity table.
//!
//! Contracts are rank-1 schemes; the bootstrap table lives in `bootstrap`.

pub mod bootstrap;

/// Identity of one environment definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Definition(pub u32);

/// The reserved identity of the resource-free `test.emit` effect.
pub const TEST_EMIT: crate::types::EffId = crate::types::EffId(0);

/// The reserved resource kind used by negative eligibility fixtures.
pub const FIXTURE_RESOURCE: crate::types::ResourceKind = crate::types::ResourceKind(0);

/// What a definition's contract constrains beyond its scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behavior {
    /// `dup`: requires `Data` on its value variable.
    Dup,
    /// `drop`: requires `Data` on its value variable.
    Drop,
    /// `swap`.
    Swap,
    /// `dip`.
    Dip,
    /// `+`, `-`, or `*`; wrapping `I64`.
    Arith,
    /// `quote`: requires `Data` on its captured variable.
    Quote,
    /// `compose`.
    Compose,
    /// `run`.
    Run,
    /// `reflect`.
    Reflect,
    /// `unit`.
    Unit,
    /// `pair`.
    Pair,
    /// `unpair`.
    Unpair,
    /// `inl`.
    Inl,
    /// `inr`.
    Inr,
    /// `case`.
    Case,
    /// `if`.
    If,
    /// `nil`.
    Nil,
    /// `cons`.
    Cons,
    /// `list.case`.
    ListCase,
    /// The supplied `test.emit` operation.
    TestEmit,
    /// An environment-supplied named definition with no extra constraint.
    Named,
}

/// The checking environment.
#[derive(Clone, Debug)]
pub struct Env {
    /// Schemes indexed by `Definition`.
    pub defs: alloc::vec::Vec<crate::words::Scheme>,
    /// Behavior kinds indexed by `Definition`.
    pub kinds: alloc::vec::Vec<Behavior>,
    /// Effect identities this environment provides; `TEST_EMIT` is first.
    pub effects: alloc::vec::Vec<crate::types::EffId>,
}

impl Env {
    /// The scheme of a definition.
    pub fn scheme(&self, def: Definition) -> Option<&crate::words::Scheme> {
        match index(def) {
            Some(index) => self.defs.get(index),
            None => None,
        }
    }

    /// The behavior of a definition.
    pub fn kind(&self, def: Definition) -> Option<Behavior> {
        match index(def) {
            Some(index) => self.kinds.get(index).copied(),
            None => None,
        }
    }

    /// Whether the environment provides this effect identity.
    pub fn knows_effect(&self, id: crate::types::EffId) -> bool {
        let mut index = 0;
        let mut is_known = false;
        while index < self.effects.len() {
            if self.effects[index] == id {
                is_known = true;
                break;
            }
            index += 1;
        }
        is_known
    }

    /// Number of definitions.
    pub fn len(&self) -> Option<u64> {
        u64::try_from(self.defs.len()).ok()
    }

    /// Whether the environment has no definitions.
    pub fn is_empty(&self) -> bool {
        self.defs.len() == 0
    }
}

fn index(def: Definition) -> Option<usize> {
    usize::try_from(def.0).ok()
}

/// Build the bootstrap environment with the fixed v0 contract table.
pub fn environment() -> Result<Env, crate::shapes::Defect> {
    let table = bootstrap::data::table();
    let mut defs: alloc::vec::Vec<crate::words::Scheme> = alloc::vec::Vec::with_capacity(32);
    let mut kinds: alloc::vec::Vec<Behavior> = alloc::vec::Vec::with_capacity(32);
    let mut index = 0;
    let mut defect: Option<crate::shapes::Defect> = None;
    while index < table.len() {
        match table[index].1.validate() {
            Ok(()) => {
                kinds.push(table[index].0);
                index += 1;
            }
            Err(problem) => {
                defect = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = defect {
        return Err(problem);
    }
    let mut table_index = 0;
    while table_index < table.len() {
        defs.push(table[table_index].1.clone());
        table_index += 1;
    }
    Ok(Env {
        defs,
        kinds,
        effects: alloc::vec![TEST_EMIT],
    })
}
