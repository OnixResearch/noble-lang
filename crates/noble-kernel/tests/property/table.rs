//! The oracle's contract table and binding accessors (DX-PROPERTY-01).
//!
//! One arm per word of the documented table in [verification/m2-fragment.md]
//! "Environment contracts": stack variables join as prefixes, value
//! variables fill single positions, and effect variables carry latent
//! bounds. `None` rejects — the kernel's instantiation-kind, arity, and
//! unknown-word paths.

use noble_kernel::words::{Binding, Inst};

use super::otypes::{union, OTy};

/// One node's derived interface under the oracle's rules.
pub struct OFace {
    pub input: Vec<OTy>,
    pub output: Vec<OTy>,
    pub latent: Vec<u32>,
}

/// The stack bound to one stack variable, if the binding has that kind.
pub fn stack_at(inst: &Inst, index: usize) -> Option<Vec<OTy>> {
    match inst.bindings.get(index) {
        Some(Binding::Stack(segment)) => Some(super::otypes::oty_stack(segment)),
        _ => None,
    }
}

/// The type bound to one value variable, if the binding has that kind.
pub fn value_at(inst: &Inst, index: usize) -> Option<OTy> {
    match inst.bindings.get(index) {
        Some(Binding::Value(ty)) => Some(super::otypes::oty(ty)),
        _ => None,
    }
}

/// The identities bound to one effect variable, if the binding has that kind.
pub fn effects_at(inst: &Inst, index: usize) -> Option<Vec<u32>> {
    match inst.bindings.get(index) {
        Some(Binding::Effect(set)) => Some(set.as_slice().iter().map(|id| id.0).collect()),
        _ => None,
    }
}

/// Append one tail to a copied stack.
pub fn append(mut stack: Vec<OTy>, tail: &[OTy]) -> Vec<OTy> {
    stack.extend_from_slice(tail);
    stack
}

/// Whether the word places a `Data` side condition on its value variable.
pub fn requires_data(def: u32) -> bool {
    def == 0 || def == 1 || def == 8
}

/// Whether the witness carries exactly the wanted number of bindings.
pub fn exact_arity(inst: &Inst, wanted: usize) -> Option<()> {
    if inst.bindings.len() == wanted {
        Some(())
    } else {
        None
    }
}

/// The documented contract table, one arm per word; `None` rejects.
pub fn word_face(def: u32, inst: &Inst) -> Option<OFace> {
    // The documented table fixes each word's variable count; a witness with
    // missing or extra bindings cannot instantiate it.
    exact_arity(
        inst,
        match def {
            0 | 1 | 19 | 20 => 2,
            2 | 8 | 10 | 13 | 14 | 15 | 16 => 3,
            3 | 11 | 18 => 4,
            4..=7 | 12 | 22 | 23 => 1,
            9 | 17 => 6,
            21 => 5,
            _ => return None,
        },
    )?;
    let pair = |a: OTy, b: OTy| OTy::Pair(Box::new(a), Box::new(b));
    let sum = |a: OTy, b: OTy| OTy::Sum(Box::new(a), Box::new(b));
    let list = |a: OTy| OTy::List(Box::new(a));
    match def {
        0 => {
            let (s, a) = (stack_at(inst, 0)?, value_at(inst, 1)?);
            Some(OFace {
                input: append(s.clone(), std::slice::from_ref(&a)),
                output: append(s, &[a.clone(), a]),
                latent: vec![],
            })
        }
        1 => {
            let (s, a) = (stack_at(inst, 0)?, value_at(inst, 1)?);
            Some(OFace {
                input: append(s.clone(), &[a]),
                output: s,
                latent: vec![],
            })
        }
        2 => {
            let (s, a, b) = (stack_at(inst, 0)?, value_at(inst, 1)?, value_at(inst, 2)?);
            Some(OFace {
                input: append(s.clone(), &[a, b.clone()]),
                output: append(s, &[b, value_at(inst, 1)?]),
                latent: vec![],
            })
        }
        3 => {
            let (s, a, t, e) = (
                stack_at(inst, 0)?,
                value_at(inst, 1)?,
                stack_at(inst, 2)?,
                effects_at(inst, 3)?,
            );
            let p = OTy::Program(s.clone(), t.clone(), e.clone());
            Some(OFace {
                input: append(s, &[a.clone(), p]),
                output: append(t, &[a]),
                latent: e,
            })
        }
        4..=6 => {
            let s = stack_at(inst, 0)?;
            Some(OFace {
                input: append(s.clone(), &[OTy::I64, OTy::I64]),
                output: append(s, &[OTy::I64]),
                latent: vec![],
            })
        }
        7 => {
            let s = stack_at(inst, 0)?;
            Some(OFace {
                input: append(s.clone(), &[OTy::I64, OTy::I64]),
                output: append(s, &[OTy::Bool]),
                latent: vec![],
            })
        }
        8 => {
            let (r, a, s) = (stack_at(inst, 0)?, value_at(inst, 1)?, stack_at(inst, 2)?);
            let p = OTy::Program(
                s.clone(),
                append(s.clone(), std::slice::from_ref(&a)),
                vec![],
            );
            Some(OFace {
                input: append(r.clone(), &[a]),
                output: append(r, &[p]),
                latent: vec![],
            })
        }
        9 => {
            let (r, a, b, c, e, f) = (
                stack_at(inst, 0)?,
                stack_at(inst, 1)?,
                stack_at(inst, 2)?,
                stack_at(inst, 3)?,
                effects_at(inst, 4)?,
                effects_at(inst, 5)?,
            );
            let p1 = OTy::Program(a.clone(), b.clone(), e.clone());
            let p2 = OTy::Program(b, c.clone(), f.clone());
            let p = OTy::Program(a, c, union(&e, &f));
            Some(OFace {
                input: append(r, &[p1, p2]),
                output: append(stack_at(inst, 0)?, &[p]),
                latent: vec![],
            })
        }
        10 => {
            let (s, t, e) = (stack_at(inst, 0)?, stack_at(inst, 1)?, effects_at(inst, 2)?);
            let p = OTy::Program(s.clone(), t.clone(), e.clone());
            Some(OFace {
                input: append(s, &[p]),
                output: t,
                latent: e,
            })
        }
        11 => {
            let (r, a, b, e) = (
                stack_at(inst, 0)?,
                stack_at(inst, 1)?,
                stack_at(inst, 2)?,
                effects_at(inst, 3)?,
            );
            let p = OTy::Program(a, b, e);
            Some(OFace {
                input: append(r.clone(), &[p]),
                output: append(r, &[OTy::Syntax]),
                latent: vec![],
            })
        }
        12 => {
            let s = stack_at(inst, 0)?;
            Some(OFace {
                input: s.clone(),
                output: append(s, &[OTy::Unit]),
                latent: vec![],
            })
        }
        13 => {
            let (s, a, b) = (stack_at(inst, 0)?, value_at(inst, 1)?, value_at(inst, 2)?);
            Some(OFace {
                input: append(s.clone(), &[a, b.clone()]),
                output: append(s, &[pair(value_at(inst, 1)?, b)]),
                latent: vec![],
            })
        }
        14 => {
            let (s, a, b) = (stack_at(inst, 0)?, value_at(inst, 1)?, value_at(inst, 2)?);
            let joined = pair(a.clone(), b.clone());
            Some(OFace {
                input: append(s.clone(), &[joined]),
                output: append(s, &[a, b]),
                latent: vec![],
            })
        }
        15 => {
            let (s, a, b) = (stack_at(inst, 0)?, value_at(inst, 1)?, value_at(inst, 2)?);
            Some(OFace {
                input: append(s.clone(), &[a]),
                output: append(s, &[sum(value_at(inst, 1)?, b)]),
                latent: vec![],
            })
        }
        16 => {
            let (s, a, b) = (stack_at(inst, 0)?, value_at(inst, 1)?, value_at(inst, 2)?);
            Some(OFace {
                input: append(s.clone(), std::slice::from_ref(&b)),
                output: append(s, &[sum(a, b)]),
                latent: vec![],
            })
        }
        17 => {
            let (s, a, b, t, e, f) = (
                stack_at(inst, 0)?,
                value_at(inst, 1)?,
                value_at(inst, 2)?,
                stack_at(inst, 3)?,
                effects_at(inst, 4)?,
                effects_at(inst, 5)?,
            );
            let left = OTy::Program(
                append(s.clone(), std::slice::from_ref(&a)),
                t.clone(),
                e.clone(),
            );
            let right = OTy::Program(
                append(s.clone(), std::slice::from_ref(&b)),
                t.clone(),
                f.clone(),
            );
            Some(OFace {
                input: append(s, &[sum(a, b), left, right]),
                output: t,
                latent: union(&e, &f),
            })
        }
        18 => {
            let (s, t, e, f) = (
                stack_at(inst, 0)?,
                stack_at(inst, 1)?,
                effects_at(inst, 2)?,
                effects_at(inst, 3)?,
            );
            let p1 = OTy::Program(s.clone(), t.clone(), e.clone());
            let p2 = OTy::Program(s.clone(), t.clone(), f.clone());
            Some(OFace {
                input: append(s, &[OTy::Bool, p1, p2]),
                output: t,
                latent: union(&e, &f),
            })
        }
        19 => {
            let (s, a) = (stack_at(inst, 0)?, value_at(inst, 1)?);
            Some(OFace {
                input: s.clone(),
                output: append(s, &[list(a)]),
                latent: vec![],
            })
        }
        20 => {
            let (s, a) = (stack_at(inst, 0)?, value_at(inst, 1)?);
            Some(OFace {
                input: append(s.clone(), &[a.clone(), list(value_at(inst, 1)?)]),
                output: append(s, &[list(a)]),
                latent: vec![],
            })
        }
        21 => {
            let (s, a, t, e, f) = (
                stack_at(inst, 0)?,
                value_at(inst, 1)?,
                stack_at(inst, 2)?,
                effects_at(inst, 3)?,
                effects_at(inst, 4)?,
            );
            let empty_branch = OTy::Program(s.clone(), t.clone(), e.clone());
            let cons_branch = OTy::Program(
                append(s.clone(), &[a.clone(), list(a.clone())]),
                t.clone(),
                f.clone(),
            );
            Some(OFace {
                input: append(s, &[list(value_at(inst, 1)?), empty_branch, cons_branch]),
                output: t,
                latent: union(&e, &f),
            })
        }
        22 => {
            let s = stack_at(inst, 0)?;
            Some(OFace {
                input: append(s.clone(), &[OTy::Text]),
                output: append(s, &[OTy::Unit]),
                latent: vec![0],
            })
        }
        23 => {
            let s = stack_at(inst, 0)?;
            Some(OFace {
                input: s.clone(),
                output: append(s, &[OTy::Resource]),
                latent: vec![],
            })
        }
        _ => None,
    }
}
