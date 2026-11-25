//! Closed, tactic-free Lean 4 lowering for the revision-3 named-call profile
//! targeting `NobleContracts.NamedV3Lifted`.
//!
//! All `NamedV3Obligation.lean` specification emission and closed `LiftedChain`
//! proof term lowering live here in `#![no_std]` without recursion.

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; imports core AST types directly from `named_v3`"
)]
use super::{Prop3, Sort3, Statement3, Term3};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; imports core form and origin types"
)]
use crate::intrinsic::{Form, NamedUseOrigin};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; imports crate diagnostic and span types"
)]
use crate::{Diagnostic, Span};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; imports alloc containers for `#![no_std]` code generation"
)]
use alloc::{boxed::Box, string::String, vec::Vec};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; imports kernel AST types for code lowering"
)]
use noble_kernel::untrusted::{Lit, Node};
fn invalid(span: Span, message: &str) -> Diagnostic {
    crate::intrinsic::reject(span, message)
}

fn push_bytes_lean(mut out: String, source: &[u8]) -> String {
    out.push('[');
    let mut index = 0usize;
    while index < source.len() {
        if index != 0 {
            out.push(',');
        }
        out.push_str("(UInt8.ofNat ");
        crate::rendering::number(&mut out, u64::from(source[index]));
        out.push(')');
        index += 1;
    }
    out.push(']');
    out
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; validates ASCII provenance strings before quoting"
)]
fn push_lean_str(mut out: String, text: &str, span: Span) -> Result<String, Diagnostic> {
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] >= 0x80 {
            return Err(invalid(span, "named provenance name is not ASCII"));
        }
        i += 1;
    }
    out.push('"');
    let mut j = 0usize;
    while j < bytes.len() {
        let b = bytes[j];
        match b {
            0 => out.push_str("\\0"),
            b'\t' => out.push_str("\\t"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            _ => {
                if b < 0x20 || b == 0x7f {
                    out.push_str("\\u{");
                    let hi = b / 16;
                    let lo = b % 16;
                    if hi > 0 {
                        out.push(hex_char(hi));
                    }
                    out.push(hex_char(lo));
                    out.push('}');
                } else {
                    out.push(char::from(b));
                }
            }
        }
        j += 1;
    }
    out.push('"');
    Ok(out)
}

const fn hex_char(v: u8) -> char {
    match v {
        0 => '0',
        1 => '1',
        2 => '2',
        3 => '3',
        4 => '4',
        5 => '5',
        6 => '6',
        7 => '7',
        8 => '8',
        9 => '9',
        10 => 'a',
        11 => 'b',
        12 => 'c',
        13 => 'd',
        14 => 'e',
        _ => 'f',
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits fixed Lean syntax for each kernel value sort"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; performs dynamic `String` formatting"
)]
fn push_ty(mut out: String, ty: &noble_kernel::types::Ty) -> Result<String, Diagnostic> {
    match ty {
        noble_kernel::types::Ty::I64 => out.push_str(".i64"),
        noble_kernel::types::Ty::Bool => out.push_str(".bool"),
        noble_kernel::types::Ty::Unit
        | noble_kernel::types::Ty::Text
        | noble_kernel::types::Ty::Syntax
        | noble_kernel::types::Ty::Contract
        | noble_kernel::types::Ty::Evidence
        | noble_kernel::types::Ty::Certified
        | noble_kernel::types::Ty::Pair(..)
        | noble_kernel::types::Ty::Sum(..)
        | noble_kernel::types::Ty::List(..)
        | noble_kernel::types::Ty::Program(..)
        | noble_kernel::types::Ty::LiveRef(..)
        | noble_kernel::types::Ty::Resource(..)
        | noble_kernel::types::Ty::Nominal(..)
        | noble_kernel::types::Ty::GenericNominal(..) => {
            return Err(crate::intrinsic::unsupported(
                Span { start: 0, end: 0 },
                "revision-3 Lean lowering supports I64/Bool types only",
            ));
        }
    }
    Ok(out)
}

fn push_ty_list(mut out: String, tys: &[noble_kernel::types::Ty]) -> Result<String, Diagnostic> {
    out.push('[');
    let mut i = 0usize;
    while i < tys.len() {
        if i != 0 {
            out.push_str(", ");
        }
        out = push_ty(out, &tys[i])?;
        i += 1;
    }
    out.push(']');
    Ok(out)
}

fn push_sort_list(mut out: String, sorts: &[Sort3]) -> String {
    out.push('[');
    let mut i = 0usize;
    while i < sorts.len() {
        if i != 0 {
            out.push_str(", ");
        }
        out.push_str(match sorts[i] {
            Sort3::I64 => ".i64",
            Sort3::Bool => ".bool",
        });
        i += 1;
    }
    out.push(']');
    out
}

const fn contains_u32(slice: &[u32], val: u32) -> bool {
    let mut i = 0usize;
    let mut is_found = false;
    while !is_found && i < slice.len() {
        if slice[i] == val {
            is_found = true;
        }
        i += 1;
    }
    is_found
}

fn fmt_str_num(prefix: &str, n: u64) -> String {
    let mut s = String::from(prefix);
    crate::rendering::number(&mut s, n);
    s
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; prefix and suffix are distinct formatting parts"
)]
fn fmt_str_num_str(prefix: &str, n: u64, suffix: &str) -> String {
    let mut s = String::from(prefix);
    crate::rendering::number(&mut s, n);
    s.push_str(suffix);
    s
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; prefix and middle tag are distinct formatting parts"
)]
fn fmt_prefix_mid_num(prefix: &str, mid: &str, n: u64) -> String {
    let mut s = String::from(prefix);
    s.push_str(mid);
    crate::rendering::number(&mut s, n);
    s
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; formatting delimiters and operands are ordered by Lean syntax"
)]
fn fmt_wrap2(open: &str, a: &str, mid: &str, b: &str, close: &str) -> String {
    let mut s = String::from(open);
    s.push_str(a);
    s.push_str(mid);
    s.push_str(b);
    s.push_str(close);
    s
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; formatting delimiters and 3 operands are ordered by Lean syntax"
)]
fn fmt_wrap3(open: &str, a: &str, b: &str, c: &str, close: &str) -> String {
    let mut s = String::from(open);
    s.push_str(a);
    s.push(' ');
    s.push_str(b);
    s.push(' ');
    s.push_str(c);
    s.push_str(close);
    s
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends Lean sort syntax into caller String buffer"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; appends sort text directly to caller String"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; String::push_str is not const"
)]
fn push_sort_lean_type_buf(out: &mut String, code: Sort3) {
    match code {
        Sort3::I64 => out.push_str("BitVec 64"),
        Sort3::Bool => out.push_str("Bool"),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; formats provenance structure with ASCII validation"
)]
fn emit_provenance(
    mut out: String,
    goal: &crate::intrinsic::ContractGoal,
    origin: &NamedUseOrigin,
) -> Result<String, Diagnostic> {
    let Some(module) = goal.subject.source_dependencies.get(origin.module_index) else {
        return Err(invalid(origin.definition_span, "named module is absent"));
    };
    let Ok(start) = usize::try_from(origin.definition_span.start) else {
        return Err(invalid(
            origin.definition_span,
            "source span exceeds address space",
        ));
    };
    let Ok(end) = usize::try_from(origin.definition_span.end) else {
        return Err(invalid(
            origin.definition_span,
            "source span exceeds address space",
        ));
    };
    let Some(def_src) = module.full_source.get(start..end) else {
        return Err(invalid(
            origin.definition_span,
            "source span lies outside retained module",
        ));
    };
    out.push_str("{ moduleName := ");
    out = push_lean_str(out, &module.module, origin.definition_span)?;
    out.push_str(", moduleVersion := ");
    crate::rendering::number(&mut out, u64::from(module.version));
    out.push_str(", moduleSource := ");
    out = push_bytes_lean(out, &module.full_source);
    out.push_str(", definitionSlot := ");
    crate::rendering::number(&mut out, u64::from(origin.definition_ordinal));
    out.push_str(", definitionName := ");
    out = push_lean_str(out, &origin.definition_name, origin.definition_span)?;
    out.push_str(", definitionSource := ");
    out = push_bytes_lean(out, def_src);
    out.push_str(" }");
    Ok(out)
}

#[octet::sealed_enum]
enum TermTermStep {
    Visit(Term3),
    CloseParen,
    Space,
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; formats Term3 AST into caller output buffer and worklist"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; updates formatting buffer and iterative worklist"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; String and Vec mutations are not const"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; bounded iterative AST formatting"
)]
fn emit_noble_term_step(
    step: TermTermStep,
    out: &mut String,
    work: &mut Vec<TermTermStep>,
) -> Result<(), Diagnostic> {
    match step {
        TermTermStep::CloseParen => out.push(')'),
        TermTermStep::Space => out.push(' '),
        TermTermStep::Visit(node) => match node {
            Term3::Input(i) => {
                out.push_str("(.input ");
                crate::rendering::number(out, i as u64);
                out.push(')');
            }
            Term3::Output(i) => {
                out.push_str("(.output ");
                crate::rendering::number(out, i as u64);
                out.push(')');
            }
            Term3::I64(n) => {
                out.push_str("(.i64 (");
                crate::rendering::integer(out, n);
                out.push_str("))");
            }
            Term3::Bool(b) => {
                out.push_str(if b { "(.bool true)" } else { "(.bool false)" });
            }
            Term3::Add(l, r) => {
                out.push_str("(.add ");
                work.push(TermTermStep::CloseParen);
                work.push(TermTermStep::Visit(*r));
                work.push(TermTermStep::Space);
                work.push(TermTermStep::Visit(*l));
            }
            Term3::Sub(l, r) => {
                out.push_str("(.sub ");
                work.push(TermTermStep::CloseParen);
                work.push(TermTermStep::Visit(*r));
                work.push(TermTermStep::Space);
                work.push(TermTermStep::Visit(*l));
            }
            Term3::Mul(l, r) => {
                out.push_str("(.mul ");
                work.push(TermTermStep::CloseParen);
                work.push(TermTermStep::Visit(*r));
                work.push(TermTermStep::Space);
                work.push(TermTermStep::Visit(*l));
            }
            Term3::Equals(l, r) => {
                out.push_str("(.eq ");
                work.push(TermTermStep::CloseParen);
                work.push(TermTermStep::Visit(*r));
                work.push(TermTermStep::Space);
                work.push(TermTermStep::Visit(*l));
            }
            Term3::Bound(_) | Term3::Ghost(..) => {
                return Err(invalid(
                    Span { start: 0, end: 0 },
                    "contract assertion cannot mention proof binders or ghosts",
                ));
            }
        },
    }
    Ok(())
}

fn emit_noble_term_from_term3(mut out: String, root: &Term3) -> Result<String, Diagnostic> {
    let mut work = Vec::new();
    work.push(TermTermStep::Visit(root.clone()));
    let mut outcome = Ok(());
    while let Some(step) = work.pop() {
        outcome = emit_noble_term_step(step, &mut out, &mut work);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    Ok(out)
}

#[octet::sealed_enum]
enum PropTermStep {
    Visit(Prop3),
    CloseParen,
    Sep,
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; formats Prop3 AST into caller output buffer and worklist"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; updates formatting buffer and iterative worklist"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; bounded iterative AST formatting"
)]
fn emit_noble_prop_step(
    step: PropTermStep,
    out: &mut String,
    work: &mut Vec<PropTermStep>,
) -> Result<(), Diagnostic> {
    match step {
        PropTermStep::CloseParen => out.push(')'),
        PropTermStep::Sep => out.push(' '),
        PropTermStep::Visit(node) => match node {
            Prop3::True => out.push_str("(.bool true)"),
            Prop3::False => out.push_str("(.bool false)"),
            Prop3::Not(b) => {
                out.push_str("(.not ");
                work.push(PropTermStep::CloseParen);
                work.push(PropTermStep::Visit(*b));
            }
            Prop3::And(l, r) => {
                out.push_str("(.and ");
                work.push(PropTermStep::CloseParen);
                work.push(PropTermStep::Visit(*r));
                work.push(PropTermStep::Sep);
                work.push(PropTermStep::Visit(*l));
            }
            Prop3::Eq(l, r) => {
                out.push_str("(.eq ");
                let s1 = emit_noble_term_from_term3(core::mem::take(out), &l)?;
                *out = s1;
                out.push(' ');
                let s2 = emit_noble_term_from_term3(core::mem::take(out), &r)?;
                *out = s2;
                out.push(')');
            }
            Prop3::Lt(l, r) => {
                out.push_str("(.lt ");
                let s1 = emit_noble_term_from_term3(core::mem::take(out), &l)?;
                *out = s1;
                out.push(' ');
                let s2 = emit_noble_term_from_term3(core::mem::take(out), &r)?;
                *out = s2;
                out.push(')');
            }
            Prop3::Le(l, r) => {
                out.push_str("(.le ");
                let s1 = emit_noble_term_from_term3(core::mem::take(out), &l)?;
                *out = s1;
                out.push(' ');
                let s2 = emit_noble_term_from_term3(core::mem::take(out), &r)?;
                *out = s2;
                out.push(')');
            }
        },
    }
    Ok(())
}

fn emit_noble_term_from_prop3(mut out: String, root: &Prop3) -> Result<String, Diagnostic> {
    let mut work = Vec::new();
    work.push(PropTermStep::Visit(root.clone()));
    let mut outcome = Ok(());
    while let Some(step) = work.pop() {
        outcome = emit_noble_prop_step(step, &mut out, &mut work);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    Ok(out)
}

struct EntryMeta {
    slot: u32,
    def_idx: usize,
    def_id: noble_kernel::contracts::Definition,
    origin_use_idx: usize,
    rank: u32,
    deps: Vec<u32>,
}

const fn opt_def_eq(
    opt: Option<noble_kernel::contracts::Definition>,
    expected: noble_kernel::contracts::Definition,
) -> bool {
    match opt {
        Some(d) => d.0 == expected.0,
        None => false,
    }
}

fn find_use_for_call(
    named_uses: &[NamedUseOrigin],
    caller_def: noble_kernel::contracts::Definition,
    node_pos: usize,
) -> Option<usize> {
    let pos_u32 = u32::try_from(node_pos).ok()?;
    let mut i = 1usize;
    let mut found = None;
    while found.is_none() && i < named_uses.len() {
        let u = &named_uses[i];
        let is_node_match = match u.source_node {
            Some(v) => v == pos_u32,
            None => false,
        };
        let is_use_match = opt_def_eq(u.caller_definition, caller_def) && is_node_match;
        if is_use_match {
            found = Some(i);
        }
        i += 1;
    }
    found
}

fn find_entry_meta_by_slot(metas: &[EntryMeta], slot: u32) -> Option<usize> {
    let mut i = 0usize;
    let mut found = None;
    while found.is_none() && i < metas.len() {
        if metas[i].slot == slot {
            found = Some(i);
        }
        i += 1;
    }
    found
}

const fn find_entry_meta_by_def(
    metas: &[EntryMeta],
    def: noble_kernel::contracts::Definition,
) -> Option<usize> {
    let mut i = 0usize;
    let mut found = None;
    while found.is_none() && i < metas.len() {
        if metas[i].def_id.0 == def.0 {
            found = Some(i);
        }
        i += 1;
    }
    found
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; validates accepted submission DAG structure"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; collects entry metadata and computes DAG longest-path ranks"
)]
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; bounded by accepted kernel definition and body counts"
)]
fn collect_entries_meta(
    goal: &crate::intrinsic::ContractGoal,
    selected_def: noble_kernel::contracts::Definition,
) -> Result<(Vec<EntryMeta>, u32), Diagnostic> {
    let defs = &goal.subject.accepted_submission.definitions;
    let uses = &goal.subject.named_uses;
    let mut metas: Vec<EntryMeta> = Vec::with_capacity(defs.len());
    let mut d_idx = 0usize;
    while d_idx < defs.len() {
        let def = &defs[d_idx];
        if def.definition.0 != selected_def.0 {
            let mut origin_use_idx = None;
            let mut u_idx = 1usize;
            while origin_use_idx.is_none() && u_idx < uses.len() {
                if uses[u_idx].definition.0 == def.definition.0 {
                    origin_use_idx = Some(u_idx);
                }
                u_idx += 1;
            }
            let Some(origin_use_idx) = origin_use_idx else {
                return Err(invalid(
                    goal.contract_span,
                    "accepted entry has no traced origin",
                ));
            };
            #[expect(
                tigerstyle::allocation_in_loop,
                reason = "Owner: noble-contracts; one dependency vector per accepted kernel entry"
            )]
            let mut deps: Vec<u32> = Vec::new();
            let body = &def.body.candidate.body;
            let mut b_idx = 0usize;
            while b_idx < body.len() {
                if let Some(use_idx) = find_use_for_call(uses, def.definition, b_idx) {
                    let target_slot = uses[use_idx].definition.0;
                    if !deps.contains(&target_slot) {
                        deps.push(target_slot);
                    }
                }
                b_idx = b_idx.saturating_add(1);
            }
            metas.push(EntryMeta {
                slot: def.definition.0,
                def_idx: d_idx,
                def_id: def.definition,
                origin_use_idx,
                rank: 0,
                deps,
            });
        }
        d_idx += 1;
    }
    let mut pass = 0usize;
    let mut is_changed = true;
    while is_changed && pass <= metas.len() {
        is_changed = false;
        let mut i = 0usize;
        while i < metas.len() {
            let mut needed = 0u32;
            let mut di = 0usize;
            while di < metas[i].deps.len() {
                let target_slot = metas[i].deps[di];
                if let Some(t_idx) = find_entry_meta_by_slot(&metas, target_slot) {
                    let candidate = metas[t_idx].rank.saturating_add(1);
                    if candidate > needed {
                        needed = candidate;
                    }
                }
                di += 1;
            }
            if needed > metas[i].rank {
                metas[i].rank = needed;
                is_changed = true;
            }
            i = i.saturating_add(1);
        }
        pass += 1;
    }
    let mut i = 1usize;
    while i < metas.len() {
        let mut j = i;
        while j > 0 && metas[j.saturating_sub(1)].rank > metas[j].rank {
            metas.swap(j.saturating_sub(1), j);
            j = j.saturating_sub(1);
        }
        i += 1;
    }
    let mut root_rank = 0u32;
    let mut ui = 1usize;
    while ui < uses.len() {
        if uses[ui].caller_definition == Some(selected_def) {
            if let Some(m_idx) = find_entry_meta_by_slot(&metas, uses[ui].definition.0) {
                let needed = metas[m_idx].rank.saturating_add(1);
                if needed > root_rank {
                    root_rank = needed;
                }
            }
        }
        ui += 1;
    }
    Ok((metas, root_rank))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits fixed syntax per validated kernel body instruction"
)]
fn emit_entry_code(
    mut out: String,
    def: &noble_kernel::execution::Definition,
    named_uses: &[NamedUseOrigin],
    span: Span,
) -> Result<String, Diagnostic> {
    out.push('[');
    let body = &def.body.candidate.body;
    let nodes = &def.body.candidate.nodes;
    let mut pos = 0usize;
    while pos < body.len() {
        if pos != 0 {
            out.push_str(", ");
        }
        let Ok(n_idx) = usize::try_from(body[pos].0) else {
            return Err(invalid(span, "node index exceeds address space"));
        };
        let Some(node) = nodes.get(n_idx) else {
            return Err(invalid(span, "missing body node"));
        };
        match node {
            Node::Literal {
                lit: Lit::I64(n), ..
            } => {
                out.push_str(".lit (.i64 (BitVec.ofInt 64 (");
                crate::rendering::integer(&mut out, *n);
                out.push_str(")))");
            }
            Node::Literal {
                lit: Lit::Bool(b), ..
            } => {
                out.push_str(if *b {
                    ".lit (.bool true)"
                } else {
                    ".lit (.bool false)"
                });
            }
            Node::Invocation { def: target, .. } => {
                if (target.0 <= 7 && target.0 != 3)
                    && find_use_for_call(named_uses, def.definition, pos).is_none()
                {
                    out.push_str(".word ");
                    crate::rendering::number(&mut out, u64::from(target.0));
                } else {
                    let Some(use_idx) = find_use_for_call(named_uses, def.definition, pos) else {
                        return Err(invalid(span, "named invocation lacks traced occurrence"));
                    };
                    out.push_str(".call use_");
                    crate::rendering::number(&mut out, use_idx as u64);
                }
            }
            Node::Literal { .. }
            | Node::SignedSelf { .. }
            | Node::Quotation { .. }
            | Node::SlotInvoke { .. } => {
                return Err(crate::intrinsic::unsupported(
                    span,
                    "body node is outside revision-3 Lean profile",
                ));
            }
        }
        pos += 1;
    }
    out.push(']');
    Ok(out)
}

struct CodeTypedSpec<'a> {
    thm_name: &'a str,
    env_name: &'a str,
    code_ref: &'a str,
    def: &'a noble_kernel::execution::Definition,
    defs: &'a [noble_kernel::execution::Definition],
    named_uses: &'a [NamedUseOrigin],
    metas: &'a [EntryMeta],
    span: Span,
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; appends formatted Lean identifier prefix to owned String"
)]
fn push_call_target_prefix(mut out: String, sub_prefix: Option<usize>) -> String {
    match sub_prefix {
        Some(ui) => {
            out.push_str("calleeSubTyped_");
            crate::rendering::number(&mut out, ui as u64);
            out.push('_');
        }
        None => out.push_str("typed_entry_"),
    }
    out
}

#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-contracts; lowers one step of a `CodeTyped` theorem without early loop returns"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; lowers one step of a `CodeTyped` theorem without early loop returns"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; malformed nodes return typed Diagnostic"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; dispatches literal, builtin, and call `OpTyped` step construction"
)]
fn emit_code_typed_step(
    env_name: &str,
    sub_prefix: Option<usize>,
    def: &noble_kernel::execution::Definition,
    defs: &[noble_kernel::execution::Definition],
    named_uses: &[NamedUseOrigin],
    metas: &[EntryMeta],
    pos: usize,
    cur: &[noble_kernel::types::Ty],
    span: Span,
) -> Result<(String, Vec<noble_kernel::types::Ty>), Diagnostic> {
    let body = &def.body.candidate.body;
    let nodes = &def.body.candidate.nodes;
    let Ok(n_idx) = usize::try_from(body[pos].0) else {
        return Err(invalid(span, "node index exceeds address space"));
    };
    let node = &nodes[n_idx];
    let mut next_cur = cur.to_vec();
    let mut step_expr = String::new();
    match node {
        Node::Literal {
            lit: Lit::I64(val), ..
        } => {
            step_expr.push_str("NamedV1.OpTyped.lit (env := ");
            step_expr.push_str(env_name);
            step_expr.push_str(") (v := .i64 (BitVec.ofInt 64 (");
            crate::rendering::integer(&mut step_expr, *val);
            step_expr.push_str("))) (s := st ++ ");
            step_expr = push_ty_list(step_expr, cur)?;
            step_expr.push_str(") .i64");
            next_cur.push(noble_kernel::types::Ty::I64);
        }
        Node::Literal {
            lit: Lit::Bool(val),
            ..
        } => {
            step_expr.push_str("NamedV1.OpTyped.lit (env := ");
            step_expr.push_str(env_name);
            step_expr.push_str(") (v := .bool ");
            step_expr.push_str(choose_str(*val, "true", "false"));
            step_expr.push_str(") (s := st ++ ");
            step_expr = push_ty_list(step_expr, cur)?;
            step_expr.push_str(") .bool");
            next_cur.push(noble_kernel::types::Ty::Bool);
        }
        Node::Invocation { def: target, .. }
            if (target.0 <= 7 && target.0 != 3)
                && find_use_for_call(named_uses, def.definition, pos).is_none() =>
        {
            return emit_builtin_optyped(env_name, next_cur, target.0, span);
        }
        Node::Invocation { .. } => {
            let Some(use_idx) = find_use_for_call(named_uses, def.definition, pos) else {
                return Err(invalid(span, "missing traced occurrence for typed call"));
            };
            let target_slot = named_uses[use_idx].definition.0;
            let Some(m_idx) = find_entry_meta_by_slot(metas, target_slot) else {
                return Err(invalid(span, "callee entry missing from environment"));
            };
            let callee_def = &defs[metas[m_idx].def_idx];
            let arity = callee_def.expected.stack_in.len();
            if next_cur.len() < arity {
                return Err(invalid(span, "type stack underflow on call"));
            }
            let pfx_len = next_cur.len().saturating_sub(arity);
            let pfx = &next_cur[..pfx_len];
            step_expr.push_str("NamedV1.OpTyped.call (env := ");
            step_expr.push_str(env_name);
            step_expr.push_str(") (use := use_");
            crate::rendering::number(&mut step_expr, use_idx as u64);
            step_expr.push_str(") (entry := entry_");
            crate::rendering::number(&mut step_expr, u64::from(target_slot));
            step_expr.push_str(") (s := st ++ ");
            step_expr = push_ty_list(step_expr, pfx)?;
            step_expr.push_str(") rfl ⟨rfl, rfl, rfl, rfl⟩ (");
            step_expr = push_call_target_prefix(step_expr, sub_prefix);
            crate::rendering::number(&mut step_expr, u64::from(target_slot));
            step_expr.push_str(" (st ++ ");
            step_expr = push_ty_list(step_expr, pfx)?;
            step_expr.push_str("))");
            next_cur.truncate(pfx_len);
            next_cur.extend_from_slice(&callee_def.expected.stack_out);
        }
        Node::Literal { .. }
        | Node::SignedSelf { .. }
        | Node::Quotation { .. }
        | Node::SlotInvoke { .. } => {
            return Err(crate::intrinsic::unsupported(
                span,
                "unsupported node in typed code",
            ));
        }
    }
    Ok((step_expr, next_cur))
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends one `CodeTyped` line and advances stack cursor"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; appends `refine .cons` step and updates type stack"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; passes environment, definitions, uses, metas, and stack cursor explicitly"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-contracts; emits step-by-step `CodeTyped` theorem"
)]
fn emit_code_typed_line(
    out: &mut String,
    env_name: &str,
    sub_prefix: Option<usize>,
    def: &noble_kernel::execution::Definition,
    defs: &[noble_kernel::execution::Definition],
    named_uses: &[NamedUseOrigin],
    metas: &[EntryMeta],
    pos: usize,
    cur: &mut Vec<noble_kernel::types::Ty>,
    span: Span,
) -> Result<(), Diagnostic> {
    let (step_expr, next_cur) = emit_code_typed_step(
        env_name, sub_prefix, def, defs, named_uses, metas, pos, cur, span,
    )?;
    out.push_str("  refine .cons (m := st ++ ");
    let next_out = push_ty_list(core::mem::take(out), &next_cur)?;
    *out = next_out;
    out.push_str(") (by have h := (");
    out.push_str(&step_expr);
    out.push_str("); simp at h ⊢; exact h) ?_\n");
    *cur = next_cur;
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits step-by-step `CodeTyped` theorem"
)]
fn emit_code_typed_theorem(mut out: String, spec: CodeTypedSpec<'_>) -> Result<String, Diagnostic> {
    let CodeTypedSpec {
        thm_name,
        env_name,
        code_ref,
        def,
        defs,
        named_uses,
        metas,
        span,
    } = spec;
    let in_tys = &def.expected.stack_in;
    let out_tys = &def.expected.stack_out;
    out.push_str("private theorem ");
    out.push_str(thm_name);
    out.push_str(" (st : List Ty) : NamedV1.CodeTyped ");
    out.push_str(env_name);
    out.push(' ');
    out.push_str(code_ref);
    out.push_str(" (st ++ ");
    out = push_ty_list(out, in_tys)?;
    out.push_str(") (st ++ ");
    out = push_ty_list(out, out_tys)?;
    out.push_str(") := by\n");
    let mut cur: Vec<noble_kernel::types::Ty> = in_tys.clone();
    let body = &def.body.candidate.body;
    let mut pos = 0usize;
    let mut outcome = Ok(());
    while pos < body.len() {
        outcome = emit_code_typed_line(
            &mut out, env_name, None, def, defs, named_uses, metas, pos, &mut cur, span,
        );
        if outcome.is_err() {
            break;
        }
        pos += 1;
    }
    outcome?;
    out.push_str("  exact .nil\n");
    Ok(out)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; checks stack arity and operand sorts with typed diagnostics"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; handles all pure stack builtins in one switch"
)]
fn emit_builtin_optyped(
    env_name: &str,
    mut cur: Vec<noble_kernel::types::Ty>,
    word: u32,
    span: Span,
) -> Result<(String, Vec<noble_kernel::types::Ty>), Diagnostic> {
    let mut step = String::new();
    match word {
        0 => {
            let Some(a) = cur.pop() else {
                return Err(invalid(span, "dup stack underflow"));
            };
            step.push_str("NamedV1.OpTyped.dup (env := ");
            step.push_str(env_name);
            step.push_str(") (s := st ++ ");
            step = push_ty_list(step, &cur)?;
            step.push_str(") (a := ");
            step = push_ty(step, &a)?;
            step.push(')');
            cur.push(a.clone());
            cur.push(a);
        }
        1 => {
            let Some(a) = cur.pop() else {
                return Err(invalid(span, "drop stack underflow"));
            };
            step.push_str("NamedV1.OpTyped.drop (env := ");
            step.push_str(env_name);
            step.push_str(") (s := st ++ ");
            step = push_ty_list(step, &cur)?;
            step.push_str(") (a := ");
            step = push_ty(step, &a)?;
            step.push(')');
        }
        2 => {
            let (Some(b), Some(a)) = (cur.pop(), cur.pop()) else {
                return Err(invalid(span, "swap stack underflow"));
            };
            step.push_str("NamedV1.OpTyped.swap (env := ");
            step.push_str(env_name);
            step.push_str(") (s := st ++ ");
            step = push_ty_list(step, &cur)?;
            step.push_str(") (a := ");
            step = push_ty(step, &a)?;
            step.push_str(") (b := ");
            step = push_ty(step, &b)?;
            step.push(')');
            cur.push(b);
            cur.push(a);
        }
        4..=7 => {
            let (Some(_b), Some(_a)) = (cur.pop(), cur.pop()) else {
                return Err(invalid(span, "binop stack underflow"));
            };
            let name = match word {
                4 => "NamedV1.OpTyped.add",
                5 => "NamedV1.OpTyped.sub",
                6 => "NamedV1.OpTyped.mul",
                _ => "NamedV1.OpTyped.equals",
            };
            step.push_str(name);
            step.push_str(" (env := ");
            step.push_str(env_name);
            step.push_str(") (s := st ++ ");
            step = push_ty_list(step, &cur)?;
            step.push(')');
            cur.push(if word == 7 {
                noble_kernel::types::Ty::Bool
            } else {
                noble_kernel::types::Ty::I64
            });
        }
        _ => {
            return Err(crate::intrinsic::unsupported(
                span,
                "unsupported builtin in typed code",
            ))
        }
    }
    Ok((step, cur))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; iterative BFS over bounded dependency graph"
)]
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; bounded by `metas.len()` distinct entries"
)]
fn collect_transitive_slots(metas: &[EntryMeta], root_slot: u32) -> Vec<u32> {
    let mut seen: Vec<u32> = Vec::new();
    let mut work: Vec<u32> = Vec::new();
    if let Some(root_idx) = find_entry_meta_by_slot(metas, root_slot) {
        let mut di = 0usize;
        while di < metas[root_idx].deps.len() {
            work.push(metas[root_idx].deps[di]);
            di += 1;
        }
    }
    while let Some(slot) = work.pop() {
        if !seen.contains(&slot) {
            seen.push(slot);
            if let Some(m_idx) = find_entry_meta_by_slot(metas, slot) {
                let mut di = 0usize;
                while di < metas[m_idx].deps.len() {
                    work.push(metas[m_idx].deps[di]);
                    di += 1;
                }
            }
        }
    }
    // Preserve ascending rank order matching `metas`.
    let mut ordered: Vec<u32> = Vec::with_capacity(seen.len());
    let mut mi = 0usize;
    while mi < metas.len() {
        if contains_u32(&seen, metas[mi].slot) {
            ordered.push(metas[mi].slot);
        }
        mi += 1;
    }
    ordered
}

const fn use_belongs_to_sub_slots(
    u: &NamedUseOrigin,
    def_id: noble_kernel::contracts::Definition,
    metas: &[EntryMeta],
    sub_slots: &[u32],
) -> bool {
    match u.caller_definition {
        Some(cd) => {
            if cd.0 == def_id.0 {
                true
            } else {
                match find_entry_meta_by_def(metas, cd) {
                    Some(idx) => contains_u32(sub_slots, metas[idx].slot),
                    None => false,
                }
            }
        }
        None => false,
    }
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; emits calleeSubject and EntryEmbedding for one caller occurrence"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; keeps per-occurrence Subject, ExactClosure, and EntryEmbedding cohesive"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits callee subject, embedding, and profile theorems"
)]
fn emit_callee_subject_and_embedding(
    mut out: String,
    uses: &[NamedUseOrigin],
    ui: usize,
    defs: &[noble_kernel::execution::Definition],
    metas: &[EntryMeta],
    max_slot: u32,
    span: Span,
) -> Result<String, Diagnostic> {
    let occ = &uses[ui];
    let slot = occ.definition.0;
    let Some(occ_node) = occ.source_node else {
        return Err(invalid(span, "occurrence lacks source node"));
    };
    let Some(m_idx) = find_entry_meta_by_slot(metas, slot) else {
        return Err(invalid(span, "occurrence entry missing"));
    };
    let meta = &metas[m_idx];
    let sub_slots = collect_transitive_slots(metas, slot);

    out.push_str("def calleeEnv_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(" : NamedV1.Environment := { entries := [");
    let mut si = 0usize;
    while si < sub_slots.len() {
        if si != 0 {
            out.push_str(", ");
        }
        out.push_str("entry_");
        crate::rendering::number(&mut out, u64::from(sub_slots[si]));
        si += 1;
    }
    out.push_str("] }\n");

    out.push_str("private theorem calleeClosure_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(" : ExactClosure calleeEnv_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(" calleeOrigin_");
    crate::rendering::number(&mut out, meta.origin_use_idx as u64);
    out.push_str(" entry_");
    crate::rendering::number(&mut out, u64::from(slot));
    out.push_str(".body entry_");
    crate::rendering::number(&mut out, u64::from(slot));
    out.push_str(".dependencies ");
    crate::rendering::number(&mut out, u64::from(meta.rank));
    out.push_str(" := by\n  simp [ExactClosure, calleeEnv_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(", entry_");
    crate::rendering::number(&mut out, u64::from(slot));
    let mut si = 0usize;
    while si < sub_slots.len() {
        out.push_str(", entry_");
        crate::rendering::number(&mut out, u64::from(sub_slots[si]));
        si += 1;
    }
    if !sub_slots.is_empty() {
        let mut all_ui = 1usize;
        while all_ui < uses.len() {
            if use_belongs_to_sub_slots(&uses[all_ui], meta.def_id, metas, &sub_slots) {
                out.push_str(", use_");
                crate::rendering::number(&mut out, all_ui as u64);
            }
            all_ui += 1;
        }
    }
    out.push_str(", usesCode, usesOp, usesValue, Environment.lookup, Entry.matches]\n");

    // CheckedEntries & typed in `calleeEnv_<ui>`.
    let env_name = fmt_str_num("calleeEnv_", ui as u64);
    if sub_slots.is_empty() {
        let thm_name = fmt_str_num("calleeTyped_", ui as u64);
        let code_ref = fmt_str_num_str("entry_", u64::from(slot), ".body");
        out = emit_code_typed_theorem(
            out,
            CodeTypedSpec {
                thm_name: &thm_name,
                env_name: &env_name,
                code_ref: &code_ref,
                def: &defs[meta.def_idx],
                defs,
                named_uses: uses,
                metas,
                span,
            },
        )?;
    } else {
        out =
            emit_code_typed_theorem_in_subenv(out, ui, slot, &sub_slots, uses, defs, metas, span)?;
    }

    out.push_str("def calleeSubject_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(" : NamedV1.Subject :=\n  { provenance := calleeOrigin_");
    crate::rendering::number(&mut out, meta.origin_use_idx as u64);
    out.push_str(", env := calleeEnv_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(", root := entry_");
    crate::rendering::number(&mut out, u64::from(slot));
    out.push_str(".body, dependencies := entry_");
    crate::rendering::number(&mut out, u64::from(slot));
    out.push_str(".dependencies, rank := ");
    crate::rendering::number(&mut out, u64::from(meta.rank));
    out.push_str(", closure := calleeClosure_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(", checkedEntries := ");
    if sub_slots.is_empty() {
        out.push_str("(fun _ h => nomatch h)");
    } else {
        out.push_str("(by\n    intro _ h\n    simp only [calleeEnv_");
        crate::rendering::number(&mut out, ui as u64);
        out.push_str(", List.mem_cons, List.mem_nil_iff, or_false] at h\n");
        if sub_slots.len() == 1 {
            out.push_str("    subst h\n    exact calleeSubTyped_");
            crate::rendering::number(&mut out, ui as u64);
            out.push('_');
            crate::rendering::number(&mut out, u64::from(sub_slots[0]));
            out.push_str(" []");
        } else {
            out.push_str("    rcases h with ");
            let mut si = 0usize;
            while si < sub_slots.len() {
                if si != 0 {
                    out.push_str(" | ");
                }
                out.push_str("rfl");
                si += 1;
            }
            out.push('\n');
            let mut si = 0usize;
            while si < sub_slots.len() {
                if si != 0 {
                    out.push('\n');
                }
                out.push_str("    · exact calleeSubTyped_");
                crate::rendering::number(&mut out, ui as u64);
                out.push('_');
                crate::rendering::number(&mut out, u64::from(sub_slots[si]));
                out.push_str(" []");
                si += 1;
            }
        }
        out.push(')');
    }
    out.push_str(", input := entry_");
    crate::rendering::number(&mut out, u64::from(slot));
    out.push_str(".input, output := entry_");
    crate::rendering::number(&mut out, u64::from(slot));
    out.push_str(".output, typed := calleeTyped_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(" [] }\n");

    let base_fresh = u64::from(max_slot).saturating_add(1);
    out.push_str("def calleeEmbedding_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(" : EntryEmbedding env calleeSubject_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(" entry_");
    crate::rendering::number(&mut out, u64::from(slot));
    out.push_str(" use_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(" where\n  embedding :=\n    { snapshot := ");
    if sub_slots.is_empty() {
        out.push_str("fun _ h => nomatch h\n");
    } else {
        out.push_str("by\n        intro entry he\n        simp only [calleeSubject_");
        crate::rendering::number(&mut out, ui as u64);
        out.push_str(", calleeEnv_");
        crate::rendering::number(&mut out, ui as u64);
        out.push_str(", List.mem_cons, List.mem_nil_iff, or_false] at he\n");
        if sub_slots.len() == 1 {
            out.push_str("        subst he; rfl\n");
        } else {
            out.push_str("        rcases he with ");
            let mut si = 0usize;
            while si < sub_slots.len() {
                if si != 0 {
                    out.push_str(" | ");
                }
                out.push_str("rfl");
                si += 1;
            }
            out.push_str(" <;> rfl\n");
        }
    }
    out.push_str("      rename := fun occ => if occ == ");
    crate::rendering::number(&mut out, u64::from(occ_node));
    out.push_str(" then ");
    crate::rendering::number(&mut out, u64::from(slot));
    out.push_str(" else ");
    crate::rendering::number(&mut out, base_fresh);
    out.push_str(" + occ\n      injective := by\n        intro a b h\n        dsimp only at h\n        split at h <;> split at h <;> simp_all <;> omega\n");
    out.push_str("      fresh := ");
    if sub_slots.is_empty() {
        out.push_str("fun _ => rfl }\n");
    } else {
        out.push_str("by\n        intro occ\n        split\n        · simp [calleeSubject_");
        crate::rendering::number(&mut out, ui as u64);
        out.push_str(", calleeEnv_");
        crate::rendering::number(&mut out, ui as u64);
        out.push_str(", NamedV1.Environment.lookup");
        let mut si = 0usize;
        while si < sub_slots.len() {
            out.push_str(", entry_");
            crate::rendering::number(&mut out, u64::from(sub_slots[si]));
            si += 1;
        }
        out.push_str("]\n        · simp [calleeSubject_");
        crate::rendering::number(&mut out, ui as u64);
        out.push_str(", calleeEnv_");
        crate::rendering::number(&mut out, ui as u64);
        out.push_str(", NamedV1.Environment.lookup");
        let mut si = 0usize;
        while si < sub_slots.len() {
            out.push_str(", entry_");
            crate::rendering::number(&mut out, u64::from(sub_slots[si]));
            si += 1;
        }
        out.push_str("]; omega }\n");
    }
    out.push_str("  placement := rfl\n  lookup := rfl\n  entryMatches := ⟨rfl, rfl, rfl, rfl⟩\n  body := rfl\n  input := rfl\n  output := rfl\n  origin := rfl\n");
    out.push_str("theorem calleeBodies_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(" : ∀ row ∈ calleeSubject_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(".env.entries, Sequential row.body := ");
    if sub_slots.is_empty() {
        out.push_str("fun _ h => nomatch h\n");
    } else {
        out.push_str("by\n  intro row hr\n  simp [calleeSubject_");
        crate::rendering::number(&mut out, ui as u64);
        out.push_str(", calleeEnv_");
        crate::rendering::number(&mut out, ui as u64);
        let mut si = 0usize;
        while si < sub_slots.len() {
            out.push_str(", entry_");
            crate::rendering::number(&mut out, u64::from(sub_slots[si]));
            si += 1;
        }
        out.push_str(", Sequential, Scalar] at hr ⊢\n  rcases hr with ");
        let mut si = 0usize;
        while si < sub_slots.len() {
            if si != 0 {
                out.push_str(" | ");
            }
            out.push_str("rfl");
            si += 1;
        }
        out.push_str(" <;> simp [Sequential, Scalar]\n");
    }
    out.push_str("theorem calleeProfile_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(" : Sequential calleeSubject_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(".root := by\n  simp [calleeSubject_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(", entry_");
    crate::rendering::number(&mut out, u64::from(slot));
    out.push_str(", Sequential, Scalar]\n");
    Ok(out)
}

#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-contracts; emits `CodeTyped` theorems inside a nested callee subgraph environment"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; emits `CodeTyped` theorems inside a nested callee subgraph environment"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends formatted theorem into caller string buffer"
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; distinguishes caller subenv name from occurrence index"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; formats Lean output into caller string buffer"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; bounded lookup and theorem emission helper"
)]
fn emit_subenv_step_one(
    out: &mut String,
    ui: usize,
    env_name: &str,
    sub_slots: &[u32],
    uses: &[NamedUseOrigin],
    defs: &[noble_kernel::execution::Definition],
    metas: &[EntryMeta],
    si: usize,
    span: Span,
) -> Result<(), Diagnostic> {
    let s = sub_slots[si];
    let Some(m_idx) = find_entry_meta_by_slot(metas, s) else {
        return Err(invalid(span, "subentry missing"));
    };
    let mut thm_name = String::from("calleeSubTyped_");
    crate::rendering::number(&mut thm_name, ui as u64);
    thm_name.push('_');
    crate::rendering::number(&mut thm_name, u64::from(s));
    let code_ref = fmt_str_num_str("entry_", u64::from(s), ".body");
    let next = emit_subenv_typed_one(
        core::mem::take(out),
        SubenvTypedSpec {
            ui,
            env_name,
            thm_name: &thm_name,
            code_ref: &code_ref,
            def: &defs[metas[m_idx].def_idx],
            defs,
            uses,
            metas,
            span,
        },
    )?;
    *out = next;
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-contracts; emits `CodeTyped` theorems across sub-environment entries"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; emits `CodeTyped` theorems across sub-environment entries"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits `CodeTyped` theorem targeting restricted callee sub-environment"
)]
fn emit_code_typed_theorem_in_subenv(
    mut out: String,
    ui: usize,
    root_slot: u32,
    sub_slots: &[u32],
    uses: &[NamedUseOrigin],
    defs: &[noble_kernel::execution::Definition],
    metas: &[EntryMeta],
    span: Span,
) -> Result<String, Diagnostic> {
    let env_name = fmt_str_num("calleeEnv_", ui as u64);
    let mut si = 0usize;
    let mut outcome = Ok(());
    while si < sub_slots.len() {
        outcome = emit_subenv_step_one(
            &mut out, ui, &env_name, sub_slots, uses, defs, metas, si, span,
        );
        if outcome.is_err() {
            break;
        }
        si += 1;
    }
    outcome?;
    let Some(r_idx) = find_entry_meta_by_slot(metas, root_slot) else {
        return Err(invalid(span, "callee root missing"));
    };
    let thm_name = fmt_str_num("calleeTyped_", ui as u64);
    let code_ref = fmt_str_num_str("entry_", u64::from(root_slot), ".body");
    out = emit_subenv_typed_one(
        out,
        SubenvTypedSpec {
            ui,
            env_name: &env_name,
            thm_name: &thm_name,
            code_ref: &code_ref,
            def: &defs[metas[r_idx].def_idx],
            defs,
            uses,
            metas,
            span,
        },
    )?;
    Ok(out)
}

struct SubenvTypedSpec<'a> {
    ui: usize,
    env_name: &'a str,
    thm_name: &'a str,
    code_ref: &'a str,
    def: &'a noble_kernel::execution::Definition,
    defs: &'a [noble_kernel::execution::Definition],
    uses: &'a [NamedUseOrigin],
    metas: &'a [EntryMeta],
    span: Span,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits step-by-step sub-environment `CodeTyped` theorem"
)]
fn emit_subenv_typed_one(mut out: String, spec: SubenvTypedSpec<'_>) -> Result<String, Diagnostic> {
    let SubenvTypedSpec {
        ui,
        env_name,
        thm_name,
        code_ref,
        def,
        defs,
        uses,
        metas,
        span,
    } = spec;
    let in_tys = &def.expected.stack_in;
    let out_tys = &def.expected.stack_out;
    out.push_str("private theorem ");
    out.push_str(thm_name);
    out.push_str(" (st : List Ty) : NamedV1.CodeTyped ");
    out.push_str(env_name);
    out.push(' ');
    out.push_str(code_ref);
    out.push_str(" (st ++ ");
    out = push_ty_list(out, in_tys)?;
    out.push_str(") (st ++ ");
    out = push_ty_list(out, out_tys)?;
    out.push_str(") := by\n");
    let mut cur = in_tys.clone();
    let body = &def.body.candidate.body;
    let mut pos = 0usize;
    let mut outcome = Ok(());
    while pos < body.len() {
        outcome = emit_code_typed_line(
            &mut out,
            env_name,
            Some(ui),
            def,
            defs,
            uses,
            metas,
            pos,
            &mut cur,
            span,
        );
        if outcome.is_err() {
            break;
        }
        pos += 1;
    }
    outcome?;
    out.push_str("  exact .nil\n");
    Ok(out)
}

/// Emit `NamedV3Obligation.lean` for one revision-3 contract goal.
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; emits one use entry with provenance and type lists"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; validates bounds and source provenance during emission"
)]
fn emit_obligation_use_entry(
    mut out: String,
    u: &NamedUseOrigin,
    ui: usize,
    selected_def: noble_kernel::contracts::Definition,
    defs: &[noble_kernel::execution::Definition],
    metas: &[EntryMeta],
    span: Span,
) -> Result<String, Diagnostic> {
    let Some(occ_node) = u.source_node else {
        return Err(invalid(span, "use lacks source node"));
    };
    let mut di = 0usize;
    let mut found_idx = None;
    while di < defs.len() && found_idx.is_none() {
        if defs[di].definition.0 == u.definition.0 {
            found_idx = Some(di);
        }
        di = di.saturating_add(1);
    }
    let Some(target_idx) = found_idx else {
        return Err(invalid(span, "use target definition absent"));
    };
    let target_def = &defs[target_idx];
    out.push_str("def use_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(" : NamedV1.Use := { slot := ");
    crate::rendering::number(&mut out, u64::from(u.definition.0));
    out.push_str(", occurrence := ");
    crate::rendering::number(&mut out, u64::from(occ_node));
    out.push_str(", caller := ");
    match u.caller_definition {
        Some(c_def) if c_def == selected_def => out.push_str("caller"),
        Some(c_def) => {
            let Some(m_idx) = find_entry_meta_by_def(metas, c_def) else {
                return Err(invalid(span, "nested caller entry missing"));
            };
            out.push_str("calleeOrigin_");
            crate::rendering::number(&mut out, metas[m_idx].origin_use_idx as u64);
        }
        None => out.push_str("caller"),
    }
    out.push_str(", origin := calleeOrigin_");
    crate::rendering::number(&mut out, ui as u64);
    out.push_str(", input := ");
    out = push_ty_list(out, &target_def.expected.stack_in)?;
    out.push_str(", output := ");
    out = push_ty_list(out, &target_def.expected.stack_out)?;
    out.push_str(" }\n");
    Ok(out)
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends formatted provenance into caller string buffer"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; formats Lean output into caller string buffer"
)]
fn emit_obligation_origin_step(
    out: &mut String,
    goal: &crate::intrinsic::ContractGoal,
    origin: &NamedUseOrigin,
    ui: usize,
) -> Result<(), Diagnostic> {
    out.push_str("private def calleeOrigin_");
    crate::rendering::number(out, ui as u64);
    out.push_str(" : Provenance := ");
    let next = emit_provenance(core::mem::take(out), goal, origin)?;
    *out = next;
    out.push('\n');
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends formatted entry into caller string buffer"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; formats Lean output into caller string buffer"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits fixed entry metadata and dependency list"
)]
fn emit_obligation_entry_step(
    out: &mut String,
    meta: &EntryMeta,
    defs: &[noble_kernel::execution::Definition],
    uses: &[NamedUseOrigin],
    span: Span,
) -> Result<(), Diagnostic> {
    let def = &defs[meta.def_idx];
    out.push_str("def entry_");
    crate::rendering::number(out, u64::from(meta.slot));
    out.push_str(" : NamedV1.Entry := { slot := ");
    crate::rendering::number(out, u64::from(meta.slot));
    out.push_str(", origin := calleeOrigin_");
    crate::rendering::number(out, meta.origin_use_idx as u64);
    out.push_str(", input := ");
    let s1 = push_ty_list(core::mem::take(out), &def.expected.stack_in)?;
    *out = s1;
    out.push_str(", output := ");
    let s2 = push_ty_list(core::mem::take(out), &def.expected.stack_out)?;
    *out = s2;
    out.push_str(", body := ");
    let s3 = emit_entry_code(core::mem::take(out), def, uses, span)?;
    *out = s3;
    out.push_str(", dependencies := [");
    let mut di = 0usize;
    while di < meta.deps.len() {
        if di != 0 {
            out.push_str(", ");
        }
        crate::rendering::number(out, u64::from(meta.deps[di]));
        di += 1;
    }
    out.push_str("], rank := ");
    crate::rendering::number(out, u64::from(meta.rank));
    out.push_str(" }\n");
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends formatted typed entry theorem into caller string buffer"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; formats Lean output into caller string buffer"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; passes entry metadata and graph context to `emit_code_typed_theorem`"
)]
fn emit_obligation_typed_step(
    out: &mut String,
    meta: &EntryMeta,
    defs: &[noble_kernel::execution::Definition],
    uses: &[NamedUseOrigin],
    metas: &[EntryMeta],
    span: Span,
) -> Result<(), Diagnostic> {
    let thm = fmt_str_num("typed_entry_", u64::from(meta.slot));
    let code_ref = fmt_str_num_str("entry_", u64::from(meta.slot), ".body");
    let next = emit_code_typed_theorem(
        core::mem::take(out),
        CodeTypedSpec {
            thm_name: &thm,
            env_name: "env",
            code_ref: &code_ref,
            def: &defs[meta.def_idx],
            defs,
            named_uses: uses,
            metas,
            span,
        },
    )?;
    *out = next;
    Ok(())
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; emits the complete self-contained `NamedV3Obligation` module in deterministic order"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; validates bounds and source provenance during emission"
)]
pub(super) fn obligation(
    goal: &crate::intrinsic::ContractGoal,
    view: &super::SubjectView,
    requires: &Prop3,
    ensures: &Prop3,
    limits: crate::Limits,
) -> Result<String, Diagnostic> {
    let span = goal.subject.source_span;
    let uses = &goal.subject.named_uses;
    let defs = &goal.subject.accepted_submission.definitions;
    let selected = &defs[view.selected_idx];
    let (metas, root_rank) = collect_entries_meta(goal, selected.definition)?;
    let mut max_slot = 0u32;
    let mut msi = 0usize;
    while msi < metas.len() {
        if metas[msi].slot > max_slot {
            max_slot = metas[msi].slot;
        }
        msi = msi.saturating_add(1);
    }
    let mut out = String::from(
        "import NobleContracts.NamedV3Lifted\nopen NobleContracts\nnamespace NamedV3Obligation\nopen NobleContracts.NamedV1\nopen NobleContracts.NamedV3\n\n",
    );
    out.push_str("private def caller : Provenance := ");
    out = emit_provenance(out, goal, &uses[0])?;
    out.push('\n');
    let mut outcome = Ok(());
    let mut ui = 1usize;
    while ui < uses.len() {
        outcome = emit_obligation_origin_step(&mut out, goal, &uses[ui], ui);
        if outcome.is_err() {
            break;
        }
        ui += 1;
    }
    outcome?;
    let mut outcome = Ok(());
    let mut ui = 1usize;
    while ui < uses.len() {
        outcome = match emit_obligation_use_entry(
            core::mem::take(&mut out),
            &uses[ui],
            ui,
            selected.definition,
            defs,
            &metas,
            span,
        ) {
            Ok(next) => {
                out = next;
                Ok(())
            }
            Err(problem) => Err(problem),
        };
        if outcome.is_err() {
            break;
        }
        ui += 1;
    }
    outcome?;
    let mut outcome = Ok(());
    let mut mi = 0usize;
    while mi < metas.len() {
        outcome = emit_obligation_entry_step(&mut out, &metas[mi], defs, uses, span);
        if outcome.is_err() {
            break;
        }
        mi += 1;
    }
    outcome?;
    out.push_str("def env : NamedV1.Environment := { entries := [");
    let mut mi = 0usize;
    while mi < metas.len() {
        if mi != 0 {
            out.push_str(", ");
        }
        out.push_str("entry_");
        crate::rendering::number(&mut out, u64::from(metas[mi].slot));
        mi += 1;
    }
    out.push_str("] }\n");
    out.push_str("def root : List NamedV1.Op := ");
    out = emit_entry_code(out, selected, uses, span)?;
    out.push('\n');
    let mut root_deps: Vec<u32> = Vec::with_capacity(uses.len());
    let mut ui = 1usize;
    while ui < uses.len() {
        if opt_def_eq(uses[ui].caller_definition, selected.definition) {
            let slot = uses[ui].definition.0;
            if !contains_u32(&root_deps, slot) {
                root_deps.push(slot);
            }
        }
        ui += 1;
    }
    out.push_str("private def rootDeps : List Nat := [");
    let mut di = 0usize;
    while di < root_deps.len() {
        if di != 0 {
            out.push_str(", ");
        }
        crate::rendering::number(&mut out, u64::from(root_deps[di]));
        di += 1;
    }
    out.push_str("]\nprivate theorem closure : ExactClosure env caller root rootDeps ");
    crate::rendering::number(&mut out, u64::from(root_rank));
    out.push_str(" := by\n  simp [ExactClosure, env, root, rootDeps");
    let mut mi = 0usize;
    while mi < metas.len() {
        out.push_str(", entry_");
        crate::rendering::number(&mut out, u64::from(metas[mi].slot));
        mi += 1;
    }
    let mut ui = 1usize;
    while ui < uses.len() {
        out.push_str(", use_");
        crate::rendering::number(&mut out, ui as u64);
        ui += 1;
    }
    out.push_str(", usesCode, usesOp, usesValue, Environment.lookup, Entry.matches]\n");
    let mut outcome = Ok(());
    let mut mi = 0usize;
    while mi < metas.len() {
        outcome = emit_obligation_typed_step(&mut out, &metas[mi], defs, uses, &metas, span);
        if outcome.is_err() {
            break;
        }
        mi += 1;
    }
    outcome?;
    out = emit_code_typed_theorem(
        out,
        CodeTypedSpec {
            thm_name: "typed_root_aux",
            env_name: "env",
            code_ref: "root",
            def: selected,
            defs,
            named_uses: uses,
            metas: &metas,
            span,
        },
    )?;
    out.push_str("private theorem typedRoot : NamedV1.CodeTyped env root ");
    out = push_ty_list(out, &goal.subject.input_types)?;
    out.push(' ');
    out = push_ty_list(out, &goal.subject.output_types)?;
    out.push_str(" := typed_root_aux []\n");
    out.push_str("def subject : NamedV1.Subject :=\n  { provenance := caller, env := env, root := root, dependencies := rootDeps, rank := ");
    crate::rendering::number(&mut out, u64::from(root_rank));
    out.push_str(", closure := closure, checkedEntries := ");
    if metas.is_empty() {
        out.push_str("(fun _ h => nomatch h)");
    } else {
        out.push_str("(by\n    intro _ h\n    simp only [env, List.mem_cons, List.mem_nil_iff, or_false] at h\n");
        if metas.len() == 1 {
            out.push_str("    subst h\n    exact typed_entry_");
            crate::rendering::number(&mut out, u64::from(metas[0].slot));
            out.push_str(" []");
        } else {
            out.push_str("    rcases h with ");
            let mut mi = 0usize;
            while mi < metas.len() {
                if mi != 0 {
                    out.push_str(" | ");
                }
                out.push_str("rfl");
                mi += 1;
            }
            out.push('\n');
            let mut mi = 0usize;
            while mi < metas.len() {
                if mi != 0 {
                    out.push('\n');
                }
                out.push_str("    · exact typed_entry_");
                crate::rendering::number(&mut out, u64::from(metas[mi].slot));
                out.push_str(" []");
                mi += 1;
            }
        }
        out.push(')');
    }
    out.push_str(", input := ");
    out = push_ty_list(out, &goal.subject.input_types)?;
    out.push_str(", output := ");
    out = push_ty_list(out, &goal.subject.output_types)?;
    out.push_str(", typed := typedRoot }\n");
    let mut outcome = Ok(());
    let mut ui = 1usize;
    while ui < uses.len() {
        if opt_def_eq(uses[ui].caller_definition, selected.definition) {
            outcome = match emit_callee_subject_and_embedding(
                core::mem::take(&mut out),
                uses,
                ui,
                defs,
                &metas,
                max_slot,
                span,
            ) {
                Ok(next) => {
                    out = next;
                    Ok(())
                }
                Err(problem) => Err(problem),
            };
            if outcome.is_err() {
                break;
            }
        }
        ui += 1;
    }
    outcome?;
    out.push_str("def precondition : NobleContracts.Term := ");
    out = emit_noble_term_from_prop3(out, requires)?;
    out.push_str("\ndef postcondition : NobleContracts.Term := ");
    out = emit_noble_term_from_prop3(out, ensures)?;
    out.push_str(
        "\ndef claim : Prop := exportedLiftedClaim subject (Holds precondition) (Holds postcondition)\n",
    );
    out.push_str("end NamedV3Obligation\n");
    let is_within_work = match u32::try_from(out.len()) {
        Ok(len) => len <= limits.work,
        Err(_) => false,
    };
    let is_within_bytes = match (
        u64::try_from(out.len()),
        u64::from(limits.bytes).checked_mul(128),
    ) {
        (Ok(len), Some(cap)) => len <= cap,
        _ => false,
    };
    if !is_within_work || !is_within_bytes {
        return Err(invalid(
            goal.contract_span,
            "revision-3 obligation exceeds bounded Lean source budget",
        ));
    }
    Ok(out)
}

// =========================================================================
// Closed Tactic-Free `LiftedChain` Proof Lowering (`lower::proof`)
// =========================================================================

fn find_use_idx_by_occ(named_uses: &[NamedUseOrigin], occ: &NamedUseOrigin) -> Option<usize> {
    let mut i = 1usize;
    let mut found = None;
    while found.is_none() && i < named_uses.len() {
        if named_uses[i].candidate_node.0 == occ.candidate_node.0
            && named_uses[i].definition.0 == occ.definition.0
        {
            found = Some(i);
        }
        i += 1;
    }
    found
}

#[octet::sealed_enum]
enum Term3LeanText {
    CloseParen,
    AddSep,
    SubSep,
    MulSep,
    EqMid,
    EqClose,
}

#[octet::sealed_enum]
enum Term3LeanStep {
    Visit(Term3),
    Text(Term3LeanText),
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; step and output indices form composite ghost identifier"
)]
const fn choose_str(cond: bool, on_true: &'static str, on_false: &'static str) -> &'static str {
    if cond {
        on_true
    } else {
        on_false
    }
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; step_id and out_id match ghost tuple (u32, u32, String)"
)]
fn find_ghost_name(ghosts: &[(u32, u32, String)], step_id: u32, out_id: u32) -> Option<&str> {
    let mut found = None;
    let mut gi = 0usize;
    while found.is_none() && gi < ghosts.len() {
        if ghosts[gi].0 == step_id && ghosts[gi].1 == out_id {
            found = Some(gi);
        }
        gi += 1;
    }
    match found {
        Some(idx) => Some(ghosts[idx].2.as_str()),
        None => None,
    }
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends formatted Lean term into caller output and worklist"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; formats Lean AST into caller buffers"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; passes Lean binder scopes and iterative worklist"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; pushes dynamically onto caller `String` and `Vec` worklist"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; validates input, bound, and ghost scopes with typed diagnostics"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; keeps every `Term3` emission case in one flat switch"
)]
fn emit_term3_lean_step(
    step: Term3LeanStep,
    out: &mut String,
    work: &mut Vec<Term3LeanStep>,
    bound_names: &[String],
    inputs: &[String],
    ghosts: &[(u32, u32, String)],
) -> Result<(), Diagnostic> {
    match step {
        Term3LeanStep::Text(kind) => match kind {
            Term3LeanText::CloseParen => out.push(')'),
            Term3LeanText::AddSep => out.push_str(" + "),
            Term3LeanText::SubSep => out.push_str(" - "),
            Term3LeanText::MulSep => out.push_str(" * "),
            Term3LeanText::EqMid => out.push_str(") = ("),
            Term3LeanText::EqClose => out.push_str(")) (Classical.propDecidable _))"),
        },
        Term3LeanStep::Visit(node) => match node {
            Term3::Input(i) => {
                let Some(v) = inputs.get(i) else {
                    return Err(invalid(Span { start: 0, end: 0 }, "input out of range"));
                };
                out.push_str(v);
            }
            Term3::Output(_) => {
                return Err(invalid(
                    Span { start: 0, end: 0 },
                    "unexpected uninstantiated output binder in proof term",
                ));
            }
            Term3::Bound(i) => {
                let Some(v) = bound_names.get(i) else {
                    return Err(invalid(Span { start: 0, end: 0 }, "bound var oob"));
                };
                out.push_str(v);
            }
            Term3::Ghost(step_id, out_id, _) => {
                let Some(v) = find_ghost_name(ghosts, step_id, out_id) else {
                    return Err(invalid(
                        Span { start: 0, end: 0 },
                        "ghost variable not in scope",
                    ));
                };
                out.push_str(v);
            }
            Term3::I64(n) => {
                out.push_str("(BitVec.ofInt 64 (");
                crate::rendering::integer(out, n);
                out.push_str("))");
            }
            Term3::Bool(b) => out.push_str(if b { "true" } else { "false" }),
            Term3::Add(l, r) => {
                out.push('(');
                work.push(Term3LeanStep::Text(Term3LeanText::CloseParen));
                work.push(Term3LeanStep::Visit(*r));
                work.push(Term3LeanStep::Text(Term3LeanText::AddSep));
                work.push(Term3LeanStep::Visit(*l));
            }
            Term3::Sub(l, r) => {
                out.push('(');
                work.push(Term3LeanStep::Text(Term3LeanText::CloseParen));
                work.push(Term3LeanStep::Visit(*r));
                work.push(Term3LeanStep::Text(Term3LeanText::SubSep));
                work.push(Term3LeanStep::Visit(*l));
            }
            Term3::Mul(l, r) => {
                out.push('(');
                work.push(Term3LeanStep::Text(Term3LeanText::CloseParen));
                work.push(Term3LeanStep::Visit(*r));
                work.push(Term3LeanStep::Text(Term3LeanText::MulSep));
                work.push(Term3LeanStep::Visit(*l));
            }
            Term3::Equals(l, r) => {
                out.push_str("(@decide ((");
                work.push(Term3LeanStep::Text(Term3LeanText::EqClose));
                work.push(Term3LeanStep::Visit(*r));
                work.push(Term3LeanStep::Text(Term3LeanText::EqMid));
                work.push(Term3LeanStep::Visit(*l));
            }
        },
    }
    Ok(())
}

fn emit_term3_lean(
    mut out: String,
    term: &Term3,
    bound_names: &[String],
    inputs: &[String],
    ghosts: &[(u32, u32, String)],
) -> Result<String, Diagnostic> {
    let mut work = Vec::new();
    work.push(Term3LeanStep::Visit(term.clone()));
    let mut outcome = Ok(());
    while let Some(step) = work.pop() {
        outcome = emit_term3_lean_step(step, &mut out, &mut work, bound_names, inputs, ghosts);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    Ok(out)
}

fn emit_term3_value_lean(
    mut out: String,
    term: &Term3,
    sort: Sort3,
    inputs: &[String],
    ghosts: &[(u32, u32, String)],
) -> Result<String, Diagnostic> {
    out.push_str(match sort {
        Sort3::I64 => "(NobleContracts.Value.i64 ",
        Sort3::Bool => "(NobleContracts.Value.bool ",
    });
    out = emit_term3_lean(out, term, &[], inputs, ghosts)?;
    out.push(')');
    Ok(out)
}

#[octet::sealed_enum]
enum Prop3LeanText {
    CloseParen,
    AndSep,
}

#[octet::sealed_enum]
enum Prop3LeanStep {
    Visit(Prop3),
    Text(Prop3LeanText),
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends formatted Lean proposition into caller output and worklist"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; formats Lean proposition AST into caller buffers"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; passes Lean binder scopes and iterative worklist"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; bounded iterative `Prop3` emission step"
)]
fn emit_prop3_lean_step(
    step: Prop3LeanStep,
    out: &mut String,
    work: &mut Vec<Prop3LeanStep>,
    bound_names: &[String],
    inputs: &[String],
    ghosts: &[(u32, u32, String)],
) -> Result<(), Diagnostic> {
    match step {
        Prop3LeanStep::Text(Prop3LeanText::CloseParen) => out.push(')'),
        Prop3LeanStep::Text(Prop3LeanText::AndSep) => out.push_str(" ∧ "),
        Prop3LeanStep::Visit(node) => match node {
            Prop3::True => out.push_str("True"),
            Prop3::False => out.push_str("False"),
            Prop3::Not(b) => {
                out.push_str("(¬ ");
                work.push(Prop3LeanStep::Text(Prop3LeanText::CloseParen));
                work.push(Prop3LeanStep::Visit(*b));
            }
            Prop3::And(l, r) => {
                out.push('(');
                work.push(Prop3LeanStep::Text(Prop3LeanText::CloseParen));
                work.push(Prop3LeanStep::Visit(*r));
                work.push(Prop3LeanStep::Text(Prop3LeanText::AndSep));
                work.push(Prop3LeanStep::Visit(*l));
            }
            Prop3::Eq(l, r) => {
                out.push('(');
                let s1 = emit_term3_lean(core::mem::take(out), &l, bound_names, inputs, ghosts)?;
                *out = s1;
                out.push_str(" = ");
                let s2 = emit_term3_lean(core::mem::take(out), &r, bound_names, inputs, ghosts)?;
                *out = s2;
                out.push(')');
            }
            Prop3::Lt(l, r) => {
                out.push_str("((");
                let s1 = emit_term3_lean(core::mem::take(out), &l, bound_names, inputs, ghosts)?;
                *out = s1;
                out.push_str(").toInt < (");
                let s2 = emit_term3_lean(core::mem::take(out), &r, bound_names, inputs, ghosts)?;
                *out = s2;
                out.push_str(").toInt)");
            }
            Prop3::Le(l, r) => {
                out.push_str("((");
                let s1 = emit_term3_lean(core::mem::take(out), &l, bound_names, inputs, ghosts)?;
                *out = s1;
                out.push_str(").toInt ≤ (");
                let s2 = emit_term3_lean(core::mem::take(out), &r, bound_names, inputs, ghosts)?;
                *out = s2;
                out.push_str(").toInt)");
            }
        },
    }
    Ok(())
}

fn emit_prop3_lean(
    mut out: String,
    prop: &Prop3,
    bound_names: &[String],
    inputs: &[String],
    ghosts: &[(u32, u32, String)],
) -> Result<String, Diagnostic> {
    let mut work = Vec::new();
    work.push(Prop3LeanStep::Visit(prop.clone()));
    let mut outcome = Ok(());
    while let Some(step) = work.pop() {
        outcome = emit_prop3_lean_step(step, &mut out, &mut work, bound_names, inputs, ghosts);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    Ok(out)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; bounded iterative `Statement3` emission"
)]
fn emit_statement3_lean(mut out: String, stmt: &Statement3) -> Result<String, Diagnostic> {
    let (binders, cur) = super::peel_statement_pi(stmt);
    let mut names = Vec::with_capacity(binders.len());
    let mut bi = 0usize;
    while bi < binders.len() {
        let mut vname = String::from("bv_");
        crate::rendering::number(&mut vname, bi as u64);
        out.push_str("(∀ (");
        out.push_str(&vname);
        out.push_str(" : ");
        push_sort_lean_type_buf(&mut out, binders[bi].1);
        out.push_str("), ");
        names.push(vname);
        bi += 1;
    }
    let Statement3::Prop(prop) = cur else {
        return Err(invalid(Span { start: 0, end: 0 }, "unclosed Pi"));
    };
    out = emit_prop3_lean(out, prop, &names, &[], &[])?;
    let mut ci = 0usize;
    while ci < binders.len() {
        out.push(')');
        ci += 1;
    }
    Ok(out)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits fixed Lean `Prop` signature for callee hypothesis"
)]
fn emit_callee_param_type(
    mut out: String,
    goal: &crate::intrinsic::ContractGoal,
    occ_idx_for_rev3: usize,
    limits: crate::Limits,
) -> Result<String, Diagnostic> {
    if goal.revision == 1 {
        let prepared = super::super::prepare_contract(goal, limits)?;
        let mut in_tys: Vec<noble_kernel::types::Ty> = Vec::with_capacity(prepared.inputs().len());
        let mut ti = 0usize;
        while ti < prepared.inputs().len() {
            in_tys.push(prepared.inputs()[ti].ty.clone());
            ti = ti.saturating_add(1);
        }
        let mut out_tys: Vec<noble_kernel::types::Ty> =
            Vec::with_capacity(prepared.outputs().len());
        let mut to = 0usize;
        while to < prepared.outputs().len() {
            out_tys.push(prepared.outputs()[to].ty.clone());
            to = to.saturating_add(1);
        }
        out.push_str("NobleContracts.exportedClaim ");
        out = emit_prepared_code(out, prepared.candidate(), goal.contract_span)?;
        out.push(' ');
        out = push_ty_list(out, &in_tys)?;
        out.push(' ');
        out = push_ty_list(out, &out_tys)?;
        out.push_str(" [] (NobleContracts.Holds ");
        out = emit_prepared_expr(
            out,
            prepared.expressions(),
            prepared.definitions(),
            prepared.requires(),
        )?;
        out.push_str(") (NobleContracts.Holds ");
        out = emit_prepared_expr(
            out,
            prepared.expressions(),
            prepared.definitions(),
            prepared.ensures(),
        )?;
        out.push(')');
        return Ok(out);
    }
    let Some(tree_bounds) = crate::intrinsic::TreeBounds::new(limits) else {
        return Err(invalid(goal.contract_span, "limits overflow"));
    };
    let mut dummy_config = crate::intrinsic::MeterConfig {
        budgets: crate::intrinsic::ProofBudgets::from_limits(limits),
        limits,
        tree_bounds,
    };
    let _ = &mut dummy_config;
    let mut dummy_meter = crate::intrinsic::Meter {
        nodes: 0,
        work: 0,
        normalization: 0,
        substitution: 0,
        config: &dummy_config,
    };
    let inputs = super::interface(&goal.inputs, goal.contract_span)?;
    let outputs = super::interface(&goal.outputs, goal.contract_span)?;
    let req = super::assertion(&goal.requires, &inputs, &outputs, &mut dummy_meter)?;
    let ens = super::assertion(&goal.ensures, &inputs, &outputs, &mut dummy_meter)?;
    out.push_str("NobleContracts.NamedV3.exportedLiftedClaim NamedV3Obligation.calleeSubject_");
    crate::rendering::number(&mut out, occ_idx_for_rev3 as u64);
    out.push_str(" (NobleContracts.Holds ");
    out = emit_noble_term_from_prop3(out, &req)?;
    out.push_str(") (NobleContracts.Holds ");
    out = emit_noble_term_from_prop3(out, &ens)?;
    out.push(')');
    Ok(out)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; formats prepared code slice as Lean expression"
)]
fn emit_prepared_code(
    mut out: String,
    candidate: &noble_kernel::untrusted::Candidate,
    span: Span,
) -> Result<String, Diagnostic> {
    out.push('[');
    let mut idx = 0usize;
    while idx < candidate.body.len() {
        if idx != 0 {
            out.push_str(", ");
        }
        let Ok(nid) = usize::try_from(candidate.body[idx].0) else {
            return Err(invalid(span, "node id overflow"));
        };
        let Some(node) = candidate.nodes.get(nid) else {
            return Err(invalid(span, "missing node"));
        };
        match node {
            Node::Literal {
                lit: Lit::I64(v), ..
            } => {
                out.push_str(".lit (.i64 (BitVec.ofInt 64 (");
                crate::rendering::integer(&mut out, *v);
                out.push_str(")))");
            }
            Node::Literal {
                lit: Lit::Bool(b), ..
            } => out.push_str(if *b {
                ".lit (.bool true)"
            } else {
                ".lit (.bool false)"
            }),
            Node::Invocation { def, .. } => {
                out.push_str(".word ");
                crate::rendering::number(&mut out, u64::from(def.0));
            }
            Node::Literal { .. }
            | Node::SignedSelf { .. }
            | Node::Quotation { .. }
            | Node::SlotInvoke { .. } => {
                return Err(crate::intrinsic::unsupported(span, "unsupported MC1 node"));
            }
        }
        idx += 1;
    }
    out.push(']');
    Ok(out)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; bounded iterative emission for `Term` expression"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; lowers every `Term` variant in one iterative loop"
)]
fn emit_prepared_expr(
    mut out: String,
    exprs: &[crate::Expr],
    definitions: &[crate::LogicDef],
    root: u32,
) -> Result<String, Diagnostic> {
    #[octet::sealed_enum]
    enum Step {
        Visit(u32),
        Text(&'static str),
    }
    let mut work = Vec::from([Step::Visit(root)]);
    while let Some(step) = work.pop() {
        match step {
            Step::Text(s) => out.push_str(s),
            Step::Visit(id) => {
                let Ok(idx) = usize::try_from(id) else {
                    return Err(invalid(Span { start: 0, end: 0 }, "expr index overflow"));
                };
                let Some(expr) = exprs.get(idx) else {
                    return Err(invalid(Span { start: 0, end: 0 }, "missing expr"));
                };
                match &expr.kind {
                    crate::ExprKind::I64(v) => {
                        out.push_str("(.i64 (");
                        crate::rendering::integer(&mut out, *v);
                        out.push_str("))");
                    }
                    crate::ExprKind::Bool(b) => {
                        out.push_str(if *b { "(.bool true)" } else { "(.bool false)" });
                    }
                    crate::ExprKind::Unit => out.push_str(".unit"),
                    crate::ExprKind::Input(n) => {
                        out.push_str("(.input ");
                        crate::rendering::number(&mut out, u64::from(*n));
                        out.push(')');
                    }
                    crate::ExprKind::Output(n) => {
                        out.push_str("(.output ");
                        crate::rendering::number(&mut out, u64::from(*n));
                        out.push(')');
                    }
                    crate::ExprKind::Param(n) => {
                        out.push_str("(.param ");
                        crate::rendering::number(&mut out, u64::from(*n));
                        out.push(')');
                    }
                    crate::ExprKind::Definition(d) => {
                        let Ok(def_idx) = usize::try_from(*d) else {
                            return Err(invalid(Span { start: 0, end: 0 }, "def index overflow"));
                        };
                        let Some(def_entry) = definitions.get(def_idx) else {
                            return Err(invalid(Span { start: 0, end: 0 }, "missing def"));
                        };
                        let body = def_entry.body;
                        out.push_str("(.definition ");
                        crate::rendering::number(&mut out, u64::from(*d));
                        out.push(' ');
                        work.push(Step::Text(")"));
                        work.push(Step::Visit(body));
                    }
                    crate::ExprKind::Not(b) => {
                        out.push_str("(.not ");
                        work.push(Step::Text(")"));
                        work.push(Step::Visit(*b));
                    }
                    crate::ExprKind::And(a, b) => {
                        out.push_str("(.and ");
                        work.push(Step::Text(")"));
                        work.push(Step::Visit(*b));
                        work.push(Step::Text(" "));
                        work.push(Step::Visit(*a));
                    }
                    crate::ExprKind::Or(a, b) => {
                        out.push_str("(.or ");
                        work.push(Step::Text(")"));
                        work.push(Step::Visit(*b));
                        work.push(Step::Text(" "));
                        work.push(Step::Visit(*a));
                    }
                    crate::ExprKind::Implies(a, b) => {
                        out.push_str("(.implies ");
                        work.push(Step::Text(")"));
                        work.push(Step::Visit(*b));
                        work.push(Step::Text(" "));
                        work.push(Step::Visit(*a));
                    }
                    crate::ExprKind::Eq(a, b) => {
                        out.push_str("(.eq ");
                        work.push(Step::Text(")"));
                        work.push(Step::Visit(*b));
                        work.push(Step::Text(" "));
                        work.push(Step::Visit(*a));
                    }
                    crate::ExprKind::Lt(a, b) => {
                        out.push_str("(.lt ");
                        work.push(Step::Text(")"));
                        work.push(Step::Visit(*b));
                        work.push(Step::Text(" "));
                        work.push(Step::Visit(*a));
                    }
                    crate::ExprKind::Le(a, b) => {
                        out.push_str("(.le ");
                        work.push(Step::Text(")"));
                        work.push(Step::Visit(*b));
                        work.push(Step::Text(" "));
                        work.push(Step::Visit(*a));
                    }
                    crate::ExprKind::Add(a, b) => {
                        out.push_str("(.add ");
                        work.push(Step::Text(")"));
                        work.push(Step::Visit(*b));
                        work.push(Step::Text(" "));
                        work.push(Step::Visit(*a));
                    }
                    crate::ExprKind::Sub(a, b) => {
                        out.push_str("(.sub ");
                        work.push(Step::Text(")"));
                        work.push(Step::Visit(*b));
                        work.push(Step::Text(" "));
                        work.push(Step::Visit(*a));
                    }
                    crate::ExprKind::Mul(a, b) => {
                        out.push_str("(.mul ");
                        work.push(Step::Text(")"));
                        work.push(Step::Visit(*b));
                        work.push(Step::Text(" "));
                        work.push(Step::Visit(*a));
                    }
                    crate::ExprKind::Pair(..)
                    | crate::ExprKind::First(_)
                    | crate::ExprKind::Second(_)
                    | crate::ExprKind::Inl(_)
                    | crate::ExprKind::Inr(_)
                    | crate::ExprKind::IsLeft(_)
                    | crate::ExprKind::Left(_)
                    | crate::ExprKind::Right(_)
                    | crate::ExprKind::Nil
                    | crate::ExprKind::Cons(..)
                    | crate::ExprKind::IsNil(_)
                    | crate::ExprKind::Head(_)
                    | crate::ExprKind::Tail(_)
                    | crate::ExprKind::Length(_)
                    | crate::ExprKind::Maps(..) => {
                        return Err(crate::intrinsic::unsupported(
                            Span { start: 0, end: 0 },
                            "unsupported MC1 expression form in Lean lowering",
                        ));
                    }
                }
            }
        }
    }
    Ok(out)
}

#[derive(Clone, Copy)]
#[octet::sealed_enum]
enum EvalCombKind {
    Add,
    Sub,
    Mul,
}

#[octet::sealed_enum]
enum EvalTermStep {
    Visit(Term3, Sort3),
    Combine(EvalCombKind),
    CombineEq,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; pure sort predicate over sealed Term3 variants"
)]
const fn bool_or_i64_sort(left: &Term3, right: &Term3) -> Sort3 {
    let is_left_bool = match left {
        Term3::Bool(_) => true,
        Term3::Input(_)
        | Term3::Output(_)
        | Term3::Bound(_)
        | Term3::Ghost(..)
        | Term3::I64(_)
        | Term3::Add(..)
        | Term3::Sub(..)
        | Term3::Mul(..)
        | Term3::Equals(..) => false,
    };
    let is_right_bool = match right {
        Term3::Bool(_) => true,
        Term3::Input(_)
        | Term3::Output(_)
        | Term3::Bound(_)
        | Term3::Ghost(..)
        | Term3::I64(_)
        | Term3::Add(..)
        | Term3::Sub(..)
        | Term3::Mul(..)
        | Term3::Equals(..) => false,
    };
    if is_left_bool || is_right_bool {
        Sort3::Bool
    } else {
        Sort3::I64
    }
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; bounded iterative evaluator step over `Term3`"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends evaluation witnesses into caller worklist and proof stack"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; mutates iterative evaluation worklist and proof stack"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; validates term variable bounds with typed diagnostics"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; handles all `Term3` evaluation cases in one flat switch"
)]
fn emit_eval_term3_step(
    step: EvalTermStep,
    ctx_expr: &str,
    before_vals: &[String],
    after_vals: &[String],
    work: &mut Vec<EvalTermStep>,
    proofs: &mut Vec<(String, String, Sort3)>,
) -> Result<(), Diagnostic> {
    match step {
        EvalTermStep::Visit(node, expected_sort) => match node {
            Term3::Input(i) => {
                let Some(val) = before_vals.get(i) else {
                    return Err(invalid(Span { start: 0, end: 0 }, "input index oob"));
                };
                let mut p = String::from("(rfl : NobleContracts.evaluate (.input ");
                crate::rendering::number(&mut p, i as u64);
                p.push_str(") ");
                p.push_str(ctx_expr);
                p.push_str(" = some ");
                p.push_str(match expected_sort {
                    Sort3::I64 => "(NobleContracts.Value.i64 (",
                    Sort3::Bool => "(NobleContracts.Value.bool (",
                });
                p.push_str(val);
                p.push_str(")))");
                proofs.push((p, val.clone(), expected_sort));
            }
            Term3::Output(i) => {
                let Some(val) = after_vals.get(i) else {
                    return Err(invalid(Span { start: 0, end: 0 }, "output index oob"));
                };
                let mut p = String::from("(rfl : NobleContracts.evaluate (.output ");
                crate::rendering::number(&mut p, i as u64);
                p.push_str(") ");
                p.push_str(ctx_expr);
                p.push_str(" = some ");
                p.push_str(match expected_sort {
                    Sort3::I64 => "(NobleContracts.Value.i64 (",
                    Sort3::Bool => "(NobleContracts.Value.bool (",
                });
                p.push_str(val);
                p.push_str(")))");
                proofs.push((p, val.clone(), expected_sort));
            }
            Term3::I64(n) => {
                let mut val = String::from("(BitVec.ofInt 64 (");
                crate::rendering::integer(&mut val, n);
                val.push_str("))");
                let mut p = String::from("(rfl : NobleContracts.evaluate (.i64 (");
                crate::rendering::integer(&mut p, n);
                p.push_str(")) ");
                p.push_str(ctx_expr);
                p.push_str(" = some (NobleContracts.Value.i64 ");
                p.push_str(&val);
                p.push_str("))");
                proofs.push((p, val, Sort3::I64));
            }
            Term3::Bool(b) => {
                let val = String::from(choose_str(b, "true", "false"));
                let mut p = String::from("(rfl : NobleContracts.evaluate (.bool ");
                p.push_str(&val);
                p.push_str(") ");
                p.push_str(ctx_expr);
                p.push_str(" = some (NobleContracts.Value.bool ");
                p.push_str(&val);
                p.push_str("))");
                proofs.push((p, val, Sort3::Bool));
            }
            Term3::Add(l, r) => {
                work.push(EvalTermStep::Combine(EvalCombKind::Add));
                work.push(EvalTermStep::Visit(*r, Sort3::I64));
                work.push(EvalTermStep::Visit(*l, Sort3::I64));
            }
            Term3::Sub(l, r) => {
                work.push(EvalTermStep::Combine(EvalCombKind::Sub));
                work.push(EvalTermStep::Visit(*r, Sort3::I64));
                work.push(EvalTermStep::Visit(*l, Sort3::I64));
            }
            Term3::Mul(l, r) => {
                work.push(EvalTermStep::Combine(EvalCombKind::Mul));
                work.push(EvalTermStep::Visit(*r, Sort3::I64));
                work.push(EvalTermStep::Visit(*l, Sort3::I64));
            }
            Term3::Equals(l, r) => {
                let ls = bool_or_i64_sort(&l, &r);
                work.push(EvalTermStep::CombineEq);
                work.push(EvalTermStep::Visit(*r, ls));
                work.push(EvalTermStep::Visit(*l, ls));
            }
            Term3::Bound(_) | Term3::Ghost(..) => {
                return Err(invalid(
                    Span { start: 0, end: 0 },
                    "assertion term has bound/ghost",
                ));
            }
        },
        EvalTermStep::Combine(kind) => {
            let (Some((rp, rv, _)), Some((lp, lv, _))) = (proofs.pop(), proofs.pop()) else {
                return Err(invalid(Span { start: 0, end: 0 }, "eval stack"));
            };
            let (helper, op) = match kind {
                EvalCombKind::Add => ("eval_add", " + "),
                EvalCombKind::Sub => ("eval_sub", " - "),
                EvalCombKind::Mul => ("eval_mul", " * "),
            };
            let mut val = String::from("(");
            val.push_str(&lv);
            val.push_str(op);
            val.push_str(&rv);
            val.push(')');
            let mut p = String::from("(");
            p.push_str(helper);
            p.push(' ');
            p.push_str(&lp);
            p.push(' ');
            p.push_str(&rp);
            p.push(')');
            proofs.push((p, val, Sort3::I64));
        }
        EvalTermStep::CombineEq => {
            let (Some((rp, rv, rs)), Some((lp, lv, _))) = (proofs.pop(), proofs.pop()) else {
                return Err(invalid(Span { start: 0, end: 0 }, "eval stack"));
            };
            let ctor = match rs {
                Sort3::I64 => "eval_eq_i64",
                Sort3::Bool => "eval_eq_bool",
            };
            let mut val = String::from("(@decide ((");
            val.push_str(&lv);
            val.push_str(") = (");
            val.push_str(&rv);
            val.push_str(")) (Classical.propDecidable _))");
            let mut p = String::from("(");
            p.push_str(ctor);
            p.push(' ');
            p.push_str(&lp);
            p.push(' ');
            p.push_str(&rp);
            p.push(')');
            proofs.push((p, val, Sort3::Bool));
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; bounded iterative evaluator over `Term3`"
)]
fn emit_eval_term3_ctx(
    term: &Term3,
    sort: Sort3,
    ctx_expr: &str,
    before_vals: &[String],
    after_vals: &[String],
) -> Result<(String, String), Diagnostic> {
    let mut work = Vec::new();
    work.push(EvalTermStep::Visit(term.clone(), sort));
    let mut proofs: Vec<(String, String, Sort3)> = Vec::new();
    let mut outcome = Ok(());
    while let Some(step) = work.pop() {
        outcome = emit_eval_term3_step(
            step,
            ctx_expr,
            before_vals,
            after_vals,
            &mut work,
            &mut proofs,
        );
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    let Some((proof, val, _)) = proofs.pop() else {
        return Err(invalid(Span { start: 0, end: 0 }, "empty eval"));
    };
    Ok((proof, val))
}

#[octet::sealed_enum]
enum EvalPropStep {
    Visit(Prop3),
    CombineNot,
    CombineAnd,
}

#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-contracts; bounded iterative equivalence step over `Prop3`"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; bounded iterative equivalence step over `Prop3`"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes Prop3 equivalence steps onto caller worklist and stack"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; mutates iterative Prop3 worklist and witness stack"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; validates Prop3 evaluation witness invariants"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; builds `Holds p ctx ↔ P` for every `Prop3` form"
)]
fn emit_eval_prop3_iff_step(
    step: EvalPropStep,
    ctx_expr: &str,
    inputs_typed: &[(String, Sort3)],
    outputs_typed: &[(String, Sort3)],
    before_vals: &[String],
    after_vals: &[String],
    work: &mut Vec<EvalPropStep>,
    stack: &mut Vec<(String, String)>,
) -> Result<(), Diagnostic> {
    match step {
        EvalPropStep::Visit(node) => match node {
            Prop3::True => {
                let mut p = String::from("⟨(rfl : NobleContracts.evaluate (.bool true) ");
                p.push_str(ctx_expr);
                p.push_str(" = some (NobleContracts.Value.bool true)), iff_true_tr⟩");
                stack.push((p, String::from("true")));
            }
            Prop3::False => {
                let mut p = String::from("⟨(rfl : NobleContracts.evaluate (.bool false) ");
                p.push_str(ctx_expr);
                p.push_str(" = some (NobleContracts.Value.bool false)), iff_false_tr⟩");
                stack.push((p, String::from("false")));
            }
            Prop3::Not(b) => {
                work.push(EvalPropStep::CombineNot);
                work.push(EvalPropStep::Visit(*b));
            }
            Prop3::And(l, r) => {
                work.push(EvalPropStep::CombineAnd);
                work.push(EvalPropStep::Visit(*r));
                work.push(EvalPropStep::Visit(*l));
            }
            Prop3::Eq(l, r) => {
                let ls = super::sort_of(&l, inputs_typed, outputs_typed, &[]).unwrap_or(Sort3::I64);
                let (lp, lv) = emit_eval_term3_ctx(&l, ls, ctx_expr, before_vals, after_vals)?;
                let (rp, rv) = emit_eval_term3_ctx(&r, ls, ctx_expr, before_vals, after_vals)?;
                let mut bval = String::from("(@decide ((");
                bval.push_str(&lv);
                bval.push_str(") = (");
                bval.push_str(&rv);
                bval.push_str(")) (Classical.propDecidable _))");
                let helper = match ls {
                    Sort3::I64 => "iff_eq_i64",
                    Sort3::Bool => "iff_eq_bool",
                };
                let mut p = String::from("(");
                p.push_str(helper);
                p.push(' ');
                p.push_str(&lp);
                p.push(' ');
                p.push_str(&rp);
                p.push(')');
                stack.push((p, bval));
            }
            Prop3::Lt(ref l, ref r) | Prop3::Le(ref l, ref r) => {
                let is_lt = match node {
                    Prop3::Lt(..) => true,
                    Prop3::True
                    | Prop3::False
                    | Prop3::Not(..)
                    | Prop3::And(..)
                    | Prop3::Eq(..)
                    | Prop3::Le(..) => false,
                };
                let (lp, lv) =
                    emit_eval_term3_ctx(l, Sort3::I64, ctx_expr, before_vals, after_vals)?;
                let (rp, rv) =
                    emit_eval_term3_ctx(r, Sort3::I64, ctx_expr, before_vals, after_vals)?;
                let mut bval = String::from("(decide ((");
                bval.push_str(&lv);
                bval.push_str(if is_lt {
                    ").toInt < ("
                } else {
                    ").toInt ≤ ("
                });
                bval.push_str(&rv);
                bval.push_str(").toInt))");
                let mut p = String::from(if is_lt { "(iff_lt " } else { "(iff_le " });
                p.push_str(&lp);
                p.push(' ');
                p.push_str(&rp);
                p.push(')');
                stack.push((p, bval));
            }
        },
        EvalPropStep::CombineNot => {
            let Some((bp, bv)) = stack.pop() else {
                return Err(invalid(Span { start: 0, end: 0 }, "prop stack"));
            };
            let mut nb = String::from("(!");
            nb.push_str(&bv);
            nb.push(')');
            let mut p = String::from("(iff_not ");
            p.push_str(&bp);
            p.push(')');
            stack.push((p, nb));
        }
        EvalPropStep::CombineAnd => {
            let Some((rp, rv)) = stack.pop() else {
                return Err(invalid(Span { start: 0, end: 0 }, "prop stack"));
            };
            let Some((lp, lv)) = stack.pop() else {
                return Err(invalid(Span { start: 0, end: 0 }, "prop stack"));
            };
            let mut ab = String::from("(");
            ab.push_str(&lv);
            ab.push_str(" && ");
            ab.push_str(&rv);
            ab.push(')');
            let mut p = String::from("(iff_and ");
            p.push_str(&lp);
            p.push(' ');
            p.push_str(&rp);
            p.push(')');
            stack.push((p, ab));
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; proves `Holds prop before after [] ↔ LeanProp` cleanly"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; iterative equivalence witness over `Prop3` AST"
)]
fn emit_eval_prop3_iff(
    prop: &Prop3,
    ctx_expr: &str,
    before_sorts: &[Sort3],
    after_sorts: &[Sort3],
    before_vals: &[String],
    after_vals: &[String],
) -> Result<String, Diagnostic> {
    let mut inputs_typed: Vec<(String, Sort3)> = Vec::with_capacity(before_sorts.len());
    let mut bi = 0usize;
    while bi < before_sorts.len() {
        inputs_typed.push((fmt_str_num("_in", bi as u64), before_sorts[bi]));
        bi += 1;
    }
    let mut outputs_typed: Vec<(String, Sort3)> = Vec::with_capacity(after_sorts.len());
    let mut ai = 0usize;
    while ai < after_sorts.len() {
        outputs_typed.push((fmt_str_num("_out", ai as u64), after_sorts[ai]));
        ai += 1;
    }
    let mut work = Vec::new();
    work.push(EvalPropStep::Visit(prop.clone()));
    let mut stack: Vec<(String, String)> = Vec::new();
    let mut outcome = Ok(());
    while let Some(step) = work.pop() {
        outcome = emit_eval_prop3_iff_step(
            step,
            ctx_expr,
            &inputs_typed,
            &outputs_typed,
            before_vals,
            after_vals,
            &mut work,
            &mut stack,
        );
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    let Some((pair_expr, _)) = stack.pop() else {
        return Err(invalid(Span { start: 0, end: 0 }, "empty prop iff"));
    };
    let mut res = String::from("(holds_of_eval_iff ");
    res.push_str(&pair_expr);
    res.push(')');
    Ok(res)
}

#[derive(Clone, Copy)]
struct ProofScope<'a> {
    facts: &'a [Prop3],
    lemmas: &'a [super::Lemma],
    scope_inputs: &'a [(String, Sort3)],
    scope_outputs: &'a [(String, Sort3)],
    in_vars: &'a [String],
    ghost_vars: &'a [(u32, u32, String)],
    fact_vars: &'a [String],
}

#[octet::sealed_enum]
enum PropProofStep {
    Eval(Form),
    Apply(Form),
    SubstAfterEq {
        binder_name: String,
        code: Sort3,
        motive_body: Prop3,
        premise: Form,
    },
    SubstAfterPremise {
        binder_name: String,
        code: Sort3,
        motive_body: Prop3,
        left: Term3,
        right: Term3,
        eq_expr: String,
    },
}

struct PropProofState {
    bound_defs: Vec<(String, Sort3)>,
    bound_names: Vec<String>,
    work: Vec<PropProofStep>,
    vals: Vec<(String, Statement3)>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; validates proof AST constructors with typed diagnostics"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; keeps every proof AST constructor dispatch in one shallow switch"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; mutates proof state stack and meter during AST traversal"
)]
fn eval_prop_proof_step(
    form: &Form,
    scope: ProofScope<'_>,
    state: &mut PropProofState,
    meter: &mut crate::intrinsic::Meter<'_>,
) -> Result<(), Diagnostic> {
    let ProofScope {
        facts,
        lemmas,
        scope_inputs,
        scope_outputs,
        in_vars,
        ghost_vars,
        fact_vars,
    } = scope;
    if let Ok(args) = crate::intrinsic::operation(form, "use", 1) {
        let name = crate::intrinsic::atom(&args[0])?;
        state.vals.push(resolve_use_proof(
            name, form.span, facts, lemmas, fact_vars,
        )?);
        return Ok(());
    }
    if let Ok(args) = crate::intrinsic::operation(form, "apply", 2) {
        state.work.push(PropProofStep::Apply(args[1].clone()));
        state.work.push(PropProofStep::Eval(args[0].clone()));
        return Ok(());
    }
    if let Ok(args) = crate::intrinsic::operation(form, "refl", 1) {
        let term_scope = super::Scope {
            inputs: scope_inputs,
            outputs: scope_outputs,
            bound: &state.bound_defs,
            interface: true,
            allow_typed_eq: true,
        };
        let mut rem = super::MAX_ASSERTION_NODES;
        let t = super::parse_term(&args[0], term_scope, 0, &mut rem, meter)?;
        let t_lean = emit_term3_lean(String::new(), &t, &state.bound_names, in_vars, ghost_vars)?;
        let mut p = String::from("(rfl : ");
        p.push_str(&t_lean);
        p.push_str(" = ");
        p.push_str(&t_lean);
        p.push(')');
        state
            .vals
            .push((p, Statement3::Prop(Prop3::Eq(t.clone(), t))));
        return Ok(());
    }
    if let Ok(args) = crate::intrinsic::operation(form, "subst", 3) {
        let motive = crate::intrinsic::list(&args[0])?;
        let binder = crate::intrinsic::list(&motive[1])?;
        let Some(code) = Sort3::from_word(crate::intrinsic::atom(&binder[1])?) else {
            return Err(invalid(binder[1].span, "sort"));
        };
        let level = state.bound_defs.len();
        let bname = fmt_str_num("mv_", level as u64);
        state
            .bound_defs
            .push((String::from(crate::intrinsic::atom(&binder[0])?), code));
        state.bound_names.push(bname.clone());
        let mscope = super::Scope {
            inputs: scope_inputs,
            outputs: scope_outputs,
            bound: &state.bound_defs,
            interface: true,
            allow_typed_eq: true,
        };
        let mut rem = super::MAX_ASSERTION_NODES;
        let mbody = super::parse_prop(&motive[2], mscope, 0, &mut rem, meter)?;
        state.bound_defs.pop();
        state.bound_names.pop();
        state.work.push(PropProofStep::SubstAfterEq {
            binder_name: bname,
            code,
            motive_body: mbody,
            premise: args[2].clone(),
        });
        state.work.push(PropProofStep::Eval(args[1].clone()));
        return Ok(());
    }
    if let Ok(args) = crate::intrinsic::operation(form, "bv-add-assoc", 3) {
        let term_scope = super::Scope {
            inputs: scope_inputs,
            outputs: scope_outputs,
            bound: &state.bound_defs,
            interface: true,
            allow_typed_eq: true,
        };
        let mut rem = super::MAX_ASSERTION_NODES;
        let a = super::parse_term(&args[0], term_scope, 0, &mut rem, meter)?;
        let b = super::parse_term(&args[1], term_scope, 0, &mut rem, meter)?;
        let c = super::parse_term(&args[2], term_scope, 0, &mut rem, meter)?;
        let al = emit_term3_lean(String::new(), &a, &state.bound_names, in_vars, ghost_vars)?;
        let bl = emit_term3_lean(String::new(), &b, &state.bound_names, in_vars, ghost_vars)?;
        let cl = emit_term3_lean(String::new(), &c, &state.bound_names, in_vars, ghost_vars)?;
        let p = fmt_wrap3("(BitVec.add_assoc ", &al, &bl, &cl, ")");
        let stmt = Prop3::Eq(
            Term3::Add(
                Box::new(Term3::Add(Box::new(a.clone()), Box::new(b.clone()))),
                Box::new(c.clone()),
            ),
            Term3::Add(Box::new(a), Box::new(Term3::Add(Box::new(b), Box::new(c)))),
        );
        state.vals.push((p, Statement3::Prop(stmt)));
        return Ok(());
    }
    if let Ok(args) = crate::intrinsic::operation(form, "bv-add-comm", 2) {
        let term_scope = super::Scope {
            inputs: scope_inputs,
            outputs: scope_outputs,
            bound: &state.bound_defs,
            interface: true,
            allow_typed_eq: true,
        };
        let mut rem = super::MAX_ASSERTION_NODES;
        let a = super::parse_term(&args[0], term_scope, 0, &mut rem, meter)?;
        let b = super::parse_term(&args[1], term_scope, 0, &mut rem, meter)?;
        let al = emit_term3_lean(String::new(), &a, &state.bound_names, in_vars, ghost_vars)?;
        let bl = emit_term3_lean(String::new(), &b, &state.bound_names, in_vars, ghost_vars)?;
        let p = fmt_wrap2("(BitVec.add_comm ", &al, " ", &bl, ")");
        state.vals.push((
            p,
            Statement3::Prop(Prop3::Eq(
                Term3::Add(Box::new(a.clone()), Box::new(b.clone())),
                Term3::Add(Box::new(b), Box::new(a)),
            )),
        ));
        return Ok(());
    }
    if let Ok(args) = crate::intrinsic::operation(form, "bv-mul-add", 3) {
        let term_scope = super::Scope {
            inputs: scope_inputs,
            outputs: scope_outputs,
            bound: &state.bound_defs,
            interface: true,
            allow_typed_eq: true,
        };
        let mut rem = super::MAX_ASSERTION_NODES;
        let a = super::parse_term(&args[0], term_scope, 0, &mut rem, meter)?;
        let b = super::parse_term(&args[1], term_scope, 0, &mut rem, meter)?;
        let c = super::parse_term(&args[2], term_scope, 0, &mut rem, meter)?;
        let al = emit_term3_lean(String::new(), &a, &state.bound_names, in_vars, ghost_vars)?;
        let bl = emit_term3_lean(String::new(), &b, &state.bound_names, in_vars, ghost_vars)?;
        let cl = emit_term3_lean(String::new(), &c, &state.bound_names, in_vars, ghost_vars)?;
        let p = fmt_wrap3("(@BitVec.mul_add 64 ", &al, &bl, &cl, ")");
        let stmt = Prop3::Eq(
            Term3::Mul(
                Box::new(a.clone()),
                Box::new(Term3::Add(Box::new(b.clone()), Box::new(c.clone()))),
            ),
            Term3::Add(
                Box::new(Term3::Mul(Box::new(a.clone()), Box::new(b))),
                Box::new(Term3::Mul(Box::new(a), Box::new(c))),
            ),
        );
        state.vals.push((p, Statement3::Prop(stmt)));
        return Ok(());
    }
    if crate::intrinsic::operation(form, "by-true", 0).is_ok() {
        state
            .vals
            .push((String::from("True.intro"), Statement3::Prop(Prop3::True)));
        return Ok(());
    }
    Err(invalid(form.span, "unknown proof form in Lean lowering"))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; resolves fact or lemma index with typed diagnostic on miss"
)]
fn resolve_use_proof(
    name: &str,
    span: Span,
    facts: &[Prop3],
    lemmas: &[super::Lemma],
    fact_vars: &[String],
) -> Result<(String, Statement3), Diagnostic> {
    if let Some(fi) = super::fact_index(name) {
        let Some(fact) = facts.get(fi) else {
            return Err(invalid(span, "fact index oob"));
        };
        let prop = super::normalize_prop(fact);
        let Some(vname) = fact_vars.get(fi) else {
            return Err(invalid(span, "fact index oob"));
        };
        return Ok((vname.clone(), Statement3::Prop(prop)));
    }
    let mut found = None;
    let mut li = 0usize;
    while found.is_none() && li < lemmas.len() {
        let is_hit = super::lemma_has_ref(&lemmas[li], name);
        if is_hit {
            found = Some(li);
        }
        li = li.saturating_add(1);
    }
    let Some(li) = found else {
        return Err(invalid(span, "missing lemma"));
    };
    Ok((
        fmt_str_num("lemma_", li as u64),
        lemmas[li].statement.clone(),
    ))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; processes one proof AST node without early loop returns"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; mutates iterative proof lowering state and meter"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; keeps every PropProofStep transition in one switch"
)]
fn emit_prop_proof_one_step(
    step: PropProofStep,
    scope: ProofScope<'_>,
    root_span: Span,
    state: &mut PropProofState,
    meter: &mut crate::intrinsic::Meter<'_>,
) -> Result<(), Diagnostic> {
    let ProofScope {
        scope_inputs,
        scope_outputs,
        in_vars,
        ghost_vars,
        ..
    } = scope;
    match step {
        PropProofStep::Eval(form) => {
            eval_prop_proof_step(&form, scope, state, meter)?;
        }
        PropProofStep::Apply(arg_form) => {
            let Some((fn_expr, fn_stmt)) = state.vals.pop() else {
                return Err(invalid(arg_form.span, "apply"));
            };
            let Statement3::Pi { body, .. } = fn_stmt else {
                return Err(invalid(arg_form.span, "apply non-pi"));
            };
            let term_scope = super::Scope {
                inputs: scope_inputs,
                outputs: scope_outputs,
                bound: &state.bound_defs,
                interface: true,
                allow_typed_eq: true,
            };
            let mut rem = super::MAX_ASSERTION_NODES;
            let arg_term = super::parse_term(&arg_form, term_scope, 0, &mut rem, meter)?;
            let arg_lean = emit_term3_lean(
                String::new(),
                &arg_term,
                &state.bound_names,
                in_vars,
                ghost_vars,
            )?;
            state.vals.push((
                fmt_wrap2("(", &fn_expr, " ", &arg_lean, ")"),
                super::replace_statement(&body, &arg_term),
            ));
        }
        PropProofStep::SubstAfterEq {
            binder_name,
            code,
            motive_body,
            premise,
        } => {
            let Some((eq_expr, eq_stmt)) = state.vals.pop() else {
                return Err(invalid(premise.span, "subst eq"));
            };
            let Statement3::Prop(eq_prop) = eq_stmt else {
                return Err(invalid(premise.span, "subst eq not prop"));
            };
            let Prop3::Eq(left, right) = eq_prop else {
                return Err(invalid(premise.span, "subst eq not eq"));
            };
            state.work.push(PropProofStep::SubstAfterPremise {
                binder_name,
                code,
                motive_body,
                left,
                right,
                eq_expr,
            });
            state.work.push(PropProofStep::Eval(premise));
        }
        PropProofStep::SubstAfterPremise {
            binder_name,
            code,
            motive_body,
            left,
            right,
            eq_expr,
        } => {
            let Some((prem_expr, prem_stmt)) = state.vals.pop() else {
                return Err(invalid(root_span, "subst prem"));
            };
            let Statement3::Prop(prem_prop) = prem_stmt else {
                return Err(invalid(root_span, "subst prem not prop"));
            };
            let level = state.bound_names.len();
            let expected_prem = super::replace_prop(&motive_body, level, &left);
            state.bound_names.push(binder_name.clone());
            let mprop_lean = emit_prop3_lean(
                String::new(),
                &motive_body,
                &state.bound_names,
                in_vars,
                ghost_vars,
            )?;
            state.bound_names.pop();
            let cast_prem = emit_cast_prop(
                &prem_prop,
                &expected_prem,
                &prem_expr,
                &state.bound_names,
                in_vars,
                ghost_vars,
            )?;
            let mut s_type = String::new();
            push_sort_lean_type_buf(&mut s_type, code);
            let mut p = String::from("(@Eq.subst ");
            p.push_str(&s_type);
            p.push_str(" (fun (");
            p.push_str(&binder_name);
            p.push_str(" : ");
            p.push_str(&s_type);
            p.push_str(") => ");
            p.push_str(&mprop_lean);
            p.push_str(") _ _ ");
            p.push_str(&eq_expr);
            p.push(' ');
            p.push_str(&cast_prem);
            p.push(')');
            state.vals.push((
                p,
                Statement3::Prop(super::replace_prop(&motive_body, level, &right)),
            ));
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; bounded iterative proof translator over validated proof AST"
)]
fn emit_prop_proof(root: &Form, scope: ProofScope<'_>) -> Result<(String, Statement3), Diagnostic> {
    let work = alloc::vec![PropProofStep::Eval(root.clone())];
    let mut state = PropProofState {
        bound_defs: Vec::new(),
        bound_names: Vec::new(),
        work,
        vals: Vec::new(),
    };
    let dummy_limits = crate::Limits::default();
    let Some(tree_bounds) = crate::intrinsic::TreeBounds::new(dummy_limits) else {
        return Err(invalid(root.span, "bounds"));
    };
    let config = crate::intrinsic::MeterConfig {
        budgets: crate::intrinsic::ProofBudgets::from_limits(dummy_limits),
        limits: dummy_limits,
        tree_bounds,
    };
    let mut meter = crate::intrinsic::Meter {
        nodes: 0,
        work: 0,
        normalization: 0,
        substitution: 0,
        config: &config,
    };
    let mut outcome = Ok(());
    while let Some(step) = state.work.pop() {
        outcome = emit_prop_proof_one_step(step, scope, root.span, &mut state, &mut meter);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    let Some(res) = state.vals.pop() else {
        return Err(invalid(root.span, "empty proof lowering"));
    };
    Ok(res)
}

#[octet::sealed_enum]
enum SegOp {
    LitI64(i64),
    LitBool(bool),
    Word(u32),
}

struct SegmentSpec {
    ops: Vec<SegOp>,
    carried: Vec<Term3>,
    carried_sorts: Vec<Sort3>,
    consumed: Vec<Term3>,
    consumed_sorts: Vec<Sort3>,
    produced_sorts: Vec<Sort3>,
    after_stack: Vec<Term3>,
    after_sorts: Vec<Sort3>,
    facts: Vec<Prop3>,
}

#[octet::sealed_enum]
enum Chunk {
    Segment(SegmentSpec),
    Call {
        call_idx: usize,
        occ_ui: usize,
        carried_len: usize,
        after_stack: Vec<Term3>,
        after_sorts: Vec<Sort3>,
        facts: Vec<Prop3>,
        new_ghosts: Vec<(u32, u32, Sort3)>,
    },
}

#[octet::sealed_enum]
enum TermSortStep {
    Visit(Term3),
    Arith,
    Eq,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; bounded iterative sort lookup over `Term3`"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes sort worklist steps and resolved sorts onto caller buffers"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; mutates iterative sort worklist and type stack"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; pushes dynamically onto `Vec` worklist and stack"
)]
fn term_sort_step(
    step: TermSortStep,
    in_sorts: &[Sort3],
    work: &mut Vec<TermSortStep>,
    stack: &mut Vec<Sort3>,
) -> Result<(), Diagnostic> {
    match step {
        TermSortStep::Visit(node) => match node {
            Term3::Input(i) => {
                let Some(s) = in_sorts.get(i) else {
                    return Err(invalid(Span { start: 0, end: 0 }, "input sort oob"));
                };
                stack.push(*s);
            }
            Term3::Ghost(_, _, s) => stack.push(s),
            Term3::I64(_) => stack.push(Sort3::I64),
            Term3::Bool(_) => stack.push(Sort3::Bool),
            Term3::Add(l, r) | Term3::Sub(l, r) | Term3::Mul(l, r) => {
                work.push(TermSortStep::Arith);
                work.push(TermSortStep::Visit(*r));
                work.push(TermSortStep::Visit(*l));
            }
            Term3::Equals(l, r) => {
                work.push(TermSortStep::Eq);
                work.push(TermSortStep::Visit(*r));
                work.push(TermSortStep::Visit(*l));
            }
            Term3::Output(_) | Term3::Bound(_) => {
                return Err(invalid(
                    Span { start: 0, end: 0 },
                    "unexpected binder in stack term",
                ));
            }
        },
        TermSortStep::Arith => {
            let _ = (stack.pop(), stack.pop());
            stack.push(Sort3::I64);
        }
        TermSortStep::Eq => {
            let _ = (stack.pop(), stack.pop());
            stack.push(Sort3::Bool);
        }
    }
    Ok(())
}

fn term_sort(
    term: &Term3,
    in_sorts: &[Sort3],
    ghost_sorts: &[(u32, u32, Sort3)],
) -> Result<Sort3, Diagnostic> {
    let _ = ghost_sorts;
    let mut work = Vec::new();
    work.push(TermSortStep::Visit(term.clone()));
    let mut stack: Vec<Sort3> = Vec::new();
    let mut outcome = Ok(());
    while let Some(step) = work.pop() {
        outcome = term_sort_step(step, in_sorts, &mut work, &mut stack);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    let Some(top) = stack.pop() else {
        return Err(invalid(Span { start: 0, end: 0 }, "empty term sort"));
    };
    Ok(top)
}

fn sorts_of_stack(
    stack: &[Term3],
    in_sorts: &[Sort3],
    ghost_sorts: &[(u32, u32, Sort3)],
) -> Result<Vec<Sort3>, Diagnostic> {
    let mut out = Vec::with_capacity(stack.len());
    let mut i = 0usize;
    while i < stack.len() {
        out.push(term_sort(&stack[i], in_sorts, ghost_sorts)?);
        i += 1;
    }
    Ok(out)
}

struct SegmentSpecInput<'a> {
    ops: Vec<SegOp>,
    before_stack: &'a [Term3],
    before_sorts: &'a [Sort3],
    after_stack: Vec<Term3>,
    facts: Vec<Prop3>,
    in_sorts: &'a [Sort3],
    ghost_sorts: &'a [(u32, u32, Sort3)],
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; computes exact consumed/carried prefix for one straight-line segment"
)]
fn build_segment_spec(input: SegmentSpecInput<'_>) -> Result<SegmentSpec, Diagnostic> {
    let SegmentSpecInput {
        ops,
        before_stack,
        before_sorts,
        after_stack,
        facts,
        in_sorts,
        ghost_sorts,
    } = input;
    let mut cur_depth = before_stack.len();
    let mut min_depth = cur_depth;
    let mut i = 0usize;
    while i < ops.len() {
        match ops[i] {
            SegOp::LitI64(_) | SegOp::LitBool(_) => cur_depth += 1,
            SegOp::Word(0) => {
                let touched = cur_depth.saturating_sub(1);
                if touched < min_depth {
                    min_depth = touched;
                }
                cur_depth += 1;
            }
            SegOp::Word(1) => {
                cur_depth = cur_depth.saturating_sub(1);
                if cur_depth < min_depth {
                    min_depth = cur_depth;
                }
            }
            SegOp::Word(2) => {
                let touched = cur_depth.saturating_sub(2);
                if touched < min_depth {
                    min_depth = touched;
                }
            }
            SegOp::Word(4..=7) => {
                let touched = cur_depth.saturating_sub(2);
                if touched < min_depth {
                    min_depth = touched;
                }
                cur_depth = cur_depth.saturating_sub(1);
            }
            SegOp::Word(_) => {}
        }
        i += 1;
    }
    let carried = before_stack[..min_depth].to_vec();
    let carried_sorts = before_sorts[..min_depth].to_vec();
    let consumed = before_stack[min_depth..].to_vec();
    let consumed_sorts = before_sorts[min_depth..].to_vec();
    let produced = after_stack[min_depth..].to_vec();
    let produced_sorts = sorts_of_stack(&produced, in_sorts, ghost_sorts)?;
    let after_sorts = sorts_of_stack(&after_stack, in_sorts, ghost_sorts)?;
    Ok(SegmentSpec {
        ops,
        carried,
        carried_sorts,
        consumed,
        consumed_sorts,
        produced_sorts,
        after_stack,
        after_sorts,
        facts,
    })
}

/// Emit `StackTyped` proof for concrete crate::intrinsic::list of typed values.
fn emit_stack_typed(mut out: String, sorts: &[Sort3]) -> String {
    let mut i = 0usize;
    while i < sorts.len() {
        out.push_str("(NobleContracts.StackTyped.cons ");
        out.push_str(match sorts[i] {
            Sort3::I64 => "(NobleContracts.HasType.i64)",
            Sort3::Bool => "(NobleContracts.HasType.bool)",
        });
        out.push(' ');
        i += 1;
    }
    out.push_str("NobleContracts.StackTyped.nil");
    let mut c = 0usize;
    while c < sorts.len() {
        out.push(')');
        c += 1;
    }
    out
}

struct SplitCarriedMatchSpec<'a> {
    eq_name: &'a str,
    carried_var: &'a str,
    tail_list: &'a str,
    carried_count: usize,
    args_count: usize,
    strip_args: bool,
    target_elems: &'a [String],
    var_prefix: &'a str,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits fixed list congruence chain by arity"
)]
fn emit_list_congr_from_prefix(mut out: String, prefix: &str, count: usize) -> String {
    if count == 0 {
        out.push_str("(rfl : ([] : List NobleContracts.Value) = [])");
        return out;
    }
    let mut i = 0usize;
    while i < count {
        out.push_str("(congr (congrArg List.cons ");
        out.push_str(prefix);
        crate::rendering::number(&mut out, i as u64);
        out.push_str(") ");
        i += 1;
    }
    out.push_str("rfl");
    let mut c = 0usize;
    while c < count {
        out.push(')');
        c += 1;
    }
    out
}

fn format_list_slice(elems: &[String]) -> String {
    let mut out = String::from("[");
    out = push_csv_items(out, elems);
    out.push(']');
    out
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; bounded list decomposition without Lean match helpers"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; emits tactic-free CPS List.casesOn append split"
)]
fn emit_split_carried_match(mut out: String, spec: SplitCarriedMatchSpec<'_>) -> String {
    let SplitCarriedMatchSpec {
        eq_name,
        carried_var,
        tail_list,
        carried_count,
        args_count,
        strip_args,
        target_elems,
        var_prefix,
    } = spec;
    let mut cur_list = String::from(carried_var);
    let mut cur_eq = String::from(eq_name);
    let mut d = 0usize;
    while d < carried_count {
        let cvar = fmt_prefix_mid_num(var_prefix, "_c_", d as u64);
        let rvar = fmt_prefix_mid_num(var_prefix, "_r_", d as u64);
        let hcar = fmt_prefix_mid_num(var_prefix, "_hcar_", d as u64);
        let eqvar = fmt_prefix_mid_num(var_prefix, "_eq_", d as u64);
        let hsvar = fmt_prefix_mid_num(var_prefix, "_hs_", d as u64);
        let rhs_tail = format_list_slice(&target_elems[d.saturating_add(1)..]);
        out.push_str("    append_split_step ");
        out.push_str(&cur_list);
        out.push(' ');
        out.push_str(tail_list);
        out.push(' ');
        out.push_str(&rhs_tail);
        out.push_str(" (");
        out.push_str(&target_elems[d]);
        out.push_str(") ");
        out.push_str(&cur_eq);
        out.push_str(" (by decide) <| fun ");
        out.push_str(&cvar);
        out.push(' ');
        out.push_str(&rvar);
        out.push(' ');
        out.push_str(&hcar);
        out.push(' ');
        out.push_str(&eqvar);
        out.push(' ');
        out.push_str(&hsvar);
        out.push_str(" =>\n");
        cur_list = rvar;
        cur_eq = hsvar;
        d += 1;
    }
    let rhs_end = format_list_slice(&target_elems[carried_count..]);
    let mut h_exact = String::from(var_prefix);
    h_exact.push_str("_exact");
    out.push_str("    append_split_nil ");
    out.push_str(&cur_list);
    out.push(' ');
    out.push_str(tail_list);
    out.push(' ');
    out.push_str(&rhs_end);
    out.push(' ');
    out.push_str(&cur_eq);
    out.push_str(" rfl <| fun ");
    out.push_str(var_prefix);
    out.push_str("_hcar_end ");
    out.push_str(&h_exact);
    out.push_str(" =>\n");
    let mut eq_expr = String::from(var_prefix);
    eq_expr.push_str("_hcar_end");
    let mut rev_i = carried_count;
    while rev_i > 0 {
        rev_i = rev_i.saturating_sub(1);
        let cvar = fmt_prefix_mid_num(var_prefix, "_c_", rev_i as u64);
        let hcar = fmt_prefix_mid_num(var_prefix, "_hcar_", rev_i as u64);
        let cons_arg = fmt_wrap2("(congrArg (List.cons ", &cvar, ") ", &eq_expr, ")");
        eq_expr = fmt_wrap2("(Eq.trans ", &hcar, " ", &cons_arg, ")");
    }
    out.push_str("    let ");
    out.push_str(var_prefix);
    out.push_str("_carried_eq := ");
    out.push_str(&eq_expr);
    out.push('\n');
    if strip_args {
        let mut rem_eq = h_exact;
        let mut ti = 0usize;
        while ti < args_count {
            let eq_i =
                fmt_prefix_mid_num(var_prefix, "_eq_", carried_count.saturating_add(ti) as u64);
            let tail_i =
                fmt_prefix_mid_num(var_prefix, "_tl_", carried_count.saturating_add(ti) as u64);
            out.push_str("    let ");
            out.push_str(&eq_i);
            out.push_str(" := (List.cons.inj ");
            out.push_str(&rem_eq);
            out.push_str(").1\n    let ");
            out.push_str(&tail_i);
            out.push_str(" := (List.cons.inj ");
            out.push_str(&rem_eq);
            out.push_str(").2\n");
            rem_eq = tail_i;
            ti += 1;
        }
    }
    out
}

struct StackUnpackSpec<'a> {
    hyp_name: &'a str,
    stack_expr: &'a str,
    sorts: &'a [Sort3],
    val_prefix: &'a str,
    val_names: &'a [String],
    step_prefix: &'a str,
    eq_name: &'a str,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; builds nested Eq.trans chain over bounded stack arity"
)]
fn emit_stack_unpack_eq(mut out: String, spec: StackUnpackSpec<'_>) -> String {
    let StackUnpackSpec {
        sorts,
        val_prefix,
        val_names,
        step_prefix,
        eq_name,
        ..
    } = spec;
    let mut eq_expr = String::from(step_prefix);
    eq_expr.push_str("_nil");
    let mut rev_i = sorts.len();
    while rev_i > 0 {
        rev_i = rev_i.saturating_sub(1);
        let val_name = if rev_i < val_names.len() {
            val_names[rev_i].clone()
        } else {
            fmt_str_num(val_prefix, rev_i as u64)
        };
        let hs = fmt_prefix_mid_num(step_prefix, "_hs_", rev_i as u64);
        let ctor = match sorts[rev_i] {
            Sort3::I64 => "NobleContracts.Value.i64 ",
            Sort3::Bool => "NobleContracts.Value.bool ",
        };
        let elem_expr = fmt_wrap2("(", ctor, &val_name, ")", "");
        let cons_arg = fmt_wrap2("(congrArg (List.cons ", &elem_expr, ") ", &eq_expr, ")");
        eq_expr = fmt_wrap2("(Eq.trans ", &hs, " ", &cons_arg, ")");
    }
    out.push_str("    let ");
    out.push_str(eq_name);
    out.push_str(" := ");
    out.push_str(&eq_expr);
    out.push('\n');
    out
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; unpacks StackTyped without match auxiliary constants"
)]
fn emit_stack_unpack(mut out: String, spec: StackUnpackSpec<'_>) -> String {
    let StackUnpackSpec {
        hyp_name,
        stack_expr,
        sorts,
        val_prefix,
        val_names,
        step_prefix,
        eq_name,
    } = spec;
    let mut cur_s = String::from(stack_expr);
    let mut cur_h = String::from(hyp_name);
    let mut i = 0usize;
    while i < sorts.len() {
        let val_name = if i < val_names.len() {
            val_names[i].clone()
        } else {
            fmt_str_num(val_prefix, i as u64)
        };
        let tl = fmt_prefix_mid_num(step_prefix, "_tl_", i as u64);
        let hs = fmt_prefix_mid_num(step_prefix, "_hs_", i as u64);
        let hr = fmt_prefix_mid_num(step_prefix, "_hr_", i as u64);
        out.push_str(match sorts[i] {
            Sort3::I64 => "    unpack_i64 ",
            Sort3::Bool => "    unpack_bool ",
        });
        out.push_str(&cur_s);
        out.push(' ');
        out = push_sort_list(out, &sorts[i.saturating_add(1)..]);
        out.push(' ');
        out.push_str(&cur_h);
        out.push_str(" <| fun ");
        out.push_str(&val_name);
        out.push(' ');
        out.push_str(&tl);
        out.push(' ');
        out.push_str(&hs);
        out.push(' ');
        out.push_str(&hr);
        out.push_str(" =>\n");
        cur_s = tl;
        cur_h = hr;
        i += 1;
    }
    out.push_str("    unpack_nil ");
    out.push_str(&cur_s);
    out.push(' ');
    out.push_str(&cur_h);
    out.push_str(" <| fun ");
    out.push_str(step_prefix);
    out.push_str("_nil =>\n");
    emit_stack_unpack_eq(
        out,
        StackUnpackSpec {
            hyp_name,
            stack_expr,
            sorts,
            val_prefix,
            val_names,
            step_prefix,
            eq_name,
        },
    )
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; emits tactic-free witness for one segment operation"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; unsupported segment word returns typed Diagnostic"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; passes step stack, target equality, and Lean exec name"
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; step_exec and target_eq are distinct Lean hypothesis names"
)]
fn emit_seg_claim_op(
    mut out: String,
    op: &SegOp,
    stack_exprs: &[(Sort3, String)],
    next_stack_exprs: &[(Sort3, String)],
    step_exec: &str,
    target_eq: &str,
) -> Result<String, Diagnostic> {
    let pre_len = stack_exprs.len();
    match op {
        SegOp::LitI64(n) => {
            let cur_list = format_value_list(stack_exprs);
            let mut val_lit = String::from("(.i64 (BitVec.ofInt 64 (");
            crate::rendering::integer(&mut val_lit, *n);
            val_lit.push_str(")))");
            out.push_str("    let ");
            out.push_str(target_eq);
            out.push_str(" := Eq.trans ((NobleContracts.exec_lit_iff (v := ");
            out.push_str(&val_lit);
            out.push_str(") (s := tail ++ ");
            out.push_str(&cur_list);
            out.push_str(")).mp ");
            out.push_str(step_exec);
            out.push_str(") (List.append_assoc tail ");
            out.push_str(&cur_list);
            out.push_str(" [");
            out.push_str(&val_lit);
            out.push_str("])\n");
        }
        SegOp::LitBool(b) => {
            let cur_list = format_value_list(stack_exprs);
            let val_lit = choose_str(*b, "(.bool true)", "(.bool false)");
            out.push_str("    let ");
            out.push_str(target_eq);
            out.push_str(" := Eq.trans ((NobleContracts.exec_lit_iff (v := ");
            out.push_str(val_lit);
            out.push_str(") (s := tail ++ ");
            out.push_str(&cur_list);
            out.push_str(")).mp ");
            out.push_str(step_exec);
            out.push_str(") (List.append_assoc tail ");
            out.push_str(&cur_list);
            out.push_str(" [");
            out.push_str(val_lit);
            out.push_str("])\n");
        }
        SegOp::Word(0) => {
            let pfx = format_value_list(&stack_exprs[..pre_len.saturating_sub(1)]);
            let top = format_single_value(&stack_exprs[pre_len.saturating_sub(1)]);
            out.push_str("    let ");
            out.push_str(target_eq);
            out.push_str(" := Eq.trans ((NobleContracts.exec_dup_iff (v := ");
            out.push_str(&top);
            out.push_str(") (s := tail ++ ");
            out.push_str(&pfx);
            out.push_str(")).mp (Eq.ndrec (motive := fun x => NobleContracts.Exec [.word 0] x _) ");
            out.push_str(step_exec);
            out.push_str(" (List.append_assoc tail ");
            out.push_str(&pfx);
            out.push_str(" [");
            out.push_str(&top);
            out.push_str("]).symm)) (List.append_assoc tail ");
            out.push_str(&pfx);
            out.push_str(" [");
            out.push_str(&top);
            out.push_str(", ");
            out.push_str(&top);
            out.push_str("])\n");
        }
        SegOp::Word(1) => {
            let pfx = format_value_list(&stack_exprs[..pre_len.saturating_sub(1)]);
            let top = format_single_value(&stack_exprs[pre_len.saturating_sub(1)]);
            out.push_str("    let ");
            out.push_str(target_eq);
            out.push_str(" := (NobleContracts.exec_drop_iff (v := ");
            out.push_str(&top);
            out.push_str(") (s := tail ++ ");
            out.push_str(&pfx);
            out.push_str(")).mp (Eq.ndrec (motive := fun x => NobleContracts.Exec [.word 1] x _) ");
            out.push_str(step_exec);
            out.push_str(" (List.append_assoc tail ");
            out.push_str(&pfx);
            out.push_str(" [");
            out.push_str(&top);
            out.push_str("]).symm)\n");
        }
        SegOp::Word(2) => {
            let pfx = format_value_list(&stack_exprs[..pre_len.saturating_sub(2)]);
            let a = format_single_value(&stack_exprs[pre_len.saturating_sub(2)]);
            let b = format_single_value(&stack_exprs[pre_len.saturating_sub(1)]);
            out.push_str("    let ");
            out.push_str(target_eq);
            out.push_str(" := Eq.trans ((NobleContracts.exec_swap_iff (a := ");
            out.push_str(&a);
            out.push_str(") (b := ");
            out.push_str(&b);
            out.push_str(") (s := tail ++ ");
            out.push_str(&pfx);
            out.push_str(")).mp (Eq.ndrec (motive := fun x => NobleContracts.Exec [.word 2] x _) ");
            out.push_str(step_exec);
            out.push_str(" (List.append_assoc tail ");
            out.push_str(&pfx);
            out.push_str(" [");
            out.push_str(&a);
            out.push_str(", ");
            out.push_str(&b);
            out.push_str("]).symm)) (List.append_assoc tail ");
            out.push_str(&pfx);
            out.push_str(" [");
            out.push_str(&b);
            out.push_str(", ");
            out.push_str(&a);
            out.push_str("])\n");
        }
        SegOp::Word(4..=7) => {
            let pfx = format_value_list(&stack_exprs[..pre_len.saturating_sub(2)]);
            let a = &stack_exprs[pre_len.saturating_sub(2)].1;
            let b = &stack_exprs[pre_len.saturating_sub(1)].1;
            let iff_thm = match op {
                SegOp::Word(4) => "NobleContracts.exec_add_iff",
                SegOp::Word(5) => "NobleContracts.exec_sub_iff",
                SegOp::Word(6) => "NobleContracts.exec_mul_iff",
                _ => "NobleContracts.exec_equals_iff",
            };
            let Some(last_expr) = next_stack_exprs.last() else {
                return Err(invalid(Span { start: 0, end: 0 }, "empty binop result"));
            };
            let res_val = format_single_value(last_expr);
            out.push_str("    let ");
            out.push_str(target_eq);
            out.push_str(" := Eq.trans ((");
            out.push_str(iff_thm);
            out.push_str(" (a := ");
            out.push_str(a);
            out.push_str(") (b := ");
            out.push_str(b);
            out.push_str(") (s := tail ++ ");
            out.push_str(&pfx);
            out.push_str(")).mp (Eq.ndrec (motive := fun x => NobleContracts.Exec [.word ");
            let w = match op {
                SegOp::Word(w) => *w,
                _ => 4,
            };
            crate::rendering::number(&mut out, u64::from(w));
            out.push_str("] x _) ");
            out.push_str(step_exec);
            out.push_str(" (List.append_assoc tail ");
            out.push_str(&pfx);
            out.push_str(" [.i64 ");
            out.push_str(a);
            out.push_str(", .i64 ");
            out.push_str(b);
            out.push_str("]).symm)) (List.append_assoc tail ");
            out.push_str(&pfx);
            out.push_str(" [");
            out.push_str(&res_val);
            out.push_str("])\n");
        }
        SegOp::Word(_) => {
            return Err(crate::intrinsic::unsupported(
                Span { start: 0, end: 0 },
                "unsupported word in segment claim",
            ));
        }
    }
    Ok(out)
}

struct SegClaimStepOut {
    out: String,
    next_exec: String,
    next_stack: Vec<(Sort3, String)>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; propagates step failure diagnostic cleanly"
)]
fn emit_seg_claim_step(
    mut out: String,
    seg: &SegmentSpec,
    oi: usize,
    cur_exec: &str,
    stack_exprs: &[(Sort3, String)],
) -> Result<SegClaimStepOut, Diagnostic> {
    let is_last = oi.saturating_add(1) == seg.ops.len();
    let step_exec = if is_last {
        String::from(cur_exec)
    } else {
        let hhead = fmt_str_num("hstep_", oi as u64);
        let hrest = fmt_str_num("hrest_", oi as u64);
        let hpair = fmt_str_num("hpair_", oi as u64);
        let mvar = fmt_str_num("mid_", oi as u64);
        out.push_str("    Exists.elim ((NobleContracts.exec_append_iff (a := [");
        out = emit_seg_op_syntax(out, &seg.ops[oi]);
        out.push_str("]) (b := ");
        out = emit_seg_ops_list(out, &seg.ops[oi.saturating_add(1)..]);
        out.push_str(")).mp ");
        out.push_str(cur_exec);
        out.push_str(") <| fun ");
        out.push_str(&mvar);
        out.push(' ');
        out.push_str(&hpair);
        out.push_str(" =>\n    let ");
        out.push_str(&hhead);
        out.push_str(" := ");
        out.push_str(&hpair);
        out.push_str(".1\n    let ");
        out.push_str(&hrest);
        out.push_str(" := ");
        out.push_str(&hpair);
        out.push_str(".2\n");
        hhead
    };
    let target_eq = if is_last {
        String::from("hfinal")
    } else {
        fmt_str_num("hmid_", oi as u64)
    };
    let next_stack_exprs = next_seg_stack(stack_exprs, &seg.ops[oi])?;
    out = emit_seg_claim_op(
        out,
        &seg.ops[oi],
        stack_exprs,
        &next_stack_exprs,
        &step_exec,
        &target_eq,
    )?;
    if is_last {
        return Ok(SegClaimStepOut {
            out,
            next_exec: step_exec,
            next_stack: next_stack_exprs,
        });
    }
    let hrest = fmt_str_num("hrest_", oi as u64);
    let hnext = fmt_str_num("hnext_", oi as u64);
    out.push_str("    let ");
    out.push_str(&hnext);
    out.push_str(" := Eq.ndrec (motive := fun x => NobleContracts.Exec ");
    out = emit_seg_ops_list(out, &seg.ops[oi.saturating_add(1)..]);
    out.push_str(" x final) ");
    out.push_str(&hrest);
    out.push_str(" hmid_");
    crate::rendering::number(&mut out, oi as u64);
    out.push('\n');
    Ok(SegClaimStepOut {
        out,
        next_exec: hnext,
        next_stack: next_stack_exprs,
    })
}

#[expect(
    clippy::single_char_add_str,
    reason = "Owner: noble-contracts; Aeneas rejects non-ASCII char literals"
)]
fn emit_seg_claim_close(
    mut out: String,
    k: usize,
    stack_exprs: &[(Sort3, String)],
    produced_sorts: &[Sort3],
) -> String {
    let after_list = format_value_list(stack_exprs);
    out.push_str("    ⟨");
    out.push_str(&after_list);
    out.push_str(", hfinal, ");
    out = emit_stack_typed(out, produced_sorts);
    out.push_str(", ");
    let mut wi = 0usize;
    while wi < k {
        out.push_str("segv_");
        crate::rendering::number(&mut out, wi as u64);
        out.push_str(", ");
        wi += 1;
    }
    out.push_str("⟨hbefore_eq, rfl⟩");
    out.push_str("⟩");
    out.push('\n');
    out
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits fixed Lean theorem structure per segment crate::intrinsic::operation"
)]
fn emit_segment_claim(mut out: String, seg: &SegmentSpec) -> Result<String, Diagnostic> {
    out.push_str("  claim := fun tail before params hbefore _ _ final hexec =>\n");
    let k = seg.consumed_sorts.len();
    out = emit_stack_unpack(
        out,
        StackUnpackSpec {
            hyp_name: "hbefore",
            stack_expr: "before",
            sorts: &seg.consumed_sorts,
            val_prefix: "segv_",
            val_names: &[],
            step_prefix: "sb",
            eq_name: "hbefore_eq",
        },
    );
    out.push_str("    let hexec_norm := Eq.ndrec (motive := fun b => NobleContracts.Exec ");
    out = emit_seg_ops_list(out, &seg.ops);
    out.push_str(" (tail ++ b) final) hexec hbefore_eq\n");
    let mut stack_exprs: Vec<(Sort3, String)> = Vec::with_capacity(k);
    let mut vi = 0usize;
    while vi < k {
        stack_exprs.push((seg.consumed_sorts[vi], fmt_str_num("segv_", vi as u64)));
        vi += 1;
    }
    let n_ops = seg.ops.len();
    let mut cur_exec = String::from("hexec_norm");
    let mut oi = 0usize;
    while oi < n_ops {
        let step_out = emit_seg_claim_step(out, seg, oi, &cur_exec, &stack_exprs)?;
        out = step_out.out;
        let next_exec = step_out.next_exec;
        let next_stack = step_out.next_stack;
        cur_exec = next_exec;
        stack_exprs = next_stack;
        oi += 1;
    }
    Ok(emit_seg_claim_close(
        out,
        k,
        &stack_exprs,
        &seg.produced_sorts,
    ))
}

fn format_single_value(pair: &(Sort3, String)) -> String {
    let prefix = match pair.0 {
        Sort3::I64 => "(NobleContracts.Value.i64 ",
        Sort3::Bool => "(NobleContracts.Value.bool ",
    };
    let mut s = String::from(prefix);
    s.push_str(&pair.1);
    s.push(')');
    s
}

fn format_value_list(elems: &[(Sort3, String)]) -> String {
    let mut s = String::from("[");
    let mut i = 0usize;
    while i < elems.len() {
        if i != 0 {
            s.push_str(", ");
        }
        s.push_str(&format_single_value(&elems[i]));
        i += 1;
    }
    s.push(']');
    s
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; updates symbolic segment stack per verified kernel opcode"
)]
fn next_seg_stack(cur: &[(Sort3, String)], op: &SegOp) -> Result<Vec<(Sort3, String)>, Diagnostic> {
    let mut next = cur.to_vec();
    let zero_span = Span { start: 0, end: 0 };
    match op {
        SegOp::LitI64(n) => {
            let mut s = String::from("(BitVec.ofInt 64 (");
            crate::rendering::integer(&mut s, *n);
            s.push_str("))");
            next.push((Sort3::I64, s));
        }
        SegOp::LitBool(b) => {
            next.push((Sort3::Bool, String::from(if *b { "true" } else { "false" })))
        }
        SegOp::Word(0) => {
            let Some(top) = next.last().cloned() else {
                return Err(invalid(zero_span, "dup underflow"));
            };
            next.push(top);
        }
        SegOp::Word(1) => {
            let Some(_) = next.pop() else {
                return Err(invalid(zero_span, "drop underflow"));
            };
        }
        SegOp::Word(2) => {
            let (Some(b), Some(a)) = (next.pop(), next.pop()) else {
                return Err(invalid(zero_span, "swap underflow"));
            };
            next.push(b);
            next.push(a);
        }
        SegOp::Word(4) => {
            let (Some(b), Some(a)) = (next.pop(), next.pop()) else {
                return Err(invalid(zero_span, "add"));
            };
            next.push((Sort3::I64, fmt_wrap2("(", &a.1, " + ", &b.1, ")")));
        }
        SegOp::Word(5) => {
            let (Some(b), Some(a)) = (next.pop(), next.pop()) else {
                return Err(invalid(zero_span, "sub"));
            };
            next.push((Sort3::I64, fmt_wrap2("(", &a.1, " - ", &b.1, ")")));
        }
        SegOp::Word(6) => {
            let (Some(b), Some(a)) = (next.pop(), next.pop()) else {
                return Err(invalid(zero_span, "mul"));
            };
            next.push((Sort3::I64, fmt_wrap2("(", &a.1, " * ", &b.1, ")")));
        }
        SegOp::Word(7) => {
            let (Some(b), Some(a)) = (next.pop(), next.pop()) else {
                return Err(invalid(zero_span, "eq"));
            };
            next.push((Sort3::Bool, fmt_wrap2("(", &a.1, " == ", &b.1, ")")));
        }
        SegOp::Word(_) => {
            return Err(invalid(zero_span, "unsupported word"));
        }
    }
    Ok(next)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; formats owned Lean `Op` syntax"
)]
fn emit_seg_op_syntax(mut out: String, op: &SegOp) -> String {
    match op {
        SegOp::LitI64(n) => {
            out.push_str(".lit (.i64 (BitVec.ofInt 64 (");
            crate::rendering::integer(&mut out, *n);
            out.push_str(")))");
        }
        SegOp::LitBool(true) => out.push_str(".lit (.bool true)"),
        SegOp::LitBool(false) => out.push_str(".lit (.bool false)"),
        SegOp::Word(w) => {
            out.push_str(".word ");
            crate::rendering::number(&mut out, u64::from(*w));
        }
    }
    out
}

fn emit_seg_ops_list(mut out: String, ops: &[SegOp]) -> String {
    out.push('[');
    let mut i = 0usize;
    while i < ops.len() {
        if i != 0 {
            out.push_str(", ");
        }
        out = emit_seg_op_syntax(out, &ops[i]);
        i += 1;
    }
    out.push(']');
    out
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits fixed byte profile tags for mc1 callee code"
)]
fn emit_mc1_profile_code(
    mut out: String,
    candidate: &noble_kernel::untrusted::Candidate,
    span: Span,
) -> Result<(String, String), Diagnostic> {
    let mut ops: Vec<SegOp> = Vec::with_capacity(candidate.body.len());
    let mut i = 0usize;
    while i < candidate.body.len() {
        let Ok(nid) = usize::try_from(candidate.body[i].0) else {
            return Err(invalid(span, "node id overflow"));
        };
        let Some(node) = candidate.nodes.get(nid) else {
            return Err(invalid(span, "missing node"));
        };
        match node {
            Node::Literal {
                lit: Lit::I64(v), ..
            } => ops.push(SegOp::LitI64(*v)),
            Node::Literal {
                lit: Lit::Bool(b), ..
            } => ops.push(SegOp::LitBool(*b)),
            Node::Invocation { def, .. } => ops.push(SegOp::Word(def.0)),
            Node::Literal { .. }
            | Node::SignedSelf { .. }
            | Node::Quotation { .. }
            | Node::SlotInvoke { .. } => {
                return Err(crate::intrinsic::unsupported(span, "unsupported node"));
            }
        }
        i += 1;
    }
    let code_list = emit_seg_ops_list(String::new(), &ops);
    out = emit_seg_profile_code(out, &ops);
    Ok((code_list, out))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits fixed byte tags per segment opcode"
)]
#[expect(
    clippy::single_char_add_str,
    reason = "Owner: noble-contracts; Aeneas rejects non-ASCII char literals"
)]
fn emit_seg_profile_code(mut out: String, ops: &[SegOp]) -> String {
    let mut i = 0usize;
    while i < ops.len() {
        out.push_str("⟨");
        match &ops[i] {
            SegOp::LitI64(_) | SegOp::LitBool(_) => out.push_str("trivial"),
            SegOp::Word(0) => out.push_str("Or.inl rfl"),
            SegOp::Word(1) => out.push_str("Or.inr (Or.inl rfl)"),
            SegOp::Word(2) => out.push_str("Or.inr (Or.inr (Or.inl rfl))"),
            SegOp::Word(4) => out.push_str("Or.inr (Or.inr (Or.inr (Or.inl rfl)))"),
            SegOp::Word(5) => out.push_str("Or.inr (Or.inr (Or.inr (Or.inr (Or.inl rfl))))"),
            SegOp::Word(6) => {
                out.push_str("Or.inr (Or.inr (Or.inr (Or.inr (Or.inr (Or.inl rfl)))))")
            }
            SegOp::Word(_) => {
                out.push_str("Or.inr (Or.inr (Or.inr (Or.inr (Or.inr (Or.inr rfl)))))")
            }
        }
        out.push_str(", ");
        i += 1;
    }
    out.push_str("True.intro");
    let mut c = 0usize;
    while c < ops.len() {
        out.push_str("⟩");
        c += 1;
    }
    out
}

/// Format `StateProp` predicate for chunk boundary `k`.
/// Note: for `k == 0`, uses `fun s => s = input ∧ (StackTyped input ... ∧ Holds pre input [] [])`.
/// For `k == chunks.len()`, uses `fun s => StackTyped s ... ∧ Holds post input s []`.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; builds state invariant predicate `P_k` for chunk index `k`"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; emits existentials, input equality, facts, and stack equality"
)]
fn emit_state_prop_pred(
    mut out: String,
    chunks: &[Chunk],
    k: usize,
    in_sorts: &[Sort3],
) -> Result<String, Diagnostic> {
    if k == 0 {
        out.push_str("(fun s => s = input ∧ (NobleContracts.StackTyped input NamedV3Obligation.subject.input ∧ NobleContracts.Holds NamedV3Obligation.precondition input [] []))");
        return Ok(out);
    }
    if k == chunks.len() {
        out.push_str("(fun s => NobleContracts.StackTyped s NamedV3Obligation.subject.output ∧ NobleContracts.Holds NamedV3Obligation.postcondition input s [])");
        return Ok(out);
    }
    let (stack, stack_sorts, facts, ghosts) = chunk_post_state(chunks, k.saturating_sub(1));
    let in_vars = numbered_vars("sx_", in_sorts.len());
    let mut g_vars: Vec<(u32, u32, String)> = Vec::with_capacity(ghosts.len());
    let mut gi = 0usize;
    while gi < ghosts.len() {
        g_vars.push((ghosts[gi].0, ghosts[gi].1, fmt_str_num("sg_", gi as u64)));
        gi += 1;
    }
    out.push_str("(fun s => ");
    let mut ii = 0usize;
    while ii < in_sorts.len() {
        out.push_str("∃ (");
        out.push_str(&in_vars[ii]);
        out.push_str(" : ");
        push_sort_lean_type_buf(&mut out, in_sorts[ii]);
        out.push_str("), ");
        ii += 1;
    }
    let mut gi = 0usize;
    while gi < ghosts.len() {
        out.push_str("∃ (");
        out.push_str(&g_vars[gi].2);
        out.push_str(" : ");
        push_sort_lean_type_buf(&mut out, ghosts[gi].2);
        out.push_str("), ");
        gi += 1;
    }
    out.push_str("input = [");
    let mut ii = 0usize;
    while ii < in_sorts.len() {
        if ii != 0 {
            out.push_str(", ");
        }
        out.push_str(match in_sorts[ii] {
            Sort3::I64 => ".i64 ",
            Sort3::Bool => ".bool ",
        });
        out.push_str(&in_vars[ii]);
        ii += 1;
    }
    out.push_str("] ∧ (");
    let mut outcome = Ok(());
    if facts.is_empty() {
        out.push_str("True");
    } else {
        let mut fi = 0usize;
        while fi < facts.len() {
            if fi != 0 {
                out.push_str(" ∧ ");
            }
            outcome = match emit_prop3_lean(
                core::mem::take(&mut out),
                &facts[fi],
                &[],
                &in_vars,
                &g_vars,
            ) {
                Ok(next) => {
                    out = next;
                    Ok(())
                }
                Err(problem) => Err(problem),
            };
            if outcome.is_err() {
                break;
            }
            fi += 1;
        }
    }
    outcome?;
    out.push_str(") ∧ s = [");
    let mut outcome = Ok(());
    let mut si = 0usize;
    while si < stack.len() {
        if si != 0 {
            out.push_str(", ");
        }
        outcome = match emit_term3_value_lean(
            core::mem::take(&mut out),
            &stack[si],
            stack_sorts[si],
            &in_vars,
            &g_vars,
        ) {
            Ok(next) => {
                out = next;
                Ok(())
            }
            Err(problem) => Err(problem),
        };
        if outcome.is_err() {
            break;
        }
        si += 1;
    }
    outcome?;
    out.push_str("])");
    Ok(out)
}

type PreStateParts<'a> = (
    &'a [Term3],
    &'a [Sort3],
    &'a [Prop3],
    Vec<(u32, u32, Sort3)>,
);

type PreStateOwned<'a> = (Vec<(u32, u32, Sort3)>, &'a [Prop3], Vec<Term3>, Vec<Sort3>);

fn chunk_post_state(chunks: &[Chunk], idx: usize) -> PreStateParts<'_> {
    let mut ghosts: Vec<(u32, u32, Sort3)> = Vec::new();
    let mut i = 0usize;
    while i <= idx {
        if let Chunk::Call { new_ghosts, .. } = &chunks[i] {
            ghosts.extend_from_slice(new_ghosts);
        }
        i += 1;
    }
    match &chunks[idx] {
        Chunk::Segment(seg) => (&seg.after_stack, &seg.after_sorts, &seg.facts, ghosts),
        Chunk::Call {
            after_stack,
            after_sorts,
            facts,
            ..
        } => (after_stack, after_sorts, facts, ghosts),
    }
}

struct ChunkStepCtx<'a> {
    trace: &'a super::LowerTrace,
    limits: crate::Limits,
    chunks: &'a [Chunk],
    k: usize,
    is_first: bool,
    is_last: bool,
    in_sorts: &'a [Sort3],
    pre_ghosts: Vec<(u32, u32, Sort3)>,
    pre_facts_ref: &'a [Prop3],
    pre_stack_ref: Vec<Term3>,
    pre_sorts_ref: Vec<Sort3>,
    in_vars: Vec<String>,
    g_vars: Vec<(u32, u32, String)>,
    fact_vars: Vec<String>,
}

#[derive(Clone, Copy)]
struct EmitChunkInput<'a> {
    trace: &'a super::LowerTrace,
    chunks: &'a [Chunk],
    in_sorts: &'a [Sort3],
    limits: crate::Limits,
}

/// Emit every `LiftedChain.segment` / `LiftedChain.cons` step iteratively and close with `.nil`.
fn emit_chunk_chain(mut out: String, input: EmitChunkInput<'_>) -> Result<String, Diagnostic> {
    let mut k = 0usize;
    while k < input.chunks.len() {
        out = emit_one_chunk(out, input, k)?;
        k = k.saturating_add(1);
    }
    out.push_str("(NobleContracts.NamedV3.LiftedChain.nil _)");
    let mut close_idx = 0usize;
    while close_idx < input.chunks.len() {
        out.push(')');
        close_idx = close_idx.saturating_add(1);
    }
    Ok(out)
}

fn numbered_vars(prefix: &str, count: usize) -> Vec<String> {
    assert!(!prefix.is_empty());
    let mut vars: Vec<String> = Vec::with_capacity(count);
    let mut idx = 0usize;
    while idx < count {
        vars.push(fmt_str_num(prefix, idx as u64));
        idx = idx.saturating_add(1);
    }
    vars
}

fn collect_ghost_vars(pre_ghosts: &[(u32, u32, Sort3)]) -> Vec<(u32, u32, String)> {
    let mut g_vars: Vec<(u32, u32, String)> = Vec::with_capacity(pre_ghosts.len());
    let mut idx = 0usize;
    while idx < pre_ghosts.len() {
        g_vars.push((
            pre_ghosts[idx].0,
            pre_ghosts[idx].1,
            fmt_str_num("sg_", idx as u64),
        ));
        idx = idx.saturating_add(1);
    }
    g_vars
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; prepares state frame and dispatches segment vs call chunk emission"
)]
fn emit_one_chunk(out: String, input: EmitChunkInput<'_>, k: usize) -> Result<String, Diagnostic> {
    let EmitChunkInput {
        trace,
        chunks,
        in_sorts,
        limits,
    } = input;
    let is_first = k == 0;
    let is_last = k.saturating_add(1) == chunks.len();
    let (pre_ghosts, pre_facts_ref, pre_stack_ref, pre_sorts_ref): PreStateOwned<'_> = if is_first {
        let mut st: Vec<Term3> = Vec::with_capacity(in_sorts.len());
        let mut idx = 0usize;
        while idx < in_sorts.len() {
            st.push(Term3::Input(idx));
            idx = idx.saturating_add(1);
        }
        (Vec::new(), &trace.initial_facts, st, in_sorts.to_vec())
    } else {
        let (st, ss, fc, gh) = chunk_post_state(chunks, k.saturating_sub(1));
        (gh, fc, st.to_vec(), ss.to_vec())
    };
    let in_vars = numbered_vars("sx_", in_sorts.len());
    let g_vars = collect_ghost_vars(&pre_ghosts);
    let fact_vars = numbered_vars("fact_", pre_facts_ref.len());
    let ctx = ChunkStepCtx {
        trace,
        limits,
        chunks,
        k,
        is_first,
        is_last,
        in_sorts,
        pre_ghosts,
        pre_facts_ref,
        pre_stack_ref,
        pre_sorts_ref,
        in_vars,
        g_vars,
        fact_vars,
    };
    match &chunks[k] {
        Chunk::Segment(seg) => emit_segment_chunk(out, seg, &ctx),
        Chunk::Call {
            call_idx,
            occ_ui,
            carried_len,
            after_stack,
            after_sorts,
            facts,
            new_ghosts,
        } => emit_call_chunk(
            out,
            CallChunkArgs {
                call_idx: *call_idx,
                call: &trace.calls[*call_idx],
                occ_ui: *occ_ui,
                carried_len: *carried_len,
                after_stack,
                after_sorts,
                facts,
                new_ghosts,
            },
            &ctx,
        ),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; lowers term values without early loop returns"
)]
fn emit_term_values_csv(
    mut out: String,
    terms: &[Term3],
    sorts: &[Sort3],
    in_vars: &[String],
    g_vars: &[(u32, u32, String)],
) -> Result<String, Diagnostic> {
    let mut i = 0usize;
    let mut outcome = Ok(());
    while i < terms.len() {
        if i != 0 {
            out.push_str(", ");
        }
        outcome = match emit_term3_value_lean(
            core::mem::take(&mut out),
            &terms[i],
            sorts[i],
            in_vars,
            g_vars,
        ) {
            Ok(next) => {
                out = next;
                Ok(())
            }
            Err(problem) => Err(problem),
        };
        if outcome.is_err() {
            break;
        }
        i += 1;
    }
    outcome?;
    Ok(out)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; lowers term value strings without early loop returns"
)]
#[expect(
    tigerstyle::allocation_in_loop,
    reason = "Owner: noble-contracts; allocates one rendered Lean term string per element"
)]
fn collect_term_values(
    terms: &[Term3],
    sorts: &[Sort3],
    in_vars: &[String],
    g_vars: &[(u32, u32, String)],
) -> Result<Vec<String>, Diagnostic> {
    let mut out = Vec::with_capacity(terms.len());
    let mut i = 0usize;
    let mut outcome = Ok(());
    while i < terms.len() {
        outcome = match emit_term3_value_lean(String::new(), &terms[i], sorts[i], in_vars, g_vars) {
            Ok(val) => {
                out.push(val);
                Ok(())
            }
            Err(problem) => Err(problem),
        };
        if outcome.is_err() {
            break;
        }
        i += 1;
    }
    outcome?;
    Ok(out)
}

#[expect(
    tigerstyle::allocation_in_loop,
    reason = "Owner: noble-contracts; allocates one rendered Lean term string per element"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; lowers Lean term expressions without early loop returns"
)]
fn collect_term3_lean_vals(
    terms: &[Term3],
    in_vars: &[String],
    g_vars: &[(u32, u32, String)],
) -> Result<Vec<String>, Diagnostic> {
    let mut out = Vec::with_capacity(terms.len());
    let mut i = 0usize;
    let mut outcome = Ok(());
    while i < terms.len() {
        outcome = match emit_term3_lean(String::new(), &terms[i], &[], in_vars, g_vars) {
            Ok(val) => {
                out.push(val);
                Ok(())
            }
            Err(problem) => Err(problem),
        };
        if outcome.is_err() {
            break;
        }
        i += 1;
    }
    outcome?;
    Ok(out)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits segment `hr` congruence chain without early loop returns"
)]
fn emit_seg_hr_chain(
    mut out: String,
    seg: &SegmentSpec,
    consumed_vals: &[String],
) -> Result<String, Diagnostic> {
    let sk = seg.consumed_sorts.len();
    out.push_str("    let seg_hr_0 := hres\n");
    let mut ai = 0usize;
    let mut outcome = Ok(());
    while ai < sk {
        outcome = match compute_seg_step_list(seg, ai, consumed_vals) {
            Ok(step_list) => {
                let next_ai = ai.saturating_add(1);
                out.push_str("    let seg_hr_");
                crate::rendering::number(&mut out, next_ai as u64);
                out.push_str(" := Eq.trans seg_hr_");
                crate::rendering::number(&mut out, ai as u64);
                out.push_str(" (congrArg (fun _seg_pv : ");
                push_sort_lean_type_buf(&mut out, seg.consumed_sorts[ai]);
                out.push_str(" => (");
                out.push_str(&step_list);
                out.push_str(" : List NobleContracts.Value)) (");
                out.push_str(match seg.consumed_sorts[ai] {
                    Sort3::I64 => "NobleContracts.Value.i64.inj sj_eq_",
                    Sort3::Bool => "NobleContracts.Value.bool.inj sj_eq_",
                });
                crate::rendering::number(&mut out, seg.carried.len().saturating_add(ai) as u64);
                out.push_str("))\n");
                Ok(())
            }
            Err(problem) => Err(problem),
        };
        if outcome.is_err() {
            break;
        }
        ai += 1;
    }
    outcome?;
    Ok(out)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; bounded by segment ops length"
)]
fn compute_seg_step_list(
    seg: &SegmentSpec,
    ai: usize,
    consumed_vals: &[String],
) -> Result<String, Diagnostic> {
    let sk = seg.consumed_sorts.len();
    let mut step_vars: Vec<(Sort3, String)> = Vec::with_capacity(sk);
    let mut si = 0usize;
    while si < sk {
        if si < ai {
            step_vars.push((seg.consumed_sorts[si], consumed_vals[si].clone()));
        } else if si == ai {
            step_vars.push((seg.consumed_sorts[si], String::from("_seg_pv")));
        } else {
            step_vars.push((seg.consumed_sorts[si], fmt_str_num("pv_", si as u64)));
        }
        si += 1;
    }
    let mut cur_step = step_vars;
    let mut oi = 0usize;
    while oi < seg.ops.len() {
        cur_step = next_seg_stack(&cur_step, &seg.ops[oi])?;
        oi += 1;
    }
    Ok(format_value_list(&cur_step))
}

fn push_pv_binders(mut out: String, sorts: &[Sort3]) -> String {
    let mut si = 0usize;
    while si < sorts.len() {
        out.push_str("∃ (pv_");
        crate::rendering::number(&mut out, si as u64);
        out.push_str(" : ");
        push_sort_lean_type_buf(&mut out, sorts[si]);
        out.push_str("), ");
        si += 1;
    }
    out
}

fn collect_pv_vars(sorts: &[Sort3]) -> Vec<(Sort3, String)> {
    let mut p_vars: Vec<(Sort3, String)> = Vec::with_capacity(sorts.len());
    let mut idx = 0usize;
    while idx < sorts.len() {
        p_vars.push((sorts[idx], fmt_str_num("pv_", idx as u64)));
        idx = idx.saturating_add(1);
    }
    p_vars
}

fn step_seg_p_vars(
    mut cur_p: Vec<(Sort3, String)>,
    ops: &[SegOp],
) -> Result<Vec<(Sort3, String)>, Diagnostic> {
    let mut outcome = Ok(());
    let mut oi = 0usize;
    while oi < ops.len() {
        match next_seg_stack(&cur_p, &ops[oi]) {
            Ok(next) => cur_p = next,
            Err(problem) => outcome = Err(problem),
        }
        if outcome.is_err() {
            break;
        }
        oi += 1;
    }
    outcome?;
    Ok(cur_p)
}

fn collect_pair_sorts(pairs: &[(String, Sort3)]) -> Vec<Sort3> {
    let mut out = Vec::with_capacity(pairs.len());
    let mut i = 0usize;
    while i < pairs.len() {
        out.push(pairs[i].1);
        i = i.saturating_add(1);
    }
    out
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits fixed Lean lemma parameter list"
)]
fn emit_lemma_params(mut out: String, lemmas: &[super::Lemma]) -> Result<String, Diagnostic> {
    let mut outcome: Result<(), Diagnostic> = Ok(());
    let mut li = 0usize;
    while li < lemmas.len() {
        out.push_str("(lemma_");
        crate::rendering::number(&mut out, li as u64);
        out.push_str(" : ");
        match emit_statement3_lean(core::mem::take(&mut out), &lemmas[li].statement) {
            Ok(next) => {
                out = next;
                out.push_str(") ");
            }
            Err(problem) => outcome = Err(problem),
        }
        if outcome.is_err() {
            break;
        }
        li += 1;
    }
    outcome?;
    Ok(out)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits fixed Lean callee parameter list"
)]
fn emit_callee_params(
    mut out: String,
    goal: &crate::intrinsic::ContractGoal,
    trace: &super::LowerTrace,
    limits: crate::Limits,
) -> Result<String, Diagnostic> {
    let mut outcome: Result<(), Diagnostic> = Ok(());
    let mut cti = 0usize;
    while cti < trace.calls.len() {
        let call = &trace.calls[cti];
        let occ = &goal.subject.named_uses[call.occ_idx];
        let occ_ui = find_use_idx_by_occ(&goal.subject.named_uses, occ).unwrap_or(1);
        out.push_str("(callee_");
        crate::rendering::number(&mut out, cti as u64);
        out.push_str(" : ");
        match emit_callee_param_type(
            core::mem::take(&mut out),
            &trace.distinct_callees[call.callee_index],
            occ_ui,
            limits,
        ) {
            Ok(next) => {
                out = next;
                out.push_str(") ");
            }
            Err(problem) => outcome = Err(problem),
        }
        if outcome.is_err() {
            break;
        }
        cti += 1;
    }
    outcome?;
    Ok(out)
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; accumulates trace chunks sequentially"
)]
fn step_trace_chunks(
    goal: &crate::intrinsic::ContractGoal,
    trace: &super::LowerTrace,
    in_sorts: &[Sort3],
    acc: &mut TraceChunkAccum,
) -> Result<(), Diagnostic> {
    let mut outcome: Result<(), Diagnostic> = Ok(());
    let mut si = 0usize;
    while si < trace.steps.len() {
        outcome = step_trace_chunk(&trace.steps[si], goal, trace, in_sorts, acc);
        if outcome.is_err() {
            break;
        }
        si += 1;
    }
    outcome
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits bridge and join witnesses for one straight-line segment"
)]
#[expect(
    clippy::single_char_add_str,
    reason = "Owner: noble-contracts; Aeneas rejects non-ASCII char literals"
)]
fn emit_segment_chunk(
    mut out: String,
    seg: &SegmentSpec,
    ctx: &ChunkStepCtx<'_>,
) -> Result<String, Diagnostic> {
    let ChunkStepCtx {
        trace,
        limits: _,
        chunks,
        k,
        is_first,
        is_last: _,
        in_sorts,
        pre_ghosts,
        pre_facts_ref,
        pre_stack_ref: _,
        pre_sorts_ref: _,
        in_vars,
        g_vars,
        fact_vars: _,
    } = ctx;
    let (k, is_first) = (*k, *is_first);
    out.push_str("(NobleContracts.NamedV3.LiftedChain.segment\n(P := ");
    out = emit_state_prop_pred(out, chunks, k, in_sorts)?;
    out.push_str(") (Q := ");
    out = emit_state_prop_pred(out, chunks, k.saturating_add(1), in_sorts)?;
    out.push_str(")\n{\n  code := ");
    out = emit_seg_ops_list(out, &seg.ops);
    out.push_str(",\n  profileCode := ");
    out = emit_seg_profile_code(out, &seg.ops);
    out.push_str(",\n  inputTypes := ");
    out = push_sort_list(out, &seg.consumed_sorts);
    out.push_str(",\n  outputTypes := ");
    out = push_sort_list(out, &seg.produced_sorts);
    out.push_str(",\n  pre := fun _ _ _ => True,\n  post := fun before after _ => ");
    out = push_pv_binders(out, &seg.consumed_sorts);
    let p_vars = collect_pv_vars(&seg.consumed_sorts);
    let cur_p = step_seg_p_vars(p_vars.clone(), &seg.ops)?;
    out.push_str("before = ");
    out.push_str(&format_value_list(&p_vars));
    out.push_str(" ∧ after = ");
    out.push_str(&format_value_list(&cur_p));
    out.push_str(",\n");
    out = emit_segment_claim(out, seg)?;
    // `bridge`:
    out.push_str(",\n  bridge := fun s hs =>\n");
    out = emit_unpack_pre_state(
        out,
        UnpackPreStateSpec {
            trace,
            is_first,
            in_vars,
            ghost_vars: g_vars,
            in_sorts,
            n_ghosts: pre_ghosts.len(),
            n_facts: pre_facts_ref.len(),
            hyp_name: "hs",
            state_expr: "s",
        },
    )?;
    out.push_str("    ⟨[");
    out = emit_term_values_csv(out, &seg.carried, &seg.carried_sorts, in_vars, g_vars)?;
    out.push_str("], [");
    out = emit_term_values_csv(out, &seg.consumed, &seg.consumed_sorts, in_vars, g_vars)?;
    out.push_str("], ");
    out.push_str("hs_eq");
    out.push_str(", True.intro, ");
    out = emit_stack_typed(out, &seg.consumed_sorts);
    out.push_str("⟩");
    out = emit_segment_join(out, SegmentJoinSpec { seg, ctx })?;
    Ok(out)
}

fn push_csv_items(mut out: String, items: &[String]) -> String {
    let mut i = 0usize;
    while i < items.len() {
        if i != 0 {
            out.push_str(", ");
        }
        out.push_str(&items[i]);
        i += 1;
    }
    out
}

fn emit_seg_hpost_unpack(mut out: String, sorts: &[Sort3]) -> String {
    let sk = sorts.len();
    let mut si = 0usize;
    while si < sk {
        out.push_str("    Exists.elim hpost <| fun pv_");
        crate::rendering::number(&mut out, si as u64);
        out.push_str(" hpost =>\n");
        si += 1;
    }
    out.push_str("    let hargs := hpost.1\n    let hres := hpost.2\n");
    out
}

fn format_typed_res_list(callee_out_sorts: &[Sort3], rv_names: &[String]) -> String {
    let mut res_list = String::from("[");
    let mut ri = 0usize;
    while ri < callee_out_sorts.len() {
        if ri != 0 {
            res_list.push_str(", ");
        }
        res_list.push_str(match callee_out_sorts[ri] {
            Sort3::I64 => "NobleContracts.Value.i64 ",
            Sort3::Bool => "NobleContracts.Value.bool ",
        });
        res_list.push_str(&rv_names[ri]);
        ri += 1;
    }
    res_list.push(']');
    res_list
}

struct SegmentJoinSpec<'a> {
    seg: &'a SegmentSpec,
    ctx: &'a ChunkStepCtx<'a>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits segment join proof witnesses"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; unpacks pre-state, splits stack equality, and proves segment join"
)]
fn emit_segment_join(mut out: String, spec: SegmentJoinSpec<'_>) -> Result<String, Diagnostic> {
    let seg = spec.seg;
    let ChunkStepCtx {
        trace,
        limits: _,
        chunks: _,
        k: _,
        is_first,
        is_last,
        in_sorts,
        pre_ghosts,
        pre_facts_ref,
        pre_stack_ref,
        pre_sorts_ref,
        in_vars,
        g_vars,
        fact_vars,
    } = spec.ctx;
    let (is_first, is_last) = (*is_first, *is_last);
    let sk = seg.consumed_sorts.len();
    // `join`:
    out.push_str(",\n  join := fun carried args res hP hpost _ =>\n");
    out = emit_unpack_pre_state(
        out,
        UnpackPreStateSpec {
            trace,
            is_first,
            in_vars,
            ghost_vars: g_vars,
            in_sorts,
            n_ghosts: pre_ghosts.len(),
            n_facts: pre_facts_ref.len(),
            hyp_name: "hP",
            state_expr: "(carried ++ args)",
        },
    )?;
    // Unpack `hpost`: `∃ pv_0 ..., args = [...] ∧ res = [...]`
    out = emit_seg_hpost_unpack(out, &seg.consumed_sorts);
    let pv_names = numbered_vars("pv_", sk);
    let pv_list = format_typed_res_list(&seg.consumed_sorts, &pv_names);
    out.push_str("    let hcomb : carried ++ ");
    out.push_str(&pv_list);
    out.push_str(" = [");
    let target_elems = collect_term_values(pre_stack_ref, pre_sorts_ref, in_vars, g_vars)?;
    out = push_csv_items(out, &target_elems);
    out.push_str("] := Eq.trans (congrArg (carried ++ ·) hargs.symm) ");
    out.push_str(choose_str(is_first, "hP_eq\n", "hs_eq\n"));
    out = emit_split_carried_match(
        out,
        SplitCarriedMatchSpec {
            eq_name: "hcomb",
            carried_var: "carried",
            tail_list: &pv_list,
            carried_count: seg.carried.len(),
            args_count: sk,
            strip_args: true,
            target_elems: &target_elems,
            var_prefix: "sj",
        },
    );
    let consumed_vals = collect_term3_lean_vals(&seg.consumed, in_vars, g_vars)?;
    out = emit_seg_hr_chain(out, seg, &consumed_vals)?;
    out.push_str("    let hres_subst : carried ++ res = [");
    out = emit_term_values_csv(out, &seg.after_stack, &seg.after_sorts, in_vars, g_vars)?;
    out.push_str("] := Eq.trans (congr (congrArg List.append sj_carried_eq) seg_hr_");
    crate::rendering::number(&mut out, sk as u64);
    out.push_str(") (Eq.trans (congr (congrArg List.append ");
    out = emit_list_congr_from_prefix(out, "sj_eq_", seg.carried.len());
    out.push_str(") rfl) rfl)\n");
    if is_last {
        out = emit_final_join_body(
            out,
            FinalJoinBodySpec {
                trace,
                after_stack: &seg.after_stack,
                after_sorts: &seg.after_sorts,
                after_facts: &seg.facts,
                in_vars,
                ghost_vars: g_vars,
                fact_vars,
                in_sorts,
                hres_subst_name: "hres_subst",
            },
        )?;
    } else {
        out = emit_mid_join_body(
            out,
            MidJoinSpec {
                in_vars,
                ghost_vars: g_vars,
                fact_vars,
                n_in: in_sorts.len(),
                hres_subst_name: "hres_subst",
            },
        );
    }
    out.push_str(" } ");
    Ok(out)
}

struct CallChunkArgs<'a> {
    call_idx: usize,
    call: &'a super::CallTrace,
    occ_ui: usize,
    carried_len: usize,
    after_stack: &'a [Term3],
    after_sorts: &'a [Sort3],
    facts: &'a [Prop3],
    new_ghosts: &'a [(u32, u32, Sort3)],
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits bridge and join witnesses for one traced call step"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; emits complete `LiftedChain.cons` call record cleanly"
)]
#[expect(
    clippy::single_char_add_str,
    reason = "Owner: noble-contracts; Aeneas rejects non-ASCII char literals"
)]
fn emit_call_chunk(
    mut out: String,
    args: CallChunkArgs<'_>,
    ctx: &ChunkStepCtx<'_>,
) -> Result<String, Diagnostic> {
    let CallChunkArgs {
        call_idx,
        call,
        occ_ui,
        carried_len,
        after_stack,
        after_sorts,
        facts,
        new_ghosts,
    } = args;
    let (occ_ui, carried_len) = (&occ_ui, &carried_len);
    let ChunkStepCtx {
        trace,
        limits,
        chunks,
        k,
        is_first,
        is_last: _,
        in_sorts,
        pre_ghosts,
        pre_facts_ref,
        pre_stack_ref,
        pre_sorts_ref,
        in_vars,
        g_vars,
        fact_vars,
    } = ctx;
    let (k, is_first) = (*k, *is_first);
    out.push_str("(NobleContracts.NamedV3.LiftedChain.cons\n(P := ");
    out = emit_state_prop_pred(out, chunks, k, in_sorts)?;
    out.push_str(") (Q := ");
    out = emit_state_prop_pred(out, chunks, k.saturating_add(1), in_sorts)?;
    out.push_str(")\n{\n  subject := NamedV3Obligation.calleeSubject_");
    crate::rendering::number(&mut out, *occ_ui as u64);
    out.push_str(",\n  entry := NamedV3Obligation.entry_");
    crate::rendering::number(&mut out, u64::from(call.occ_slot));
    out.push_str(",\n  use := NamedV3Obligation.use_");
    crate::rendering::number(&mut out, *occ_ui as u64);
    out.push_str(",\n  embedding := NamedV3Obligation.calleeEmbedding_");
    crate::rendering::number(&mut out, *occ_ui as u64);
    out.push_str(",\n  bodies := NamedV3Obligation.calleeBodies_");
    crate::rendering::number(&mut out, *occ_ui as u64);
    out.push_str(",\n  profile := NamedV3Obligation.calleeProfile_");
    crate::rendering::number(&mut out, *occ_ui as u64);
    let callee_in_sorts = collect_pair_sorts(&call.callee_inputs);
    let callee_out_sorts = collect_pair_sorts(&call.callee_outputs);
    let c_in_str = emit_noble_term_from_prop3(String::new(), &call.callee_requires)?;
    let c_out_str = emit_noble_term_from_prop3(String::new(), &call.callee_ensures)?;
    out.push_str(",\n  pre := NobleContracts.Holds ");
    out.push_str(&c_in_str);
    out.push_str(",\n  post := fun before after _ => NobleContracts.StackTyped before NamedV3Obligation.calleeSubject_");
    crate::rendering::number(&mut out, *occ_ui as u64);
    out.push_str(".input ∧ NobleContracts.Holds ");
    out.push_str(&c_out_str);
    out.push_str(" before after [],\n  claim := fun tail before final hbefore hpre execution => Exists.elim (");
    let callee_goal = &trace.distinct_callees[call.callee_index];
    if callee_goal.revision == 1 {
        let prepared = super::super::prepare_contract(callee_goal, *limits)?;
        let (code_list, prof) = emit_mc1_profile_code(
            String::new(),
            prepared.candidate(),
            callee_goal.contract_span,
        )?;
        out.push_str("(NobleContracts.NamedV3.liftedClaim_of_mc1 (code := ");
        out.push_str(&code_list);
        out.push_str(") NamedV3Obligation.calleeSubject_");
        crate::rendering::number(&mut out, *occ_ui as u64);
        out.push_str(" (by simp only [NamedV3Obligation.calleeSubject_");
        crate::rendering::number(&mut out, *occ_ui as u64);
        out.push_str(", NamedV3Obligation.entry_");
        crate::rendering::number(&mut out, u64::from(call.occ_slot));
        out.push_str(", NobleContracts.NamedV1.liftCode.eq_1, List.map, NobleContracts.NamedV1.liftOp, NobleContracts.NamedV1.liftValue]) ");
        out.push_str(&prof);
        out.push_str(" callee_");
        crate::rendering::number(&mut out, call_idx as u64);
        out.push(')');
    } else {
        out.push_str("callee_");
        crate::rendering::number(&mut out, call_idx as u64);
    }
    out.push_str(" tail before final hbefore hpre execution) <| fun after h => Exists.intro after (And.intro h.1 (And.intro h.2.1 (And.intro hbefore h.2.2)))");
    // `bridge`:
    let split = *carried_len;
    let arg_terms = &pre_stack_ref[split..];
    out.push_str(",\n  bridge := fun s hs =>\n");
    out = emit_unpack_pre_state(
        out,
        UnpackPreStateSpec {
            trace,
            is_first,
            in_vars,
            ghost_vars: g_vars,
            in_sorts,
            n_ghosts: pre_ghosts.len(),
            n_facts: pre_facts_ref.len(),
            hyp_name: "hs",
            state_expr: "s",
        },
    )?;
    // `h_bridge` proves `Prop3_to_Prop(call.bridge_goal)`!
    let (bridge_proof_raw, bridge_stmt) = emit_prop_proof(
        &call.bridge_form,
        ProofScope {
            facts: pre_facts_ref,
            lemmas: &trace.lemmas,
            scope_inputs: &trace.view.inputs,
            scope_outputs: &trace.view.outputs,
            in_vars,
            ghost_vars: g_vars,
            fact_vars,
        },
    )?;
    let Statement3::Prop(bridge_prop) = bridge_stmt else {
        return Err(invalid(Span { start: 0, end: 0 }, "bridge proof not prop"));
    };
    let expected_bridge = super::instantiate_prop(&call.callee_requires, arg_terms, &[]);
    let bridge_proof_expr = emit_cast_prop(
        &bridge_prop,
        &expected_bridge,
        &bridge_proof_raw,
        &[],
        in_vars,
        g_vars,
    )?;
    let arg_vals_only = collect_term3_lean_vals(arg_terms, in_vars, g_vars)?;
    let mut arg_list = String::from("[");
    arg_list = emit_term_values_csv(arg_list, arg_terms, &callee_in_sorts, in_vars, g_vars)?;
    arg_list.push(']');
    let ctx_bridge = fmt_wrap2("⟨", &arg_list, ", ", "[], []", "⟩");
    let iff_bridge = emit_eval_prop3_iff(
        &call.callee_requires,
        &ctx_bridge,
        &callee_in_sorts,
        &callee_out_sorts,
        &arg_vals_only,
        &[],
    )?;
    out.push_str("    ⟨[");
    out = emit_term_values_csv(
        out,
        &pre_stack_ref[..split],
        &pre_sorts_ref[..split],
        in_vars,
        g_vars,
    )?;
    out.push_str("], ");
    out.push_str(&arg_list);
    out.push_str(", ");
    out.push_str("hs_eq");
    out.push_str(", (");
    out.push_str(&iff_bridge);
    out.push_str(").mpr (");
    out.push_str(&bridge_proof_expr);
    out.push_str("), ");
    out = emit_stack_typed(out, &callee_in_sorts);
    out.push_str("⟩");
    emit_call_join(
        out,
        CallJoinSpec {
            call,
            after_stack,
            after_sorts,
            facts,
            new_ghosts,
            callee_in_sorts: &callee_in_sorts,
            callee_out_sorts: &callee_out_sorts,
            c_out_str: &c_out_str,
            arg_terms,
            arg_list: &arg_list,
            arg_vals_only: &arg_vals_only,
            split,
            ctx,
        },
    )
}

struct CallJoinSpec<'a> {
    call: &'a super::CallTrace,
    after_stack: &'a [Term3],
    after_sorts: &'a [Sort3],
    facts: &'a [Prop3],
    new_ghosts: &'a [(u32, u32, Sort3)],
    callee_in_sorts: &'a [Sort3],
    callee_out_sorts: &'a [Sort3],
    c_out_str: &'a str,
    arg_terms: &'a [Term3],
    arg_list: &'a str,
    arg_vals_only: &'a [String],
    split: usize,
    ctx: &'a ChunkStepCtx<'a>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits call join proof witnesses"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; unpacks call arguments/results and proves call join"
)]
fn emit_call_join(mut out: String, spec: CallJoinSpec<'_>) -> Result<String, Diagnostic> {
    let CallJoinSpec {
        call,
        after_stack,
        after_sorts,
        facts,
        new_ghosts,
        callee_in_sorts,
        callee_out_sorts,
        c_out_str,
        arg_terms,
        arg_list,
        arg_vals_only,
        split,
        ctx,
    } = spec;
    let ChunkStepCtx {
        trace,
        limits: _,
        chunks: _,
        k: _,
        is_first,
        is_last,
        in_sorts,
        pre_ghosts,
        pre_facts_ref,
        pre_stack_ref,
        pre_sorts_ref,
        in_vars,
        g_vars,
        fact_vars,
    } = ctx;
    let (is_first, is_last) = (*is_first, *is_last);
    let arity = callee_in_sorts.len();
    let rlen = callee_out_sorts.len();
    out.push_str(",\n  join := fun carried args res hP hpost hres_typed =>\n");
    out = emit_unpack_pre_state(
        out,
        UnpackPreStateSpec {
            trace,
            is_first,
            in_vars,
            ghost_vars: g_vars,
            in_sorts,
            n_ghosts: pre_ghosts.len(),
            n_facts: pre_facts_ref.len(),
            hyp_name: "hP",
            state_expr: "(carried ++ args)",
        },
    )?;
    out = emit_stack_unpack(
        out,
        StackUnpackSpec {
            hyp_name: "hpost.1",
            stack_expr: "args",
            sorts: callee_in_sorts,
            val_prefix: "ca_",
            val_names: &[],
            step_prefix: "ca",
            eq_name: "hargs_typed_eq",
        },
    );
    out = emit_stack_unpack(
        out,
        StackUnpackSpec {
            hyp_name: "hres_typed",
            stack_expr: "res",
            sorts: callee_out_sorts,
            val_prefix: "rv_",
            val_names: &[],
            step_prefix: "cr",
            eq_name: "hres_val",
        },
    );
    let ca_names = numbered_vars("ca_", arity);
    let ca_list = format_typed_res_list(callee_in_sorts, &ca_names);
    let target_elems = collect_term_values(pre_stack_ref, pre_sorts_ref, in_vars, g_vars)?;
    let eq_h_p = choose_str(is_first, "hP_eq", "hs_eq");
    out.push_str("    let hcomb_cj := Eq.trans (congrArg (carried ++ ·) hargs_typed_eq.symm) ");
    out.push_str(eq_h_p);
    out.push('\n');
    out = emit_split_carried_match(
        out,
        SplitCarriedMatchSpec {
            eq_name: "hcomb_cj",
            carried_var: "carried",
            tail_list: &ca_list,
            carried_count: split,
            args_count: arity,
            strip_args: false,
            target_elems: &target_elems,
            var_prefix: "cj",
        },
    );
    let rv_names = numbered_vars("rv_", rlen);
    let res_list = format_typed_res_list(callee_out_sorts, &rv_names);
    out.push_str("    let hargs_val : args = ");
    out.push_str(arg_list);
    out.push_str(" := Eq.trans hargs_typed_eq cj_exact\n");
    let ctx_post = fmt_wrap2("⟨", arg_list, ", ", &res_list, ", []⟩");
    let iff_callee_post = emit_eval_prop3_iff(
        &call.callee_ensures,
        &ctx_post,
        callee_in_sorts,
        callee_out_sorts,
        arg_vals_only,
        &rv_names,
    )?;
    out.push_str("    have hcallee_facts := (");
    out.push_str(&iff_callee_post);
    out.push_str(").mp (Eq.ndrec (motive := fun rs => NobleContracts.Holds ");
    out.push_str(c_out_str);
    out.push(' ');
    out.push_str(arg_list);
    out.push_str(" rs []) (Eq.ndrec (motive := fun ag => NobleContracts.Holds ");
    out.push_str(c_out_str);
    out.push_str(" ag res []) hpost.2 hargs_val) hres_val)\n");
    out = emit_call_join_close(
        out,
        CallJoinCloseSpec {
            trace,
            call,
            callee_out_sorts,
            rv_names: &rv_names,
            pre_g_vars: g_vars,
            in_vars,
            fact_vars,
            in_sorts,
            arg_terms,
            new_ghosts,
            after_stack,
            after_sorts,
            after_facts: facts,
            split,
            is_last,
        },
    )?;
    out.push_str(" } ");
    Ok(out)
}

struct CallJoinCloseSpec<'a> {
    trace: &'a super::LowerTrace,
    call: &'a super::CallTrace,
    callee_out_sorts: &'a [Sort3],
    rv_names: &'a [String],
    pre_g_vars: &'a [(u32, u32, String)],
    in_vars: &'a [String],
    fact_vars: &'a [String],
    in_sorts: &'a [Sort3],
    arg_terms: &'a [Term3],
    new_ghosts: &'a [(u32, u32, Sort3)],
    after_stack: &'a [Term3],
    after_sorts: &'a [Sort3],
    after_facts: &'a [Prop3],
    split: usize,
    is_last: bool,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; scans bounded callee conjunction slice for direct output equality"
)]
fn find_direct_cfact(
    cconj: &[Prop3],
    direct_outputs: &[Option<Term3>],
    dj: usize,
) -> Option<usize> {
    if dj >= direct_outputs.len() || direct_outputs[dj].is_none() {
        return None;
    }
    let mut found = None;
    let mut k = 0usize;
    while found.is_none() && k < cconj.len() {
        let is_out_eq = match &cconj[k] {
            Prop3::Eq(lhs, _) => match lhs {
                Term3::Output(out_idx) => *out_idx == dj,
                Term3::Input(_)
                | Term3::Bound(_)
                | Term3::Ghost(..)
                | Term3::I64(_)
                | Term3::Bool(_)
                | Term3::Add(..)
                | Term3::Sub(..)
                | Term3::Mul(..)
                | Term3::Equals(..) => false,
            },
            Prop3::True
            | Prop3::False
            | Prop3::Not(..)
            | Prop3::And(..)
            | Prop3::Lt(..)
            | Prop3::Le(..) => false,
        };
        if is_out_eq {
            found = Some(k);
        }
        k += 1;
    }
    found
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits congruence chain for callee outputs"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; receives callee output specs and normalization scope directly"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-contracts; emits congruence chain with optional constant-folding normalization"
)]
fn emit_call_res_congr(
    mut out: String,
    direct_outputs: &[Option<Term3>],
    cconj: &[Prop3],
    callee_out_sorts: &[Sort3],
    rv_names: &[String],
    arg_terms: &[Term3],
    in_vars: &[String],
    pre_g_vars: &[(u32, u32, String)],
) -> Result<String, Diagnostic> {
    let rlen = callee_out_sorts.len();
    if rlen == 0 {
        out.push_str("(rfl : ([] : List NobleContracts.Value) = [])");
        return Ok(out);
    }
    let mut dj = 0usize;
    while dj < rlen {
        out.push_str("(congr (congrArg List.cons ");
        let found_cfact = find_direct_cfact(cconj, direct_outputs, dj);
        if let (Some(k), Some(dir)) = (found_cfact, &direct_outputs[dj]) {
            let raw_rhs = match &cconj[k] {
                Prop3::Eq(_, r) => super::map_term(
                    r,
                    super::TermSubst::Interface {
                        inputs: arg_terms,
                        outputs: &[],
                    },
                ),
                Prop3::True
                | Prop3::False
                | Prop3::Not(..)
                | Prop3::And(..)
                | Prop3::Lt(..)
                | Prop3::Le(..) => dir.clone(),
            };
            out.push_str(match callee_out_sorts[dj] {
                Sort3::I64 => "(congrArg NobleContracts.Value.i64 ",
                Sort3::Bool => "(congrArg NobleContracts.Value.bool ",
            });
            if &raw_rhs == dir {
                out.push_str("cfact_");
                crate::rendering::number(&mut out, k as u64);
            } else {
                let (eq_norm, _, _) = emit_term_norm_eq(&raw_rhs, &[], in_vars, pre_g_vars)?;
                out.push_str("(Eq.trans cfact_");
                crate::rendering::number(&mut out, k as u64);
                out.push(' ');
                out.push_str(&eq_norm);
                out.push(')');
            }
            out.push(')');
        } else {
            out.push_str(match callee_out_sorts[dj] {
                Sort3::I64 => "(rfl : NobleContracts.Value.i64 ",
                Sort3::Bool => "(rfl : NobleContracts.Value.bool ",
            });
            out.push_str(&rv_names[dj]);
            out.push_str(match callee_out_sorts[dj] {
                Sort3::I64 => " = NobleContracts.Value.i64 ",
                Sort3::Bool => " = NobleContracts.Value.bool ",
            });
            out.push_str(&rv_names[dj]);
            out.push(')');
        }
        out.push_str(") ");
        dj += 1;
    }
    out.push_str("rfl");
    let mut ri = 0usize;
    while ri < rlen {
        out.push(')');
        ri += 1;
    }
    Ok(out)
}

struct CallSubstFactSpec<'a> {
    call: &'a super::CallTrace,
    cconj: &'a [Prop3],
    callee_out_sorts: &'a [Sort3],
    init_outs: &'a [Term3],
    subst_g_vars: &'a [(u32, u32, String)],
    post_g_vars: &'a [(u32, u32, String)],
    in_vars: &'a [String],
    arg_terms: &'a [Term3],
    target_fact: Option<&'a Prop3>,
    ci: usize,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; builds one output-substitution step cleanly"
)]
fn emit_call_subst_step(
    spec: &CallSubstFactSpec<'_>,
    cur_proof: String,
    mut cur_outs: Vec<Term3>,
    dj: usize,
) -> Result<(String, Vec<Term3>), Diagnostic> {
    if spec.call.direct_outputs[dj].is_none() {
        return Ok((cur_proof, cur_outs));
    }
    let Some(k) = find_direct_cfact(spec.cconj, &spec.call.direct_outputs, dj) else {
        return Ok((cur_proof, cur_outs));
    };
    let raw_dir_term = match &spec.cconj[k] {
        Prop3::Eq(_, r) => super::map_term(
            r,
            super::TermSubst::Interface {
                inputs: spec.arg_terms,
                outputs: &[],
            },
        ),
        Prop3::True
        | Prop3::False
        | Prop3::Not(..)
        | Prop3::And(..)
        | Prop3::Lt(..)
        | Prop3::Le(..) => return Ok((cur_proof, cur_outs)),
    };
    let out_u32 = u32::try_from(dj).unwrap_or(0);
    let mut motive_outs = cur_outs.clone();
    motive_outs[dj] = Term3::Ghost(
        u32::MAX.saturating_sub(1),
        out_u32,
        spec.callee_out_sorts[dj],
    );
    let motive_prop = super::instantiate_prop(&spec.cconj[spec.ci], spec.arg_terms, &motive_outs);
    let mut motive_g_vars = spec.subst_g_vars.to_vec();
    motive_g_vars.push((u32::MAX.saturating_sub(1), out_u32, String::from("_sub_rv")));
    let motive_str = emit_prop3_lean(
        String::new(),
        &motive_prop,
        &[],
        spec.in_vars,
        &motive_g_vars,
    )?;
    let mut next_proof = String::from("(Eq.ndrec (motive := fun (_sub_rv : ");
    push_sort_lean_type_buf(&mut next_proof, spec.callee_out_sorts[dj]);
    next_proof.push_str(") => ");
    next_proof.push_str(&motive_str);
    next_proof.push_str(") ");
    next_proof.push_str(&cur_proof);
    next_proof.push_str(" cfact_");
    crate::rendering::number(&mut next_proof, k as u64);
    next_proof.push(')');
    cur_outs[dj] = raw_dir_term;
    Ok((next_proof, cur_outs))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; rewrites callee output variables into direct expressions cleanly"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; handles trivial/refl fast paths, output substitution loop, and final cast together"
)]
fn emit_call_subst_fact(
    mut out: String,
    spec: CallSubstFactSpec<'_>,
) -> Result<(String, String), Diagnostic> {
    let sfact_name = fmt_str_num("sfact_", spec.ci as u64);
    if let Some(tf) = spec.target_fact {
        let is_trivial = match tf {
            Prop3::True => true,
            Prop3::False
            | Prop3::Not(..)
            | Prop3::And(..)
            | Prop3::Eq(..)
            | Prop3::Lt(..)
            | Prop3::Le(..) => false,
        };
        if is_trivial {
            out.push_str("    let ");
            out.push_str(&sfact_name);
            out.push_str(" : True := True.intro\n");
            return Ok((out, sfact_name));
        }
        let is_refl = match tf {
            Prop3::Eq(l, r) => l == r,
            Prop3::True
            | Prop3::False
            | Prop3::Not(..)
            | Prop3::And(..)
            | Prop3::Lt(..)
            | Prop3::Le(..) => false,
        };
        if is_refl {
            let target_str =
                emit_prop3_lean(String::new(), tf, &[], spec.in_vars, spec.post_g_vars)?;
            out.push_str("    let ");
            out.push_str(&sfact_name);
            out.push_str(" : ");
            out.push_str(&target_str);
            out.push_str(" := rfl\n");
            return Ok((out, sfact_name));
        }
    }
    let mut cur_state: Result<(String, Vec<Term3>), Diagnostic> = Ok((
        fmt_str_num("cfact_", spec.ci as u64),
        spec.init_outs.to_vec(),
    ));
    let mut dj = 0usize;
    while dj < spec.callee_out_sorts.len() {
        cur_state = match cur_state {
            Ok((cur_proof, cur_outs)) => emit_call_subst_step(&spec, cur_proof, cur_outs, dj),
            Err(err) => Err(err),
        };
        dj += 1;
    }
    let (cur_proof, raw_outs) = cur_state?;
    let final_proof = if let Some(tf) = spec.target_fact {
        let raw_post_fact =
            super::instantiate_prop(&spec.cconj[spec.ci], spec.arg_terms, &raw_outs);
        emit_cast_prop(
            &raw_post_fact,
            tf,
            &cur_proof,
            &[],
            spec.in_vars,
            spec.post_g_vars,
        )?
    } else {
        cur_proof
    };
    out.push_str("    let ");
    out.push_str(&sfact_name);
    out.push_str(" := ");
    out.push_str(&final_proof);
    out.push('\n');
    Ok((out, sfact_name))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits post-call join proof obligations"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; binds call post-facts, rewrites stack equality, and proves join"
)]
fn emit_call_join_close(
    mut out: String,
    spec: CallJoinCloseSpec<'_>,
) -> Result<String, Diagnostic> {
    let CallJoinCloseSpec {
        trace,
        call,
        callee_out_sorts,
        rv_names,
        pre_g_vars,
        in_vars,
        fact_vars,
        in_sorts,
        arg_terms,
        new_ghosts,
        after_stack,
        after_sorts,
        after_facts,
        split,
        is_last,
    } = spec;
    // Unpack conjunctions of `hcallee_facts`:
    let mut cconj: Vec<Prop3> = Vec::new();
    super::collect_conjuncts_owned(&call.callee_ensures, &mut cconj);
    out = unpack_prop_conjuncts(out, "hcallee_facts", "cfact_", &call.callee_ensures);
    let mut new_fact_vars = Vec::with_capacity(fact_vars.len().saturating_add(cconj.len()));
    new_fact_vars.extend_from_slice(fact_vars);
    let mut subst_g_vars = pre_g_vars.to_vec();
    let mut init_outs: Vec<Term3> = Vec::with_capacity(callee_out_sorts.len());
    let mut dj_init = 0usize;
    while dj_init < callee_out_sorts.len() {
        let out_u32 = u32::try_from(dj_init).unwrap_or(0);
        subst_g_vars.push((u32::MAX, out_u32, rv_names[dj_init].clone()));
        init_outs.push(Term3::Ghost(u32::MAX, out_u32, callee_out_sorts[dj_init]));
        dj_init += 1;
    }
    // Build `post_g_vars`: extend `pre_g_vars` with `new_ghosts` bound to corresponding `rv_j`!
    let mut post_g_vars = pre_g_vars.to_vec();
    let mut ghost_err: Option<Diagnostic> = None;
    let mut ngi = 0usize;
    while ngi < new_ghosts.len() {
        let (_, out_id, _) = new_ghosts[ngi];
        let mut matched_idx: Option<usize> = None;
        if let Ok(idx) = usize::try_from(out_id) {
            if idx < rv_names.len() {
                matched_idx = Some(idx);
            }
        }
        if let Some(idx) = matched_idx {
            post_g_vars.push((new_ghosts[ngi].0, out_id, rv_names[idx].clone()));
        } else if ghost_err.is_none() {
            ghost_err = Some(invalid(Span { start: 0, end: 0 }, "ghost rv oob"));
        }
        ngi += 1;
    }
    if let Some(err) = ghost_err {
        return Err(err);
    }

    let mut out_state: Result<String, Diagnostic> = Ok(out);
    let mut ci = 0usize;
    while ci < cconj.len() {
        let target_fact = after_facts.get(fact_vars.len().saturating_add(ci));
        if let Ok(cur_out) = out_state {
            let res = emit_call_subst_fact(
                cur_out,
                CallSubstFactSpec {
                    call,
                    cconj: &cconj,
                    callee_out_sorts,
                    init_outs: &init_outs,
                    subst_g_vars: &subst_g_vars,
                    post_g_vars: &post_g_vars,
                    in_vars,
                    arg_terms,
                    target_fact,
                    ci,
                },
            );
            out_state = match res {
                Ok((next_out, sfact_name)) => {
                    new_fact_vars.push(sfact_name);
                    Ok(next_out)
                }
                Err(err) => Err(err),
            };
        }
        ci += 1;
    }
    let mut out = out_state?;

    // Prove `carried ++ res = [after_0, ...]` without tactics.
    out.push_str("    let hres_subst : carried ++ res = [");
    out = emit_term_values_csv(out, after_stack, after_sorts, in_vars, &post_g_vars)?;
    out.push_str("] := Eq.trans (congr (congrArg List.append cj_carried_eq) hres_val) (Eq.trans (congr (congrArg List.append ");
    out = emit_list_congr_from_prefix(out, "cj_eq_", split);
    out.push_str(") ");
    out = emit_call_res_congr(
        out,
        &call.direct_outputs,
        &cconj,
        callee_out_sorts,
        rv_names,
        arg_terms,
        in_vars,
        pre_g_vars,
    )?;
    out.push_str(
        ") rfl)
",
    );

    if is_last {
        out = emit_final_join_body(
            out,
            FinalJoinBodySpec {
                trace,
                after_stack,
                after_sorts,
                after_facts,
                in_vars,
                ghost_vars: &post_g_vars,
                fact_vars: &new_fact_vars,
                in_sorts,
                hres_subst_name: "hres_subst",
            },
        )?;
    } else {
        out = emit_mid_join_body(
            out,
            MidJoinSpec {
                in_vars,
                ghost_vars: &post_g_vars,
                fact_vars: &new_fact_vars,
                n_in: in_sorts.len(),
                hres_subst_name: "hres_subst",
            },
        );
    }
    Ok(out)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; splits right-nested Lean `And` hypothesis linearly"
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; root hypothesis and binding prefix are distinct Lean identifiers"
)]
fn unpack_conjuncts_bind(mut out: String, root: &str, prefix: &str, count: usize) -> String {
    if count == 0 {
        return out;
    }
    if count == 1 {
        out.push_str("    let ");
        out.push_str(prefix);
        out.push_str("0 := ");
        out.push_str(root);
        out.push('\n');
        return out;
    }
    let mut cur = String::from(root);
    let mut i = 0usize;
    while i.saturating_add(1) < count {
        let next = i.saturating_add(1);
        let tail_name = if i.saturating_add(2) == count {
            fmt_str_num(prefix, next as u64)
        } else {
            fmt_prefix_mid_num(prefix, "rest_", i as u64)
        };
        out.push_str("    let ");
        out.push_str(prefix);
        crate::rendering::number(&mut out, i as u64);
        out.push_str(" := ");
        out.push_str(&cur);
        out.push_str(".1\n    let ");
        out.push_str(&tail_name);
        out.push_str(" := ");
        out.push_str(&cur);
        out.push_str(".2\n");
        cur = tail_name;
        i = next;
    }
    out
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; unpacks arbitrary binary `And` proposition tree iteratively"
)]
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; bounded by validated proposition AST node count"
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; root hypothesis and binding prefix are distinct Lean identifiers"
)]
fn unpack_prop_conjuncts(mut out: String, root: &str, prefix: &str, prop: &Prop3) -> String {
    let mut pending: Vec<(String, Prop3)> = Vec::new();
    pending.push((String::from(root), prop.clone()));
    let mut rest_id = 0usize;
    let mut leaf_id = 0usize;
    while let Some((cur_hyp, node)) = pending.pop() {
        match node {
            Prop3::And(left, right) => {
                let l_name = fmt_prefix_mid_num(prefix, "rest_", rest_id as u64);
                rest_id = rest_id.saturating_add(1);
                let r_name = fmt_prefix_mid_num(prefix, "rest_", rest_id as u64);
                rest_id = rest_id.saturating_add(1);
                out.push_str("    let ");
                out.push_str(&l_name);
                out.push_str(" := ");
                out.push_str(&cur_hyp);
                out.push_str(".1\n    let ");
                out.push_str(&r_name);
                out.push_str(" := ");
                out.push_str(&cur_hyp);
                out.push_str(".2\n");
                pending.push((r_name, *right));
                pending.push((l_name, *left));
            }
            Prop3::True
            | Prop3::False
            | Prop3::Not(..)
            | Prop3::Eq(..)
            | Prop3::Lt(..)
            | Prop3::Le(..) => {
                out.push_str("    let ");
                out.push_str(prefix);
                crate::rendering::number(&mut out, leaf_id as u64);
                out.push_str(" := ");
                out.push_str(&cur_hyp);
                out.push('\n');
                leaf_id = leaf_id.saturating_add(1);
            }
        }
    }
    out
}

#[octet::sealed_enum]
enum NormTermEqStep {
    Visit(Term3),
    Combine(super::BinTermOp),
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; advances term normalization proof worklist and stack"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; updates bounded normalization worklist and output stack"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; passes workstack, output stack, and Lean binder scopes explicitly"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; validates term operands and builds equality witness"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; handles term leaves and binary folding witnesses in one switch"
)]
fn emit_term_norm_step(
    step: NormTermEqStep,
    work: &mut Vec<NormTermEqStep>,
    out: &mut Vec<(String, Term3, bool)>,
    bound_names: &[String],
    inputs: &[String],
    ghosts: &[(u32, u32, String)],
) -> Result<(), Diagnostic> {
    match step {
        NormTermEqStep::Visit(node) => match node {
            Term3::Add(l, r) => {
                work.push(NormTermEqStep::Combine(super::BinTermOp::Add));
                work.push(NormTermEqStep::Visit(*r));
                work.push(NormTermEqStep::Visit(*l));
            }
            Term3::Sub(l, r) => {
                work.push(NormTermEqStep::Combine(super::BinTermOp::Sub));
                work.push(NormTermEqStep::Visit(*r));
                work.push(NormTermEqStep::Visit(*l));
            }
            Term3::Mul(l, r) => {
                work.push(NormTermEqStep::Combine(super::BinTermOp::Mul));
                work.push(NormTermEqStep::Visit(*r));
                work.push(NormTermEqStep::Visit(*l));
            }
            Term3::Equals(l, r) => {
                work.push(NormTermEqStep::Combine(super::BinTermOp::Equals));
                work.push(NormTermEqStep::Visit(*r));
                work.push(NormTermEqStep::Visit(*l));
            }
            Term3::Input(_)
            | Term3::Output(_)
            | Term3::Bound(_)
            | Term3::Ghost(..)
            | Term3::I64(_)
            | Term3::Bool(_) => {
                let t_lean = emit_term3_lean(String::new(), &node, bound_names, inputs, ghosts)?;
                let eq_expr = fmt_wrap2("(rfl : ", &t_lean, " = ", &t_lean, ")");
                out.push((eq_expr, node, true));
            }
        },
        NormTermEqStep::Combine(op) => {
            let (Some((eq_r, nr, is_r_refl)), Some((eq_l, nl, is_l_refl))) = (out.pop(), out.pop())
            else {
                return Err(invalid(Span { start: 0, end: 0 }, "term norm stack"));
            };
            let is_congr_refl = is_l_refl && is_r_refl;
            let congr_name = match op {
                super::BinTermOp::Add => "(eq_add_congr ",
                super::BinTermOp::Sub => "(eq_sub_congr ",
                super::BinTermOp::Mul => "(eq_mul_congr ",
                super::BinTermOp::Equals => "(eq_equals_congr ",
            };
            let congr_expr = if is_congr_refl {
                String::new()
            } else {
                fmt_wrap2(congr_name, &eq_l, " ", &eq_r, ")")
            };
            let op_sym = match op {
                super::BinTermOp::Add => " + ",
                super::BinTermOp::Sub => " - ",
                super::BinTermOp::Mul => " * ",
                super::BinTermOp::Equals => "",
            };
            let folded = super::fold_binary_term(op, nl.clone(), nr.clone());
            match (&op, &folded) {
                (
                    super::BinTermOp::Add | super::BinTermOp::Sub | super::BinTermOp::Mul,
                    Term3::I64(_),
                ) => {
                    let nl_lean = emit_term3_lean(String::new(), &nl, bound_names, inputs, ghosts)?;
                    let nr_lean = emit_term3_lean(String::new(), &nr, bound_names, inputs, ghosts)?;
                    let f_lean =
                        emit_term3_lean(String::new(), &folded, bound_names, inputs, ghosts)?;
                    let fold_eq = fmt_wrap3("(by decide : (", &nl_lean, op_sym, &nr_lean, ") = ")
                        + &f_lean
                        + ")";
                    let res_expr = if is_congr_refl {
                        fold_eq
                    } else {
                        fmt_wrap2("(Eq.trans ", &congr_expr, " ", &fold_eq, ")")
                    };
                    out.push((res_expr, folded, false));
                }
                (super::BinTermOp::Equals, Term3::Bool(is_eq_true)) => {
                    let nl_lean = emit_term3_lean(String::new(), &nl, bound_names, inputs, ghosts)?;
                    let nr_lean = emit_term3_lean(String::new(), &nr, bound_names, inputs, ghosts)?;
                    let fold_eq = if *is_eq_true {
                        fmt_wrap2(
                            "(decide_eq_true_of_eq (rfl : ",
                            &nl_lean,
                            " = ",
                            &nr_lean,
                            "))",
                        )
                    } else {
                        fmt_wrap2(
                            "(decide_eq_false_of_ne (by decide : ",
                            &nl_lean,
                            " ≠ ",
                            &nr_lean,
                            "))",
                        )
                    };
                    let res_expr = if is_congr_refl {
                        fold_eq
                    } else {
                        fmt_wrap2("(Eq.trans ", &congr_expr, " ", &fold_eq, ")")
                    };
                    out.push((res_expr, folded, false));
                }
                (
                    super::BinTermOp::Add
                    | super::BinTermOp::Sub
                    | super::BinTermOp::Mul
                    | super::BinTermOp::Equals,
                    Term3::Input(_)
                    | Term3::Output(_)
                    | Term3::Bound(_)
                    | Term3::Ghost(..)
                    | Term3::I64(_)
                    | Term3::Bool(_)
                    | Term3::Add(..)
                    | Term3::Sub(..)
                    | Term3::Mul(..)
                    | Term3::Equals(..),
                ) => {
                    if is_congr_refl {
                        let f_lean =
                            emit_term3_lean(String::new(), &folded, bound_names, inputs, ghosts)?;
                        out.push((
                            fmt_wrap2("(rfl : ", &f_lean, " = ", &f_lean, ")"),
                            folded,
                            true,
                        ));
                    } else {
                        out.push((congr_expr, folded, false));
                    }
                }
            }
        }
    }
    Ok(())
}

fn emit_term_norm_eq(
    term: &Term3,
    bound_names: &[String],
    inputs: &[String],
    ghosts: &[(u32, u32, String)],
) -> Result<(String, Term3, bool), Diagnostic> {
    let mut work = Vec::new();
    work.push(NormTermEqStep::Visit(term.clone()));
    let mut out: Vec<(String, Term3, bool)> = Vec::new();
    let mut outcome = Ok(());
    while let Some(step) = work.pop() {
        outcome = emit_term_norm_step(step, &mut work, &mut out, bound_names, inputs, ghosts);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    out.pop()
        .ok_or_else(|| invalid(Span { start: 0, end: 0 }, "empty term norm stack"))
}

#[octet::sealed_enum]
enum NormPropIffStep {
    Visit(Prop3),
    CombineNot,
    CombineAnd,
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; advances proposition normalization proof worklist and stack"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; updates bounded normalization worklist and output stack"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; passes workstack, output stack, and Lean binder scopes explicitly"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; validates proposition operands and builds Iff witness"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; handles all Prop3 normalization cases in one flat switch"
)]
#[expect(
    tigerstyle::nested_conditionals,
    reason = "Owner: noble-contracts; flat AST dispatch over step, Prop3 node, folded result, and congruence flag"
)]
fn emit_prop_norm_step(
    step: NormPropIffStep,
    work: &mut Vec<NormPropIffStep>,
    out: &mut Vec<(String, Prop3, bool)>,
    bound_names: &[String],
    inputs: &[String],
    ghosts: &[(u32, u32, String)],
) -> Result<(), Diagnostic> {
    match step {
        NormPropIffStep::Visit(node) => match node {
            Prop3::True => out.push((String::from("(Iff.rfl : True ↔ True)"), Prop3::True, true)),
            Prop3::False => out.push((
                String::from("(Iff.rfl : False ↔ False)"),
                Prop3::False,
                true,
            )),
            Prop3::Not(body) => {
                work.push(NormPropIffStep::CombineNot);
                work.push(NormPropIffStep::Visit(*body));
            }
            Prop3::And(left, right) => {
                work.push(NormPropIffStep::CombineAnd);
                work.push(NormPropIffStep::Visit(*right));
                work.push(NormPropIffStep::Visit(*left));
            }
            Prop3::Eq(l, r) => {
                let (eq_l, nl, is_l_refl) = emit_term_norm_eq(&l, bound_names, inputs, ghosts)?;
                let (eq_r, nr, is_r_refl) = emit_term_norm_eq(&r, bound_names, inputs, ghosts)?;
                let is_congr_refl = is_l_refl && is_r_refl;
                let congr_expr = if is_congr_refl {
                    String::new()
                } else {
                    fmt_wrap2("(iff_eq_congr ", &eq_l, " ", &eq_r, ")")
                };
                let folded = super::fold_eq_prop(nl.clone(), nr.clone());
                let nl_lean = emit_term3_lean(String::new(), &nl, bound_names, inputs, ghosts)?;
                let nr_lean = emit_term3_lean(String::new(), &nr, bound_names, inputs, ghosts)?;
                match &folded {
                    Prop3::True => {
                        let fold_iff =
                            fmt_wrap2("(iff_true_intro (rfl : ", &nl_lean, " = ", &nr_lean, "))");
                        let res = if is_congr_refl {
                            fold_iff
                        } else {
                            fmt_wrap2("(Iff.trans ", &congr_expr, " ", &fold_iff, ")")
                        };
                        out.push((res, Prop3::True, false));
                    }
                    Prop3::False => {
                        let fold_iff = fmt_wrap2(
                            "(iff_false_intro (by decide : ¬ (",
                            &nl_lean,
                            " = ",
                            &nr_lean,
                            ")))",
                        );
                        let res = if is_congr_refl {
                            fold_iff
                        } else {
                            fmt_wrap2("(Iff.trans ", &congr_expr, " ", &fold_iff, ")")
                        };
                        out.push((res, Prop3::False, false));
                    }
                    Prop3::Not(..)
                    | Prop3::And(..)
                    | Prop3::Eq(..)
                    | Prop3::Lt(..)
                    | Prop3::Le(..) => {
                        if is_congr_refl {
                            let f_lean = emit_prop3_lean(
                                String::new(),
                                &folded,
                                bound_names,
                                inputs,
                                ghosts,
                            )?;
                            out.push((
                                fmt_wrap2("(Iff.rfl : ", &f_lean, " ↔ ", &f_lean, ")"),
                                folded,
                                true,
                            ));
                        } else {
                            out.push((congr_expr, folded, false));
                        }
                    }
                }
            }
            Prop3::Lt(l, r) => {
                let (eq_l, nl, is_l_refl) = emit_term_norm_eq(&l, bound_names, inputs, ghosts)?;
                let (eq_r, nr, is_r_refl) = emit_term_norm_eq(&r, bound_names, inputs, ghosts)?;
                let is_congr_refl = is_l_refl && is_r_refl;
                let congr_expr = if is_congr_refl {
                    String::new()
                } else {
                    fmt_wrap2("(iff_lt_congr ", &eq_l, " ", &eq_r, ")")
                };
                let folded = super::fold_order_prop(nl.clone(), nr.clone(), true);
                let nl_lean = emit_term3_lean(String::new(), &nl, bound_names, inputs, ghosts)?;
                let nr_lean = emit_term3_lean(String::new(), &nr, bound_names, inputs, ghosts)?;
                match &folded {
                    Prop3::True => {
                        let fold_iff = fmt_wrap2(
                            "(iff_true_intro (by decide : (",
                            &nl_lean,
                            ").toInt < (",
                            &nr_lean,
                            ").toInt))",
                        );
                        let res = if is_congr_refl {
                            fold_iff
                        } else {
                            fmt_wrap2("(Iff.trans ", &congr_expr, " ", &fold_iff, ")")
                        };
                        out.push((res, Prop3::True, false));
                    }
                    Prop3::False => {
                        let fold_iff = fmt_wrap2(
                            "(iff_false_intro (by decide : ¬ ((",
                            &nl_lean,
                            ").toInt < (",
                            &nr_lean,
                            ").toInt)))",
                        );
                        let res = if is_congr_refl {
                            fold_iff
                        } else {
                            fmt_wrap2("(Iff.trans ", &congr_expr, " ", &fold_iff, ")")
                        };
                        out.push((res, Prop3::False, false));
                    }
                    Prop3::Not(..)
                    | Prop3::And(..)
                    | Prop3::Eq(..)
                    | Prop3::Lt(..)
                    | Prop3::Le(..) => {
                        if is_congr_refl {
                            let f_lean = emit_prop3_lean(
                                String::new(),
                                &folded,
                                bound_names,
                                inputs,
                                ghosts,
                            )?;
                            out.push((
                                fmt_wrap2("(Iff.rfl : ", &f_lean, " ↔ ", &f_lean, ")"),
                                folded,
                                true,
                            ));
                        } else {
                            out.push((congr_expr, folded, false));
                        }
                    }
                }
            }
            Prop3::Le(l, r) => {
                let (eq_l, nl, is_l_refl) = emit_term_norm_eq(&l, bound_names, inputs, ghosts)?;
                let (eq_r, nr, is_r_refl) = emit_term_norm_eq(&r, bound_names, inputs, ghosts)?;
                let is_congr_refl = is_l_refl && is_r_refl;
                let congr_expr = if is_congr_refl {
                    String::new()
                } else {
                    fmt_wrap2("(iff_le_congr ", &eq_l, " ", &eq_r, ")")
                };
                let folded = super::fold_order_prop(nl.clone(), nr.clone(), false);
                let nl_lean = emit_term3_lean(String::new(), &nl, bound_names, inputs, ghosts)?;
                let nr_lean = emit_term3_lean(String::new(), &nr, bound_names, inputs, ghosts)?;
                match &folded {
                    Prop3::True => {
                        let fold_iff = fmt_wrap2(
                            "(iff_true_intro (by decide : (",
                            &nl_lean,
                            ").toInt ≤ (",
                            &nr_lean,
                            ").toInt))",
                        );
                        let res = if is_congr_refl {
                            fold_iff
                        } else {
                            fmt_wrap2("(Iff.trans ", &congr_expr, " ", &fold_iff, ")")
                        };
                        out.push((res, Prop3::True, false));
                    }
                    Prop3::False => {
                        let fold_iff = fmt_wrap2(
                            "(iff_false_intro (by decide : ¬ ((",
                            &nl_lean,
                            ").toInt ≤ (",
                            &nr_lean,
                            ").toInt)))",
                        );
                        let res = if is_congr_refl {
                            fold_iff
                        } else {
                            fmt_wrap2("(Iff.trans ", &congr_expr, " ", &fold_iff, ")")
                        };
                        out.push((res, Prop3::False, false));
                    }
                    Prop3::Not(..)
                    | Prop3::And(..)
                    | Prop3::Eq(..)
                    | Prop3::Lt(..)
                    | Prop3::Le(..) => {
                        if is_congr_refl {
                            let f_lean = emit_prop3_lean(
                                String::new(),
                                &folded,
                                bound_names,
                                inputs,
                                ghosts,
                            )?;
                            out.push((
                                fmt_wrap2("(Iff.rfl : ", &f_lean, " ↔ ", &f_lean, ")"),
                                folded,
                                true,
                            ));
                        } else {
                            out.push((congr_expr, folded, false));
                        }
                    }
                }
            }
        },
        NormPropIffStep::CombineNot => {
            let Some((iff_b, nb, is_b_refl)) = out.pop() else {
                return Err(invalid(Span { start: 0, end: 0 }, "prop norm not stack"));
            };
            let congr_expr = if is_b_refl {
                String::new()
            } else {
                let mut s = String::from("(not_congr ");
                s.push_str(&iff_b);
                s.push(')');
                s
            };
            match nb {
                Prop3::True => {
                    let res = if is_b_refl {
                        String::from("iff_not_true")
                    } else {
                        fmt_wrap2("(Iff.trans ", &congr_expr, " ", "iff_not_true", ")")
                    };
                    out.push((res, Prop3::False, false));
                }
                Prop3::False => {
                    let res = if is_b_refl {
                        String::from("iff_not_false")
                    } else {
                        fmt_wrap2("(Iff.trans ", &congr_expr, " ", "iff_not_false", ")")
                    };
                    out.push((res, Prop3::True, false));
                }
                Prop3::Not(inner) => {
                    let inner_lean =
                        emit_prop3_lean(String::new(), &inner, bound_names, inputs, ghosts)?;
                    let fold_iff = fmt_wrap2("(@iff_not_not ", &inner_lean, "", "", ")");
                    let res = if is_b_refl {
                        fold_iff
                    } else {
                        fmt_wrap2("(Iff.trans ", &congr_expr, " ", &fold_iff, ")")
                    };
                    out.push((res, *inner, false));
                }
                other => {
                    let wrapped = Prop3::Not(Box::new(other));
                    if is_b_refl {
                        let w_lean =
                            emit_prop3_lean(String::new(), &wrapped, bound_names, inputs, ghosts)?;
                        out.push((
                            fmt_wrap2("(Iff.rfl : ", &w_lean, " ↔ ", &w_lean, ")"),
                            wrapped,
                            true,
                        ));
                    } else {
                        out.push((congr_expr, wrapped, false));
                    }
                }
            }
        }
        NormPropIffStep::CombineAnd => {
            let (Some((iff_r, nr, is_r_refl)), Some((iff_l, nl, is_l_refl))) =
                (out.pop(), out.pop())
            else {
                return Err(invalid(Span { start: 0, end: 0 }, "prop norm and stack"));
            };
            let is_congr_refl = is_l_refl && is_r_refl;
            let congr_expr = if is_congr_refl {
                String::new()
            } else {
                fmt_wrap2("(and_congr ", &iff_l, " ", &iff_r, ")")
            };
            let (fold_iff, res_prop) = match (&nl, &nr) {
                (Prop3::False, _) => {
                    let nr_lean = emit_prop3_lean(String::new(), &nr, bound_names, inputs, ghosts)?;
                    (
                        Some(fmt_wrap2("(@iff_and_false_l ", &nr_lean, "", "", ")")),
                        Prop3::False,
                    )
                }
                (_, Prop3::False) => {
                    let nl_lean = emit_prop3_lean(String::new(), &nl, bound_names, inputs, ghosts)?;
                    (
                        Some(fmt_wrap2("(@iff_and_false_r ", &nl_lean, "", "", ")")),
                        Prop3::False,
                    )
                }
                (Prop3::True, _) => {
                    let nr_lean = emit_prop3_lean(String::new(), &nr, bound_names, inputs, ghosts)?;
                    (
                        Some(fmt_wrap2("(@iff_and_true_l ", &nr_lean, "", "", ")")),
                        nr,
                    )
                }
                (_, Prop3::True) => {
                    let nl_lean = emit_prop3_lean(String::new(), &nl, bound_names, inputs, ghosts)?;
                    (
                        Some(fmt_wrap2("(@iff_and_true_r ", &nl_lean, "", "", ")")),
                        nl,
                    )
                }
                _ => (None, Prop3::And(Box::new(nl), Box::new(nr))),
            };
            if let Some(f_iff) = fold_iff {
                let expr = if is_congr_refl {
                    f_iff
                } else {
                    fmt_wrap2("(Iff.trans ", &congr_expr, " ", &f_iff, ")")
                };
                out.push((expr, res_prop, false));
            } else if is_congr_refl {
                let r_lean =
                    emit_prop3_lean(String::new(), &res_prop, bound_names, inputs, ghosts)?;
                out.push((
                    fmt_wrap2("(Iff.rfl : ", &r_lean, " ↔ ", &r_lean, ")"),
                    res_prop,
                    true,
                ));
            } else {
                out.push((congr_expr, res_prop, false));
            }
        }
    }
    Ok(())
}

fn emit_norm_prop_iff(
    prop: &Prop3,
    bound_names: &[String],
    inputs: &[String],
    ghosts: &[(u32, u32, String)],
) -> Result<(String, Prop3, bool), Diagnostic> {
    let mut work = Vec::new();
    work.push(NormPropIffStep::Visit(prop.clone()));
    let mut out: Vec<(String, Prop3, bool)> = Vec::new();
    let mut outcome = Ok(());
    while let Some(step) = work.pop() {
        outcome = emit_prop_norm_step(step, &mut work, &mut out, bound_names, inputs, ghosts);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    out.pop()
        .ok_or_else(|| invalid(Span { start: 0, end: 0 }, "empty prop norm stack"))
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; passes source, target, expression, and Lean binder scopes explicitly"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; verifies normalized proposition equivalence before casting"
)]
fn emit_cast_prop(
    src: &Prop3,
    dst: &Prop3,
    expr: &str,
    bound_names: &[String],
    inputs: &[String],
    ghosts: &[(u32, u32, String)],
) -> Result<String, Diagnostic> {
    if src == dst {
        return Ok(String::from(expr));
    }
    if let Prop3::True = dst {
        return Ok(String::from("True.intro"));
    }
    let (iff_src, norm_src, is_src_refl) = emit_norm_prop_iff(src, bound_names, inputs, ghosts)?;
    let (iff_dst, norm_dst, is_dst_refl) = emit_norm_prop_iff(dst, bound_names, inputs, ghosts)?;
    if norm_src != norm_dst {
        return Err(invalid(
            Span { start: 0, end: 0 },
            "prop cast norm mismatch",
        ));
    }
    if is_src_refl && is_dst_refl {
        return Ok(String::from(expr));
    }
    let to_norm = if is_src_refl {
        String::from(expr)
    } else {
        fmt_wrap2("(", &iff_src, ").mp (", expr, ")")
    };
    if is_dst_refl {
        Ok(to_norm)
    } else {
        Ok(fmt_wrap2("(", &iff_dst, ").mpr (", &to_norm, ")"))
    }
}

struct MidJoinSpec<'a> {
    in_vars: &'a [String],
    ghost_vars: &'a [(u32, u32, String)],
    fact_vars: &'a [String],
    n_in: usize,
    hres_subst_name: &'a str,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; constructs nested existential witness for intermediate chunk boundary"
)]
#[expect(
    clippy::single_char_add_str,
    reason = "Owner: noble-contracts; Aeneas rejects non-ASCII char literals"
)]
fn emit_mid_join_body(mut out: String, spec: MidJoinSpec<'_>) -> String {
    let MidJoinSpec {
        in_vars,
        ghost_vars,
        fact_vars,
        n_in,
        hres_subst_name,
    } = spec;
    out.push_str("    ");
    let mut ii = 0usize;
    while ii < n_in {
        out.push_str("⟨");
        out.push_str(&in_vars[ii]);
        out.push_str(", ");
        ii = ii.saturating_add(1);
    }
    let mut gi = 0usize;
    while gi < ghost_vars.len() {
        out.push_str("⟨");
        out.push_str(&ghost_vars[gi].2);
        out.push_str(", ");
        gi = gi.saturating_add(1);
    }
    out.push_str("⟨hinput_eq, ");
    if fact_vars.is_empty() {
        out.push_str("True.intro");
    } else {
        let mut fi = 0usize;
        while fi.saturating_add(1) < fact_vars.len() {
            out.push_str("⟨");
            out.push_str(&fact_vars[fi]);
            out.push_str(", ");
            fi = fi.saturating_add(1);
        }
        out.push_str(&fact_vars[fact_vars.len().saturating_sub(1)]);
        let mut c = 0usize;
        while c.saturating_add(1) < fact_vars.len() {
            out.push_str("⟩");
            c = c.saturating_add(1);
        }
    }
    out.push_str(", ");
    out.push_str(hres_subst_name);
    out.push_str("⟩");
    let mut c = 0usize;
    let total_binders = n_in.saturating_add(ghost_vars.len());
    while c < total_binders {
        out.push_str("⟩");
        c = c.saturating_add(1);
    }
    out.push('\n');
    out
}

struct FinalJoinBodySpec<'a> {
    trace: &'a super::LowerTrace,
    after_stack: &'a [Term3],
    after_sorts: &'a [Sort3],
    after_facts: &'a [Prop3],
    in_vars: &'a [String],
    ghost_vars: &'a [(u32, u32, String)],
    fact_vars: &'a [String],
    in_sorts: &'a [Sort3],
    hres_subst_name: &'a str,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; translates user join proof and closes contract postcondition"
)]
#[expect(
    tigerstyle::allocation_in_loop,
    reason = "Owner: noble-contracts; bounded by final stack length"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; lowers join proof expression, output typing, and postcondition witness"
)]
fn emit_final_join_body(
    mut out: String,
    spec: FinalJoinBodySpec<'_>,
) -> Result<String, Diagnostic> {
    let FinalJoinBodySpec {
        trace,
        after_stack,
        after_sorts,
        after_facts,
        in_vars,
        ghost_vars,
        fact_vars,
        in_sorts,
        hres_subst_name,
    } = spec;
    let (join_proof_raw, join_stmt) = emit_prop_proof(
        &trace.join_form,
        ProofScope {
            facts: after_facts,
            lemmas: &trace.lemmas,
            scope_inputs: &trace.view.inputs,
            scope_outputs: &trace.view.outputs,
            in_vars,
            ghost_vars,
            fact_vars,
        },
    )?;
    let Statement3::Prop(join_prop) = join_stmt else {
        return Err(invalid(Span { start: 0, end: 0 }, "join proof not prop"));
    };
    let expected_join = super::instantiate_prop(
        &trace.ensures,
        &super::collect_input_terms(in_sorts.len()),
        after_stack,
    );
    let join_proof_expr = emit_cast_prop(
        &join_prop,
        &expected_join,
        &join_proof_raw,
        &[],
        in_vars,
        ghost_vars,
    )?;
    let mut out_vals_only: Vec<String> = Vec::with_capacity(after_stack.len());
    let mut outcome: Result<(), Diagnostic> = Ok(());
    let mut oi = 0usize;
    while oi < after_stack.len() {
        match emit_term3_lean(String::new(), &after_stack[oi], &[], in_vars, ghost_vars) {
            Ok(rendered) => out_vals_only.push(rendered),
            Err(e) => outcome = Err(e),
        }
        if outcome.is_err() {
            break;
        }
        oi += 1;
    }
    outcome?;
    let mut in_list = String::from("[");
    let mut ii = 0usize;
    while ii < in_vars.len() {
        if ii != 0 {
            in_list.push_str(", ");
        }
        in_list.push_str(match in_sorts[ii] {
            Sort3::I64 => ".i64 ",
            Sort3::Bool => ".bool ",
        });
        in_list.push_str(&in_vars[ii]);
        ii += 1;
    }
    in_list.push(']');
    let mut out_list = String::from("[");
    let mut outcome_val: Result<(), Diagnostic> = Ok(());
    let mut oi = 0usize;
    while oi < after_stack.len() {
        if oi != 0 {
            out_list.push_str(", ");
        }
        match emit_term3_value_lean(
            core::mem::take(&mut out_list),
            &after_stack[oi],
            after_sorts[oi],
            in_vars,
            ghost_vars,
        ) {
            Ok(next) => out_list = next,
            Err(e) => outcome_val = Err(e),
        }
        if outcome_val.is_err() {
            break;
        }
        oi += 1;
    }
    outcome_val?;
    out_list.push(']');
    let ctx_join = fmt_wrap2("⟨", &in_list, ", ", &out_list, ", []⟩");
    let iff_post = emit_eval_prop3_iff(
        &trace.ensures,
        &ctx_join,
        in_sorts,
        after_sorts,
        in_vars,
        &out_vals_only,
    )?;
    out.push_str("    And.intro (Eq.ndrec (motive := fun outp => NobleContracts.StackTyped outp NamedV3Obligation.subject.output) ");
    out = emit_stack_typed(out, after_sorts);
    out.push(' ');
    out.push_str(hres_subst_name);
    out.push_str(".symm) (Eq.ndrec (motive := fun inp => NobleContracts.Holds NamedV3Obligation.postcondition inp (carried ++ res) []) (Eq.ndrec (motive := fun outp => NobleContracts.Holds NamedV3Obligation.postcondition ");
    out.push_str(&in_list);
    out.push_str(" outp []) ((");
    out.push_str(&iff_post);
    out.push_str(").mpr (");
    out.push_str(&join_proof_expr);
    out.push_str(")) ");
    out.push_str(hres_subst_name);
    out.push_str(".symm) hinput_eq.symm)\n");
    Ok(out)
}

struct UnpackPreStateSpec<'a> {
    trace: &'a super::LowerTrace,
    is_first: bool,
    in_vars: &'a [String],
    ghost_vars: &'a [(u32, u32, String)],
    in_sorts: &'a [Sort3],
    n_ghosts: usize,
    n_facts: usize,
    hyp_name: &'a str,
    state_expr: &'a str,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; unpacks pre-chunk existential state variables and hypotheses"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; unpacks first and intermediate state invariants uniformly"
)]
fn emit_unpack_pre_state(
    mut out: String,
    spec: UnpackPreStateSpec<'_>,
) -> Result<String, Diagnostic> {
    let UnpackPreStateSpec {
        trace,
        is_first,
        in_vars,
        ghost_vars,
        in_sorts,
        n_ghosts,
        n_facts,
        hyp_name,
        state_expr,
    } = spec;
    if is_first {
        let mut h_eq_in = String::from(hyp_name);
        h_eq_in.push_str(".1");
        let mut h_typed_in = String::from(hyp_name);
        h_typed_in.push_str(".2.1");
        let mut h_pre_in = String::from(hyp_name);
        h_pre_in.push_str(".2.2");
        let mut eq_out = String::from(hyp_name);
        eq_out.push_str("_eq");
        out = emit_stack_unpack(
            out,
            StackUnpackSpec {
                hyp_name: &h_typed_in,
                stack_expr: "input",
                sorts: in_sorts,
                val_prefix: "",
                val_names: in_vars,
                step_prefix: "fp",
                eq_name: "f_input_eq",
            },
        );
        let n_in = in_sorts.len();
        let mut in_list = String::from("[");
        let mut ii = 0usize;
        while ii < n_in {
            if ii != 0 {
                in_list.push_str(", ");
            }
            in_list.push_str(match in_sorts[ii] {
                Sort3::I64 => "NobleContracts.Value.i64 ",
                Sort3::Bool => "NobleContracts.Value.bool ",
            });
            in_list.push_str(&in_vars[ii]);
            ii += 1;
        }
        in_list.push(']');
        out.push_str("    let ");
        out.push_str(&eq_out);
        out.push_str(" : ");
        out.push_str(state_expr);
        out.push_str(" = ");
        out.push_str(&in_list);
        out.push_str(" := Eq.trans ");
        out.push_str(&h_eq_in);
        out.push_str(" f_input_eq\n    let hinput_eq := f_input_eq\n    let ");
        out.push_str(hyp_name);
        out.push_str("_pre : NobleContracts.Holds NamedV3Obligation.precondition ");
        out.push_str(&in_list);
        out.push_str(" [] [] := Eq.ndrec (motive := fun st => NobleContracts.Holds NamedV3Obligation.precondition st [] []) ");
        out.push_str(&h_pre_in);
        out.push_str(" f_input_eq\n");
        let mut ctx0 = String::from("⟨");
        ctx0.push_str(&in_list);
        ctx0.push_str(", [], []⟩");
        let iff0 = emit_eval_prop3_iff(&trace.requires, &ctx0, in_sorts, &[], in_vars, &[])?;
        out.push_str("    let hfacts_all := (");
        out.push_str(&iff0);
        out.push_str(").mp ");
        out.push_str(hyp_name);
        out.push_str("_pre\n");
        out = unpack_prop_conjuncts(out, "hfacts_all", "rfact_", &trace.requires);
        let mut rconj: Vec<Prop3> = Vec::new();
        super::collect_conjuncts_owned(&trace.requires, &mut rconj);
        let mut fi = 0usize;
        let mut cast_outcome = Ok(());
        while fi < rconj.len() {
            let rfact_name = fmt_str_num("rfact_", fi as u64);
            let Some(dst) = trace.initial_facts.get(fi) else {
                cast_outcome = Err(invalid(
                    Span { start: 0, end: 0 },
                    "initial fact count mismatch",
                ));
                break;
            };
            cast_outcome = match emit_cast_prop(&rconj[fi], dst, &rfact_name, &[], in_vars, &[]) {
                Ok(cast_expr) => {
                    out.push_str("    let fact_");
                    crate::rendering::number(&mut out, fi as u64);
                    out.push_str(" := ");
                    out.push_str(&cast_expr);
                    out.push('\n');
                    Ok(())
                }
                Err(problem) => Err(problem),
            };
            if cast_outcome.is_err() {
                break;
            }
            fi += 1;
        }
        cast_outcome?;
        return Ok(out);
    }
    let _ = n_ghosts;
    let mut ii = 0usize;
    while ii < in_sorts.len() {
        out.push_str("    Exists.elim ");
        out.push_str(hyp_name);
        out.push_str(" <| fun ");
        out.push_str(&in_vars[ii]);
        out.push(' ');
        out.push_str(hyp_name);
        out.push_str(" =>\n");
        ii += 1;
    }
    let mut gi = 0usize;
    while gi < ghost_vars.len() {
        out.push_str("    Exists.elim ");
        out.push_str(hyp_name);
        out.push_str(" <| fun ");
        out.push_str(&ghost_vars[gi].2);
        out.push(' ');
        out.push_str(hyp_name);
        out.push_str(" =>\n");
        gi += 1;
    }
    out.push_str("    let hinput_eq := ");
    out.push_str(hyp_name);
    out.push_str(".1\n    let hfacts_all := ");
    out.push_str(hyp_name);
    out.push_str(".2.1\n    let hs_eq := ");
    out.push_str(hyp_name);
    out.push_str(".2.2\n");
    out = unpack_conjuncts_bind(out, "hfacts_all", "fact_", n_facts);
    Ok(out)
}

struct TraceChunkAccum {
    chunks: Vec<Chunk>,
    cur_ops: Vec<SegOp>,
    seg_start_stack: Vec<Term3>,
    seg_start_sorts: Vec<Sort3>,
    cur_stack: Vec<Term3>,
    cur_facts: Vec<Prop3>,
    ghosts_so_far: Vec<(u32, u32, Sort3)>,
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends new call output ghosts onto caller vector"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; records uneliminated call output ghosts into accumulated vector"
)]
fn record_call_ghosts(
    call: &super::CallTrace,
    c_idx: usize,
    ghosts_so_far: &mut Vec<(u32, u32, Sort3)>,
) -> Vec<(u32, u32, Sort3)> {
    let mut new_ghosts: Vec<(u32, u32, Sort3)> = Vec::with_capacity(call.direct_outputs.len());
    let step_ghost_id = u32::try_from(c_idx).unwrap_or(0);
    let mut di = 0usize;
    while di < call.direct_outputs.len() {
        if call.direct_outputs[di].is_none() {
            let out_id = u32::try_from(di).unwrap_or(0);
            let sort = call.callee_outputs[di].1;
            new_ghosts.push((step_ghost_id, out_id, sort));
            ghosts_so_far.push((step_ghost_id, out_id, sort));
        }
        di += 1;
    }
    new_ghosts
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; lowers one trace step into segment/call chunks without early loop returns"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; appends lowered chunks and ghost outputs to `acc`"
)]
fn step_trace_chunk(
    step: &super::TraceStep,
    goal: &crate::intrinsic::ContractGoal,
    trace: &super::LowerTrace,
    in_sorts: &[Sort3],
    acc: &mut TraceChunkAccum,
) -> Result<(), Diagnostic> {
    match step {
        super::TraceStep::Literal(Lit::I64(n)) => {
            acc.cur_ops.push(SegOp::LitI64(*n));
            acc.cur_stack.push(Term3::I64(*n));
        }
        super::TraceStep::Literal(Lit::Bool(b)) => {
            acc.cur_ops.push(SegOp::LitBool(*b));
            acc.cur_stack.push(Term3::Bool(*b));
        }
        super::TraceStep::Builtin(w) => {
            acc.cur_ops.push(SegOp::Word(*w));
            super::builtin_step(&mut acc.cur_stack, *w, goal.contract_span)?;
        }
        super::TraceStep::Call(c_idx) => {
            let call = &trace.calls[*c_idx];
            if !acc.cur_ops.is_empty() {
                let seg = build_segment_spec(SegmentSpecInput {
                    ops: core::mem::take(&mut acc.cur_ops),
                    before_stack: &acc.seg_start_stack,
                    before_sorts: &acc.seg_start_sorts,
                    after_stack: call.pre_stack.clone(),
                    facts: call.pre_facts.clone(),
                    in_sorts,
                    ghost_sorts: &acc.ghosts_so_far,
                })?;
                acc.chunks.push(Chunk::Segment(seg));
            }
            let new_ghosts = record_call_ghosts(call, *c_idx, &mut acc.ghosts_so_far);
            let occ = &goal.subject.named_uses[call.occ_idx];
            let Some(occ_ui) = find_use_idx_by_occ(&goal.subject.named_uses, occ) else {
                return Err(invalid(goal.contract_span, "call use index missing"));
            };
            let after_sorts = sorts_of_stack(&call.post_stack, in_sorts, &acc.ghosts_so_far)?;
            let carried_len = call
                .pre_stack
                .len()
                .saturating_sub(call.callee_inputs.len());
            acc.chunks.push(Chunk::Call {
                call_idx: *c_idx,
                occ_ui,
                carried_len,
                after_stack: call.post_stack.clone(),
                after_sorts: after_sorts.clone(),
                facts: call.post_facts.clone(),
                new_ghosts,
            });
            acc.seg_start_stack = call.post_stack.clone();
            acc.seg_start_sorts = after_sorts;
            acc.cur_stack = call.post_stack.clone();
            acc.cur_facts = call.post_facts.clone();
        }
        super::TraceStep::Literal(Lit::Text) | super::TraceStep::Literal(Lit::Unit) => {
            return Err(crate::intrinsic::unsupported(
                goal.contract_span,
                "unsupported literal in Lean lowering",
            ));
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; emits fixed simp lemma list for root chain equality"
)]
fn emit_root_chain_eq(chunks: &[Chunk]) -> String {
    let mut has_call = false;
    let mut has_seg = false;
    let mut i = 0usize;
    while i < chunks.len() {
        match &chunks[i] {
            Chunk::Call { .. } => has_call = true,
            Chunk::Segment(_) => has_seg = true,
        }
        i += 1;
    }
    let mut out = String::from(
        "(by simp only [NamedV3Obligation.subject, NamedV3Obligation.root, NobleContracts.NamedV3.LiftedChain.code.eq_1",
    );
    if has_call {
        out.push_str(", NobleContracts.NamedV3.LiftedChain.code.eq_2");
    }
    if has_seg {
        out.push_str(
            ", NobleContracts.NamedV3.LiftedChain.code.eq_3, NobleContracts.NamedV1.liftCode.eq_1, List.map, NobleContracts.NamedV1.liftOp, NobleContracts.NamedV1.liftValue, List.cons_append, List.nil_append",
        );
    }
    out.push_str("])");
    out
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; charges proof emission size against `meter`"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; validates trace invariants during code emission"
)]
pub(super) fn proof(
    goal: &crate::intrinsic::ContractGoal,
    trace: &super::LowerTrace,
    meter: &mut crate::intrinsic::Meter<'_>,
) -> Result<String, Diagnostic> {
    let limits = meter.limits;
    let in_sorts = &trace.view.input_codes;
    let mut out = String::new();
    if !trace.lemmas.is_empty() || !trace.calls.is_empty() {
        out.push_str("(fun ");
        out = emit_lemma_params(out, &trace.lemmas)?;
        out = emit_callee_params(out, goal, trace, limits)?;
        out.push_str("=>\n");
    } else {
        out.push_str("(\n");
    }
    out.push_str(HELPERS_PRELUDE);
    let seg_start_stack = super::collect_input_terms(in_sorts.len());
    let seg_start_sorts: Vec<Sort3> = in_sorts.clone();
    let cur_stack = seg_start_stack.clone();
    let cur_facts = trace.initial_facts.clone();
    let mut acc = TraceChunkAccum {
        chunks: Vec::new(),
        cur_ops: Vec::new(),
        seg_start_stack,
        seg_start_sorts,
        cur_stack,
        cur_facts,
        ghosts_so_far: Vec::new(),
    };
    step_trace_chunks(goal, trace, in_sorts, &mut acc)?;
    if !acc.cur_ops.is_empty() {
        let seg = build_segment_spec(SegmentSpecInput {
            ops: acc.cur_ops,
            before_stack: &acc.seg_start_stack,
            before_sorts: &acc.seg_start_sorts,
            after_stack: trace.final_stack.clone(),
            facts: acc.cur_facts,
            in_sorts,
            ghost_sorts: &acc.ghosts_so_far,
        })?;
        acc.chunks.push(Chunk::Segment(seg));
    }
    let chunks = acc.chunks;
    let _ = &chunks;
    out.push_str(
        "(fun tail before final hbefore hpre execution => Exists.elim (NobleContracts.NamedV3.liftedChainClaim NamedV3Obligation.subject ",
    );
    out.push_str("(fun before => NobleContracts.StackTyped before NamedV3Obligation.subject.input ∧ NobleContracts.Holds NamedV3Obligation.precondition before [] []) ");
    out.push_str("(fun before after => NobleContracts.StackTyped after NamedV3Obligation.subject.output ∧ NobleContracts.Holds NamedV3Obligation.postcondition before after []) ");
    out.push_str("(fun input =>\n");
    out = emit_chunk_chain(
        out,
        EmitChunkInput {
            trace,
            chunks: &chunks,
            in_sorts,
            limits,
        },
    )?;
    let root_eq = emit_root_chain_eq(&chunks);
    out.push_str(") (fun _ => ");
    out.push_str(&root_eq);
    out.push_str(") (fun _ _ _ h => h.1) tail before final hbefore ⟨hbefore, hpre⟩ execution) (fun after hres_claim => Exists.intro after (And.intro hres_claim.1 (And.intro hres_claim.2.1 hres_claim.2.2.2))))");
    out.push(')');
    let (next, outcome) = meter.charge(out.len(), goal.contract_span);
    *meter = next;
    outcome?;
    Ok(out)
}

const HELPERS_PRELUDE: &str = r#"let eval_add {a b : NobleContracts.Term} {ctx : NobleContracts.Context} {va vb : BitVec 64}
    (ha : NobleContracts.evaluate a ctx = some (NobleContracts.Value.i64 va))
    (hb : NobleContracts.evaluate b ctx = some (NobleContracts.Value.i64 vb)) :
    NobleContracts.evaluate (.add a b) ctx = some (NobleContracts.Value.i64 (va + vb)) := by
  dsimp [NobleContracts.evaluate]; rw [ha, hb]
let eval_sub {a b : NobleContracts.Term} {ctx : NobleContracts.Context} {va vb : BitVec 64}
    (ha : NobleContracts.evaluate a ctx = some (NobleContracts.Value.i64 va))
    (hb : NobleContracts.evaluate b ctx = some (NobleContracts.Value.i64 vb)) :
    NobleContracts.evaluate (.sub a b) ctx = some (NobleContracts.Value.i64 (va - vb)) := by
  dsimp [NobleContracts.evaluate]; rw [ha, hb]
let eval_mul {a b : NobleContracts.Term} {ctx : NobleContracts.Context} {va vb : BitVec 64}
    (ha : NobleContracts.evaluate a ctx = some (NobleContracts.Value.i64 va))
    (hb : NobleContracts.evaluate b ctx = some (NobleContracts.Value.i64 vb)) :
    NobleContracts.evaluate (.mul a b) ctx = some (NobleContracts.Value.i64 (va * vb)) := by
  dsimp [NobleContracts.evaluate]; rw [ha, hb]
let eval_eq_i64 {a b : NobleContracts.Term} {ctx : NobleContracts.Context} {va vb : BitVec 64}
    (ha : NobleContracts.evaluate a ctx = some (NobleContracts.Value.i64 va))
    (hb : NobleContracts.evaluate b ctx = some (NobleContracts.Value.i64 vb)) :
    NobleContracts.evaluate (.eq a b) ctx =
      some (NobleContracts.Value.bool (@decide (NobleContracts.Value.i64 va = NobleContracts.Value.i64 vb) (Classical.propDecidable _))) := by
  dsimp [NobleContracts.evaluate]; rw [ha, hb]
let eval_eq_bool {a b : NobleContracts.Term} {ctx : NobleContracts.Context} {ba bb : Bool}
    (ha : NobleContracts.evaluate a ctx = some (NobleContracts.Value.bool ba))
    (hb : NobleContracts.evaluate b ctx = some (NobleContracts.Value.bool bb)) :
    NobleContracts.evaluate (.eq a b) ctx =
      some (NobleContracts.Value.bool (@decide (NobleContracts.Value.bool ba = NobleContracts.Value.bool bb) (Classical.propDecidable _))) := by
  dsimp [NobleContracts.evaluate]; rw [ha, hb]
let iff_true_tr : (true = true ↔ True) := Iff.intro (fun _ => True.intro) (fun _ => rfl)
let iff_false_tr : (false = true ↔ False) := Iff.intro (fun h => Bool.false_ne_true h) False.elim
let iff_eq_i64 {a b : NobleContracts.Term} {ctx : NobleContracts.Context} {va vb : BitVec 64}
    (ha : NobleContracts.evaluate a ctx = some (NobleContracts.Value.i64 va))
    (hb : NobleContracts.evaluate b ctx = some (NobleContracts.Value.i64 vb)) :
    NobleContracts.evaluate (.eq a b) ctx =
      some (NobleContracts.Value.bool (@decide (NobleContracts.Value.i64 va = NobleContracts.Value.i64 vb) (Classical.propDecidable _))) ∧
      (@decide (NobleContracts.Value.i64 va = NobleContracts.Value.i64 vb) (Classical.propDecidable _) = true ↔ va = vb) :=
  ⟨eval_eq_i64 ha hb, Iff.trans (@decide_eq_true_iff _ (Classical.propDecidable _)) (Iff.intro NobleContracts.Value.i64.inj (congrArg NobleContracts.Value.i64))⟩
let iff_eq_bool {a b : NobleContracts.Term} {ctx : NobleContracts.Context} {ba bb : Bool}
    (ha : NobleContracts.evaluate a ctx = some (NobleContracts.Value.bool ba))
    (hb : NobleContracts.evaluate b ctx = some (NobleContracts.Value.bool bb)) :
    NobleContracts.evaluate (.eq a b) ctx =
      some (NobleContracts.Value.bool (@decide (NobleContracts.Value.bool ba = NobleContracts.Value.bool bb) (Classical.propDecidable _))) ∧
      (@decide (NobleContracts.Value.bool ba = NobleContracts.Value.bool bb) (Classical.propDecidable _) = true ↔ ba = bb) :=
  ⟨eval_eq_bool ha hb, Iff.trans (@decide_eq_true_iff _ (Classical.propDecidable _)) (Iff.intro NobleContracts.Value.bool.inj (congrArg NobleContracts.Value.bool))⟩
let iff_lt {a b : NobleContracts.Term} {ctx : NobleContracts.Context} {va vb : BitVec 64}
    (ha : NobleContracts.evaluate a ctx = some (NobleContracts.Value.i64 va))
    (hb : NobleContracts.evaluate b ctx = some (NobleContracts.Value.i64 vb)) :
    NobleContracts.evaluate (.lt a b) ctx = some (NobleContracts.Value.bool (decide (va.toInt < vb.toInt))) ∧
      (decide (va.toInt < vb.toInt) = true ↔ va.toInt < vb.toInt) :=
  ⟨(by dsimp [NobleContracts.evaluate]; rw [ha, hb]), decide_eq_true_iff⟩
let iff_le {a b : NobleContracts.Term} {ctx : NobleContracts.Context} {va vb : BitVec 64}
    (ha : NobleContracts.evaluate a ctx = some (NobleContracts.Value.i64 va))
    (hb : NobleContracts.evaluate b ctx = some (NobleContracts.Value.i64 vb)) :
    NobleContracts.evaluate (.le a b) ctx = some (NobleContracts.Value.bool (decide (va.toInt ≤ vb.toInt))) ∧
      (decide (va.toInt ≤ vb.toInt) = true ↔ va.toInt ≤ vb.toInt) :=
  ⟨(by dsimp [NobleContracts.evaluate]; rw [ha, hb]), decide_eq_true_iff⟩
let iff_not {a : NobleContracts.Term} {ctx : NobleContracts.Context} {ba : Bool} {P : Prop}
    (h : NobleContracts.evaluate a ctx = some (NobleContracts.Value.bool ba) ∧ (ba = true ↔ P)) :
    NobleContracts.evaluate (.not a) ctx = some (NobleContracts.Value.bool (!ba)) ∧ ((!ba) = true ↔ ¬ P) :=
  ⟨(by dsimp [NobleContracts.evaluate]; rw [h.1]),
   Iff.trans (Bool.casesOn (motive := fun (b : Bool) => ((!b) = true ↔ ¬ (b = true))) ba
     (Iff.intro (fun _ hn => Bool.false_ne_true hn) (fun _ => rfl))
     (Iff.intro (fun hn => False.elim (Bool.false_ne_true hn)) (fun hn => False.elim (hn rfl)))) (not_congr h.2)⟩
let iff_and {a b : NobleContracts.Term} {ctx : NobleContracts.Context} {ba bb : Bool} {P Q : Prop}
    (hl : NobleContracts.evaluate a ctx = some (NobleContracts.Value.bool ba) ∧ (ba = true ↔ P))
    (hr : NobleContracts.evaluate b ctx = some (NobleContracts.Value.bool bb) ∧ (bb = true ↔ Q)) :
    NobleContracts.evaluate (.and a b) ctx = some (NobleContracts.Value.bool (ba && bb)) ∧ ((ba && bb) = true ↔ P ∧ Q) :=
  ⟨(by dsimp [NobleContracts.evaluate]; rw [hl.1, hr.1]),
   Iff.trans (Eq.to_iff (Bool.and_eq_true ba bb)) (and_congr hl.2 hr.2)⟩
let holds_of_eval_iff {t : NobleContracts.Term} {before after : NobleContracts.Stack} {b : Bool} {P : Prop}
    (h : NobleContracts.evaluate t ⟨before, after, []⟩ = some (NobleContracts.Value.bool b) ∧ (b = true ↔ P)) :
    NobleContracts.Holds t before after [] ↔ P :=
  Iff.trans
    (Iff.trans
      (Eq.ndrec (motive := fun (x : Option NobleContracts.Value) => x = some (NobleContracts.Value.bool true) ↔ some (NobleContracts.Value.bool b) = some (NobleContracts.Value.bool true)) Iff.rfl h.1.symm)
      (Iff.intro (fun hs => NobleContracts.Value.bool.inj (Option.some.inj hs)) (congrArg (fun (x : Bool) => (some (NobleContracts.Value.bool x) : Option NobleContracts.Value)))))
    h.2
let unpack_i64 {P : Prop} (s : NobleContracts.Stack) (ts : List NobleContracts.Ty) (h : NobleContracts.StackTyped s (.i64 :: ts))
    (k : ∀ (v : BitVec 64) (tl : NobleContracts.Stack), s = NobleContracts.Value.i64 v :: tl → NobleContracts.StackTyped tl ts → P) : P :=
  Exists.elim ((NobleContracts.stackTyped_cons_iff s .i64 ts).mp h) fun v h0 =>
  Exists.elim h0 fun tl h1 =>
  Exists.elim ((NobleContracts.hasType_i64_iff v).mp h1.2.1) fun n hn =>
  k n tl (Eq.trans h1.1 (congrArg (fun x => x :: tl) hn)) h1.2.2
let unpack_bool {P : Prop} (s : NobleContracts.Stack) (ts : List NobleContracts.Ty) (h : NobleContracts.StackTyped s (.bool :: ts))
    (k : ∀ (b : Bool) (tl : NobleContracts.Stack), s = NobleContracts.Value.bool b :: tl → NobleContracts.StackTyped tl ts → P) : P :=
  Exists.elim ((NobleContracts.stackTyped_cons_iff s .bool ts).mp h) fun v h0 =>
  Exists.elim h0 fun tl h1 =>
  Exists.elim ((NobleContracts.hasType_bool_iff v).mp h1.2.1) fun b hb =>
  k b tl (Eq.trans h1.1 (congrArg (fun x => x :: tl) hb)) h1.2.2
let unpack_nil {P : Prop} (s : NobleContracts.Stack) (h : NobleContracts.StackTyped s []) (k : s = [] → P) : P :=
  k ((NobleContracts.stackTyped_nil_iff s).mp h)
let append_split_step {α : Type} {P : Prop} (carried tail rhs : List α) (e : α)
    (h : carried ++ tail = e :: rhs) (h_lt : tail.length < (e :: rhs).length)
    (k : ∀ (hd : α) (tl : List α), carried = hd :: tl → hd = e → tl ++ tail = rhs → P) : P :=
  List.casesOn (motive := fun l => carried = l → P) carried
    (fun hnil =>
      False.elim (Nat.lt_irrefl tail.length
        (Eq.ndrec (motive := fun n => tail.length < n) h_lt
          (congrArg List.length (Eq.trans (Eq.ndrec (motive := fun l => l ++ tail = e :: rhs) h hnil).symm rfl)))))
    (fun hd tl hcons =>
      let hstep : hd :: (tl ++ tail) = e :: rhs := Eq.ndrec (motive := fun l => l ++ tail = e :: rhs) h hcons
      k hd tl hcons (List.cons.inj hstep).1 (List.cons.inj hstep).2)
    rfl
let append_split_nil {α : Type} {P : Prop} (carried tail rhs : List α)
    (h : carried ++ tail = rhs) (h_eq : tail.length = rhs.length)
    (k : carried = [] → tail = rhs → P) : P :=
  List.casesOn (motive := fun l => carried = l → P) carried
    (fun hnil => k hnil (Eq.ndrec (motive := fun l => l ++ tail = rhs) h hnil))
    (fun hd tl hcons =>
      let hlen : (hd :: tl).length + tail.length = rhs.length :=
        Eq.trans (@List.length_append _ (hd :: tl) tail).symm (congrArg List.length (Eq.ndrec (motive := fun l => l ++ tail = rhs) h hcons))
      False.elim (Nat.lt_irrefl rhs.length
        (Eq.ndrec (motive := fun n => rhs.length < n)
          (Eq.ndrec (motive := fun t => t < (hd :: tl).length + tail.length)
            (Nat.lt_add_of_pos_left (Nat.zero_lt_succ tl.length)) h_eq)
          hlen)))
    rfl
let iff_not_true : (¬ True) ↔ False := Iff.intro (fun h => h True.intro) False.elim
let iff_not_false : (¬ False) ↔ True := Iff.intro (fun _ => True.intro) (fun _ h => h)
let iff_not_not {P : Prop} : (¬ ¬ P) ↔ P := Iff.intro Classical.byContradiction (fun hp hnp => hnp hp)
let iff_and_false_l {P : Prop} : (False ∧ P) ↔ False := Iff.intro (fun h => h.1) False.elim
let iff_and_false_r {P : Prop} : (P ∧ False) ↔ False := Iff.intro (fun h => h.2) False.elim
let iff_and_true_l {P : Prop} : (True ∧ P) ↔ P := Iff.intro (fun h => h.2) (fun h => ⟨True.intro, h⟩)
let iff_and_true_r {P : Prop} : (P ∧ True) ↔ P := Iff.intro (fun h => h.1) (fun h => ⟨h, True.intro⟩)
let decide_eq_true_of_eq {α : Type} {a b : α} (h : a = b) :
    (@decide (a = b) (Classical.propDecidable _)) = true :=
  @decide_eq_true (a = b) (Classical.propDecidable _) h
let decide_eq_false_of_ne {α : Type} {a b : α} (h : a ≠ b) :
    (@decide (a = b) (Classical.propDecidable _)) = false :=
  @decide_eq_false (a = b) (Classical.propDecidable _) h
let eq_add_congr {a1 a2 b1 b2 : BitVec 64} (ha : a1 = a2) (hb : b1 = b2) : a1 + b1 = a2 + b2 :=
  congr (congrArg (· + ·) ha) hb
let eq_sub_congr {a1 a2 b1 b2 : BitVec 64} (ha : a1 = a2) (hb : b1 = b2) : a1 - b1 = a2 - b2 :=
  congr (congrArg (· - ·) ha) hb
let eq_mul_congr {a1 a2 b1 b2 : BitVec 64} (ha : a1 = a2) (hb : b1 = b2) : a1 * b1 = a2 * b2 :=
  congr (congrArg (· * ·) ha) hb
let eq_equals_congr {α : Type} {a1 a2 b1 b2 : α} (ha : a1 = a2) (hb : b1 = b2) :
    (@decide ((a1) = (b1)) (Classical.propDecidable _)) = (@decide ((a2) = (b2)) (Classical.propDecidable _)) :=
  congr (congrArg (fun x y => (@decide ((x) = (y)) (Classical.propDecidable _))) ha) hb
let iff_eq_congr {α : Type} {a1 a2 b1 b2 : α} (ha : a1 = a2) (hb : b1 = b2) :
    (a1 = b1) ↔ (a2 = b2) :=
  Iff.intro (fun h => Eq.trans ha.symm (Eq.trans h hb)) (fun h => Eq.trans ha (Eq.trans h hb.symm))
let iff_lt_congr {a1 a2 b1 b2 : BitVec 64} (ha : a1 = a2) (hb : b1 = b2) :
    (a1.toInt < b1.toInt) ↔ (a2.toInt < b2.toInt) :=
  Eq.ndrec (motive := fun x => (a1.toInt < b1.toInt) ↔ (x.toInt < b2.toInt))
    (Eq.ndrec (motive := fun y => (a1.toInt < b1.toInt) ↔ (a1.toInt < y.toInt)) Iff.rfl hb) ha
let iff_le_congr {a1 a2 b1 b2 : BitVec 64} (ha : a1 = a2) (hb : b1 = b2) :
    (a1.toInt ≤ b1.toInt) ↔ (a2.toInt ≤ b2.toInt) :=
  Eq.ndrec (motive := fun x => (a1.toInt ≤ b1.toInt) ↔ (x.toInt ≤ b2.toInt))
    (Eq.ndrec (motive := fun y => (a1.toInt ≤ b1.toInt) ↔ (a1.toInt ≤ y.toInt)) Iff.rfl hb) ha
"#;
