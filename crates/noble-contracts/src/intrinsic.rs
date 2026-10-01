//! Bounded, non-executable source proof terms. No Lean source is read from a module.
use alloc::{boxed::Box, format, string::{String, ToString}, vec::Vec};
use crate::{Diagnostic, DiagnosticKind, Limits, Span};

mod named_v2;
pub use named_v2::prepare_named_contract;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Form { pub kind: FormKind, pub span: Span }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormKind { Atom(String), List(Vec<Form>) }
#[derive(Clone, Debug)]
pub struct ResolvedSourceModule {
    pub module: String,
    pub version: u32,
    pub full_source: Vec<u8>,
    /// The retained namespace owner; never a portable module identity.
    pub owner: u64,
}
#[derive(Clone, Debug)]
pub struct NamedImportBinding {
    /// The import alias as resolved when the original caller was installed.
    pub alias: String,
    pub owner: u64,
    pub module_index: usize,
}
#[derive(Clone, Debug)]
pub struct NamedUseOrigin {
    pub definition: noble_kernel::contracts::Definition,
    pub definition_identity: u64,
    pub definition_owner: u64,
    pub definition_name: String,
    /// Lexical definition order in the original owner module, not a kernel slot.
    pub definition_ordinal: u32,
    pub definition_span: Span,
    pub body_span: Span,
    pub module_index: usize,
    /// None only for the enclosing, single-invocation subject root.
    pub caller_definition: Option<noble_kernel::contracts::Definition>,
    pub source_node: Option<u32>,
    pub source_span: Option<Span>,
    pub candidate_node: noble_kernel::untrusted::NodeId,
}
#[derive(Clone, Debug)]
pub struct SubjectBinding {
    pub module: String,
    pub version: u32,
    pub module_source: Vec<u8>,
    pub definition: String,
    pub definition_source: Vec<u8>,
    pub source_dependencies: Vec<ResolvedSourceModule>,
    /// Alias snapshot at source installation, not the current mutable alias map.
    pub named_imports: Vec<NamedImportBinding>,
    /// Source-resolved, per-use provenance; empty for historical contract 1.
    pub named_uses: Vec<NamedUseOrigin>,
    /// Session-local only; this number is never used as a portable artifact hash.
    pub definition_identity: u64,
    /// Declaration order within the original module (not a specialization slot).
    pub definition_ordinal: u32,
    /// Session-local owner recorded on the resolved definition.
    pub definition_owner: u64,
    pub accepted_submission: noble_kernel::execution::Submission,
    pub input_types: Vec<noble_kernel::types::Ty>,
    pub output_types: Vec<noble_kernel::types::Ty>,
    pub effects: noble_kernel::types::EffSet,
    pub source_span: Span,
}
#[derive(Clone, Debug)]
pub struct ContractGoal {
    pub contract_name: String,
    pub revision: u32,
    pub contract_span: Span,
    pub subject: SubjectBinding,
    pub inputs: Vec<(String, String)>,
    pub outputs: Vec<(String, String)>,
    pub requires: Form,
    pub ensures: Form,
}
#[derive(Clone, Debug)]
pub enum PendingGoal { Pure { proposition: Form }, Contract { contract: Box<ContractGoal> } }
#[derive(Clone, Debug)]
pub struct ProofObligation { pub name: String, pub revision: u32, pub goal: PendingGoal, pub term: Form, pub span: Span }
#[derive(Clone, Debug)]
pub struct ProofDependency {
    pub name: String,
    pub module: String,
    pub version: u32,
    pub source: Vec<u8>,
    pub declaration: ProofObligation,
    pub exported: bool,
    pub dependencies: Vec<ProofDependency>,
    /// Exact host-reviewed model library identity at original acceptance.
    pub model_revision: String,
    /// Exact checker/lowering/consumer identity at original acceptance.
    pub checker_revision: String,
}
#[derive(Clone, Debug)]
pub struct ProofBatch {
    pub module: String,
    pub version: u32,
    pub source: Vec<u8>,
    pub generation: u64,
    pub dependencies: Vec<ProofDependency>,
    pub obligations: Vec<ProofObligation>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofKind { Pure, Contract, NamedContract }
#[derive(Clone, Debug)]
pub struct CheckedProof {
    pub name: String,
    pub kind: ProofKind,
    pub lean_term: String,
    pub claim: String,
    pub model_revision: String,
    pub checker_revision: String,
}

#[derive(Clone, Copy, Debug)]
pub struct ProofBudgets { pub normalization_work: u32, pub substitution_work: u32 }
impl ProofBudgets {
    pub const fn from_limits(limits: Limits) -> Self {
        Self { normalization_work: limits.work, substitution_work: limits.work }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct ProofWork { pub normalization: u32, pub substitution: u32, pub total: u32 }
fn reject(span: Span, reason: &str) -> Diagnostic { Diagnostic::new(DiagnosticKind::Invalid, span, reason) }
fn exhausted(span: Span) -> Diagnostic { Diagnostic::new(DiagnosticKind::Exhausted, span, "proof size, depth or work limit exhausted") }
struct Meter { nodes: u32, work: u32, normalization: u32, substitution: u32, budgets: ProofBudgets, limits: Limits }
impl Meter {
    fn visit(&mut self, depth: u32, span: Span) -> Result<(), Diagnostic> {
        self.nodes = self.nodes.checked_add(1).ok_or_else(|| exhausted(span))?;
        self.work = self.work.checked_add(1).ok_or_else(|| exhausted(span))?;
        if self.nodes > self.limits.nodes || self.work > self.limits.work || depth > self.limits.depth { return Err(exhausted(span)); }
        Ok(())
    }
    fn charge(&mut self, amount: usize, span: Span) -> Result<(), Diagnostic> {
        self.work = self.work.checked_add(u32::try_from(amount).map_err(|_| exhausted(span))?).ok_or_else(|| exhausted(span))?;
        if self.work > self.limits.work { return Err(exhausted(span)); }
        Ok(())
    }
    fn normalize(&mut self, amount: usize, span: Span) -> Result<(), Diagnostic> {
        let amount = u32::try_from(amount).map_err(|_| exhausted(span))?;
        self.normalization = self.normalization.checked_add(amount).ok_or_else(|| exhausted(span))?;
        if self.normalization > self.budgets.normalization_work {
            return Err(Diagnostic::new(DiagnosticKind::Exhausted,span,"normalization work exhausted"));
        }
        self.charge(amount as usize,span)
    }
    fn substitute(&mut self, amount: usize, span: Span) -> Result<(), Diagnostic> {
        let amount = u32::try_from(amount).map_err(|_| exhausted(span))?;
        self.substitution = self.substitution.checked_add(amount).ok_or_else(|| exhausted(span))?;
        if self.substitution > self.budgets.substitution_work {
            return Err(Diagnostic::new(DiagnosticKind::Exhausted,span,"substitution work exhausted"));
        }
        self.charge(amount as usize,span)
    }
}
fn atom(form: &Form) -> Result<&str, Diagnostic> {
    match &form.kind { FormKind::Atom(value) => Ok(value), _ => Err(reject(form.span, "expected logical atom")) }
}
fn list(form: &Form) -> Result<&[Form], Diagnostic> {
    match &form.kind { FormKind::List(items) => Ok(items), _ => Err(reject(form.span, "expected logical form")) }
}
fn operation<'a>(form: &'a Form, head: &str, count: usize) -> Result<&'a [Form], Diagnostic> {
    let items = list(form)?;
    if items.len() != count + 1 || items.first().and_then(|a| atom(a).ok()) != Some(head) { return Err(reject(form.span, "unknown or ill-formed proof constructor")); }
    Ok(&items[1..])
}
fn validated(form: &Form, meter: &mut Meter, depth: u32) -> Result<(), Diagnostic> {
    meter.visit(depth, form.span)?;
    match &form.kind {
        FormKind::Atom(text) => { meter.charge(text.len(), form.span)?; if text.is_empty() || !text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'@') { return Err(reject(form.span, "invalid logical identifier")); } }
        FormKind::List(items) => { for item in items { validated(item, meter, depth + 1)?; } }
    }
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Code { Unit, Bool, I64, Text, Variable(usize), Pair(Box<Code>, Box<Code>), Sum(Box<Code>, Box<Code>), List(Box<Code>) }
#[derive(Clone, Debug, PartialEq, Eq)]
enum Sort { Codes, Value(Code), Premise(Box<Proposition>) }
#[derive(Clone, Debug, PartialEq, Eq)]
enum Proposition { Pi(usize, Sort, Box<Proposition>), Eq(Code, usize, usize) }
fn code_size(code: &Code) -> usize {
    match code {
        Code::Pair(a,b) | Code::Sum(a,b) => 1+code_size(a)+code_size(b),
        Code::List(item) => 1+code_size(item),
        _ => 1,
    }
}
fn proposition_size(prop: &Proposition) -> usize {
    match prop {
        Proposition::Pi(_,sort,body) => 1+match sort {
            Sort::Codes => 1,
            Sort::Value(code) => code_size(code),
            Sort::Premise(prop) => proposition_size(prop),
        }+proposition_size(body),
        Proposition::Eq(code,_,_) => 3+code_size(code),
    }
}
#[derive(Clone)]
struct Binder { name: String, sort: Sort }
#[derive(Clone)]
struct Available { name: String, goal: Proposition, lean_term: String }
fn shifted_code(code: &Code, base: usize) -> Code {
    match code {
        Code::Variable(i) => Code::Variable(i + base),
        Code::Pair(a,b) => Code::Pair(Box::new(shifted_code(a,base)),Box::new(shifted_code(b,base))),
        Code::Sum(a,b) => Code::Sum(Box::new(shifted_code(a,base)),Box::new(shifted_code(b,base))),
        Code::List(a) => Code::List(Box::new(shifted_code(a,base))),
        other => other.clone(),
    }
}
fn shifted_prop(prop: &Proposition, base: usize) -> Proposition {
    match prop {
        Proposition::Pi(i,sort,body) => Proposition::Pi(i+base,shifted_sort(sort,base),Box::new(shifted_prop(body,base))),
        Proposition::Eq(ty,a,b) => Proposition::Eq(shifted_code(ty,base),a+base,b+base),
    }
}
fn shifted_sort(sort: &Sort, base: usize) -> Sort {
    match sort { Sort::Codes => Sort::Codes, Sort::Value(code) => Sort::Value(shifted_code(code,base)),
        Sort::Premise(p) => Sort::Premise(Box::new(shifted_prop(p,base))) }
}
fn replace_code(code: &Code, binder: usize, argument: &Code) -> Code {
    match code {
        Code::Variable(i) if *i == binder => argument.clone(),
        Code::Variable(i) if *i > binder => Code::Variable(i-1),
        Code::Pair(a,b) => Code::Pair(Box::new(replace_code(a,binder,argument)),Box::new(replace_code(b,binder,argument))),
        Code::Sum(a,b) => Code::Sum(Box::new(replace_code(a,binder,argument)),Box::new(replace_code(b,binder,argument))),
        Code::List(a) => Code::List(Box::new(replace_code(a,binder,argument))),
        other => other.clone(),
    }
}
fn replace_prop(prop: &Proposition, binder: usize, argument: &Argument) -> Proposition {
    match prop {
        Proposition::Pi(i,sort,body) =>
            Proposition::Pi(i-1,replace_sort(sort,binder,argument),Box::new(replace_prop(body,binder,argument))),
        Proposition::Eq(ty,a,b) =>
            Proposition::Eq(replace_type(ty,binder,argument),replace_index(*a,binder,argument),replace_index(*b,binder,argument)),
    }
}
fn replace_type(ty: &Code, binder: usize, argument: &Argument) -> Code {
    match argument { Argument::Code(code) => replace_code(ty,binder,code), _ => ty.clone() }
}
fn replace_index(index: usize, binder: usize, argument: &Argument) -> usize {
    if index == binder { if let Argument::Value(i) = argument { *i } else { index } }
    else if index > binder { index - 1 } else { index }
}
fn replace_sort(sort: &Sort, binder: usize, argument: &Argument) -> Sort {
    match sort {
        Sort::Codes => Sort::Codes,
        Sort::Value(code) => Sort::Value(replace_type(code,binder,argument)),
        Sort::Premise(p) => Sort::Premise(Box::new(replace_prop(p,binder,argument))),
    }
}
enum Argument { Code(Code), Value(usize), Premise }
fn code(form: &Form, context: &[Binder], meter: &mut Meter, depth: u32) -> Result<Code, Diagnostic> {
    meter.visit(depth, form.span)?;
    if let Ok(value) = atom(form) {
        return match value {
            "Unit" => Ok(Code::Unit), "Bool" => Ok(Code::Bool), "I64" => Ok(Code::I64), "Text" => Ok(Code::Text),
            _ => context.iter().rposition(|b| b.name == value).filter(|&i| context[i].sort == Sort::Codes)
                .map(Code::Variable).ok_or_else(|| reject(form.span, "not a bound Type0 code")),
        };
    }
    let items = list(form)?;
    match items.first().and_then(|item| atom(item).ok()) {
        Some("Pair") | Some("Sum") if items.len() == 3 => {
            let a = Box::new(code(&items[1], context, meter, depth + 1)?);
            let b = Box::new(code(&items[2], context, meter, depth + 1)?);
            if atom(&items[0])? == "Pair" { Ok(Code::Pair(a,b)) } else { Ok(Code::Sum(a,b)) }
        }
        Some("List") if items.len() == 2 => Ok(Code::List(Box::new(code(&items[1], context, meter, depth + 1)?))),
        _ => Err(reject(form.span, "not a predicative Type0 code")),
    }
}
fn sort(form: &Form, context: &[Binder], meter: &mut Meter, depth: u32) -> Result<Sort, Diagnostic> {
    if atom(form).ok() == Some("Type0") { Ok(Sort::Codes) }
    else if list(form).ok().and_then(|x| x.first()).and_then(|x| atom(x).ok()) == Some("Eq") {
        Ok(Sort::Premise(Box::new(proposition(form, context, meter, depth + 1)?)))
    } else { Ok(Sort::Value(code(form, context, meter, depth + 1)?)) }
}
fn proposition(form: &Form, context: &[Binder], meter: &mut Meter, depth: u32) -> Result<Proposition, Diagnostic> {
    meter.visit(depth, form.span)?;
    let items = list(form)?;
    match items.first().and_then(|item| atom(item).ok()) {
        Some("Pi") if items.len() == 3 => {
            let binder = list(&items[1])?;
            if binder.len() != 2 { return Err(reject(items[1].span, "Pi needs one typed binder")); }
            let name = atom(&binder[0])?.to_string();
            let ty = sort(&binder[1], context, meter, depth + 1)?;
            let mut extended = Vec::from(context);
            extended.push(Binder { name, sort: ty.clone() });
            Ok(Proposition::Pi(context.len(), ty, Box::new(proposition(&items[2], &extended, meter, depth + 1)?)))
        }
        Some("Eq") if items.len() == 4 => {
            let ty = code(&items[1], context, meter, depth + 1)?;
            let mut vars = [0;2];
            for (slot, value) in vars.iter_mut().zip(&items[2..]) {
                let name = atom(value)?;
                let pos = context.iter().rposition(|b| b.name == name).ok_or_else(|| reject(value.span, "unbound equality value"))?;
                if context[pos].sort != Sort::Value(ty.clone()) { return Err(reject(value.span, "equality operand has wrong code")); }
                *slot = pos;
            }
            Ok(Proposition::Eq(ty, vars[0], vars[1]))
        }
        _ => Err(reject(form.span, "expected dependent Pi or typed Eq proposition")),
    }
}
fn lean_code(ty: &Code) -> String {
    match ty { Code::Unit => ".unit".into(), Code::Bool => ".bool".into(), Code::I64 => ".i64".into(), Code::Text => ".text".into(),
        Code::Variable(i) => format!("v{i}"), Code::Pair(a,b) => format!("(.pair {} {})",lean_code(a),lean_code(b)),
        Code::Sum(a,b) => format!("(.sum {} {})",lean_code(a),lean_code(b)), Code::List(a) => format!("(.list {})",lean_code(a)) }
}
fn lean_sort(ty: &Sort) -> String {
    match ty { Sort::Codes => "PureTyCode".into(), Sort::Value(code) => format!("El {}",lean_code(code)), Sort::Premise(p) => lean_proposition(p) }
}
fn lean_proposition(p: &Proposition) -> String {
    match p { Proposition::Pi(index, ty, body) => format!("(∀ (v{index} : {}), {})", lean_sort(ty), lean_proposition(body)),
        Proposition::Eq(_, a,b) => format!("v{a} = v{b}") }
}
fn infer(term: &Form, context: &mut Vec<Binder>, available: &[Available], meter: &mut Meter, depth: u32) -> Result<(Proposition,String), Diagnostic> {
    meter.visit(depth,term.span)?;
    if let Ok(args) = operation(term,"use",1) {
        let name = atom(&args[0])?;
        if let Some(index) = context.iter().rposition(|binder| binder.name == name) {
            if let Sort::Premise(p) = &context[index].sort { return Ok(((**p).clone(),format!("v{index}"))); }
            return Err(reject(term.span,"use does not name a proof premise"));
        }
        let dependency = available.iter().rev().find(|item| item.name == name)
            .ok_or_else(|| reject(term.span,"unresolved, private, forward or cyclic proof use"))?;
        meter.charge(dependency.lean_term.len(),term.span)?;
        meter.substitute(proposition_size(&dependency.goal),term.span)?;
        return Ok((shifted_prop(&dependency.goal,context.len()),dependency.lean_term.clone()));
    }
    if let Ok(args) = operation(term,"apply",2) {
        let (function, lowered) = infer(&args[0],context,available,meter,depth+1)?;
        let Proposition::Pi(index,domain,body) = function else { return Err(reject(term.span,"apply requires dependent Pi proof")); };
        if index != context.len() { return Err(reject(term.span,"apply binder scope mismatch")); }
        let (argument, expression) = match domain {
            Sort::Codes => { let ty = code(&args[1],context,meter,depth+1)?; let lean = lean_code(&ty); (Argument::Code(ty),lean) }
            Sort::Value(expected) => {
                let name = atom(&args[1])?;
                let value = context.iter().rposition(|binder| binder.name == name)
                    .ok_or_else(|| reject(args[1].span,"apply value is unbound"))?;
                meter.normalize(code_size(&expected),args[1].span)?;
                if context[value].sort != Sort::Value(expected) { return Err(reject(args[1].span,"apply value has wrong code")); }
                (Argument::Value(value),format!("v{value}"))
            }
            Sort::Premise(expected) => {
                let proof = checked_intro(&args[1],&expected,context,available,meter,depth+1)?;
                (Argument::Premise,proof)
            }
        };
        meter.charge(1+depth as usize,term.span)?;
        meter.charge(lowered.len()+expression.len(),term.span)?;
        meter.substitute(proposition_size(&body),term.span)?;
        return Ok((replace_prop(&body,index,&argument),format!("({lowered} {expression})")));
    }
    if let Ok(args) = operation(term,"refl",1) {
        let name = atom(&args[0])?;
        let index = context.iter().rposition(|binder| binder.name == name)
            .ok_or_else(|| reject(args[0].span,"unbound refl value"))?;
        if let Sort::Value(code) = &context[index].sort {
            return Ok((Proposition::Eq(code.clone(),index,index),format!("(Eq.refl v{index})")));
        }
        return Err(reject(term.span,"refl operand is not a total value"));
    }
    Err(reject(term.span,"proof term has no inferable proposition"))
}
fn checked_intro(term: &Form, goal: &Proposition, context: &mut Vec<Binder>, available: &[Available], meter: &mut Meter, depth: u32) -> Result<String, Diagnostic> {
    meter.visit(depth, term.span)?;
    match goal {
        Proposition::Pi(index, expected, rest) => {
            let args = match operation(term, "intro", 2) {
                Ok(args) => args,
                Err(_) => {
                    let (inferred,lowered)=infer(term,context,available,meter,depth+1)?;
                    meter.normalize(proposition_size(goal),term.span)?;
                    if inferred != *goal { return Err(reject(term.span,"inferred proof does not match dependent Pi goal")); }
                    return Ok(lowered);
                }
            };
            let binder = list(&args[0])?;
            if binder.len()!=2 { return Err(reject(args[0].span,"intro needs a typed binder")); }
            let name = atom(&binder[0])?.to_string();
            let actual = sort(&binder[1], context, meter, depth+1)?;
            meter.normalize(proposition_size(goal),args[0].span)?;
            if &actual != expected || *index != context.len() { return Err(reject(args[0].span,"intro binder type differs from goal")); }
            context.push(Binder { name, sort: actual });
            let index = context.len()-1;
            let body = checked_intro(&args[1], rest, context, available, meter, depth+1);
            context.pop();
            meter.charge(body.as_ref().map_or(0,String::len)+lean_sort(expected).len(),term.span)?;
            Ok(format!("(fun (v{index} : {}) => {})",lean_sort(expected),body?))
        }
        Proposition::Eq(_, a,b) => {
            if let Ok(parts) = operation(term,"subst",3) {
                let motive = operation(&parts[0],"motive",2)?;
                let binder = list(&motive[0])?;
                if binder.len()!=2 { return Err(reject(motive[0].span,"subst motive needs one typed value binder")); }
                let name = atom(&binder[0])?.to_string();
                let Sort::Value(code) = sort(&binder[1],context,meter,depth+1)? else {
                    return Err(reject(motive[0].span,"subst motive binder must be a total value"));
                };
                let (equation,equality_lean) = infer(&parts[1],context,available,meter,depth+1)?;
                let Proposition::Eq(equation_code,left,right) = equation else {
                    return Err(reject(parts[1].span,"subst requires equality evidence"));
                };
                meter.normalize(code_size(&code),parts[1].span)?;
                if equation_code != code { return Err(reject(motive[0].span,"subst motive has wrong equality type")); }
                let index = context.len();
                context.push(Binder {name,sort:Sort::Value(code.clone())});
                let body = proposition(&motive[1],context,meter,depth+1);
                context.pop();
                let body = body?;
                if !matches!(body,Proposition::Eq(..)) { return Err(reject(motive[1].span,"subst motive must be a total Eq proposition")); }
                meter.substitute(proposition_size(&body)*2,term.span)?;
                let before = replace_prop(&body,index,&Argument::Value(left));
                let after = replace_prop(&body,index,&Argument::Value(right));
                meter.normalize(proposition_size(goal),term.span)?;
                if after != *goal { return Err(reject(term.span,"subst result differs from expected goal")); }
                let premise = checked_intro(&parts[2],&before,context,available,meter,depth+1)?;
                meter.charge(premise.len()+equality_lean.len(),term.span)?;
                return Ok(format!("(Eq.ndrec (motive := fun (v{index} : El {}) => {}) {premise} {equality_lean})",
                    lean_code(&code),lean_proposition(&body)));
            }
            let (inferred,lowered) = infer(term,context,available,meter,depth+1)?;
            meter.normalize(proposition_size(goal),term.span)?;
            if inferred != Proposition::Eq(match goal { Proposition::Eq(ty,_,_) => ty.clone(),_ => unreachable!() },*a,*b) {
                return Err(reject(term.span,"proof does not establish exact expected equality"));
            }
            Ok(lowered)
        }
    }
}
/// Checks only the closed predicative fragment; contract proofs have a separate
/// finite reviewed MC1 rule checker below. An unchecked term is never lowered.
pub fn check_batch(batch: &ProofBatch, limits: Limits) -> Result<Vec<CheckedProof>, Diagnostic> {
    check_batch_with_budgets(batch,limits,ProofBudgets::from_limits(limits)).map(|(checked,_)|checked)
}
pub fn check_batch_with_budgets(
    batch: &ProofBatch,
    limits: Limits,
    budgets: ProofBudgets,
) -> Result<(Vec<CheckedProof>,ProofWork), Diagnostic> {
    if batch.source.len() > usize::try_from(limits.bytes).unwrap_or(usize::MAX) { return Err(exhausted(Span { start:0,end:0 })); }
    let mut meter = Meter { nodes:0, work:0, normalization:0, substitution:0, budgets, limits };
    let mut result = Vec::new();
    let mut available = Vec::new();
    let mut active = Vec::new();
    for dependency in &batch.dependencies {
        if !dependency.exported { return Err(reject(dependency.declaration.span,"private proof cannot be imported")); }
        let checked = check_dependency(dependency,&mut meter,&mut active,0,None)?;
        available.push(checked);
    }
    for obligation in &batch.obligations {
        let (checked, exported) = check_obligation(obligation,&available,&mut meter)?;
        if let Some(goal) = exported { available.push(goal); }
        result.push(checked);
    }
    Ok((result,ProofWork {normalization:meter.normalization,substitution:meter.substitution,total:meter.work}))
}
fn check_obligation(obligation: &ProofObligation, available: &[Available], meter: &mut Meter)
    -> Result<(CheckedProof,Option<Available>), Diagnostic> {
    meter.visit(0,obligation.span)?;
    validated(&obligation.term,meter,0)?;
    match &obligation.goal {
        PendingGoal::Pure {proposition: statement} => {
            if obligation.revision != 1 { return Err(reject(obligation.span,"pure proof requires revision 1")); }
            validated(statement,meter,0)?;
            let goal = proposition(statement,&[],meter,0)?;
            let lowered = checked_intro(&obligation.term,&goal,&mut Vec::new(),available,meter,0)?;
            let checked = CheckedProof {name:obligation.name.clone(),kind:ProofKind::Pure,
                lean_term:lowered.clone(),claim:lean_proposition(&goal),
                model_revision:String::new(),checker_revision:String::new()};
            Ok((checked,Some(Available {name:obligation.name.clone(),goal,lean_term:lowered})))
        }
        PendingGoal::Contract {contract} => {
            if obligation.revision != contract.revision {
                return Err(reject(obligation.span,"proof and contract revisions differ"));
            }
            let checked = match contract.revision {
                1 => check_contract(obligation,contract,meter)?,
                2 => named_v2::check_contract(obligation,contract,meter)?,
                _ => return Err(reject(obligation.span,"unknown contract revision")),
            };
            Ok((checked,None))
        }
    }
}
fn check_dependency(
    dependency: &ProofDependency,
    meter: &mut Meter,
    active: &mut Vec<(String,u32,String)>,
    depth:u32,
    parent: Option<(&str,u32,&[u8],u32)>,
)
    -> Result<Available,Diagnostic> {
    meter.visit(depth,dependency.declaration.span)?;
    meter.charge(dependency.source.len(),dependency.declaration.span)?;
    let identity = (dependency.module.clone(),dependency.version,dependency.declaration.name.clone());
    if active.contains(&identity) { return Err(reject(dependency.declaration.span,"cyclic proof dependency")); }
    let private_local = parent.is_some_and(|(module,version,source,span)|
        dependency.module == module && dependency.version == version &&
        dependency.source == source && dependency.declaration.span.start < span);
    if (!dependency.exported && !private_local) || dependency.source.is_empty() || dependency.declaration.name.is_empty() {
        return Err(reject(dependency.declaration.span,"unpublished or private proof dependency"));
    }
    active.push(identity);
    let mut available = Vec::new();
    for prerequisite in &dependency.dependencies {
        available.push(check_dependency(prerequisite,meter,active,depth+1,
            Some((&dependency.module,dependency.version,&dependency.source,dependency.declaration.span.start)))?);
    }
    let (_,closed) = check_obligation(&dependency.declaration,&available,meter)?;
    active.pop();
    let mut closed = closed.ok_or_else(|| reject(dependency.declaration.span,"contract theorem dependency is not in the pure fragment"))?;
    closed.name = dependency.name.clone();
    Ok(closed)
}
fn check_contract(obligation: &ProofObligation, contract: &ContractGoal, meter: &mut Meter) -> Result<CheckedProof, Diagnostic> {
    meter.visit(0, obligation.span)?;
    let (_,literal) = prepare_checked_contract(contract,meter.limits)?;
    if contract.inputs.len()!=1 || contract.outputs.len()!=1 ||
        contract.inputs[0].1 != "I64" || contract.outputs[0].1 != "I64" ||
        atom(&contract.requires)? != "true" {
        return Err(reject(obligation.span,"selected unary I64 rule requires typed True precondition"));
    }
    let literal = literal.ok_or_else(|| reject(obligation.span,"selected proof rules require exact literal then wrapping-add subject slices"))?;
    if !matches_eq_add(&contract.ensures,&contract.inputs[0].0,&contract.outputs[0].0,literal) {
        return Err(reject(contract.ensures.span,"proof consequence does not match exact typed MC1 postcondition"));
    }
    let root = operation(&obligation.term,"export-unary-I64",2)?;
    let outer = operation(&root[0],"intro",2)?;
    let tail_binder = list(&outer[0])?;
    if tail_binder.len()!=2 || atom(&tail_binder[1])? != "Stack" { return Err(reject(outer[0].span,"expected explicit Stack tail binder")); }
    let tail_name = atom(&tail_binder[0])?;
    let inner = operation(&outer[1],"intro",2)?;
    let x = list(&inner[0])?;
    if x.len()!=2 || atom(&x[1])? != "I64" || atom(&x[0])? == tail_name {
        return Err(reject(inner[0].span,"expected distinct explicit I64 input binder"));
    }
    let sequence = operation(&inner[1],"pc-sequence",4)?;
    let exact1 = operation(&sequence[0],"pc-exact",1)?;
    let lit = operation(&exact1[0],"exec-literal",1)?;
    if atom(&lit[0])?.parse::<i64>().ok() != Some(literal) { return Err(reject(lit[0].span,"literal rule does not match first subject slice")); }
    let exact2 = operation(&sequence[1],"pc-exact",1)?;
    operation(&exact2[0],"exec-add",0)?;
    let bridge = operation(&sequence[2],"bridge",1)?;
    operation(&bridge[0],"by-exact-append-assoc",0)?;
    let join = operation(&sequence[3],"join",1)?;
    operation(&join[0],"by-exact-result",0)?;
    operation(&root[1],"mc1-true-eq-wrap",0)?;
    let lean_term = lower_unary_add(literal);
    meter.charge(lean_term.len(),obligation.span)?;
    Ok(CheckedProof { name: obligation.name.clone(), kind: ProofKind::Contract,
        lean_term, claim: "MC1Obligation.claim".into(),
        model_revision:String::new(),checker_revision:String::new() })
}
fn mc1_expression(form: &Form, meter: &mut Meter, depth: u32) -> Result<String, Diagnostic> {
    meter.visit(depth,form.span)?;
    match &form.kind {
        FormKind::Atom(word) => {
            if word.is_empty() || !word.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
                return Err(reject(form.span,"invalid MC1 logical word"));
            }
            Ok(word.clone())
        }
        FormKind::List(forms) => {
            let mut text = String::from("(");
            for (index,part) in forms.iter().enumerate() {
                if index != 0 { text.push(' '); }
                text.push_str(&mc1_expression(part,meter,depth+1)?);
            }
            text.push(')');
            Ok(text)
        }
    }
}
/// Cross-checks the asserted source binding, reaccepted complete definition
/// recipe and MC1-v1 frontend. Host authority over these public fields must
/// come from the retained `ModuleSession`, not this consistency check.
pub fn prepare_contract(contract: &ContractGoal, limits: Limits) -> Result<crate::Prepared, Diagnostic> {
    prepare_checked_contract(contract,limits).map(|(prepared,_)| prepared)
}
fn prepare_checked_contract(contract: &ContractGoal, limits: Limits) -> Result<(crate::Prepared, Option<i64>), Diagnostic> {
    let subject = &contract.subject;
    let mut meter = Meter { nodes:0,work:0,normalization:0,substitution:0,budgets:ProofBudgets::from_limits(limits),limits };
    meter.charge(subject.module_source.len().saturating_add(subject.definition_source.len()),contract.contract_span)?;
    if !matches!(subject.source_dependencies.as_slice(), [origin] if
        origin.module == subject.module && origin.version == subject.version &&
        origin.full_source == subject.module_source) {
        return Err(reject(subject.source_span,"subject source dependency differs from original module"));
    }
    if subject.effects != noble_kernel::types::EffSet::empty() {
        return Err(reject(subject.source_span,"MC1 contract subject has resource effects"));
    }
    let accepted = &subject.accepted_submission;
    let noble_kernel::untrusted::Outcome::Accepted(interface) =
        noble_kernel::acceptance::check(&accepted.environment,&accepted.request,&accepted.body.candidate)
    else { return Err(reject(subject.source_span,"subject candidate is not independently accepted")); };
    if interface.interface.stack_in != subject.input_types ||
       interface.interface.stack_out != subject.output_types ||
       interface.interface.effects != subject.effects {
        return Err(reject(subject.source_span,"accepted subject interface differs from contract"));
    }
    let mut matching = accepted.definitions.iter().filter(|item| item.identity == subject.definition_identity);
    let Some(definition) = matching.next() else {
        return Err(reject(subject.source_span,"resolved subject identity absent from accepted recipe"));
    };
    if matching.next().is_some() || accepted.definitions.len() != 1 {
        return Err(reject(subject.source_span,"ambiguous resolved subject identity"));
    }
    let slot = usize::try_from(definition.definition.0)
        .map_err(|_| reject(subject.source_span,"subject definition slot exceeds address space"))?;
    if accepted.environment.definition_owners.get(slot) != Some(&Some(subject.definition_owner)) ||
        accepted.environment.kind(definition.definition) != Some(noble_kernel::contracts::Behavior::Named) ||
        !matches!(accepted.body.candidate.body.as_slice(), [node] if
            matches!(accepted.body.candidate.nodes.get(node.0 as usize),
                Some(noble_kernel::untrusted::Node::Invocation {def,..}) if *def == definition.definition)) ||
        accepted.body.candidate.nodes.len() != 1 ||
        !accepted.body.texts.is_empty() {
        return Err(reject(subject.source_span,"resolved subject owner or invocation differs from accepted recipe"));
    }
    if definition.expected.stack_in != subject.input_types ||
       definition.expected.stack_out != subject.output_types ||
       definition.expected.allowed_effects != subject.effects {
        return Err(reject(subject.source_span,"subject definition has different stack or effect bound"));
    }
    let body_span = crate::source::verify_definition_origin(
        crate::source::DefinitionOrigin {
            module_source: &subject.module_source,
            module: &subject.module,
            version: subject.version,
            definition: &subject.definition,
            ordinal: subject.definition_ordinal,
            span: subject.source_span,
        },
        &subject.definition_source, limits)?;
    let start = usize::try_from(body_span.start).map_err(|_| reject(body_span,"subject body exceeds address space"))?;
    let end = usize::try_from(body_span.end).map_err(|_| reject(body_span,"subject body exceeds address space"))?;
    let body = subject.module_source.get(start..end)
        .ok_or_else(|| reject(body_span,"subject body is outside original module source"))?;
    let body = core::str::from_utf8(body)
        .map_err(|_| reject(subject.source_span,"subject body must be UTF-8 for MC1"))?;
    let mut text = format!("(contract 1 {} (input",contract.contract_name);
    for (name,ty) in &contract.inputs { text.push_str(&format!(" ({name} {ty})")); }
    text.push_str(") (output");
    for (name,ty) in &contract.outputs { text.push_str(&format!(" ({name} {ty})")); }
    text.push_str(&format!(") (program {body}) (requires {}) (ensures {}))",
        mc1_expression(&contract.requires,&mut meter,0)?,mc1_expression(&contract.ensures,&mut meter,0)?));
    meter.charge(text.len(),contract.contract_span)?;
    let result = crate::prepare(text.as_bytes(),limits)?;
    if result.inputs().len()!=contract.inputs.len() || result.outputs().len()!=contract.outputs.len() ||
        result.checked().interface.stack_in != subject.input_types ||
        result.checked().interface.stack_out != subject.output_types ||
        result.checked().interface.effects != subject.effects {
        return Err(reject(subject.source_span,"MC1 typed contract differs from accepted subject interface"));
    }
    // The standalone MC1 program must be the accepted definition's complete
    // operation list. Any unresolved named call or different recipe is refused.
    let expected = &definition.body.candidate;
    let actual = result.candidate();
    if !definition.body.texts.is_empty() || expected.format != actual.format ||
        expected.revision != actual.revision || expected.body.len() != actual.body.len() ||
        expected.nodes.len() != actual.nodes.len() || expected.nodes.len() != expected.body.len() ||
        expected.body.len() > limits.nodes as usize {
        return Err(reject(subject.source_span,"MC1 program differs from resolved subject recipe"));
    }
    for (a,b) in expected.body.iter().zip(&actual.body) {
        let Some(left) = expected.nodes.get(a.0 as usize) else { return Err(reject(subject.source_span,"invalid accepted subject node")); };
        let Some(right) = actual.nodes.get(b.0 as usize) else { return Err(reject(subject.source_span,"invalid MC1 subject node")); };
        match (left,right) {
            (noble_kernel::untrusted::Node::Literal {lit:a,inst:ia},noble_kernel::untrusted::Node::Literal {lit:b,inst:ib})
                if a==b && same_inst(ia,ib) => (),
            (noble_kernel::untrusted::Node::Invocation {def:a,inst:ia},noble_kernel::untrusted::Node::Invocation {def:b,inst:ib})
                if a==b && same_inst(ia,ib) => (),
            _ => return Err(reject(subject.source_span,"MC1 program differs from resolved subject operation")),
        }
    }
    let request = noble_kernel::untrusted::Request {
        input_bytes: accepted.request.input_bytes,
        expected: definition.expected.clone(),
        limits: accepted.request.limits,
    };
    let noble_kernel::untrusted::Outcome::Accepted(checked_definition) =
        noble_kernel::acceptance::check(&accepted.environment,&request,expected)
    else { return Err(reject(subject.source_span,"subject definition body is not independently accepted")); };
    if checked_definition.interface.stack_in != subject.input_types ||
        checked_definition.interface.stack_out != subject.output_types ||
        checked_definition.interface.effects != subject.effects {
        return Err(reject(subject.source_span,"accepted definition body differs from contract interface"));
    }
    if checked_definition.derivations.len() != result.checked().derivations.len() ||
        !checked_definition.derivations.iter().zip(&result.checked().derivations).all(|(left,right)| {
            left.node == right.node &&
            left.interface.stack_in == right.interface.stack_in &&
            left.interface.stack_out == right.interface.stack_out &&
            left.interface.effects == right.interface.effects
        }) {
        return Err(reject(subject.source_span,"MC1 node derivations differ from accepted definition body"));
    }
    let literal = match expected.nodes.as_slice() {
        [noble_kernel::untrusted::Node::Literal {lit:noble_kernel::untrusted::Lit::I64(value),..},
         noble_kernel::untrusted::Node::Invocation {def:noble_kernel::contracts::Definition(4),..}]
            if expected.body.as_slice() == [noble_kernel::untrusted::NodeId(0),noble_kernel::untrusted::NodeId(1)]
                && subject.input_types == [noble_kernel::types::Ty::I64]
                && subject.output_types == [noble_kernel::types::Ty::I64] => Some(*value),
        _ => None,
    };
    if literal.is_some() &&
        (!canonical_wrapping_add(&accepted.environment) ||
         accepted.environment.deps.get(slot).map(Vec::as_slice) !=
            Some(&[noble_kernel::contracts::Definition(4)][..])) {
        return Err(reject(subject.source_span,"subject addition is not the fixed MC1 builtin"));
    }
    Ok((result,literal))
}

fn canonical_wrapping_add(env: &noble_kernel::contracts::Env) -> bool {
    use noble_kernel::{contracts::{Behavior,Definition},shapes::Pattern,words::{Variable,VariableKind}};
    let Some(scheme) = env.scheme(Definition(4)) else { return false; };
    env.kind(Definition(4)) == Some(Behavior::Arith) &&
        env.definition_owners.get(4) == Some(&None) &&
        env.deps.get(4).is_some_and(Vec::is_empty) &&
        scheme.var_kinds == [VariableKind::Stack] &&
        matches!(scheme.stack_in.as_slice(),
            [Pattern::StackVar(Variable(0)),Pattern::I64,Pattern::I64]) &&
        matches!(scheme.stack_out.as_slice(),
            [Pattern::StackVar(Variable(0)),Pattern::I64]) &&
        scheme.effects.is_empty()
}

fn same_inst(a: &noble_kernel::words::Inst, b: &noble_kernel::words::Inst) -> bool {
    use noble_kernel::words::Binding;
    a.bindings.len() == b.bindings.len() && a.bindings.iter().zip(&b.bindings).all(|(a,b)| match (a,b) {
        (Binding::Stack(a),Binding::Stack(b)) => a == b,
        (Binding::Value(a),Binding::Value(b)) => a == b,
        (Binding::Effect(a),Binding::Effect(b)) => a == b,
        (Binding::Ref(a),Binding::Ref(b)) => a == b,
        _ => false,
    })
}
fn matches_eq_add(form: &Form, input_name: &str, output_name: &str, literal: i64) -> bool {
    let Ok(eq) = operation(form,"eq",2) else { return false };
    let Ok(out) = operation(&eq[0],"out",1) else { return false };
    let Ok(add) = operation(&eq[1],"add",2) else { return false };
    let Ok(input) = operation(&add[0],"in",1) else { return false };
    atom(&out[0]).ok()==Some(output_name) &&
        atom(&input[0]).ok()==Some(input_name) &&
        atom(&add[1]).ok().and_then(|word|word.parse::<i64>().ok())==Some(literal)
}

/// Expansion of the checked constructors in `check_contract`. Every instance
/// of `__N__` is sourced from the independently accepted literal node. These
/// are actual PC introduction/elimination terms, not a selected theorem name.
fn lower_unary_add(literal: i64) -> String {
    const TERM: &str = r#"
(exportedClaim_consequence
  (exportedClaim_unaryI64
    (code := [.lit (.i64 __N__)] ++ [.word 4])
    (f := fun x => x + (__N__ : BitVec 64))
    (fun tail x final execution =>
      let first : PC [.lit (.i64 __N__)]
          (fun before => before = tail ++ [.i64 x])
          (fun _ middle => middle = (tail ++ [.i64 x]) ++ [.i64 __N__]) :=
        pc_exact (code := [.lit (.i64 __N__)]) (s := tail ++ [.i64 x])
          (expected := (tail ++ [.i64 x]) ++ [.i64 __N__])
          (fun actual => exec_lit_iff (v := .i64 __N__)
            (s := tail ++ [.i64 x]) (t := actual))
      let second : PC [.word 4]
          (fun middle => middle = tail ++ [.i64 x, .i64 __N__])
          (fun _ actual => actual = tail ++ [.i64 (x + (__N__ : BitVec 64))]) :=
        pc_exact (code := [.word 4]) (s := tail ++ [.i64 x, .i64 __N__])
          (expected := tail ++ [.i64 (x + (__N__ : BitVec 64))])
          (fun actual => exec_add_iff (a := x) (b := __N__)
            (s := tail) (t := actual))
      let composed : PC ([.lit (.i64 __N__)] ++ [.word 4])
          (fun before => before = tail ++ [.i64 x])
          (fun before actual => before = tail ++ [.i64 x] ∧
            actual = tail ++ [.i64 (x + (__N__ : BitVec 64))]) :=
        @pc_sequence
          [.lit (.i64 __N__)] [.word 4]
          (fun (before : Stack) => before = tail ++ [.i64 x])
          (fun (_ middle : Stack) => middle = (tail ++ [.i64 x]) ++ [.i64 __N__])
          (fun (middle : Stack) => middle = tail ++ [.i64 x, .i64 __N__])
          (fun (_ actual : Stack) => actual = tail ++ [.i64 (x + (__N__ : BitVec 64))])
          (fun (before actual : Stack) => before = tail ++ [.i64 x] ∧
            actual = tail ++ [.i64 (x + (__N__ : BitVec 64))])
          first second
          (fun _ middle _ exactMiddle =>
            exactMiddle.trans (List.append_assoc tail [.i64 x] [.i64 __N__]))
          (fun _ _ _ initial _ exactFinal => ⟨initial, exactFinal⟩)
      ((composed (tail ++ [.i64 x]) final rfl execution).2)))
  (fun _ _ _ => True.intro)
  (fun before after params _ exactPost =>
    match exactPost with
    | ⟨x, hbefore, hafter⟩ =>
      hbefore.symm ▸ hafter.symm ▸
      (holds_eq (.output 0) (.add (.input 0) (.i64 __N__))
        ⟨[.i64 x], [.i64 (x + (__N__ : BitVec 64))], params⟩
        (.i64 (x + (__N__ : BitVec 64))) (.i64 (x + (__N__ : BitVec 64)))
        rfl rfl).mpr rfl))
"#;
    TERM.replace("__N__", &literal.to_string())
}
