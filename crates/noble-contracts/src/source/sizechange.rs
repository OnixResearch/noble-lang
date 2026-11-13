//! Opt-in size-change termination evidence for signature-checked recursive definitions
//! (issue #101).
//!
//! The specification and its proof live in `proofs/m3/NobleM2/SizeChange*.lean`. This module is the
//! front end that builds the model's call graphs from a checked definition: it classifies what each
//! self-call site passes for each declared measure, decides the shared-measure criterion
//! (`certifiedAll`), and on refusal names, for every measure, the first site that does not decrease
//! it — the witness `certifiedAll_refusal_iff` proves is complete.
//!
//! ## What is analyzed
//!
//! A definition whose declared signature is one polymorphic stack prefix followed by its measures:
//! `S I64 -- S I64`, `S (List I64) -- S I64`, `S I64 (List I64) -- S I64`, and so on. Each concrete
//! position is a measure of one of two kinds:
//!
//! - `Counter`: a wrapping `I64`. A successor is not a decrease, and `n - 1` is a decrease only
//!   under an active `0 < n` guard at the site, because wrapping two's-complement subtraction can
//!   raise `i64::MIN - 1`.
//! - `Structural`: a `List` value. The cons branch of `list.case` gives its tail, which is strictly
//!   shorter, so a tail argument is a decrease with no guard.
//!
//! The walk is a symbolic stack machine over the checked body. Values are tracked only as far as
//! the classifier needs — the measure itself, its predecessor, a list tail, a `0 <` comparison —
//! and everything else is opaque. Guards are the `0 <` comparisons in force along the branch that
//! reaches the call site. Prefix rows of a stack prefix are never inspected: a word that would
//! consume or rearrange the prefix is refused, not modelled.
//!
//! ## Soundness of the verdict
//!
//! `Accepted` is a Lean-checked certificate about the classified relations, not a theorem about an
//! execution: the model proves that a certified family admits no infinite run *that respects those
//! relations*, and the guarded run model supplies the positivity premise. Every path this module
//! cannot model becomes `Unsupported` or `Opaque`, never an assumed decrease, so the verdict never
//! claims more than the classifier observed. A refusal means **not shown terminating**, never
//! "diverges".
//!
//! ## What is not done here
//!
//! The general size-change closure (composed graphs over paths) is not decided; the bounded
//! shared-measure criterion is. The correspondence between `List` structure and its length, and
//! between a wrapped `I64` counter and a natural number, is the front end's obligation and is
//! documented in the lane (`verification/size-change`).

/// The rectangular zero-offset of one `Target` in the checked environment.
const MAX_MEASURES: usize = 8;
/// The most self-call sites one definition's report carries.
const MAX_SITES: usize = 256;
/// The most stack cells the symbolic walk tracks; a checked body needs far fewer.
const MAX_FRAME: usize = 64;
/// The most word steps one definition's walk spends before refusing.
const MAX_STEPS: u32 = 65_536;
/// The most nested quotation runs one walk descends through.
const MAX_DEPTH: u32 = 32;

/// One declared measure's shape, from the definition's signature.
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// A wrapping `I64` counter: a predecessor decreases it only under `0 <` at the site.
    Counter,
    /// A `List` value: its cons tail is structurally shorter.
    Structural,
}

impl Measure {
    /// The spelling the report uses.
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Counter => "counter",
            Self::Structural => "structural",
        }
    }
}

/// What one self-call site passes for one measure.
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Argument {
    /// The measure itself, unchanged.
    Same,
    /// The measure's predecessor, under the site's positivity guard.
    Predecessor,
    /// The structural tail of a list measure.
    Tail,
    /// Anything the bounded classifier cannot classify.
    Opaque,
}

impl Argument {
    /// The spelling the report uses.
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Same => "same",
            Self::Predecessor => "predecessor",
            Self::Tail => "tail",
            Self::Opaque => "opaque",
        }
    }

    /// The `NobleM2.SizeChange.Arg` constructor this class certifies.
    pub const fn constructor(self) -> &'static str {
        match self {
            Self::Same => "same",
            Self::Predecessor | Self::Tail => "predOfSame",
            Self::Opaque => "opaque",
        }
    }
}

/// One self-call site the checker classified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    /// The site's source span.
    pub span: crate::Span,
    /// What the site passes for each measure, in declaration order.
    pub arguments: alloc::vec::Vec<Argument>,
    /// Whether a `0 <` guard on each measure is in force at the site.
    pub guarded: alloc::vec::Vec<bool>,
}

/// The reason a definition is outside the bounded classifier.
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsupported {
    /// The declared signature is not a stack prefix followed by `I64`/`List` measures.
    SignatureOutsideProfile,
    /// The walk would consume or rearrange the declared stack prefix.
    PrefixTouched,
    /// A word form the symbolic walk does not model.
    UnhandledWord,
    /// A named helper call, quotation or call whose behavior is not modelled.
    UnmodelledCall,
    /// A self call whose current frame is not the declared argument list.
    SelfArityMismatch,
    /// More sites, measures or stack cells than the bounded walk tracks.
    BudgetExceeded,
    /// The body's final frame does not match the declared result.
    FinalFrameMismatch,
    /// The definition declares no self call, so there is nothing to classify.
    NoSelfCall,
}

impl Unsupported {
    /// The spelling the report uses.
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::SignatureOutsideProfile => "signature-outside-profile",
            Self::PrefixTouched => "prefix-touched",
            Self::UnhandledWord => "unhandled-word",
            Self::UnmodelledCall => "unmodelled-call",
            Self::SelfArityMismatch => "self-arity-mismatch",
            Self::BudgetExceeded => "budget-exceeded",
            Self::FinalFrameMismatch => "final-frame-mismatch",
            Self::NoSelfCall => "no-self-call",
        }
    }
}

/// A measure a refusal reports, with the first site that does not decrease it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Witness {
    /// The measure index that fails.
    pub measure: u32,
    /// The site index (into [`Evidence::sites`]) that fails it.
    pub site: u32,
    /// What that site passes for the measure.
    pub argument: Argument,
}

/// The checker's verdict.
#[octet::sealed_enum]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// One measure decreases at every site.
    Accepted {
        /// The shared measure index.
        measure: u32,
    },
    /// No measure decreases at every site; each measure's failing site is named.
    Refused {
        /// One witness per measure, in declaration order.
        witnesses: alloc::vec::Vec<Witness>,
    },
    /// The definition is outside the bounded classifier; no claim is made either way.
    Unsupported {
        /// Why the walk stopped.
        reason: Unsupported,
        /// Where it stopped.
        span: crate::Span,
    },
}

/// The evidence one recursive definition carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evidence {
    /// The definition's name.
    pub definition: alloc::string::String,
    /// The declared measures, in signature order.
    pub measures: alloc::vec::Vec<Measure>,
    /// The self-call sites, in body order.
    pub sites: alloc::vec::Vec<Site>,
    /// The verdict.
    pub verdict: Verdict,
}

/// One symbolic stack value: only what the classifier needs is tracked.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    /// The declared stack prefix, as one opaque cell below the concrete frame.
    Prefix,
    /// The entry value of measure `j`, unchanged.
    Formal(u32),
    /// The predecessor of measure `j`.
    Predecessor(u32),
    /// The structural tail of list measure `j`.
    Tail(u32),
    /// The literal `0`.
    Zero,
    /// The literal `1`.
    One,
    /// A boolean that is the comparison `0 < formal (j)`.
    Positive(u32),
    /// A boolean of any other provenance.
    OtherBoolean,
    /// A quotation the walk may run, by node identity.
    Program(u32),
    /// Anything else.
    Other,
}

/// One word step's outcome: the walk either continues or records its halt.
type Step = Result<(), ()>;

#[derive(Clone, Copy)]
struct ProgramRun {
    node: u32,
    depth: u32,
}

#[derive(Clone, Copy)]
struct BranchRun {
    node: u32,
    guard: u32,
    depth: u32,
}

#[derive(Clone, Copy)]
struct BuiltinRun {
    identity: u32,
    depth: u32,
}

#[derive(Clone, Copy)]
struct StackEffect {
    inputs: usize,
    outputs: usize,
}

/// The bounded symbolic walk over one checked body.
struct Walker<'a> {
    tree: &'a crate::source::Tree,
    environment: &'a noble_kernel::contracts::Env,
    measures: &'a [Measure],
    /// Concrete cells the declared result frame carries.
    result_frame: usize,
    stack: alloc::vec::Vec<Value>,
    guard: u32,
    sites: alloc::vec::Vec<Site>,
    halt: Option<(Unsupported, crate::Span)>,
    steps: u32,
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; Walker is created per definition and never escapes evidence(). Its mutations are confined to its owned symbolic stack, guard, and site buffers; no external state or caller-owned program is changed, and every budget/profile refusal is returned as Unsupported."
)]
impl<'a> Walker<'a> {
    /// Record the first halt and stop the walk.
    const fn fail<T>(&mut self, reason: Unsupported, span: crate::Span) -> Result<T, ()> {
        if self.halt.is_none() {
            self.halt = Some((reason, span));
        }
        Err(())
    }

    fn push(&mut self, value: Value, span: crate::Span) -> Step {
        if self.stack.len() >= MAX_FRAME {
            return self.fail(Unsupported::BudgetExceeded, span);
        }
        self.stack.push(value);
        Ok(())
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; pop removes one runtime Vec stack cell, and the pinned Rust 1.96 toolchain does not provide const Vec::pop. Replacing the checked operation with manual indexing would weaken the stack boundary."
    )]
    fn pop(&mut self, span: crate::Span) -> Result<Value, ()> {
        if self.stack.len() <= 1 {
            return self.fail(Unsupported::PrefixTouched, span);
        }
        if matches!(self.stack.as_slice().last(), Some(Value::Prefix)) {
            return self.fail(Unsupported::PrefixTouched, span);
        }
        match self.stack.pop() {
            Some(value) => Ok(value),
            None => self.fail(Unsupported::PrefixTouched, span),
        }
    }

    /// Pop a program value the walk can run, or refuse.
    fn pop_program(&mut self, span: crate::Span) -> Result<u32, ()> {
        match self.pop(span)? {
            Value::Program(node) => Ok(node),
            _ => self.fail(Unsupported::UnmodelledCall, span),
        }
    }

    /// Run one program value on the current stack.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; program follows a dynamically parsed source tree through Tree::node and recursively executes its checked quotation nodes; that runtime lookup is not const-evaluable."
    )]
    fn program(&mut self, invocation: ProgramRun) -> Step {
        if invocation.depth >= MAX_DEPTH {
            return self.fail(Unsupported::BudgetExceeded, self.tree.span);
        }
        let quotation = match self.tree.node(invocation.node) {
            Ok(quotation) => quotation,
            Err(_) => return self.fail(Unsupported::UnhandledWord, self.tree.span),
        };
        match &quotation.kind {
            crate::source::Kind::Quotation(items) => {
                self.sequence(items, invocation.depth.saturating_add(1))
            }
            _ => self.fail(Unsupported::UnhandledWord, quotation.span),
        }
    }

    /// Run a word sequence on the current stack.
    fn sequence(&mut self, items: &'a [u32], depth: u32) -> Step {
        for node in items {
            self.word(ProgramRun { node: *node, depth })?;
        }
        Ok(())
    }

    /// Run one branch from `base` under an explicit guard, restoring the caller's stack and guard.
    fn branch(
        &mut self,
        base: &[Value],
        invocation: BranchRun,
    ) -> Result<alloc::vec::Vec<Value>, ()> {
        let saved_guard = self.guard;
        self.stack = base.to_vec();
        self.guard = invocation.guard;
        let outcome = self.program(ProgramRun {
            node: invocation.node,
            depth: invocation.depth.saturating_add(1),
        });
        let produced = core::mem::take(&mut self.stack);
        self.guard = saved_guard;
        outcome?;
        Ok(produced)
    }

    /// Record one self-call site and replace the frame with the declared result.
    fn self_call(&mut self, span: crate::Span) -> Step {
        let measures = self.measures;
        if !matches!(self.stack.first(), Some(Value::Prefix)) {
            return self.fail(Unsupported::PrefixTouched, span);
        }
        if self.stack.len() != measures.len().saturating_add(1) {
            return self.fail(Unsupported::SelfArityMismatch, span);
        }
        if self.sites.len() >= MAX_SITES {
            return self.fail(Unsupported::BudgetExceeded, span);
        }
        let mut arguments = alloc::vec::Vec::with_capacity(measures.len());
        let mut guarded = alloc::vec::Vec::with_capacity(measures.len());
        for (index, measure) in measures.iter().enumerate() {
            let is_guarded = self.guard & Self::measure_bit(index) != 0;
            let value = match self.stack.get(index.saturating_add(1)) {
                Some(value) => value,
                None => return self.fail(Unsupported::SelfArityMismatch, span),
            };
            let argument = classify(*measure, index, value, is_guarded);
            arguments.push(argument);
            guarded.push(is_guarded);
        }
        self.sites.push(Site {
            span,
            arguments,
            guarded,
        });
        self.stack.truncate(1);
        for _ in 0..self.result_frame {
            self.push(Value::Other, span)?;
        }
        Ok(())
    }

    /// The guard bit of one measure index.
    const fn measure_bit(index: usize) -> u32 {
        1_u32 << (index & 31)
    }

    /// The value of one measure index as a `u32` index, if it is in range.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; this boundary converts a source u32 index to target-width usize with TryFrom and must remain checked on 16/32/64-bit targets rather than using a truncating cast."
    )]
    fn measure_index(candidate: u32, bound: usize) -> Option<usize> {
        usize::try_from(candidate)
            .ok()
            .filter(|value| *value < bound)
    }

    /// One word.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; word resolves a runtime source node from the checked tree and updates the bounded, allocation-backed symbolic stack; const evaluation is not a valid execution mode."
    )]
    fn word(&mut self, invocation: ProgramRun) -> Step {
        if self.steps >= MAX_STEPS {
            return self.fail(Unsupported::BudgetExceeded, self.tree.span);
        }
        self.steps = self.steps.saturating_add(1);
        let node = match self.tree.node(invocation.node) {
            Ok(node) => node,
            Err(_) => return self.fail(Unsupported::UnhandledWord, self.tree.span),
        };
        let span = node.span;
        match &node.kind {
            crate::source::Kind::Literal(literal) => self.literal(*literal, span),
            crate::source::Kind::Text(_) => self.push(Value::Other, span),
            crate::source::Kind::Quotation(_) => self.push(Value::Program(invocation.node), span),
            crate::source::Kind::Word(_) | crate::source::Kind::EditorHole => {
                self.fail(Unsupported::UnhandledWord, span)
            }
            crate::source::Kind::Call(crate::source::Target::SelfOwner) => self.self_call(span),
            crate::source::Kind::Call(crate::source::Target::Builtin(identity)) => self.builtin(
                BuiltinRun {
                    identity: *identity,
                    depth: invocation.depth,
                },
                span,
            ),
            crate::source::Kind::Call(crate::source::Target::Named(_)) => {
                self.fail(Unsupported::UnmodelledCall, span)
            }
            crate::source::Kind::Call(crate::source::Target::SlotInvoke(_)) => {
                self.fail(Unsupported::UnmodelledCall, span)
            }
        }
    }

    /// One literal value.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; literal updates the runtime Vec-backed symbolic stack; its bounded allocation state is part of the per-definition interpreter, not a const computation."
    )]
    fn literal(&mut self, literal: noble_kernel::untrusted::Lit, span: crate::Span) -> Step {
        match literal {
            noble_kernel::untrusted::Lit::I64(0) => self.push(Value::Zero, span),
            noble_kernel::untrusted::Lit::I64(1) => self.push(Value::One, span),
            noble_kernel::untrusted::Lit::I64(_)
            | noble_kernel::untrusted::Lit::Bool(_)
            | noble_kernel::untrusted::Lit::Text
            | noble_kernel::untrusted::Lit::Unit => self.push(Value::Other, span),
        }
    }

    /// One bootstrap or declared-text word.
    fn builtin(&mut self, invocation: BuiltinRun, span: crate::Span) -> Step {
        match invocation.identity {
            0 => self.duplicate(span),
            1 => self.pop(span).map(|_| ()),
            2 => self.swap(span),
            3 => self.dip(span, invocation.depth),
            4..=6 => self.arithmetic(invocation.identity, span),
            7 => self.compare_equals(span),
            9 => self.fail(Unsupported::UnmodelledCall, span),
            10 => {
                let program = self.pop_program(span)?;
                self.program(ProgramRun {
                    node: program,
                    depth: invocation.depth.saturating_add(1),
                })
            }
            17 => self.branch_both(span, invocation.depth),
            18 => self.conditional(span, invocation.depth),
            19 => self.push(Value::Other, span),
            20 => self.cons(span),
            21 => self.list_case(span, invocation.depth),
            27 => self.compare_less(span),
            _ => self.generic(
                self.environment
                    .scheme(noble_kernel::contracts::Definition(invocation.identity)),
                span,
            ),
        }
    }

    /// A word whose stack effect is read from its scheme: consume the declared frame, produce
    /// opaque results.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; generic derives a stack effect from a runtime scheme and mutates the Vec-backed symbolic stack, so it is an interpreter step rather than a const helper."
    )]
    fn generic(&mut self, scheme: Option<&noble_kernel::words::Scheme>, span: crate::Span) -> Step {
        let (inputs, outputs) = match scheme.and_then(split_frame) {
            Some(frame) => frame,
            None => return self.fail(Unsupported::UnmodelledCall, span),
        };
        self.consume(StackEffect { inputs, outputs }, span)
    }

    /// Consume `inputs` concrete cells and produce `outputs` opaque ones.
    fn consume(&mut self, effect: StackEffect, span: crate::Span) -> Step {
        for _ in 0..effect.inputs {
            self.pop(span)?;
        }
        for _ in 0..effect.outputs {
            self.push(Value::Other, span)?;
        }
        Ok(())
    }

    /// `dup`.
    fn duplicate(&mut self, span: crate::Span) -> Step {
        let value = self.pop(span)?;
        self.push(value.clone(), span)?;
        self.push(value, span)
    }

    /// `swap`.
    fn swap(&mut self, span: crate::Span) -> Step {
        let second = self.pop(span)?;
        let first = self.pop(span)?;
        self.push(second, span)?;
        self.push(first, span)
    }

    /// `dip`: run one program under the top value, then restore it.
    fn dip(&mut self, span: crate::Span, depth: u32) -> Step {
        let program = self.pop_program(span)?;
        let value = self.pop(span)?;
        self.program(ProgramRun {
            node: program,
            depth: depth.saturating_add(1),
        })?;
        self.push(value, span)
    }

    /// `+`, `-` and `*`.
    fn arithmetic(&mut self, identity: u32, span: crate::Span) -> Step {
        let right = self.pop(span)?;
        let left = self.pop(span)?;
        let value = if identity == 5 {
            predecessor(self.measures, left, right)
        } else {
            Value::Other
        };
        self.push(value, span)
    }

    /// `=`.
    fn compare_equals(&mut self, span: crate::Span) -> Step {
        self.pop(span)?;
        self.pop(span)?;
        self.push(Value::OtherBoolean, span)
    }

    /// `<`: the signed comparison the declared-text profile adds. `0 < n` is the positivity guard.
    fn compare_less(&mut self, span: crate::Span) -> Step {
        let upper = self.pop(span)?;
        let lower = self.pop(span)?;
        let value = match (&lower, &upper) {
            (Value::Zero, Value::Formal(index))
                if Self::measure_index(*index, self.measures.len()).is_some() =>
            {
                Value::Positive(*index)
            }
            _ => Value::OtherBoolean,
        };
        self.push(value, span)
    }

    /// `cons`.
    fn cons(&mut self, span: crate::Span) -> Step {
        self.pop(span)?;
        self.pop(span)?;
        self.push(Value::Other, span)
    }

    /// `if`: the positive guard holds only in the taken branch.
    fn conditional(&mut self, span: crate::Span, depth: u32) -> Step {
        let alternative = self.pop_program(span)?;
        let consequent = self.pop_program(span)?;
        let condition = self.pop(span)?;
        let outer_guard = self.guard;
        let guarded = match condition {
            Value::Positive(index) => match Self::measure_index(index, self.measures.len()) {
                Some(index) => outer_guard | Self::measure_bit(index),
                None => outer_guard,
            },
            _ => outer_guard,
        };
        let base = self.stack.clone();
        let taken = self.branch(
            &base,
            BranchRun {
                node: consequent,
                guard: guarded,
                depth,
            },
        )?;
        let other = self.branch(
            &base,
            BranchRun {
                node: alternative,
                guard: outer_guard,
                depth,
            },
        )?;
        self.join_branches(taken, other, span)
    }

    /// `case`: either constructor is possible, so both payloads are opaque and both branches are
    /// joined without a positivity guard.
    fn branch_both(&mut self, span: crate::Span, depth: u32) -> Step {
        let right = self.pop_program(span)?;
        let left = self.pop_program(span)?;
        self.pop(span)?;
        let guard = self.guard;
        let mut payload = self.stack.clone();
        payload.push(Value::Other);
        let first = self.branch(
            &payload,
            BranchRun {
                node: left,
                guard,
                depth,
            },
        )?;
        let second = self.branch(
            &payload,
            BranchRun {
                node: right,
                guard,
                depth,
            },
        )?;
        self.join_branches(first, second, span)
    }

    /// `list.case`: the cons branch receives the structural tail of a list measure.
    fn list_case(&mut self, span: crate::Span, depth: u32) -> Step {
        let cons = self.pop_program(span)?;
        let nil = self.pop_program(span)?;
        let list = self.pop(span)?;
        let tail = match &list {
            Value::Formal(index)
                if Self::measure_index(*index, self.measures.len())
                    .is_some_and(|index| self.measures[index] == Measure::Structural) =>
            {
                Value::Tail(*index)
            }
            _ => Value::Other,
        };
        let base = self.stack.clone();
        let guard = self.guard;
        let mut extended = base.clone();
        extended.push(Value::Other);
        extended.push(tail);
        let first = self.branch(
            &base,
            BranchRun {
                node: nil,
                guard,
                depth,
            },
        )?;
        let second = self.branch(
            &extended,
            BranchRun {
                node: cons,
                guard,
                depth,
            },
        )?;
        self.join_branches(first, second, span)
    }

    /// Join two branch results; a cell that differs becomes opaque.
    fn join_branches(
        &mut self,
        first: alloc::vec::Vec<Value>,
        second: alloc::vec::Vec<Value>,
        span: crate::Span,
    ) -> Step {
        if first.len() != second.len() {
            return self.fail(Unsupported::FinalFrameMismatch, span);
        }
        let mut joined = alloc::vec::Vec::with_capacity(first.len());
        for (left, right) in first.into_iter().zip(second) {
            joined.push(if left == right { left } else { Value::Other });
        }
        self.stack = joined;
        Ok(())
    }
}

/// What one self-call site passes for one measure.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; classify safely converts a platform-width measure index into the model's u32 index; the checked TryFrom boundary must remain portable rather than becoming a cast."
)]
fn classify(measure: Measure, index: usize, value: &Value, guarded: bool) -> Argument {
    let candidate = match u32::try_from(index) {
        Ok(candidate) => candidate,
        Err(_) => return Argument::Opaque,
    };
    match (measure, value) {
        (_, Value::Formal(formal)) if *formal == candidate => Argument::Same,
        (Measure::Counter, Value::Predecessor(predecessor)) if *predecessor == candidate => {
            if guarded {
                Argument::Predecessor
            } else {
                Argument::Opaque
            }
        }
        (Measure::Structural, Value::Tail(tail)) if *tail == candidate => Argument::Tail,
        _ => Argument::Opaque,
    }
}

/// `n - 1` for a counter measure, tracked so a guarded site can classify it.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; predecessor resolves a runtime u32 measure index into a target-width declaration slice using checked conversion and lookup."
)]
fn predecessor(measures: &[Measure], left: Value, right: Value) -> Value {
    match (&left, &right) {
        (Value::Formal(index), Value::One)
            if usize::try_from(*index)
                .ok()
                .and_then(|index| measures.get(index))
                .is_some_and(|measure| *measure == Measure::Counter) =>
        {
            Value::Predecessor(*index)
        }
        _ => Value::Other,
    }
}

/// The concrete frame one scheme exposes: a single stack prefix followed by fixed positions.
fn split_frame(scheme: &noble_kernel::words::Scheme) -> Option<(usize, usize)> {
    if scheme.stack_in.first()? != scheme.stack_out.first()? {
        return None;
    }
    Some((
        concrete_positions(&scheme.stack_in)?,
        concrete_positions(&scheme.stack_out)?,
    ))
}

/// The number of fixed positions after a scheme stack's single prefix variable.
fn concrete_positions(stack: &[noble_kernel::shapes::Pattern]) -> Option<usize> {
    let (prefix, fixed) = stack.split_first()?;
    if !matches!(prefix, noble_kernel::shapes::Pattern::StackVar(_)) {
        return None;
    }
    if fixed
        .iter()
        .any(|pattern| matches!(pattern, noble_kernel::shapes::Pattern::StackVar(_)))
    {
        return None;
    }
    Some(fixed.len())
}

/// The measures and output frame one signature exposes, if it is in the bounded profile.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; measures_of is a fail-closed signature classifier: every unsupported prefix, stack shape, or measure returns None, and assertion panics would turn valid out-of-profile user programs into crashes."
)]
fn measures_of(scheme: &noble_kernel::words::Scheme) -> Option<(alloc::vec::Vec<Measure>, usize)> {
    let (prefix, fixed) = scheme.stack_in.split_first()?;
    if !matches!(prefix, noble_kernel::shapes::Pattern::StackVar(_)) {
        return None;
    }
    let result_frame = concrete_positions(&scheme.stack_out)?;
    if scheme.stack_in.first() != scheme.stack_out.first() {
        return None;
    }
    let mut measures = alloc::vec::Vec::with_capacity(fixed.len());
    for pattern in fixed {
        measures.push(match pattern {
            noble_kernel::shapes::Pattern::I64 => Measure::Counter,
            noble_kernel::shapes::Pattern::List(_) => Measure::Structural,
            _ => return None,
        });
    }
    if measures.is_empty() || measures.len() > MAX_MEASURES {
        return None;
    }
    Some((measures, result_frame))
}

/// Whether a definition's tree contains a lexical self call.
pub fn calls_self(definition: &crate::source::Named) -> bool {
    definition.tree.nodes.iter().any(|node| {
        matches!(
            node.kind,
            crate::source::Kind::Call(crate::source::Target::SelfOwner)
        )
    })
}

/// Build the size-change evidence for one checked definition.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; evidence() treats checked source signatures and bodies as bounded analysis inputs, returning an Unsupported verdict for out-of-profile forms and walk failures. Assertions would turn expected user-program refusals into panics."
)]
pub fn evidence(
    definition: &crate::source::Named,
    environment: &noble_kernel::contracts::Env,
) -> Evidence {
    let (measures, result_frame) = match definition.signature.as_ref().and_then(measures_of) {
        Some(profile) => profile,
        None => {
            return Evidence {
                definition: definition.name.clone(),
                measures: alloc::vec::Vec::new(),
                sites: alloc::vec::Vec::new(),
                verdict: Verdict::Unsupported {
                    reason: Unsupported::SignatureOutsideProfile,
                    span: definition.tree.span,
                },
            }
        }
    };
    let (halt, sites) = {
        let mut walker = Walker {
            tree: &definition.tree,
            environment,
            measures: &measures,
            result_frame,
            stack: alloc::vec::Vec::with_capacity(MAX_FRAME),
            guard: 0,
            sites: alloc::vec::Vec::new(),
            halt: None,
            steps: 0,
        };
        walker.stack.push(Value::Prefix);
        for index in 0..measures.len() {
            walker
                .stack
                .push(Value::Formal(u32::try_from(index).unwrap_or(0)));
        }
        let walked = walker.sequence(&definition.tree.body, 0);
        let halt = match walker.halt {
            Some(halt) => Some(halt),
            None => match walked {
                Err(()) => Some((Unsupported::UnhandledWord, definition.tree.span)),
                Ok(()) => final_shape(&walker),
            },
        };
        (halt, walker.sites)
    };
    match halt {
        Some((reason, span)) => Evidence {
            definition: definition.name.clone(),
            measures,
            sites,
            verdict: Verdict::Unsupported { reason, span },
        },
        None => verdict(definition, measures, sites),
    }
}

/// Check the walked final frame against the declared result.
const fn final_shape(walker: &Walker<'_>) -> Option<(Unsupported, crate::Span)> {
    if walker.sites.is_empty() {
        return Some((Unsupported::NoSelfCall, walker.tree.span));
    }
    if walker.stack.len() != walker.result_frame.saturating_add(1) {
        return Some((Unsupported::FinalFrameMismatch, walker.tree.span));
    }
    if !matches!(walker.stack.as_slice().first(), Some(Value::Prefix)) {
        return Some((Unsupported::PrefixTouched, walker.tree.span));
    }
    debug_assert!(!walker.measures.is_empty());
    None
}

/// Decide the shared-measure criterion and build the evidence.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; verdict allocates the refusal-witness vector and clones the definition name into the owned report, which cannot be a const operation."
)]
fn verdict(
    definition: &crate::source::Named,
    measures: alloc::vec::Vec<Measure>,
    sites: alloc::vec::Vec<Site>,
) -> Evidence {
    let accepted = shared_measure(&measures, &sites);
    let outcome = match accepted {
        Some(measure) => Verdict::Accepted { measure },
        None => Verdict::Refused {
            witnesses: refusal_witnesses(&measures, &sites),
        },
    };
    Evidence {
        definition: definition.name.clone(),
        measures,
        sites,
        verdict: outcome,
    }
}

/// The first measure that decreases at every site, mirroring `certifiedAll`.
fn shared_measure(measures: &[Measure], sites: &[Site]) -> Option<u32> {
    for index in 0..measures.len() {
        let is_everywhere = sites.iter().all(|site| {
            matches!(
                site.arguments.get(index),
                Some(Argument::Predecessor | Argument::Tail)
            )
        });
        if is_everywhere {
            return u32::try_from(index).ok();
        }
    }
    None
}

/// One witness per measure: the first site that does not decrease it.
fn refusal_witnesses(measures: &[Measure], sites: &[Site]) -> alloc::vec::Vec<Witness> {
    debug_assert!(!measures.is_empty());
    debug_assert!(!sites.is_empty());
    let mut witnesses = alloc::vec::Vec::with_capacity(measures.len());
    for index in 0..measures.len() {
        let failing = sites.iter().enumerate().find(|(_, site)| {
            !matches!(
                site.arguments.get(index),
                Some(Argument::Predecessor | Argument::Tail)
            )
        });
        if let Some((site, entry)) = failing {
            witnesses.push(Witness {
                measure: u32::try_from(index).unwrap_or(0),
                site: u32::try_from(site).unwrap_or(0),
                argument: entry
                    .arguments
                    .get(index)
                    .copied()
                    .unwrap_or(Argument::Opaque),
            });
        }
    }
    witnesses
}

/// Render the Lean certificate for the accepted definitions of one report.
///
/// Each accepted definition becomes concrete `Args` plus a `decide` proof of `certifiedAll` and of
/// the reported shared measure. Refused and unsupported definitions produce no theorem.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; certificate serializes already-classified report data into Lean source and deliberately emits no assertion about front-end input. The isolated Lean checker is the authority that accepts or rejects the generated theorem text."
)]
pub fn certificate(evidence: &[Evidence]) -> alloc::string::String {
    let mut output = alloc::string::String::with_capacity(4096);
    output.push_str(
        "/-\n  Generated size-change certificate (issue #101).\n\n  Each `Args` vector is the front end's classification of one self-call site; the theorems\n  re-check the model's own decision procedure (`certifiedAll`) by evaluation. This is evidence\n  about the classified relations, not about an execution: see `verification/size-change`.\n-/\nimport NobleM2.SizeChangeFixtures\n\nnamespace NobleSizeChangeCertificate\n\nopen NobleM2.SizeChange\n",
    );
    for (entry_index, entry) in evidence.iter().enumerate() {
        let accepted = match entry.verdict {
            Verdict::Accepted { measure } => measure,
            Verdict::Refused { .. } | Verdict::Unsupported { .. } => continue,
        };
        let accepted = match usize::try_from(accepted) {
            Ok(measure) => measure,
            Err(_) => continue,
        };
        let sites = entry.sites.len();
        let measures = entry.measures.len();
        let name = append_decimal(alloc::string::String::from("definition_"), entry_index);
        output.push_str("\n/-- Certified definition ");
        output = append_decimal(output, entry_index);
        output.push_str(": ");
        output = append_decimal(output, sites);
        output.push_str(" self-call site(s) over ");
        output = append_decimal(output, measures);
        output.push_str(" declared measure(s). -/\n");
        output.push_str("def ");
        output.push_str(&name);
        output.push_str("Args : Fin ");
        output = append_decimal(output, sites);
        output.push_str(" → Args ");
        output = append_decimal(output, measures);
        output.push_str(" := fun site measure =>\n  match site.val, measure.val with\n");
        for (site, row) in entry.sites.iter().enumerate() {
            for (measure, argument) in row.arguments.iter().enumerate() {
                output.push_str("  | ");
                output = append_decimal(output, site);
                output.push_str(", ");
                output = append_decimal(output, measure);
                output.push_str(" => .");
                output.push_str(argument.constructor());
                output.push('\n');
            }
        }
        output.push_str("  | _, _ => .opaque\n\n");
        output.push_str("def ");
        output.push_str(&name);
        output.push_str("Measure : Fin ");
        output = append_decimal(output, measures);
        output.push_str(" := ⟨");
        output = append_decimal(output, accepted);
        output.push_str(", by decide⟩\n\ntheorem ");
        output.push_str(&name);
        output.push_str("Certified : certifiedAll (fun site => graphOf (");
        output.push_str(&name);
        output.push_str("Args site)) = true := by\n  decide\n\ntheorem ");
        output.push_str(&name);
        output.push_str("Shared : ∀ site : Fin ");
        output = append_decimal(output, sites);
        output.push_str(", graphOf (");
        output.push_str(&name);
        output.push_str("Args site) ");
        output.push_str(&name);
        output.push_str("Measure = .decrease := by\n  decide\n");
    }
    output.push_str("\nend NobleSizeChangeCertificate");
    output
}

/// Append one decimal number without pulling in a formatting layer.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; append_decimal is a total serializer for usize values and returns before emitting a partial digit sequence if checked digit conversion or buffer bounds fail; serialization must not assert on its numeric input."
)]
fn append_decimal(mut output: alloc::string::String, mut value: usize) -> alloc::string::String {
    let mut digits = [0_u8; core::mem::size_of::<usize>().saturating_mul(3)];
    let mut at = digits.len();
    loop {
        at = match at.checked_sub(1) {
            Some(at) => at,
            None => return output,
        };
        let digit = match u8::try_from(value % 10) {
            Ok(digit) => digit,
            Err(_) => return output,
        };
        digits[at] = match b'0'.checked_add(digit) {
            Some(ascii) => ascii,
            None => return output,
        };
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for digit in &digits[at..] {
        output.push(char::from(*digit));
    }
    output
}
