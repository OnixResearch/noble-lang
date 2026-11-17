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
const CLAIM_CAPACITY_BYTES: usize = 154;

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
pub enum Certificate {
    /// The old 32-byte claim is kept only to parse old wire records. It can
    /// never authorize an alias, irrespective of its digest value.
    LegacyDigest([u8; 32]),
    ExactPair(Box<ExactPairCertificate>),
}

fn refused(kind: RefusalKind, message: &str) -> Refusal {
    Refusal::new(
        kind,
        Diagnostic {
            field: "alias_of",
            message,
        },
    )
}

fn charge(work: &mut u32, amount: usize) -> Result<(), Refusal> {
    debug_assert!(*work <= MAX_ALIAS_WORK);
    let amount = u32::try_from(amount).map_err(|_| {
        refused(
            RefusalKind::Exhausted,
            "alias polynomial work count exceeds its domain",
        )
    })?;
    *work = work.checked_add(amount).ok_or_else(|| {
        refused(
            RefusalKind::Exhausted,
            "alias polynomial work count overflow",
        )
    })?;
    debug_assert!(*work >= amount);
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
        let mut outcome = Ok(());
        let mut index = 0;
        while outcome.is_ok() && index < other.terms.len() {
            let mut term = other.terms[index].clone();
            if subtract {
                term.coefficient = term.coefficient.wrapping_neg();
            }
            outcome = self.insert(term, work);
            index += 1;
        }
        outcome?;
        Ok(self)
    }

    fn multiply(&self, other: &Self, work: &mut u32) -> Result<Self, Refusal> {
        charge(work, self.terms.len().saturating_mul(other.terms.len()))?;
        let mut result = Self { terms: Vec::new() };
        let mut outcome = Ok(());
        let mut left_index = 0;
        while outcome.is_ok() && left_index < self.terms.len() {
            outcome = result.add_products(&self.terms[left_index], other, work);
            left_index += 1;
        }
        outcome?;
        Ok(result)
    }

    /// Insert the product of `left` with each term of `other`, in order.
    fn add_products(
        &mut self,
        left: &Monomial,
        other: &Self,
        work: &mut u32,
    ) -> Result<(), Refusal> {
        let mut outcome = Ok(());
        let mut right_index = 0;
        while outcome.is_ok() && right_index < other.terms.len() {
            let right = &other.terms[right_index];
            outcome = match product_powers(left, right) {
                Ok(powers) => self.insert(
                    Monomial {
                        coefficient: left.coefficient.wrapping_mul(right.coefficient),
                        powers,
                    },
                    work,
                ),
                Err(refusal) => Err(refusal),
            };
            right_index += 1;
        }
        outcome
    }
}

/// The exponents of the product of two monomials, within the degree bound.
fn product_powers(left: &Monomial, right: &Monomial) -> Result<[u8; MAX_ALIAS_INPUTS], Refusal> {
    let mut powers = [0; MAX_ALIAS_INPUTS];
    let mut degree = 0u16;
    let mut failure = None;
    let mut index = 0;
    while failure.is_none() && index < powers.len() {
        match product_exponent(left.powers[index], right.powers[index], &mut degree) {
            Ok(exponent) => powers[index] = exponent,
            Err(refusal) => failure = Some(refusal),
        }
        index += 1;
    }
    match failure {
        Some(refusal) => Err(refusal),
        None => Ok(powers),
    }
}

/// One exponent of [`product_powers`], added to the running total `degree`.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; exponent overflow and the degree bound depend on the submitted recipe and are refused as typed `exhausted` refusals, not asserted"
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the two exponents are only added, so swapping them cannot change the result"
)]
fn product_exponent(left: u8, right: u8, degree: &mut u16) -> Result<u8, Refusal> {
    let next = u16::from(left)
        .checked_add(u16::from(right))
        .ok_or_else(|| refused(RefusalKind::Exhausted, "alias exponent overflows"))?;
    *degree = degree
        .checked_add(next)
        .ok_or_else(|| refused(RefusalKind::Exhausted, "alias total degree overflows"))?;
    if next > MAX_ALIAS_DEGREE || *degree > MAX_ALIAS_DEGREE {
        return Err(refused(
            RefusalKind::Exhausted,
            "alias polynomial exceeds the degree bound",
        ));
    }
    u8::try_from(next).map_err(|_| {
        refused(
            RefusalKind::Exhausted,
            "alias exponent overflows the wire domain",
        )
    })
}

struct PolynomialStack {
    values: Vec<Polynomial>,
}

impl PolynomialStack {
    fn new(input_count: usize) -> Self {
        Self {
            values: Vec::with_capacity(input_count),
        }
    }

    fn pop(&mut self) -> Result<Polynomial, Refusal> {
        self.values
            .pop()
            .ok_or_else(|| refused(RefusalKind::Unsupported, "alias symbolic stack underflow"))
    }

    fn push(&mut self, value: Polynomial) -> Result<(), Refusal> {
        if self.values.len() >= 32 {
            return Err(refused(
                RefusalKind::Exhausted,
                "alias symbolic stack exceeds the profile bound",
            ));
        }
        self.values.push(value);
        Ok(())
    }
}

fn check_fragment(record: &ProcedureRecord) -> Result<(), Refusal> {
    let chunk = &record.chunk;
    if !record.library_dependencies.is_empty() || !chunk.dependencies.is_empty() {
        return Err(refused(
            RefusalKind::Unsupported,
            "dependency-bearing aliases require a verified expansion model",
        ));
    }
    if chunk.interface.stack_in.len() > MAX_ALIAS_INPUTS
        || chunk.interface.stack_out.len() > MAX_ALIAS_OUTPUTS
    {
        return Err(refused(
            RefusalKind::Unsupported,
            "alias interface exceeds the polynomial input or output limit",
        ));
    }
    if crate::scan::find_first(&chunk.interface.stack_in, |ty| *ty != Ty::I64).is_some()
        || crate::scan::find_first(&chunk.interface.stack_out, |ty| *ty != Ty::I64).is_some()
    {
        return Err(refused(
            RefusalKind::Unsupported,
            "alias interface is not entirely I64",
        ));
    }
    if chunk.body.entry.len() > 64 {
        return Err(refused(
            RefusalKind::Unsupported,
            "alias recipe exceeds the bounded polynomial fragment",
        ));
    }
    debug_assert!(chunk.interface.stack_in.len() <= MAX_ALIAS_INPUTS);
    Ok(())
}

fn normalize_op(
    op: chunks::Op,
    stack: &mut PolynomialStack,
    work: &mut u32,
) -> Result<(), Refusal> {
    debug_assert!(stack.values.len() <= 32);
    match op {
        chunks::Op::Dup => {
            let top = stack.values.last().cloned().ok_or_else(|| {
                refused(RefusalKind::Unsupported, "alias duplicate needs a value")
            })?;
            stack.push(top)
        }
        chunks::Op::Drop => {
            let _ = stack.pop()?;
            Ok(())
        }
        chunks::Op::Swap => {
            let top = stack.pop()?;
            let below = stack.pop()?;
            stack.push(top)?;
            stack.push(below)
        }
        chunks::Op::Add | chunks::Op::Sub | chunks::Op::Mul => {
            let right = stack.pop()?;
            let left = stack.pop()?;
            let result = if op == chunks::Op::Mul {
                left.multiply(&right, work)?
            } else {
                left.combine(&right, op == chunks::Op::Sub, work)?
            };
            stack.push(result)
        }
        chunks::Op::Eq | chunks::Op::Run => Err(refused(
            RefusalKind::Unsupported,
            "Boolean or quotation alias operation is outside the polynomial fragment",
        )),
    }
}

fn normalize_node(
    chunk: &chunks::Envelope,
    index: u32,
    stack: &mut PolynomialStack,
    work: &mut u32,
) -> Result<(), Refusal> {
    debug_assert!(stack.values.len() <= 32);
    charge(work, 1)?;
    let node_index = usize::try_from(index).map_err(|_| {
        refused(
            RefusalKind::Unsupported,
            "alias node index exceeds the host domain",
        )
    })?;
    let node = chunk.body.nodes.get(node_index).ok_or_else(|| {
        refused(
            RefusalKind::Unsupported,
            "alias references a missing recipe node",
        )
    })?;
    match node {
        chunks::Node::I64(value) => stack.push(Polynomial::constant(*value as u64)),
        chunks::Node::Capture { index } => {
            let capture = chunk.captures.get(usize::from(*index)).ok_or_else(|| {
                refused(
                    RefusalKind::Unsupported,
                    "alias references a missing capture",
                )
            })?;
            let chunks::Scalar::I64(value) = capture.value else {
                return Err(refused(RefusalKind::Unsupported, "Boolean alias capture"));
            };
            stack.push(Polynomial::constant(value as u64))
        }
        chunks::Node::Operation { op, instantiation } => {
            if chunk
                .instantiations
                .get(usize::from(*instantiation))
                .is_none()
            {
                return Err(refused(
                    RefusalKind::Unsupported,
                    "alias references a missing instantiation witness",
                ));
            }
            normalize_op(*op, stack, work)
        }
        chunks::Node::Bool(_) | chunks::Node::Invoke { .. } | chunks::Node::Quotation { .. } => {
            Err(refused(
                RefusalKind::Unsupported,
                "alias recipe node is outside the I64 polynomial fragment",
            ))
        }
    }
}

fn normalize(record: &ProcedureRecord) -> Result<Vec<Polynomial>, Refusal> {
    check_fragment(record)?;
    let chunk = &record.chunk;
    let mut stack = PolynomialStack::new(chunk.interface.stack_in.len());
    let mut outcome = Ok(());
    let mut index = 0;
    while outcome.is_ok() && index < chunk.interface.stack_in.len() {
        outcome = stack.push(Polynomial::variable(index));
        index += 1;
    }
    outcome?;
    let mut work = 0;
    let mut outcome = Ok(());
    let mut position = 0;
    while outcome.is_ok() && position < chunk.body.entry.len() {
        outcome = normalize_node(chunk, chunk.body.entry[position], &mut stack, &mut work);
        debug_assert!(outcome.is_err() || stack.values.len() <= 32);
        position += 1;
    }
    outcome?;
    if stack.values.len() != chunk.interface.stack_out.len() {
        return Err(refused(
            RefusalKind::Unsupported,
            "alias symbolic output does not match the declared stack",
        ));
    }
    Ok(stack.values)
}

impl Alias {
    /// Produce a claim from two immutable subjects. This helper never
    /// authorizes retention: admission recomputes both forms and both closure
    /// digests against the actual retained target on each rebuild and reload.
    pub fn certify(left: &ProcedureRecord, right: &ProcedureRecord) -> Result<Self, Refusal> {
        let left_form = normalize(left)?;
        let right_form = normalize(right)?;
        if left.procedure_id == right.procedure_id || left.chunk.interface != right.chunk.interface
        {
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
            certificate: Certificate::ExactPair(Box::new(ExactPairCertificate {
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
        const HEX: [u8; 16] = *b"0123456789abcdef";
        let mut claim = String::with_capacity(CLAIM_CAPACITY_BYTES);
        claim.push_str("exact-pair:");
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
    let mut outcome = Ok(());
    let mut index = 0;
    while outcome.is_ok() && index < records.len() {
        outcome = verify_record(records, &records[index]);
        index += 1;
    }
    outcome
}

/// One record of [`verify`]: its exact-pair claim, if any, against the retained target.
fn verify_record(records: &[ProcedureRecord], record: &ProcedureRecord) -> Result<(), Refusal> {
    let Some(alias) = &record.alias_of else {
        return Ok(());
    };
    let Certificate::ExactPair(certificate) = &alias.certificate else {
        return Err(refused(
            RefusalKind::Unsupported,
            "a legacy alias digest is not an exact-pair proof",
        ));
    };
    let target = crate::scan::find_first(records, |candidate| {
        candidate.procedure_id == alias.procedure_id
    })
    .ok_or_else(|| refused(RefusalKind::StaleContext, "alias target is not retained"))?;
    verify_pair(record, alias, target, certificate)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; a missing, altered or repeated own exact-pair claim is submitted record content refused as a typed `invalid` refusal, not an internal invariant"
)]
fn verify_evidence(record: &ProcedureRecord, alias: &Alias) -> Result<(), Refusal> {
    let evidence = alias.evidence(record.procedure_id);
    let mut first_own = None;
    let mut is_repeated = false;
    let mut index = 0;
    while !is_repeated && index < record.evidence.len() {
        if record.evidence[index].obligation == ALIAS_EVIDENCE_OBLIGATION {
            if first_own.is_none() {
                first_own = Some(index);
            } else {
                is_repeated = true;
            }
        }
        index += 1;
    }
    let is_bound = match first_own {
        Some(at) => record.evidence[at] == evidence,
        None => false,
    };
    if !is_bound || is_repeated {
        return Err(refused(
            RefusalKind::Invalid,
            "alias lacks its own exact-pair validation evidence binding",
        ));
    }
    Ok(())
}

fn verify_pair(
    record: &ProcedureRecord,
    alias: &Alias,
    target: &ProcedureRecord,
    certificate: &ExactPairCertificate,
) -> Result<(), Refusal> {
    if record.procedure_id == target.procedure_id
        || record.chunk.interface != target.chunk.interface
    {
        return Err(refused(
            RefusalKind::Invalid,
            "alias pair has unequal identities or stack interfaces",
        ));
    }
    verify_evidence(record, alias)?;
    // The only accepted closure is exactly empty on both subjects. A
    // digest can never substitute for the dependency inspection in normalize.
    let left = normalize(record)?;
    let right = normalize(target)?;
    let empty_closure = persistence::empty_alias_closure_digest();
    if certificate.left_closure != empty_closure || certificate.right_closure != empty_closure {
        return Err(refused(
            RefusalKind::Invalid,
            "alias certificate closure differs from independently checked empty closures",
        ));
    }
    if certificate.left != left || certificate.right != right {
        return Err(refused(
            RefusalKind::Invalid,
            "alias certificate normal forms differ from recomputation",
        ));
    }
    if left != right {
        return Err(refused(
            RefusalKind::Invalid,
            "alias pair has unequal polynomials",
        ));
    }
    debug_assert_eq!(certificate.left, certificate.right);
    Ok(())
}
