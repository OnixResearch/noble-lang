//! The bootstrap checking environment: the fixed word-contract table, each
//! definition's kind, and the supplied effect-identity table.
//!
//! Contracts are rank-1 schemes; the table follows `.cairn/specs/language`
//! sections 6-7 and the fragment definition in `verification/m2-fragment.md`.

use crate::scheme::{EffSlot, PItem, PSig, PTy, Scheme, SchemeError, VarId, VarKind};
use crate::types::{EffId, ResourceKind};
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

const PTY_UNIT: PTy = PTy::Unit;
const PTY_BOOL: PTy = PTy::Bool;
const PTY_I64: PTy = PTy::I64;
const PTY_TEXT: PTy = PTy::Text;
const PTY_SYNTAX: PTy = PTy::Syntax;

/// Identity of one environment definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefId(pub u32);

/// The reserved identity of the resource-free `test.emit` effect.
pub const TEST_EMIT: EffId = EffId(0);

/// The reserved resource kind used by negative eligibility fixtures.
pub const FIXTURE_RESOURCE: ResourceKind = ResourceKind(0);

/// What a definition's contract constrains beyond its scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordKind {
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

/// The checking environment: one scheme per definition plus its kind, and the
/// concrete effect identities the environment provides.
#[derive(Clone, Debug)]
pub struct Env {
    /// Schemes indexed by `DefId`.
    pub defs: Vec<Scheme>,
    /// Kinds indexed by `DefId`.
    pub kinds: Vec<WordKind>,
    /// Effect identities this environment provides; `TEST_EMIT` is always first.
    pub effects: Vec<EffId>,
}

impl Env {
    /// The scheme of a definition.
    pub fn scheme(&self, def: DefId) -> Option<&Scheme> {
        self.defs.get(def.0 as usize)
    }

    /// The kind of a definition.
    pub fn kind(&self, def: DefId) -> Option<WordKind> {
        self.kinds.get(def.0 as usize).copied()
    }

    /// Whether the environment provides this effect identity.
    pub fn knows_effect(&self, id: EffId) -> bool {
        self.effects.contains(&id)
    }

    /// Number of definitions.
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    /// Whether the environment has no definitions.
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

fn stack_var(index: u32) -> PItem {
    PItem::Stack(VarId(index))
}

fn value_var(index: u32) -> PItem {
    PItem::Ty(PTy::Var(VarId(index)))
}

fn effect_var(index: u32) -> EffSlot {
    EffSlot::Var(VarId(index))
}

fn program(item_in: Vec<PItem>, item_out: Vec<PItem>, effects: Vec<EffSlot>) -> PItem {
    PItem::Ty(PTy::Program(Box::new(PSig {
        stack_in: item_in,
        stack_out: item_out,
        effects,
    })))
}

fn scheme(
    vars: Vec<VarKind>,
    item_in: Vec<PItem>,
    item_out: Vec<PItem>,
    effects: Vec<EffSlot>,
) -> Scheme {
    Scheme {
        var_kinds: vars,
        stack_in: item_in,
        stack_out: item_out,
        effects,
    }
}

/// Build the bootstrap environment with the fixed v0 contract table.
///
/// The table order fixes each builtin's `DefId`; named environment
/// definitions are appended after it.
pub fn bootstrap_environment() -> Result<Env, SchemeError> {
    let mut defs: Vec<Scheme> = Vec::new();
    let mut kinds: Vec<WordKind> = Vec::new();

    let mut push = |kind: WordKind, s: Scheme| -> Result<(), SchemeError> {
        s.validate()?;
        defs.push(s);
        kinds.push(kind);
        Ok(())
    };

    // dup : S a -- S a a
    push(
        WordKind::Dup,
        scheme(
            vec![VarKind::Stack, VarKind::Value],
            vec![stack_var(0), value_var(1)],
            vec![stack_var(0), value_var(1), value_var(1)],
            vec![],
        ),
    )?;
    // drop : S a -- S
    push(
        WordKind::Drop,
        scheme(
            vec![VarKind::Stack, VarKind::Value],
            vec![stack_var(0), value_var(1)],
            vec![stack_var(0)],
            vec![],
        ),
    )?;
    // swap : S a b -- S b a
    push(
        WordKind::Swap,
        scheme(
            vec![VarKind::Stack, VarKind::Value, VarKind::Value],
            vec![stack_var(0), value_var(1), value_var(2)],
            vec![stack_var(0), value_var(2), value_var(1)],
            vec![],
        ),
    )?;
    // dip : S a Program<S,T,e> -- T a ! e
    push(
        WordKind::Dip,
        scheme(
            vec![
                VarKind::Stack,
                VarKind::Value,
                VarKind::Stack,
                VarKind::Effect,
            ],
            vec![
                stack_var(0),
                value_var(1),
                program(vec![stack_var(0)], vec![stack_var(2)], vec![effect_var(3)]),
            ],
            vec![stack_var(2), value_var(1)],
            vec![effect_var(3)],
        ),
    )?;
    // + - * : S I64 I64 -- S I64 ! {}  (wrapping; three definitions share the shape)
    push(
        WordKind::Arith,
        scheme(
            vec![VarKind::Stack],
            vec![stack_var(0), PItem::Ty(PTY_I64), PItem::Ty(PTY_I64)],
            vec![stack_var(0), PItem::Ty(PTY_I64)],
            vec![],
        ),
    )?;
    push(
        WordKind::Arith,
        scheme(
            vec![VarKind::Stack],
            vec![stack_var(0), PItem::Ty(PTY_I64), PItem::Ty(PTY_I64)],
            vec![stack_var(0), PItem::Ty(PTY_I64)],
            vec![],
        ),
    )?;
    push(
        WordKind::Arith,
        scheme(
            vec![VarKind::Stack],
            vec![stack_var(0), PItem::Ty(PTY_I64), PItem::Ty(PTY_I64)],
            vec![stack_var(0), PItem::Ty(PTY_I64)],
            vec![],
        ),
    )?;
    // quote : R a -- R Program<S, S a, {}> ! {}
    push(
        WordKind::Quote,
        scheme(
            vec![VarKind::Stack, VarKind::Value, VarKind::Stack],
            vec![stack_var(0), value_var(1)],
            vec![
                stack_var(0),
                program(vec![stack_var(2)], vec![stack_var(2), value_var(1)], vec![]),
            ],
            vec![],
        ),
    )?;
    // compose : R Program<A,B,e> Program<B,C,f> -- R Program<A,C,union(e,f)> ! {}
    push(
        WordKind::Compose,
        scheme(
            vec![
                VarKind::Stack,
                VarKind::Stack,
                VarKind::Stack,
                VarKind::Stack,
                VarKind::Effect,
                VarKind::Effect,
            ],
            vec![
                stack_var(0),
                program(vec![stack_var(1)], vec![stack_var(2)], vec![effect_var(4)]),
                program(vec![stack_var(2)], vec![stack_var(3)], vec![effect_var(5)]),
            ],
            vec![
                stack_var(0),
                program(
                    vec![stack_var(1)],
                    vec![stack_var(3)],
                    vec![effect_var(4), effect_var(5)],
                ),
            ],
            vec![],
        ),
    )?;
    // run : S Program<S,T,e> -- T ! e
    push(
        WordKind::Run,
        scheme(
            vec![VarKind::Stack, VarKind::Stack, VarKind::Effect],
            vec![
                stack_var(0),
                program(vec![stack_var(0)], vec![stack_var(1)], vec![effect_var(2)]),
            ],
            vec![stack_var(1)],
            vec![effect_var(2)],
        ),
    )?;
    // reflect : R Program<S,T,e> -- R Syntax ! {}
    push(
        WordKind::Reflect,
        scheme(
            vec![
                VarKind::Stack,
                VarKind::Stack,
                VarKind::Stack,
                VarKind::Effect,
            ],
            vec![
                stack_var(0),
                program(vec![stack_var(1)], vec![stack_var(2)], vec![effect_var(3)]),
            ],
            vec![stack_var(0), PItem::Ty(PTY_SYNTAX)],
            vec![],
        ),
    )?;
    // unit : S -- S Unit ! {}
    push(
        WordKind::Unit,
        scheme(
            vec![VarKind::Stack],
            vec![stack_var(0)],
            vec![stack_var(0), PItem::Ty(PTY_UNIT)],
            vec![],
        ),
    )?;
    // pair : S a b -- S Pair<a,b> ! {}
    push(
        WordKind::Pair,
        scheme(
            vec![VarKind::Stack, VarKind::Value, VarKind::Value],
            vec![stack_var(0), value_var(1), value_var(2)],
            vec![
                stack_var(0),
                PItem::Ty(PTy::Pair(
                    Box::new(PTy::Var(VarId(1))),
                    Box::new(PTy::Var(VarId(2))),
                )),
            ],
            vec![],
        ),
    )?;
    // unpair : S Pair<a,b> -- S a b ! {}
    push(
        WordKind::Unpair,
        scheme(
            vec![VarKind::Stack, VarKind::Value, VarKind::Value],
            vec![
                stack_var(0),
                PItem::Ty(PTy::Pair(
                    Box::new(PTy::Var(VarId(1))),
                    Box::new(PTy::Var(VarId(2))),
                )),
            ],
            vec![stack_var(0), value_var(1), value_var(2)],
            vec![],
        ),
    )?;
    // inl : S a -- S Sum<a,b> ! {}
    push(
        WordKind::Inl,
        scheme(
            vec![VarKind::Stack, VarKind::Value, VarKind::Value],
            vec![stack_var(0), value_var(1)],
            vec![
                stack_var(0),
                PItem::Ty(PTy::Sum(
                    Box::new(PTy::Var(VarId(1))),
                    Box::new(PTy::Var(VarId(2))),
                )),
            ],
            vec![],
        ),
    )?;
    // inr : S b -- S Sum<a,b> ! {}
    push(
        WordKind::Inr,
        scheme(
            vec![VarKind::Stack, VarKind::Value, VarKind::Value],
            vec![stack_var(0), value_var(2)],
            vec![
                stack_var(0),
                PItem::Ty(PTy::Sum(
                    Box::new(PTy::Var(VarId(1))),
                    Box::new(PTy::Var(VarId(2))),
                )),
            ],
            vec![],
        ),
    )?;
    // case : S Sum<a,b> Program<S a,T,e> Program<S b,T,f> -- T ! union(e,f)
    push(
        WordKind::Case,
        scheme(
            vec![
                VarKind::Stack,
                VarKind::Value,
                VarKind::Value,
                VarKind::Stack,
                VarKind::Effect,
                VarKind::Effect,
            ],
            vec![
                stack_var(0),
                PItem::Ty(PTy::Sum(
                    Box::new(PTy::Var(VarId(1))),
                    Box::new(PTy::Var(VarId(2))),
                )),
                program(
                    vec![stack_var(0), value_var(1)],
                    vec![stack_var(3)],
                    vec![effect_var(4)],
                ),
                program(
                    vec![stack_var(0), value_var(2)],
                    vec![stack_var(3)],
                    vec![effect_var(5)],
                ),
            ],
            vec![stack_var(3)],
            vec![effect_var(4), effect_var(5)],
        ),
    )?;
    // if : S Bool Program<S,T,e> Program<S,T,f> -- T ! union(e,f)
    push(
        WordKind::If,
        scheme(
            vec![
                VarKind::Stack,
                VarKind::Stack,
                VarKind::Effect,
                VarKind::Effect,
            ],
            vec![
                stack_var(0),
                PItem::Ty(PTY_BOOL),
                program(vec![stack_var(0)], vec![stack_var(1)], vec![effect_var(2)]),
                program(vec![stack_var(0)], vec![stack_var(1)], vec![effect_var(3)]),
            ],
            vec![stack_var(1)],
            vec![effect_var(2), effect_var(3)],
        ),
    )?;
    // nil : S -- S List<a> ! {}
    push(
        WordKind::Nil,
        scheme(
            vec![VarKind::Stack, VarKind::Value],
            vec![stack_var(0)],
            vec![
                stack_var(0),
                PItem::Ty(PTy::List(Box::new(PTy::Var(VarId(1))))),
            ],
            vec![],
        ),
    )?;
    // cons : S a List<a> -- S List<a> ! {}
    push(
        WordKind::Cons,
        scheme(
            vec![VarKind::Stack, VarKind::Value],
            vec![
                stack_var(0),
                value_var(1),
                PItem::Ty(PTy::List(Box::new(PTy::Var(VarId(1))))),
            ],
            vec![
                stack_var(0),
                PItem::Ty(PTy::List(Box::new(PTy::Var(VarId(1))))),
            ],
            vec![],
        ),
    )?;
    // list.case : S List<a> Program<S,T,e> Program<S a List<a>,T,f> -- T ! union(e,f)
    push(
        WordKind::ListCase,
        scheme(
            vec![
                VarKind::Stack,
                VarKind::Value,
                VarKind::Stack,
                VarKind::Effect,
                VarKind::Effect,
            ],
            vec![
                stack_var(0),
                PItem::Ty(PTy::List(Box::new(PTy::Var(VarId(1))))),
                program(vec![stack_var(0)], vec![stack_var(2)], vec![effect_var(3)]),
                program(
                    vec![
                        stack_var(0),
                        value_var(1),
                        PItem::Ty(PTy::List(Box::new(PTy::Var(VarId(1))))),
                    ],
                    vec![stack_var(2)],
                    vec![effect_var(4)],
                ),
            ],
            vec![stack_var(2)],
            vec![effect_var(3), effect_var(4)],
        ),
    )?;
    // test.emit : S Text -- S Unit ! {test.emit}
    push(
        WordKind::TestEmit,
        scheme(
            vec![VarKind::Stack],
            vec![stack_var(0), PItem::Ty(PTY_TEXT)],
            vec![stack_var(0), PItem::Ty(PTY_UNIT)],
            vec![EffSlot::Id(TEST_EMIT)],
        ),
    )?;

    Ok(Env {
        defs,
        kinds,
        effects: alloc::vec![TEST_EMIT],
    })
}
