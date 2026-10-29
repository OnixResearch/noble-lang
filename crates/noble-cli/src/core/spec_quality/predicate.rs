//! Checked-native predicate validation and evaluation for specification quality.
//!
//! The frontend has already lowered the source into immutable expression and definition arenas.
//! This module only walks that retained representation: it never parses a manifest term, compiles a
//! candidate, or executes a program.  Validation is kept separate from evaluation so the parent
//! can validate both roots once before enumerating the finite relation.

use super::relation::{Refused, Scalar};

const DEPTH_LIMIT: usize = 8;

pub(super) struct Binding<'a> {
    pub(super) params: &'a [Scalar],
    pub(super) inputs: &'a [Scalar],
    pub(super) output: Option<&'a [Scalar]>,
}

fn refused(class: &'static str, message: &'static str) -> Refused {
    (class, std::string::String::from(message))
}

fn undefined(message: &'static str) -> Refused {
    refused("undefined-relation", message)
}

fn unsupported(message: &'static str) -> Refused {
    refused("unsupported-relation", message)
}

fn expression(
    contract: &noble_contracts::Prepared,
    id: u32,
) -> Result<&noble_contracts::Expr, Refused> {
    let index = usize::try_from(id).map_err(|_| undefined("native expression index is invalid"))?;
    contract
        .expressions()
        .get(index)
        .ok_or_else(|| undefined("native expression index is invalid"))
}

fn definition_body(contract: &noble_contracts::Prepared, id: u32) -> Result<u32, Refused> {
    let index = usize::try_from(id).map_err(|_| undefined("native definition index is invalid"))?;
    contract
        .definitions()
        .get(index)
        .map(|definition| definition.body)
        .ok_or_else(|| undefined("native definition index is invalid"))
}

fn binding_index(index: u32, length: usize) -> Result<usize, Refused> {
    let index = usize::try_from(index).map_err(|_| undefined("native binding index is invalid"))?;
    if index < length {
        Ok(index)
    } else {
        Err(undefined("native binding index is invalid"))
    }
}

fn validate_binary(
    contract: &noble_contracts::Prepared,
    left: u32,
    right: u32,
    depth: usize,
) -> Result<(), Refused> {
    let left = validate(contract, left, depth);
    let right = validate(contract, right, depth);
    match left {
        Err(error) => Err(error),
        Ok(()) => right,
    }
}

fn validate(contract: &noble_contracts::Prepared, id: u32, depth: usize) -> Result<(), Refused> {
    if depth >= DEPTH_LIMIT {
        return Err(unsupported("native predicate nesting exceeds the bound"));
    }
    let expr = expression(contract, id)?;
    match &expr.kind {
        noble_contracts::ExprKind::I64(_) | noble_contracts::ExprKind::Bool(_) => Ok(()),
        noble_contracts::ExprKind::Input(index) => {
            binding_index(*index, contract.inputs().len()).map(|_| ())
        }
        noble_contracts::ExprKind::Output(index) => {
            binding_index(*index, contract.outputs().len()).map(|_| ())
        }
        noble_contracts::ExprKind::Param(index) => {
            binding_index(*index, contract.params().len()).map(|_| ())
        }
        noble_contracts::ExprKind::Definition(index) => {
            let body = definition_body(contract, *index)?;
            validate(contract, body, depth.saturating_add(1))
        }
        noble_contracts::ExprKind::Not(inner) => {
            validate(contract, *inner, depth.saturating_add(1))
        }
        noble_contracts::ExprKind::And(left, right)
        | noble_contracts::ExprKind::Or(left, right)
        | noble_contracts::ExprKind::Eq(left, right)
        | noble_contracts::ExprKind::Lt(left, right)
        | noble_contracts::ExprKind::Add(left, right) => {
            validate_binary(contract, *left, *right, depth.saturating_add(1))
        }
        noble_contracts::ExprKind::Unit
        | noble_contracts::ExprKind::Implies(_, _)
        | noble_contracts::ExprKind::Le(_, _)
        | noble_contracts::ExprKind::Sub(_, _)
        | noble_contracts::ExprKind::Mul(_, _)
        | noble_contracts::ExprKind::Pair(_, _)
        | noble_contracts::ExprKind::First(_)
        | noble_contracts::ExprKind::Second(_)
        | noble_contracts::ExprKind::Inl(_)
        | noble_contracts::ExprKind::Inr(_)
        | noble_contracts::ExprKind::IsLeft(_)
        | noble_contracts::ExprKind::Left(_)
        | noble_contracts::ExprKind::Right(_)
        | noble_contracts::ExprKind::Nil
        | noble_contracts::ExprKind::Cons(_, _)
        | noble_contracts::ExprKind::IsNil(_)
        | noble_contracts::ExprKind::Head(_)
        | noble_contracts::ExprKind::Tail(_)
        | noble_contracts::ExprKind::Length(_)
        | noble_contracts::ExprKind::Maps(_, _, _) => Err(unsupported(
            "native expression kind is outside the bounded predicate profile",
        )),
    }
}

fn scalar(stack: &[Scalar], index: u32) -> Result<Scalar, Refused> {
    let index = binding_index(index, stack.len())?;
    stack
        .get(index)
        .copied()
        .ok_or_else(|| undefined("native scalar binding is unavailable"))
}

fn evaluate(
    contract: &noble_contracts::Prepared,
    id: u32,
    at: &Binding<'_>,
    depth: usize,
) -> Result<Scalar, Refused> {
    if depth >= DEPTH_LIMIT {
        return Err(unsupported("native predicate nesting exceeds the bound"));
    }
    let expr = expression(contract, id)?;
    match &expr.kind {
        noble_contracts::ExprKind::I64(value) => Ok(Scalar::I64(*value)),
        noble_contracts::ExprKind::Bool(value) => Ok(Scalar::Bool(*value)),
        noble_contracts::ExprKind::Input(index) => scalar(at.inputs, *index),
        noble_contracts::ExprKind::Output(index) => match at.output {
            Some(output) => scalar(output, *index),
            None => Err(undefined("native output binding is unavailable")),
        },
        noble_contracts::ExprKind::Param(index) => scalar(at.params, *index),
        noble_contracts::ExprKind::Definition(index) => {
            let body = definition_body(contract, *index)?;
            evaluate(contract, body, at, depth.saturating_add(1))
        }
        noble_contracts::ExprKind::Not(inner) => {
            let value = evaluate(contract, *inner, at, depth.saturating_add(1))?;
            match value {
                Scalar::Bool(value) => Ok(Scalar::Bool(!value)),
                Scalar::I64(_) => Err(undefined("native not operand is not Boolean")),
            }
        }
        noble_contracts::ExprKind::And(left, right) => {
            let left = evaluate(contract, *left, at, depth.saturating_add(1));
            let right = evaluate(contract, *right, at, depth.saturating_add(1));
            match (left?, right?) {
                (Scalar::Bool(left), Scalar::Bool(right)) => Ok(Scalar::Bool(left && right)),
                _ => Err(undefined("native and operands are not Boolean")),
            }
        }
        noble_contracts::ExprKind::Or(left, right) => {
            let left = evaluate(contract, *left, at, depth.saturating_add(1));
            let right = evaluate(contract, *right, at, depth.saturating_add(1));
            match (left?, right?) {
                (Scalar::Bool(left), Scalar::Bool(right)) => Ok(Scalar::Bool(left || right)),
                _ => Err(undefined("native or operands are not Boolean")),
            }
        }
        noble_contracts::ExprKind::Eq(left, right) => {
            let left = evaluate(contract, *left, at, depth.saturating_add(1))?;
            let right = evaluate(contract, *right, at, depth.saturating_add(1))?;
            match (left, right) {
                (Scalar::I64(left), Scalar::I64(right)) => Ok(Scalar::Bool(left == right)),
                (Scalar::Bool(left), Scalar::Bool(right)) => Ok(Scalar::Bool(left == right)),
                _ => Err(undefined("native equality operands have different kinds")),
            }
        }
        noble_contracts::ExprKind::Lt(left, right) => {
            let left = evaluate(contract, *left, at, depth.saturating_add(1))?;
            let right = evaluate(contract, *right, at, depth.saturating_add(1))?;
            match (left, right) {
                (Scalar::I64(left), Scalar::I64(right)) => Ok(Scalar::Bool(left < right)),
                _ => Err(undefined("native less-than operands are not I64")),
            }
        }
        noble_contracts::ExprKind::Add(left, right) => {
            let left = evaluate(contract, *left, at, depth.saturating_add(1))?;
            let right = evaluate(contract, *right, at, depth.saturating_add(1))?;
            match (left, right) {
                (Scalar::I64(left), Scalar::I64(right)) => {
                    Ok(Scalar::I64(left.wrapping_add(right)))
                }
                _ => Err(undefined("native addition operands are not I64")),
            }
        }
        noble_contracts::ExprKind::Unit
        | noble_contracts::ExprKind::Implies(_, _)
        | noble_contracts::ExprKind::Le(_, _)
        | noble_contracts::ExprKind::Sub(_, _)
        | noble_contracts::ExprKind::Mul(_, _)
        | noble_contracts::ExprKind::Pair(_, _)
        | noble_contracts::ExprKind::First(_)
        | noble_contracts::ExprKind::Second(_)
        | noble_contracts::ExprKind::Inl(_)
        | noble_contracts::ExprKind::Inr(_)
        | noble_contracts::ExprKind::IsLeft(_)
        | noble_contracts::ExprKind::Left(_)
        | noble_contracts::ExprKind::Right(_)
        | noble_contracts::ExprKind::Nil
        | noble_contracts::ExprKind::Cons(_, _)
        | noble_contracts::ExprKind::IsNil(_)
        | noble_contracts::ExprKind::Head(_)
        | noble_contracts::ExprKind::Tail(_)
        | noble_contracts::ExprKind::Length(_)
        | noble_contracts::ExprKind::Maps(_, _, _) => Err(unsupported(
            "native expression kind is outside the bounded predicate profile",
        )),
    }
}

pub(super) fn check(contract: &noble_contracts::Prepared) -> Result<(), Refused> {
    let requires = validate(contract, contract.requires(), 0);
    let ensures = validate(contract, contract.ensures(), 0);
    match requires {
        Err(error) => Err(error),
        Ok(()) => ensures,
    }
}

pub(super) fn holds(
    contract: &noble_contracts::Prepared,
    root: u32,
    at: &Binding<'_>,
    what: &str,
) -> Result<bool, Refused> {
    match evaluate(contract, root, at, 0)? {
        Scalar::Bool(value) => Ok(value),
        Scalar::I64(_) => Err((
            "undefined-relation",
            std::format!("{what} is undefined for an assignment in the declared domain"),
        )),
    }
}
