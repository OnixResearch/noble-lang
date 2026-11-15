//! Exact-pair verifier for the dependency-free I64 stack fragment.
//!
//! This is a local, finite symbolic evaluation, not a digest equivalence
//! assertion or a complete decision procedure for typed chunks. Quotation,
//! invocation, Boolean operations and any dependency closure are refused.
//! The canonical sparse polynomial is over Z/2^64; each term is ordered by its
//! eight input exponents and has a nonzero wrapping coefficient. The chunk
//! library separately checks retained source and witnesses before publication.

use super::*;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

pub const MAX_ALIAS_INPUTS: usize = 8;
pub const MAX_ALIAS_OUTPUTS: usize = 8;
pub const MAX_ALIAS_TERMS: usize = 64;
const MAX_ALIAS_DEGREE: u16 = 8;
const MAX_ALIAS_WORK: u32 = 8_192;
const ALIAS_EVIDENCE_OBLIGATION: u32 = 0x414c_4941;
const ALIAS_CHECKER: &str = "noble-procedure-alias-poly/1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Monomial {
    pub coefficient: u64,
    pub powers: [u8; MAX_ALIAS_INPUTS],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Polynomial {
    pub terms: Vec<Monomial>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactPairCertificate {
    pub left: Vec<Polynomial>,
    pub right: Vec<Polynomial>,
    pub left_closure: ProcedureId,
    pub right_closure: ProcedureId,
}

#[octet::sealed_enum]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AliasCertificate {
    /// The old 32-byte claim is kept only to parse old wire records. It can
    /// never authorize an alias, irrespective of its digest value.
    LegacyDigest([u8; 32]),
    ExactPair(Box<ExactPairCertificate>),
}

fn refused(kind: RefusalKind, message: &'static str) -> Refusal {
    Refusal::new(
        kind,
        Diagnostic {
            field: "alias_of",
            message,
        },
    )
}

fn charge(work: &mut u32, amount: usize) -> Result<(), Refusal> {
    let amount = u32::try_from(amount).map_err(|_| {
        refused(RefusalKind::Exhausted, "alias polynomial work count exceeds its domain")
    })?;
    *work = work.checked_add(amount).ok_or_else(|| {
        refused(RefusalKind::Exhausted, "alias polynomial work count overflow")
    })?;
    if *work > MAX_ALIAS_WORK {
        return Err(refused(
            RefusalKind::Exhausted,
            "alias polynomial normalization exceeds the fixed work bound",
        ));
    }
    Ok(())
}

impl Polynomial {
    fn constant(value: u64) -> Self {
        let terms = if value == 0 {
            Vec::new()
        } else {
            alloc::vec![Monomial {
                coefficient: value,
                powers: [0; MAX_ALIAS_INPUTS],
            }]
        };
        Self { terms }
    }

    fn variable(index: usize) -> Self {
        let mut powers = [0; MAX_ALIAS_INPUTS];
        powers[index] = 1;
        Self {
            terms: alloc::vec![Monomial {
                coefficient: 1,
                powers,
            }],
        }
    }

    fn insert(&mut self, term: Monomial, work: &mut u32) -> Result<(), Refusal> {
        if term.coefficient == 0 {
            return Ok(());
        }
        charge(work, self.terms.len())?;
        match self
            .terms
            .binary_search_by_key(&term.powers, |existing| existing.powers)
        {
            Ok(index) => {
                let coefficient = self.terms[index].coefficient.wrapping_add(term.coefficient);
                if coefficient == 0 {
                    self.terms.remove(index);
                } else {
                    self.terms[index].coefficient = coefficient;
                }
            }
            Err(index) => {
                if self.terms.len() == MAX_ALIAS_TERMS {
                    return Err(refused(
                        RefusalKind::Exhausted,
                        "alias polynomial exceeds the term bound",
                    ));
                }
                self.terms.insert(index, term);
            }
        }
        Ok(())
    }

    fn combine(mut self, other: &Self, subtract: bool, work: &mut u32) -> Result<Self, Refusal> {
        for term in &other.terms {
            let mut term = term.clone();
            if subtract {
                term.coefficient = term.coefficient.wrapping_neg();
            }
            self.insert(term, work)?;
        }
        Ok(self)
    }

    fn multiply(&self, other: &Self, work: &mut u32) -> Result<Self, Refusal> {
        charge(work, self.terms.len().saturating_mul(other.terms.len()))?;
        let mut result = Self { terms: Vec::new() };
        for left in &self.terms {
            for right in &other.terms {
                let mut powers = [0; MAX_ALIAS_INPUTS];
                let mut degree = 0u16;
                for (index, exponent) in powers.iter_mut().enumerate() {
                    let next = u16::from(left.powers[index]) + u16::from(right.powers[index]);
                    degree += next;
                    if next > MAX_ALIAS_DEGREE || degree > MAX_ALIAS_DEGREE {
                        return Err(refused(
                            RefusalKind::Exhausted,
                            "alias polynomial exceeds the degree bound",
                        ));
                    }
                    *exponent = u8::try_from(next).map_err(|_| {
                        refused(RefusalKind::Exhausted, "alias exponent overflows the wire domain")
                    })?;
                }
                result.insert(
                    Monomial {
                        coefficient: left.coefficient.wrapping_mul(right.coefficient),
                        powers,
                    },
                    work,
                )?;
            }
        }
        Ok(result)
    }
}

fn pop(stack: &mut Vec<Polynomial>) -> Result<Polynomial, Refusal> {
    stack.pop().ok_or_else(|| {
        refused(
            RefusalKind::Unsupported,
            "alias symbolic stack underflow",
        )
    })
}

fn push(stack: &mut Vec<Polynomial>, value: Polynomial) -> Result<(), Refusal> {
    if stack.len() >= 32 {
        return Err(refused(
            RefusalKind::Exhausted,
            "alias symbolic stack exceeds the profile bound",
        ));
    }
    stack.push(value);
    Ok(())
}

fn normalize(record: &ProcedureRecord) -> Result<Vec<Polynomial>, Refusal> {
    let chunk = &record.chunk;
    if !record.library_dependencies.is_empty() || !chunk.dependencies.is_empty() {
        return Err(refused(
            RefusalKind::Unsupported,
            "dependency-bearing aliases require a verified expansion model",
        ));
    }
    if chunk.interface.stack_in.len() > MAX_ALIAS_INPUTS
        || chunk.interface.stack_out.len() > MAX_ALIAS_OUTPUTS
        || chunk.interface.stack_in.iter().any(|ty| *ty != Ty::I64)
        || chunk.interface.stack_out.iter().any(|ty| *ty != Ty::I64)
        || chunk.body.entry.len() > 64
    {
        return Err(refused(
            RefusalKind::Unsupported,
            "alias interface or recipe exceeds the I64 polynomial fragment",
        ));
    }
    let mut stack = Vec::with_capacity(chunk.interface.stack_in.len());
    for index in 0..chunk.interface.stack_in.len() {
        push(&mut stack, Polynomial::variable(index))?;
    }
    let mut work = 0;
    for index in &chunk.body.entry {
        charge(&mut work, 1)?;
        let node_index = usize::try_from(*index).map_err(|_| {
            refused(RefusalKind::Unsupported, "alias node index exceeds the host domain")
        })?;
        let node = chunk.body.nodes.get(node_index).ok_or_else(|| {
            refused(RefusalKind::Unsupported, "alias references a missing recipe node")
        })?;
        match node {
            chunks::Node::I64(value) => push(&mut stack, Polynomial::constant(*value as u64))?,
            chunks::Node::Capture { index } => {
                let capture = chunk.captures.get(usize::from(*index)).ok_or_else(|| {
                    refused(RefusalKind::Unsupported, "alias references a missing capture")
                })?;
                let chunks::Scalar::I64(value) = capture.value else {
                    return Err(refused(RefusalKind::Unsupported, "Boolean alias capture"));
                };
                push(&mut stack, Polynomial::constant(value as u64))?;
            }
            chunks::Node::Operation { op, instantiation } => {
                if chunk.instantiations.get(usize::from(*instantiation)).is_none() {
                    return Err(refused(
                        RefusalKind::Unsupported,
                        "alias references a missing instantiation witness",
                    ));
                }
                match op {
                    chunks::Op::Dup => {
                        let top = stack.last().cloned().ok_or_else(|| {
                            refused(RefusalKind::Unsupported, "alias duplicate needs a value")
                        })?;
                        push(&mut stack, top)?;
                    }
                    chunks::Op::Drop => {
                        let _ = pop(&mut stack)?;
                    }
                    chunks::Op::Swap => {
                        let top = pop(&mut stack)?;
                        let below = pop(&mut stack)?;
                        push(&mut stack, top)?;
                        push(&mut stack, below)?;
                    }
                    chunks::Op::Add | chunks::Op::Sub | chunks::Op::Mul => {
                        let right = pop(&mut stack)?;
                        let left = pop(&mut stack)?;
                        let result = if *op == chunks::Op::Mul {
                            left.multiply(&right, &mut work)?
                        } else {
                            left.combine(&right, *op == chunks::Op::Sub, &mut work)?
                        };
                        push(&mut stack, result)?;
                    }
                    chunks::Op::Eq | chunks::Op::Run => {
                        return Err(refused(
                            RefusalKind::Unsupported,
                            "Boolean or quotation alias operation is outside the polynomial fragment",
                        ));
                    }
                }
            }
            chunks::Node::Bool(_) | chunks::Node::Invoke { .. } | chunks::Node::Quotation { .. } => {
                return Err(refused(
                    RefusalKind::Unsupported,
                    "alias recipe node is outside the I64 polynomial fragment",
                ));
            }
        }
    }
    if stack.len() != chunk.interface.stack_out.len() {
        return Err(refused(
            RefusalKind::Unsupported,
            "alias symbolic output does not match the declared stack",
        ));
    }
    Ok(stack)
}

impl Alias {
    /// Produce a claim from two immutable subjects. This helper never
    /// authorizes retention: admission recomputes both forms and both closure
    /// digests against the actual retained target on each rebuild and reload.
    pub fn certify(left: &ProcedureRecord, right: &ProcedureRecord) -> Result<Self, Refusal> {
        let left_form = normalize(left)?;
        let right_form = normalize(right)?;
        if left.procedure_id == right.procedure_id || left.chunk.interface != right.chunk.interface {
            return Err(refused(
                RefusalKind::Invalid,
                "alias requires distinct subjects with the same exact stack interface",
            ));
        }
        if left_form != right_form {
            return Err(refused(
                RefusalKind::Invalid,
                "alias pair has different canonical polynomials",
            ));
        }
        let closure = persistence::empty_alias_closure_digest();
        Ok(Self {
            procedure_id: right.procedure_id,
            certificate: AliasCertificate::ExactPair(Box::new(ExactPairCertificate {
                left: left_form,
                right: right_form,
                left_closure: closure,
                right_closure: closure,
            })),
        })
    }

    /// Attach the exact-pair claim together with its own intrinsically
    /// generated validation evidence. The checker re-derives and compares
    /// this record on each admission/reload; it does not claim Lean proof.
    pub fn attach(self, record: &mut ProcedureRecord) -> Result<(), Refusal> {
        if record.alias_of.is_some() {
            return Err(refused(
                RefusalKind::Invalid,
                "alias record already carries an exact-pair claim",
            ));
        }
        record.evidence.push(self.evidence(record.procedure_id));
        record.alias_of = Some(self);
        Ok(())
    }

    fn evidence(&self, subject: ProcedureId) -> EvidenceRecord {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut claim = String::from("exact-pair:");
        for id in [subject, self.procedure_id] {
            for byte in id {
                claim.push(char::from(HEX[usize::from(byte >> 4)]));
                claim.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
            claim.push(':');
        }
        claim.push_str("empty-closure");
        EvidenceRecord {
            obligation: ALIAS_EVIDENCE_OBLIGATION,
            class: EvidenceClass::Validation,
            outcome: EvidenceOutcome::Passed,
            toolchain: alloc::vec![String::from(ALIAS_CHECKER)],
            claim,
            subject_binding: subject,
            policy_revision: 0,
            capability: None,
        }
    }
}

pub(super) fn verify(records: &[ProcedureRecord]) -> Result<(), Refusal> {
    for record in records {
        let Some(alias) = &record.alias_of else {
            continue;
        };
        let AliasCertificate::ExactPair(certificate) = &alias.certificate else {
            return Err(refused(
                RefusalKind::Unsupported,
                "a legacy alias digest is not an exact-pair proof",
            ));
        };
        let target = records
            .iter()
            .find(|candidate| candidate.procedure_id == alias.procedure_id)
            .ok_or_else(|| refused(RefusalKind::StaleContext, "alias target is not retained"))?;
        if record.procedure_id == target.procedure_id || record.chunk.interface != target.chunk.interface {
            return Err(refused(
                RefusalKind::Invalid,
                "alias pair has unequal identities or stack interfaces",
            ));
        }
        let evidence = alias.evidence(record.procedure_id);
        let mut own_evidence = record
            .evidence
            .iter()
            .filter(|entry| entry.obligation == ALIAS_EVIDENCE_OBLIGATION);
        if own_evidence.next() != Some(&evidence) || own_evidence.next().is_some() {
            return Err(refused(
                RefusalKind::Invalid,
                "alias lacks its own exact-pair validation evidence binding",
            ));
        }
        // The only accepted closure is exactly empty on both subjects. A
        // digest can never substitute for the dependency inspection above.
        let left = normalize(record)?;
        let right = normalize(target)?;
        let empty_closure = persistence::empty_alias_closure_digest();
        if certificate.left_closure != empty_closure
            || certificate.right_closure != empty_closure
            || certificate.left != left
            || certificate.right != right
            || left != right
        {
            return Err(refused(
                RefusalKind::Invalid,
                "alias certificate or exact-pair polynomial differs from recomputation",
            ));
        }
    }
    Ok(())
}
