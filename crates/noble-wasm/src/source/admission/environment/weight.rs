//! Calculator-only borrowed structural bounds before kernel-owned scheme clones.
//!
//! Every embedded nominal descriptor is charged at each occurrence, and
//! deeper ancestors are charged repeatedly for the kernel's owned marker,
//! substitution and equality walks. The ordinary/cursor admission meter is
//! unchanged. All arithmetic fails closed before a producer-owned tree is
//! cloned by kernel validation.

fn add(left: u64, right: u64) -> Result<u64, crate::Diagnostic> {
    left.checked_add(right).ok_or(crate::Diagnostic::Exhausted)
}

fn step(depth: u64) -> Result<u64, crate::Diagnostic> {
    let next = attempt!(add(depth, 1));
    if next > 64 {
        Err(crate::Diagnostic::Exhausted)
    } else {
        Ok(next)
    }
}

/// One pending step of the weighted walk. A node's charge, depth step,
/// effect entries and lookup count happen when it is visited; its children
/// are visited next, in order, and its checked sums run after them: the
/// order and amounts of the recursive definition (node weight = its depth
/// step plus its children's weights).
enum Task<'a> {
    /// Visit a type node at a depth.
    VisitType(&'a noble_kernel::types::Ty, u64),
    /// Visit a pattern node at a depth.
    VisitPattern(&'a noble_kernel::shapes::Pattern, u64),
    /// Fold a type stack's entries from an index on into its running total.
    TypesFrom(&'a [noble_kernel::types::Ty], usize, u64),
    /// Fold a pattern stack's entries from an index on into its running total.
    PatternsFrom(&'a [noble_kernel::shapes::Pattern], usize, u64),
    /// Start a stack's running total at zero.
    StartTotal,
    /// Replace the top two weights with their checked sum.
    AddTop,
    /// Add a fixed amount (a depth step or an effect count) to the top weight.
    AddAmount(u64),
}

/// The pending tasks, the weights of finished subtrees, and the nominal
/// descriptor lookups counted so far.
struct Walk<'a> {
    tasks: alloc::vec::Vec<Task<'a>>,
    weights: alloc::vec::Vec<u64>,
    lookups: u64,
}

impl<'a> Walk<'a> {
    fn run(
        &mut self,
        task: Task<'a>,
        work: &mut super::super::super::Work,
    ) -> Result<(), crate::Diagnostic> {
        match task {
            Task::VisitType(value, depth) => self.visit_type(value, depth, work),
            Task::VisitPattern(value, depth) => self.visit_pattern(value, depth, work),
            Task::TypesFrom(values, index, depth) => {
                self.types_from(values, index, depth);
                Ok(())
            }
            Task::PatternsFrom(values, index, depth) => {
                self.patterns_from(values, index, depth);
                Ok(())
            }
            Task::StartTotal => {
                self.weights.push(0);
                Ok(())
            }
            Task::AddTop => self.add_top(),
            Task::AddAmount(amount) => self.add_amount(amount),
        }
    }

    /// Charge and step one type node, then queue its children and sums.
    fn visit_type(
        &mut self,
        value: &'a noble_kernel::types::Ty,
        depth: u64,
        work: &mut super::super::super::Work,
    ) -> Result<(), crate::Diagnostic> {
        attempt!(work.charge(1));
        let next = attempt!(step(depth));
        match value {
            noble_kernel::types::Ty::Pair(left, right)
            | noble_kernel::types::Ty::Sum(left, right) => {
                self.tasks.push(Task::AddAmount(next));
                self.tasks.push(Task::AddTop);
                self.tasks.push(Task::VisitType(right, next));
                self.tasks.push(Task::VisitType(left, next));
            }
            noble_kernel::types::Ty::List(item) => {
                self.tasks.push(Task::AddAmount(next));
                self.tasks.push(Task::VisitType(item, next));
            }
            noble_kernel::types::Ty::Program(input, output, effects)
            | noble_kernel::types::Ty::LiveRef(input, output, effects) => {
                attempt!(work.entries(effects.as_slice().len()));
                self.tasks.push(Task::AddAmount(next));
                self.tasks
                    .push(Task::AddAmount(effects.as_slice().len() as u64));
                self.tasks.push(Task::AddTop);
                self.tasks.push(Task::TypesFrom(output, 0, next));
                self.tasks.push(Task::StartTotal);
                self.tasks.push(Task::TypesFrom(input, 0, next));
                self.tasks.push(Task::StartTotal);
            }
            noble_kernel::types::Ty::Nominal(_, descriptor) => {
                self.lookups = attempt!(add(self.lookups, 1));
                self.tasks.push(Task::AddAmount(next));
                self.queue_shape(descriptor, next);
            }
            noble_kernel::types::Ty::GenericNominal(_, arguments, descriptor) => {
                self.lookups = attempt!(add(self.lookups, 1));
                self.tasks.push(Task::AddAmount(next));
                self.tasks.push(Task::AddTop);
                self.queue_shape(descriptor, next);
                self.tasks.push(Task::AddTop);
                self.tasks.push(Task::VisitType(&arguments[1], next));
                self.tasks.push(Task::VisitType(&arguments[0], next));
            }
            noble_kernel::types::Ty::Unit
            | noble_kernel::types::Ty::Bool
            | noble_kernel::types::Ty::I64
            | noble_kernel::types::Ty::Text
            | noble_kernel::types::Ty::Syntax
            | noble_kernel::types::Ty::Contract
            | noble_kernel::types::Ty::Evidence
            | noble_kernel::types::Ty::Certified
            | noble_kernel::types::Ty::Resource(_) => self.weights.push(next),
        }
        Ok(())
    }

    /// Charge and step one pattern node, then queue its children and sums.
    fn visit_pattern(
        &mut self,
        value: &'a noble_kernel::shapes::Pattern,
        depth: u64,
        work: &mut super::super::super::Work,
    ) -> Result<(), crate::Diagnostic> {
        attempt!(work.charge(1));
        let next = attempt!(step(depth));
        match value {
            noble_kernel::shapes::Pattern::Pair(left, right)
            | noble_kernel::shapes::Pattern::Sum(left, right) => {
                self.tasks.push(Task::AddAmount(next));
                self.tasks.push(Task::AddTop);
                self.tasks.push(Task::VisitPattern(right, next));
                self.tasks.push(Task::VisitPattern(left, next));
            }
            noble_kernel::shapes::Pattern::List(item) => {
                self.tasks.push(Task::AddAmount(next));
                self.tasks.push(Task::VisitPattern(item, next));
            }
            noble_kernel::shapes::Pattern::Program(input, output, effects)
            | noble_kernel::shapes::Pattern::LiveRef(input, output, effects) => {
                attempt!(work.entries(effects.len()));
                self.tasks.push(Task::AddAmount(next));
                self.tasks.push(Task::AddAmount(effects.len() as u64));
                self.tasks.push(Task::AddTop);
                self.tasks.push(Task::PatternsFrom(output, 0, next));
                self.tasks.push(Task::StartTotal);
                self.tasks.push(Task::PatternsFrom(input, 0, next));
                self.tasks.push(Task::StartTotal);
            }
            noble_kernel::shapes::Pattern::Nominal(_, descriptor) => {
                self.lookups = attempt!(add(self.lookups, 1));
                self.tasks.push(Task::AddAmount(next));
                self.queue_shape(descriptor, next);
            }
            noble_kernel::shapes::Pattern::GenericNominal(_, arguments, _) => {
                self.lookups = attempt!(add(self.lookups, 1));
                self.tasks.push(Task::AddAmount(next));
                self.tasks.push(Task::AddTop);
                self.tasks.push(Task::VisitPattern(&arguments[1], next));
                self.tasks.push(Task::VisitPattern(&arguments[0], next));
            }
            noble_kernel::shapes::Pattern::Unit
            | noble_kernel::shapes::Pattern::Bool
            | noble_kernel::shapes::Pattern::I64
            | noble_kernel::shapes::Pattern::Text
            | noble_kernel::shapes::Pattern::Syntax
            | noble_kernel::shapes::Pattern::Contract
            | noble_kernel::shapes::Pattern::Evidence
            | noble_kernel::shapes::Pattern::Certified
            | noble_kernel::shapes::Pattern::Resource(_)
            | noble_kernel::shapes::Pattern::StackVar(_) => self.weights.push(next),
            // Kernel substitution clones the bound value once per occurrence
            // before checking the expanded interface. Borrowed preflight caps a
            // concrete binding at 512 Ty constructors, not the total fan-out.
            noble_kernel::shapes::Pattern::Var(_) => self.weights.push(attempt!(add(next, 512))),
        }
        Ok(())
    }

    /// Queue a nominal descriptor's payload types, summed in order.
    fn queue_shape(&mut self, shape: &'a noble_kernel::types::NominalShape, depth: u64) {
        match shape {
            noble_kernel::types::NominalShape::Opaque(value) => {
                self.tasks.push(Task::VisitType(value, depth));
            }
            noble_kernel::types::NominalShape::Variant(left, right) => {
                self.tasks.push(Task::AddTop);
                self.tasks.push(Task::VisitType(right, depth));
                self.tasks.push(Task::VisitType(left, depth));
            }
        }
    }

    /// Visit type stack entry `index`, add it to the running total, then
    /// continue with the next entry: one entry per task, so no loop creates
    /// borrows into the pending tasks.
    fn types_from(&mut self, values: &'a [noble_kernel::types::Ty], index: usize, depth: u64) {
        if index < values.len() {
            self.tasks.push(Task::TypesFrom(values, index + 1, depth));
            self.tasks.push(Task::AddTop);
            self.tasks.push(Task::VisitType(&values[index], depth));
        }
    }

    /// Visit pattern stack entry `index`, add it to the running total, then
    /// continue with the next entry.
    fn patterns_from(
        &mut self,
        values: &'a [noble_kernel::shapes::Pattern],
        index: usize,
        depth: u64,
    ) {
        if index < values.len() {
            self.tasks
                .push(Task::PatternsFrom(values, index + 1, depth));
            self.tasks.push(Task::AddTop);
            self.tasks.push(Task::VisitPattern(&values[index], depth));
        }
    }

    fn add_top(&mut self) -> Result<(), crate::Diagnostic> {
        let right = match self.weights.pop() {
            Some(weight) => weight,
            None => return Err(crate::Diagnostic::Defective),
        };
        let left = match self.weights.pop() {
            Some(weight) => weight,
            None => return Err(crate::Diagnostic::Defective),
        };
        self.weights.push(attempt!(add(left, right)));
        Ok(())
    }

    fn add_amount(&mut self, amount: u64) -> Result<(), crate::Diagnostic> {
        let weight = match self.weights.pop() {
            Some(weight) => weight,
            None => return Err(crate::Diagnostic::Defective),
        };
        self.weights.push(attempt!(add(amount, weight)));
        Ok(())
    }
}

/// The weighted size of root pattern `value` and its nominal lookup count.
/// An explicit task stack replaces recursion, so no loop sits on a
/// recursive cycle; every node is still charged, stepped and summed in the
/// recursive walk's order, stopping at its first refusal.
fn walk(
    value: &noble_kernel::shapes::Pattern,
    work: &mut super::super::super::Work,
) -> Result<(u64, u64), crate::Diagnostic> {
    let mut state = Walk {
        tasks: alloc::vec::Vec::new(),
        weights: alloc::vec::Vec::new(),
        lookups: 0,
    };
    state.tasks.push(Task::VisitPattern(value, 0));
    let mut failure = None;
    while let Some(task) = state.tasks.pop() {
        if let Err(problem) = state.run(task, work) {
            failure = Some(problem);
            break;
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    match state.weights.pop() {
        Some(weight) => Ok((weight, state.lookups)),
        None => Err(crate::Diagnostic::Defective),
    }
}

fn root(
    value: &noble_kernel::shapes::Pattern,
    lookup_rows: usize,
    work: &mut super::super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    let (weighted, lookups) = attempt!(walk(value, work));
    // The same accepted scheme is validated, copied into an owned kernel
    // environment, compared against nominal descriptors, and substituted.
    // Each pass may copy ancestor subtrees; W counts their descendant sizes.
    let scans = attempt!(weighted.checked_mul(16).ok_or(crate::Diagnostic::Exhausted));
    let rows = attempt!(u64::try_from(lookup_rows).map_err(|_| crate::Diagnostic::Exhausted));
    let lookup_cost = attempt!(lookups
        .checked_mul(rows)
        .and_then(|count| count.checked_mul(2))
        .ok_or(crate::Diagnostic::Exhausted));
    Ok(attempt!(add(scans, lookup_cost)).max(512))
}

pub(super) fn scheme(
    scheme: &noble_kernel::words::Scheme,
    lookup_rows: usize,
    work: &mut super::super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    // Each bound is tested directly, in the original order: a named `bool`
    // of this disjunction would elaborate as an undecidable proposition.
    if scheme.stack_in.len() > super::super::super::NODE_LIMIT
        || scheme.stack_out.len() > super::super::super::NODE_LIMIT
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    if scheme.var_kinds.len() > super::super::super::NODE_LIMIT
        || scheme.effects.len() > super::super::super::NODE_LIMIT
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    let scalar = attempt!(u64::try_from(
        scheme
            .var_kinds
            .len()
            .saturating_add(scheme.effects.len())
            .saturating_add(1)
    )
    .map_err(|_| crate::Diagnostic::Exhausted));
    attempt!(work.entries(scheme.var_kinds.len().saturating_add(scheme.effects.len())));
    let cost = attempt!(scalar.checked_mul(512).ok_or(crate::Diagnostic::Exhausted));
    let cost = attempt!(roots(&scheme.stack_in, lookup_rows, cost, work));
    roots(&scheme.stack_out, lookup_rows, cost, work)
}

/// Add each root pattern's weighted cost to `cost`, in order, stopping at
/// the first exhausted weight or sum.
fn roots(
    values: &[noble_kernel::shapes::Pattern],
    lookup_rows: usize,
    cost: u64,
    work: &mut super::super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    let mut total = cost;
    let mut index = 0usize;
    let mut failure = None;
    while index < values.len() {
        let weight = match root(&values[index], lookup_rows, work) {
            Ok(weight) => weight,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        };
        match add(total, weight) {
            Ok(sum) => total = sum,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
        index += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(total),
    }
}

// Test harness only: the recursive reference definition and its generators
// recurse over fixture trees, and fixture literals are the readable form.
#[cfg(test)]
#[allow(
    tigerstyle::assertion_density,
    tigerstyle::function_length,
    tigerstyle::magic_number,
    tigerstyle::no_recursion,
    tigerstyle::non_trait_imports,
    tigerstyle::platform_dependent_cast,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::unbounded_collection_growth,
    tigerstyle::unchecked_narrowing,
    reason = "Owner: noble-wasm-maintainers; test harness: the recursive reference walk and fixture generators recurse over bounded fixture trees, and their literal seeds and shapes are the readable form."
)]
mod tests {
    use crate::source::Work;
    use crate::Diagnostic;
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    use noble_kernel::shapes::{EffectSlot, Pattern};
    use noble_kernel::types::{EffId, EffSet, NominalShape, NominalTypeId, ResourceKind, Ty};
    use noble_kernel::words::Variable;

    fn budget(remaining: u64) -> Work {
        Work {
            remaining,
            issued: remaining,
            calculator_preflight: false,
            calculator_plan: false,
            max_type_nodes: 0,
        }
    }

    // The recursive definition the task stack replaced, kept verbatim as
    // the reference for weights, lookups, charges and refusals.
    fn recursive_shape(
        shape: &NominalShape,
        depth: u64,
        lookups: &mut u64,
        work: &mut Work,
    ) -> Result<u64, Diagnostic> {
        match shape {
            NominalShape::Opaque(value) => recursive_ty(value, depth, lookups, work),
            NominalShape::Variant(left, right) => {
                let left = recursive_ty(left, depth, lookups, work)?;
                super::add(left, recursive_ty(right, depth, lookups, work)?)
            }
        }
    }

    fn recursive_types(
        values: &[Ty],
        depth: u64,
        lookups: &mut u64,
        work: &mut Work,
    ) -> Result<u64, Diagnostic> {
        let mut total = 0;
        for value in values {
            total = super::add(total, recursive_ty(value, depth, lookups, work)?)?;
        }
        Ok(total)
    }

    fn recursive_ty(
        value: &Ty,
        depth: u64,
        lookups: &mut u64,
        work: &mut Work,
    ) -> Result<u64, Diagnostic> {
        work.charge(1)?;
        let next = super::step(depth)?;
        let children = match value {
            Ty::Pair(left, right) | Ty::Sum(left, right) => {
                let left = recursive_ty(left, next, lookups, work)?;
                super::add(left, recursive_ty(right, next, lookups, work)?)?
            }
            Ty::List(item) => recursive_ty(item, next, lookups, work)?,
            Ty::Program(input, output, effects) | Ty::LiveRef(input, output, effects) => {
                work.entries(effects.as_slice().len())?;
                let input = recursive_types(input, next, lookups, work)?;
                let stacks = super::add(input, recursive_types(output, next, lookups, work)?)?;
                super::add(stacks, effects.as_slice().len() as u64)?
            }
            Ty::Nominal(_, descriptor) => {
                *lookups = super::add(*lookups, 1)?;
                recursive_shape(descriptor, next, lookups, work)?
            }
            Ty::GenericNominal(_, arguments, descriptor) => {
                *lookups = super::add(*lookups, 1)?;
                let first = recursive_ty(&arguments[0], next, lookups, work)?;
                let args = super::add(first, recursive_ty(&arguments[1], next, lookups, work)?)?;
                super::add(args, recursive_shape(descriptor, next, lookups, work)?)?
            }
            Ty::Unit
            | Ty::Bool
            | Ty::I64
            | Ty::Text
            | Ty::Syntax
            | Ty::Contract
            | Ty::Evidence
            | Ty::Certified
            | Ty::Resource(_) => 0,
        };
        super::add(next, children)
    }

    fn recursive_patterns(
        values: &[Pattern],
        depth: u64,
        lookups: &mut u64,
        work: &mut Work,
    ) -> Result<u64, Diagnostic> {
        let mut total = 0;
        for value in values {
            total = super::add(total, recursive_pattern(value, depth, lookups, work)?)?;
        }
        Ok(total)
    }

    fn recursive_pattern(
        value: &Pattern,
        depth: u64,
        lookups: &mut u64,
        work: &mut Work,
    ) -> Result<u64, Diagnostic> {
        work.charge(1)?;
        let next = super::step(depth)?;
        let children = match value {
            Pattern::Pair(left, right) | Pattern::Sum(left, right) => {
                let left = recursive_pattern(left, next, lookups, work)?;
                super::add(left, recursive_pattern(right, next, lookups, work)?)?
            }
            Pattern::List(item) => recursive_pattern(item, next, lookups, work)?,
            Pattern::Program(input, output, effects) | Pattern::LiveRef(input, output, effects) => {
                work.entries(effects.len())?;
                let input = recursive_patterns(input, next, lookups, work)?;
                let stacks = super::add(input, recursive_patterns(output, next, lookups, work)?)?;
                super::add(stacks, effects.len() as u64)?
            }
            Pattern::Nominal(_, descriptor) => {
                *lookups = super::add(*lookups, 1)?;
                recursive_shape(descriptor, next, lookups, work)?
            }
            Pattern::GenericNominal(_, arguments, _) => {
                *lookups = super::add(*lookups, 1)?;
                let first = recursive_pattern(&arguments[0], next, lookups, work)?;
                super::add(
                    first,
                    recursive_pattern(&arguments[1], next, lookups, work)?,
                )?
            }
            Pattern::Unit
            | Pattern::Bool
            | Pattern::I64
            | Pattern::Text
            | Pattern::Syntax
            | Pattern::Contract
            | Pattern::Evidence
            | Pattern::Certified
            | Pattern::Resource(_)
            | Pattern::StackVar(_) => 0,
            Pattern::Var(_) => 512,
        };
        super::add(next, children)
    }

    /// A deterministic fixture source.
    struct Fixtures(u64);

    impl Fixtures {
        fn below(&mut self, bound: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) % bound
        }

        fn id(&mut self) -> NominalTypeId {
            NominalTypeId {
                module: self.below(3),
                ordinal: self.below(4) as u32,
            }
        }

        fn shape(&mut self, height: u32) -> NominalShape {
            if self.below(2) == 0 {
                NominalShape::Opaque(Box::new(self.ty(height)))
            } else {
                NominalShape::Variant(Box::new(self.ty(height)), Box::new(self.ty(height)))
            }
        }

        fn types(&mut self, height: u32) -> Vec<Ty> {
            let count = self.below(4);
            (0..count).map(|_| self.ty(height)).collect()
        }

        fn effects(&mut self) -> EffSet {
            let count = self.below(3) as u32;
            let ids: Vec<EffId> = (0..count).map(|index| EffId(index + 1)).collect();
            EffSet::from_ids(&ids)
        }

        fn ty(&mut self, height: u32) -> Ty {
            let choice = if height == 0 {
                self.below(4)
            } else {
                self.below(14)
            };
            let below = height.saturating_sub(1);
            match choice {
                0 => Ty::I64,
                1 => Ty::Unit,
                2 => Ty::Text,
                3 => Ty::Resource(ResourceKind(self.below(5) as u32)),
                4 => Ty::Pair(Box::new(self.ty(below)), Box::new(self.ty(below))),
                5 => Ty::Sum(Box::new(self.ty(below)), Box::new(self.ty(below))),
                6 => Ty::List(Box::new(self.ty(below))),
                7 => Ty::Program(
                    Box::new(self.types(below)),
                    Box::new(self.types(below)),
                    self.effects(),
                ),
                8 => Ty::LiveRef(
                    Box::new(self.types(below)),
                    Box::new(self.types(below)),
                    self.effects(),
                ),
                9 | 10 => Ty::Nominal(self.id(), Box::new(self.shape(below))),
                11 => Ty::GenericNominal(
                    self.id(),
                    Box::new([self.ty(below), self.ty(below)]),
                    Box::new(self.shape(below)),
                ),
                12 => Ty::Bool,
                _ => Ty::Certified,
            }
        }

        fn patterns(&mut self, height: u32) -> Vec<Pattern> {
            let count = self.below(4);
            (0..count).map(|_| self.pattern(height)).collect()
        }

        fn slots(&mut self) -> Vec<EffectSlot> {
            let count = self.below(3) as u32;
            (0..count)
                .map(|index| {
                    if index == 0 {
                        EffectSlot::Var(Variable(index))
                    } else {
                        EffectSlot::Effect(EffId(index))
                    }
                })
                .collect()
        }

        fn pattern(&mut self, height: u32) -> Pattern {
            let choice = if height == 0 {
                self.below(5)
            } else {
                self.below(15)
            };
            let below = height.saturating_sub(1);
            match choice {
                0 => Pattern::I64,
                1 => Pattern::Var(Variable(self.below(3) as u32)),
                2 => Pattern::StackVar(Variable(0)),
                3 => Pattern::Resource(ResourceKind(1)),
                4 => Pattern::Unit,
                5 => Pattern::Pair(Box::new(self.pattern(below)), Box::new(self.pattern(below))),
                6 => Pattern::Sum(Box::new(self.pattern(below)), Box::new(self.pattern(below))),
                7 => Pattern::List(Box::new(self.pattern(below))),
                8 => Pattern::Program(
                    Box::new(self.patterns(below)),
                    Box::new(self.patterns(below)),
                    Box::new(self.slots()),
                ),
                9 => Pattern::LiveRef(
                    Box::new(self.patterns(below)),
                    Box::new(self.patterns(below)),
                    Box::new(self.slots()),
                ),
                10 | 11 => Pattern::Nominal(self.id(), Box::new(self.shape(below))),
                12 => Pattern::GenericNominal(
                    self.id(),
                    Box::new([self.pattern(below), self.pattern(below)]),
                    [0, 1],
                ),
                13 => Pattern::Text,
                _ => Pattern::Evidence,
            }
        }
    }

    /// A list chain `levels` deep, which passes the 64-level step bound only
    /// when it is short enough.
    fn chain(levels: u32) -> Pattern {
        let mut value = Pattern::I64;
        for _ in 0..levels {
            value = Pattern::List(Box::new(value));
        }
        value
    }

    /// The work the reference walk charges with an unlimited budget.
    fn spent(value: &Pattern) -> u64 {
        let mut unlimited = budget(u64::MAX);
        let mut lookups = 0;
        match recursive_pattern(value, 0, &mut lookups, &mut unlimited) {
            Ok(_) | Err(_) => u64::MAX - unlimited.remaining,
        }
    }

    /// Both walks must agree on the result (weight and lookups, or the
    /// refusal) and leave the same budget, i.e. charge the same amounts in
    /// the same order up to any refusal.
    fn assert_same_walk(value: &Pattern) {
        let spent = spent(value);
        let budgets = [u64::MAX, spent, spent / 2, spent.saturating_sub(1), 1, 0];
        for remaining in budgets {
            let mut reference_work = budget(remaining);
            let mut reference_lookups = 0;
            let expected = recursive_pattern(value, 0, &mut reference_lookups, &mut reference_work)
                .map(|weight| (weight, reference_lookups));
            let mut work = budget(remaining);
            let actual = super::walk(value, &mut work);
            assert!(expected == actual, "walk differs with budget {remaining}");
            assert_eq!(work.remaining, reference_work.remaining);
        }
    }

    #[test]
    fn task_stack_matches_the_recursive_definition() {
        let mut fixtures = Fixtures(0x5eed);
        for _ in 0..400 {
            let height = fixtures.below(6) as u32;
            let value = fixtures.pattern(height);
            assert_same_walk(&value);
        }
        for levels in [62, 63, 64, 65, 80] {
            assert_same_walk(&chain(levels));
        }
    }
}
