//! Bounded, standalone Boolean interval-ISOP experiment for issue #35.
//!
//! This module has no source-language or `noble-ngraph` correspondence. Its
//! input is an explicitly validated Boolean DAG, and its output is an
//! optional cube cover checked by a separate finite evaluator.

use std::mem::size_of;
use std::time::{Duration, Instant};

mod benchmark;
pub use benchmark::run_benchmark;

/// Maximum number of Boolean atoms in one experiment.
pub const MAX_ATOMS: usize = 6;
/// Maximum number of nodes in an admitted Boolean DAG.
pub const MAX_DAG_NODES: usize = 512;
/// Maximum number of memoized interval states.
pub const MAX_MEMO_STATES: usize = 1_024;
/// Maximum cubes in a candidate cover.
pub const MAX_CUBES: usize = 256;
/// Maximum work units consumed while building a cover.
pub const MAX_WORK: usize = 4_000_000;
/// Maximum retained memo bytes, including fixed cover buffers.
pub const MAX_MEMO_BYTES: usize = 1 << 20;
/// Maximum elapsed construction time.
pub const MAX_ELAPSED: Duration = Duration::from_secs(1);

/// A pure Boolean DAG node. Operands refer only to earlier nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolNode {
    /// A Boolean constant.
    Const(bool),
    /// Input atom slot.
    Atom(u8),
    /// Logical negation of an earlier node.
    Not(u16),
    /// Conjunction of two earlier nodes.
    And(u16, u16),
    /// Disjunction of two earlier nodes.
    Or(u16, u16),
}

/// A topologically ordered, finite, total Boolean expression DAG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BooleanDag {
    atom_count: u8,
    nodes: Vec<BoolNode>,
    root: u16,
}

/// Invalid DAG, candidate cover, interval, or caller cap configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Atom count exceeds the hard bound.
    TooManyAtoms,
    /// Node count exceeds the hard bound or is empty.
    InvalidNodeCount,
    /// Root does not identify a node.
    RootOutOfRange,
    /// An operand is not a strict predecessor.
    ForwardReference,
    /// An atom slot is outside the DAG interface.
    AtomOutOfRange,
    /// Lower and upper DAGs do not share an interface.
    AtomCountMismatch,
    /// The interval is inconsistent on at least one assignment.
    InvalidInterval,
    /// The independent checker exceeded its fixed verification-time bound.
    VerificationTimeLimit,
    /// A cube has bits outside its interface or values outside its mask.
    InvalidCube,
    /// A cover exceeds the fixed cube bound.
    TooManyCubes,
    /// A requested caller limit exceeds a hard cap.
    LimitAboveHardCap,
}

impl BooleanDag {
    /// Validates and stores a topologically ordered Boolean DAG.
    pub fn try_new(atom_count: usize, nodes: Vec<BoolNode>, root: usize) -> Result<Self, Error> {
        if atom_count > MAX_ATOMS {
            return Err(Error::TooManyAtoms);
        }
        if nodes.is_empty() || nodes.len() > MAX_DAG_NODES {
            return Err(Error::InvalidNodeCount);
        }
        if root >= nodes.len() {
            return Err(Error::RootOutOfRange);
        }
        for (index, node) in nodes.iter().enumerate() {
            let index = index as u16;
            match *node {
                BoolNode::Const(_) => {}
                BoolNode::Atom(slot) if usize::from(slot) < atom_count => {}
                BoolNode::Atom(_) => return Err(Error::AtomOutOfRange),
                BoolNode::Not(value) if value < index => {}
                BoolNode::And(left, right) | BoolNode::Or(left, right)
                    if left < index && right < index => {}
                BoolNode::Not(_) | BoolNode::And(_, _) | BoolNode::Or(_, _) => {
                    return Err(Error::ForwardReference);
                }
            }
        }
        Ok(Self {
            atom_count: atom_count as u8,
            nodes,
            root: root as u16,
        })
    }

    /// Number of Boolean atom slots.
    #[must_use]
    pub const fn atom_count(&self) -> usize {
        self.atom_count as usize
    }

    /// Number of nodes in the checked DAG.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

/// One conjunction of zero or more Boolean literals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cube {
    care: u8,
    values: u8,
}

impl Cube {
    /// Constructs a cube from the literal mask and required bit values.
    pub fn try_new(care: u8, values: u8) -> Result<Self, Error> {
        if values & !care != 0 {
            return Err(Error::InvalidCube);
        }
        Ok(Self { care, values })
    }

    /// Bits constrained by this cube.
    #[must_use]
    pub const fn care_mask(self) -> u8 {
        self.care
    }

    /// Required values for the constrained bits.
    #[must_use]
    pub const fn values(self) -> u8 {
        self.values
    }

    fn matches(self, assignment: u8) -> bool {
        assignment & self.care == self.values
    }

    fn lifted(self, variable_mask: u8, value: Option<bool>) -> Self {
        match value {
            Some(true) => Self {
                care: self.care | variable_mask,
                values: self.values | variable_mask,
            },
            Some(false) => Self {
                care: self.care | variable_mask,
                values: self.values & !variable_mask,
            },
            None => self,
        }
    }
}

/// A bounded disjunction of cubes over a fixed atom interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cover {
    atom_count: u8,
    cubes: Vec<Cube>,
}

impl Cover {
    /// Validates a proposed cover before it reaches the independent checker.
    pub fn try_new(atom_count: usize, cubes: Vec<Cube>) -> Result<Self, Error> {
        if atom_count > MAX_ATOMS {
            return Err(Error::TooManyAtoms);
        }
        if cubes.len() > MAX_CUBES {
            return Err(Error::TooManyCubes);
        }
        let valid_mask = atom_mask(atom_count as u8);
        if cubes
            .iter()
            .any(|cube| cube.care & !valid_mask != 0 || cube.values & !cube.care != 0)
        {
            return Err(Error::InvalidCube);
        }
        Ok(Self {
            atom_count: atom_count as u8,
            cubes,
        })
    }

    /// Cubes in their emitted order.
    #[must_use]
    pub fn cubes(&self) -> &[Cube] {
        &self.cubes
    }

    /// Number of emitted cubes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cubes.len()
    }

    /// Whether the cover is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cubes.is_empty()
    }

    /// Evaluates the cube list for one compact atom assignment.
    #[must_use]
    pub fn evaluate(&self, assignment: u8) -> bool {
        self.cubes.iter().any(|cube| cube.matches(assignment))
    }
}

/// Lowerable caller limits. Values may be reduced, never raised above hard caps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    max_atoms: usize,
    max_nodes: usize,
    max_memo_states: usize,
    max_cubes: usize,
    max_work: usize,
    max_elapsed: Duration,
}

impl Limits {
    /// Constructs limits no larger than the module's fixed hard bounds.
    pub fn with_caps(
        max_atoms: usize,
        max_nodes: usize,
        max_memo_states: usize,
        max_cubes: usize,
        max_work: usize,
        max_elapsed: Duration,
    ) -> Result<Self, Error> {
        let memo_states_by_bytes = MAX_MEMO_BYTES
            .checked_div(MEMO_ENTRY_SIZE)
            .expect("MemoEntry has nonzero size");
        let is_over_hard_cap = max_atoms > MAX_ATOMS
            || max_nodes > MAX_DAG_NODES
            || max_memo_states > MAX_MEMO_STATES.min(memo_states_by_bytes)
            || max_cubes > MAX_CUBES
            || max_work > MAX_WORK
            || max_elapsed > MAX_ELAPSED;
        if is_over_hard_cap {
            return Err(Error::LimitAboveHardCap);
        }
        Ok(Self {
            max_atoms,
            max_nodes,
            max_memo_states,
            max_cubes,
            max_work,
            max_elapsed,
        })
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_atoms: MAX_ATOMS,
            max_nodes: MAX_DAG_NODES,
            max_memo_states: MAX_MEMO_STATES,
            max_cubes: MAX_CUBES,
            max_work: MAX_WORK,
            max_elapsed: MAX_ELAPSED,
        }
    }
}

/// Named reason that the optimizer returned the original rather than a partial cover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FallbackReason {
    /// Caller atom limit was reached.
    AtomLimit,
    /// Caller DAG-node limit was reached.
    NodeLimit,
    /// Memo-state or retained-byte limit was reached.
    MemoLimit,
    /// Output cube limit was reached.
    CubeLimit,
    /// Deterministic work budget was reached.
    WorkLimit,
    /// Elapsed-time budget was reached.
    TimeLimit,
    /// The bounded allocator could not reserve the memo table or output.
    AllocationLimit,
}

/// Optimization result; fallback contains no partial rewrite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Optimization {
    /// Fully constructed cover; it still requires independent checking.
    Optimized(Cover),
    /// No candidate was published because a fixed or caller cap was reached.
    Fallback(FallbackReason),
}

/// Computes a bounded Minato–Morreale support-interval cover.
pub fn optimize_interval(
    lower: &BooleanDag,
    upper: &BooleanDag,
    limits: Limits,
) -> Result<Optimization, Error> {
    if lower.atom_count != upper.atom_count {
        return Err(Error::AtomCountMismatch);
    }
    if lower.atom_count() > limits.max_atoms {
        return Ok(Optimization::Fallback(FallbackReason::AtomLimit));
    }
    if lower.node_count() > limits.max_nodes || upper.node_count() > limits.max_nodes {
        return Ok(Optimization::Fallback(FallbackReason::NodeLimit));
    }
    if limits.max_elapsed == Duration::ZERO {
        return Ok(Optimization::Fallback(FallbackReason::TimeLimit));
    }
    let started = Instant::now();
    let lower_bits = match truth_table(lower, started, limits.max_elapsed) {
        Ok(bits) => bits,
        Err(reason) => return Ok(Optimization::Fallback(reason)),
    };
    let upper_bits = match truth_table(upper, started, limits.max_elapsed) {
        Ok(bits) => bits,
        Err(reason) => return Ok(Optimization::Fallback(reason)),
    };
    let variables = atom_mask(lower.atom_count);
    let rows = row_mask(variables);
    if lower_bits & !upper_bits & rows != 0 {
        return Err(Error::InvalidInterval);
    }
    let mut engine = Engine::new(limits, started);
    let result = match engine.solve(variables, lower_bits, upper_bits) {
        Ok(index) => index,
        Err(BuildFailure::Cap(reason)) => return Ok(Optimization::Fallback(reason)),
        Err(BuildFailure::InvalidInterval) => return Err(Error::InvalidInterval),
    };
    let entry = &engine.memo[result];
    let mut cubes = Vec::new();
    if cubes.try_reserve_exact(usize::from(entry.len)).is_err() {
        return Ok(Optimization::Fallback(FallbackReason::AllocationLimit));
    }
    cubes.extend_from_slice(&entry.cubes[..usize::from(entry.len)]);
    if started.elapsed() > limits.max_elapsed {
        return Ok(Optimization::Fallback(FallbackReason::TimeLimit));
    }
    Ok(Optimization::Optimized(Cover {
        atom_count: lower.atom_count,
        cubes,
    }))
}

/// Exhaustively checks a candidate interval cover without using optimizer tables or memo state.
pub fn verify_interval(
    lower: &BooleanDag,
    upper: &BooleanDag,
    candidate: &Cover,
) -> Result<bool, Error> {
    if lower.atom_count != upper.atom_count || lower.atom_count != candidate.atom_count {
        return Err(Error::AtomCountMismatch);
    }
    validate_dag(lower)?;
    validate_dag(upper)?;
    validate_cover(candidate)?;
    let started = Instant::now();
    for row in 0..(1u8 << lower.atom_count) {
        if started.elapsed() > MAX_ELAPSED {
            return Err(Error::VerificationTimeLimit);
        }
        let low = check_dag(lower, row);
        let high = check_dag(upper, row);
        if low && !high {
            return Err(Error::InvalidInterval);
        }
        let output = candidate.evaluate(row);
        let is_outside_interval = (low && !output) || (output && !high);
        if is_outside_interval {
            return Ok(false);
        }
    }
    Ok(true)
}

fn validate_dag(dag: &BooleanDag) -> Result<(), Error> {
    if usize::from(dag.atom_count) > MAX_ATOMS {
        return Err(Error::TooManyAtoms);
    }
    if dag.nodes.is_empty() || dag.nodes.len() > MAX_DAG_NODES {
        return Err(Error::InvalidNodeCount);
    }
    if usize::from(dag.root) >= dag.nodes.len() {
        return Err(Error::RootOutOfRange);
    }
    for (index, node) in dag.nodes.iter().enumerate() {
        let index = index as u16;
        match *node {
            BoolNode::Const(_) => {}
            BoolNode::Atom(slot) if usize::from(slot) < usize::from(dag.atom_count) => {}
            BoolNode::Atom(_) => return Err(Error::AtomOutOfRange),
            BoolNode::Not(value) if value < index => {}
            BoolNode::And(left, right) | BoolNode::Or(left, right)
                if left < index && right < index => {}
            BoolNode::Not(_) | BoolNode::And(_, _) | BoolNode::Or(_, _) => {
                return Err(Error::ForwardReference);
            }
        }
    }
    Ok(())
}

fn validate_cover(cover: &Cover) -> Result<(), Error> {
    if usize::from(cover.atom_count) > MAX_ATOMS {
        return Err(Error::TooManyAtoms);
    }
    if cover.cubes.len() > MAX_CUBES {
        return Err(Error::TooManyCubes);
    }
    let valid_mask = atom_mask(cover.atom_count);
    if cover
        .cubes
        .iter()
        .any(|cube| cube.care & !valid_mask != 0 || cube.values & !cube.care != 0)
    {
        return Err(Error::InvalidCube);
    }
    Ok(())
}

fn truth_table(
    dag: &BooleanDag,
    started: Instant,
    max_elapsed: Duration,
) -> Result<u64, FallbackReason> {
    let mut table = 0u64;
    for row in 0..(1usize << dag.atom_count) {
        if started.elapsed() > max_elapsed {
            return Err(FallbackReason::TimeLimit);
        }
        if evaluate_topological(dag, row as u8) {
            table |= 1u64 << row;
        }
    }
    Ok(table)
}

fn evaluate_topological(dag: &BooleanDag, assignment: u8) -> bool {
    let mut values = [false; MAX_DAG_NODES];
    for (index, node) in dag.nodes.iter().enumerate() {
        values[index] = match *node {
            BoolNode::Const(value) => value,
            BoolNode::Atom(slot) => assignment & (1u8 << slot) != 0,
            BoolNode::Not(value) => !values[usize::from(value)],
            BoolNode::And(left, right) => values[usize::from(left)] && values[usize::from(right)],
            BoolNode::Or(left, right) => values[usize::from(left)] || values[usize::from(right)],
        };
    }
    values[usize::from(dag.root)]
}

fn check_dag(dag: &BooleanDag, assignment: u8) -> bool {
    fn visit(
        dag: &BooleanDag,
        index: usize,
        assignment: u8,
        cache: &mut [Option<bool>; MAX_DAG_NODES],
    ) -> bool {
        if let Some(value) = cache[index] {
            return value;
        }
        let value = match dag.nodes[index] {
            BoolNode::Const(value) => value,
            BoolNode::Atom(slot) => assignment & (1u8 << slot) != 0,
            BoolNode::Not(input) => !visit(dag, usize::from(input), assignment, cache),
            BoolNode::And(left, right) => {
                visit(dag, usize::from(left), assignment, cache)
                    && visit(dag, usize::from(right), assignment, cache)
            }
            BoolNode::Or(left, right) => {
                visit(dag, usize::from(left), assignment, cache)
                    || visit(dag, usize::from(right), assignment, cache)
            }
        };
        cache[index] = Some(value);
        value
    }

    visit(
        dag,
        usize::from(dag.root),
        assignment,
        &mut [None; MAX_DAG_NODES],
    )
}

fn atom_mask(atom_count: u8) -> u8 {
    if atom_count == 0 {
        0
    } else {
        (1u8 << atom_count) - 1
    }
}

fn row_mask(variables: u8) -> u64 {
    let rows = 1usize << variables.count_ones();
    if rows == 64 {
        u64::MAX
    } else {
        (1u64 << rows) - 1
    }
}

fn assignment_for(variables: u8, compact: usize) -> u8 {
    let mut assignment = 0u8;
    let mut source_bit = 0usize;
    for variable in 0..MAX_ATOMS {
        let bit = 1u8 << variable;
        if variables & bit != 0 {
            if compact & (1usize << source_bit) != 0 {
                assignment |= bit;
            }
            source_bit += 1;
        }
    }
    assignment
}

fn split_table(bits: u64, child_rows: usize) -> (u64, u64) {
    let low_mask = if child_rows == 64 {
        u64::MAX
    } else {
        (1u64 << child_rows) - 1
    };
    (bits & low_mask, bits >> child_rows)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct State {
    variables: u8,
    lower: u64,
    upper: u64,
}

struct MemoEntry {
    state: State,
    len: u16,
    cubes: [Cube; MAX_CUBES],
}

const MEMO_ENTRY_SIZE: usize = size_of::<MemoEntry>();

enum BuildFailure {
    Cap(FallbackReason),
    InvalidInterval,
}

impl From<FallbackReason> for BuildFailure {
    fn from(reason: FallbackReason) -> Self {
        Self::Cap(reason)
    }
}

impl MemoEntry {
    fn empty(state: State) -> Self {
        Self {
            state,
            len: 0,
            cubes: [Cube::default(); MAX_CUBES],
        }
    }

    fn push(&mut self, cube: Cube, cap: usize) -> Result<(), FallbackReason> {
        let len = usize::from(self.len);
        if len >= cap || len >= MAX_CUBES {
            return Err(FallbackReason::CubeLimit);
        }
        self.cubes[len] = cube;
        self.len += 1;
        Ok(())
    }
}

struct Engine {
    limits: Limits,
    started: Instant,
    work: usize,
    memo: Vec<MemoEntry>,
}

impl Engine {
    fn new(limits: Limits, started: Instant) -> Self {
        Self {
            limits,
            started,
            work: 0,
            memo: Vec::new(),
        }
    }

    fn tick(&mut self) -> Result<(), FallbackReason> {
        if self.work >= self.limits.max_work {
            return Err(FallbackReason::WorkLimit);
        }
        self.work += 1;
        if self.work & 0x3f == 0 && self.started.elapsed() > self.limits.max_elapsed {
            return Err(FallbackReason::TimeLimit);
        }
        Ok(())
    }

    fn solve(&mut self, variables: u8, lower: u64, upper: u64) -> Result<usize, BuildFailure> {
        if self.started.elapsed() > self.limits.max_elapsed {
            return Err(FallbackReason::TimeLimit.into());
        }
        let state = State {
            variables,
            lower: lower & row_mask(variables),
            upper: upper & row_mask(variables),
        };
        if state.lower & !state.upper & row_mask(variables) != 0 {
            return Err(BuildFailure::InvalidInterval);
        }
        for index in 0..self.memo.len() {
            self.tick()?;
            if self.memo[index].state == state {
                return Ok(index);
            }
        }
        if self.memo.len() >= self.limits.max_memo_states {
            return Err(FallbackReason::MemoLimit.into());
        }
        let mut entry = MemoEntry::empty(state);
        if state.lower == 0 {
            // The empty cover is the lower endpoint's exact representation.
        } else if state.upper == row_mask(variables) {
            entry.push(Cube::default(), self.limits.max_cubes)?;
        } else {
            if variables == 0 {
                return Err(BuildFailure::InvalidInterval);
            }
            let top = 1u8 << (7 - variables.leading_zeros() as u8);
            let child_variables = variables & !top;
            let child_rows = 1usize << child_variables.count_ones();
            let (lower0, lower1) = split_table(state.lower, child_rows);
            let (upper0, upper1) = split_table(state.upper, child_rows);
            let child_mask = row_mask(child_variables);
            let lower0_only = lower0 & !upper1 & child_mask;
            let lower1_only = lower1 & !upper0 & child_mask;
            let r0 = self.solve(child_variables, lower0_only, upper0)?;
            let r1 = self.solve(child_variables, lower1_only, upper1)?;
            let r0_bits = self.cover_bits(r0, child_variables)?;
            let r1_bits = self.cover_bits(r1, child_variables)?;
            let mut lower2 = 0u64;
            for row in 0..child_rows {
                self.tick()?;
                let bit = 1u64 << row;
                let low = lower0 & bit != 0 && r0_bits & bit == 0;
                let high = lower1 & bit != 0 && r1_bits & bit == 0;
                if low || high {
                    lower2 |= bit;
                }
            }
            let upper2 = upper0 & upper1 & child_mask;
            let r2 = self.solve(child_variables, lower2, upper2)?;
            self.append_lifted(&mut entry, r0, top, Some(false))?;
            self.append_lifted(&mut entry, r1, top, Some(true))?;
            self.append_lifted(&mut entry, r2, top, None)?;
        }
        if self.memo.len() >= self.limits.max_memo_states {
            return Err(FallbackReason::MemoLimit.into());
        }
        let retained_state_cap = MAX_MEMO_BYTES
            .checked_div(MEMO_ENTRY_SIZE)
            .expect("MemoEntry has nonzero size");
        if self.memo.len() == self.memo.capacity() {
            if self.memo.len() >= retained_state_cap {
                return Err(FallbackReason::MemoLimit.into());
            }
            self.memo
                .try_reserve_exact(1)
                .map_err(|_| FallbackReason::AllocationLimit)?;
            if self.memo.capacity() > retained_state_cap {
                return Err(FallbackReason::AllocationLimit.into());
            }
        }
        let index = self.memo.len();
        self.memo.push(entry);
        Ok(index)
    }

    fn cover_bits(&mut self, index: usize, variables: u8) -> Result<u64, FallbackReason> {
        let len = usize::from(self.memo[index].len);
        let rows = 1usize << variables.count_ones();
        let mut result = 0u64;
        for row in 0..rows {
            self.tick()?;
            let assignment = assignment_for(variables, row);
            for cube_index in 0..len {
                self.tick()?;
                let cube = self.memo[index].cubes[cube_index];
                if cube.matches(assignment) {
                    result |= 1u64 << row;
                    break;
                }
            }
        }
        Ok(result)
    }

    fn append_lifted(
        &mut self,
        destination: &mut MemoEntry,
        source: usize,
        variable: u8,
        value: Option<bool>,
    ) -> Result<(), FallbackReason> {
        let len = usize::from(self.memo[source].len);
        for index in 0..len {
            self.tick()?;
            let cube = self.memo[source].cubes[index].lifted(variable, value);
            destination.push(cube, self.limits.max_cubes)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        optimize_interval, verify_interval, BoolNode, BooleanDag, Cover, Cube, Error,
        FallbackReason, Limits, Optimization, MAX_ATOMS, MAX_CUBES, MAX_DAG_NODES, MAX_ELAPSED,
        MAX_MEMO_STATES, MAX_WORK,
    };
    use std::time::Duration;

    fn dag(atoms: usize, nodes: Vec<BoolNode>, root: usize) -> BooleanDag {
        match BooleanDag::try_new(atoms, nodes, root) {
            Ok(dag) => dag,
            Err(error) => panic!("invalid Boolean DAG fixture: {error:?}"),
        }
    }

    fn optimize_exact(dag: &BooleanDag) -> Cover {
        match optimize_interval(dag, dag, Limits::default()) {
            Ok(Optimization::Optimized(cover)) => {
                assert!(verify_interval(dag, dag, &cover).expect("independent check"));
                cover
            }
            result => panic!("unexpected exact ISOP result: {result:?}"),
        }
    }

    fn check_exact(dag: &BooleanDag) {
        let cover = optimize_exact(dag);
        assert!(verify_interval(dag, dag, &cover).expect("independent check"));
    }

    fn pair_and() -> BooleanDag {
        dag(
            2,
            vec![BoolNode::Atom(0), BoolNode::Atom(1), BoolNode::And(0, 1)],
            2,
        )
    }

    fn parity4() -> BooleanDag {
        dag(
            4,
            vec![
                BoolNode::Atom(0),
                BoolNode::Atom(1),
                BoolNode::Atom(2),
                BoolNode::Atom(3),
                BoolNode::Or(0, 1),
                BoolNode::And(0, 1),
                BoolNode::Not(5),
                BoolNode::And(4, 6),
                BoolNode::Or(7, 2),
                BoolNode::And(7, 2),
                BoolNode::Not(9),
                BoolNode::And(8, 10),
                BoolNode::Or(11, 3),
                BoolNode::And(11, 3),
                BoolNode::Not(13),
                BoolNode::And(12, 14),
            ],
            15,
        )
    }

    fn const_dag(value: bool) -> BooleanDag {
        dag(0, vec![BoolNode::Const(value)], 0)
    }

    fn repeated_atom() -> BooleanDag {
        dag(1, vec![BoolNode::Atom(0), BoolNode::And(0, 0)], 1)
    }

    fn contradiction() -> BooleanDag {
        dag(
            1,
            vec![BoolNode::Atom(0), BoolNode::Not(0), BoolNode::And(0, 1)],
            2,
        )
    }

    #[test]
    fn exact_fixtures_pass_independent_checker() {
        check_exact(&const_dag(false));
        check_exact(&const_dag(true));
        check_exact(&repeated_atom());
        check_exact(&contradiction());
        check_exact(&pair_and());
        check_exact(&parity4());
    }

    #[test]
    fn shared_subexpression_and_output_growth_are_observable() {
        let shared = dag(
            3,
            vec![
                BoolNode::Atom(0),
                BoolNode::Atom(1),
                BoolNode::Or(0, 1),
                BoolNode::Atom(2),
                BoolNode::And(2, 3),
                BoolNode::Not(3),
                BoolNode::And(2, 5),
                BoolNode::Or(4, 6),
            ],
            7,
        );
        check_exact(&shared);
        assert!(optimize_exact(&parity4()).len() > optimize_exact(&pair_and()).len());
    }

    #[test]
    fn independent_checker_rejects_wrong_atom_mapping_and_care_set() {
        let original = pair_and();
        let wrong_mapping = dag(2, vec![BoolNode::Atom(0), BoolNode::And(0, 0)], 1);
        let wrong_cover = optimize_exact(&wrong_mapping);
        assert_eq!(
            verify_interval(&original, &original, &wrong_cover),
            Ok(false)
        );

        let false_lower = dag(1, vec![BoolNode::Const(false)], 0);
        let x = dag(1, vec![BoolNode::Atom(0)], 0);
        let care_candidate = match optimize_interval(&false_lower, &x, Limits::default())
            .expect("consistent care interval")
        {
            Optimization::Optimized(cover) => cover,
            Optimization::Fallback(reason) => panic!("care interval fallback: {reason:?}"),
        };
        assert_eq!(verify_interval(&false_lower, &x, &care_candidate), Ok(true));
        let invalid_care_candidate =
            Cover::try_new(1, vec![Cube::try_new(0, 0).expect("universal cube")])
                .expect("well-formed candidate");
        assert_eq!(
            verify_interval(&false_lower, &x, &invalid_care_candidate),
            Ok(false)
        );
    }

    #[test]
    fn independent_checker_rejects_wrong_cofactor_and_complement() {
        let original = pair_and();
        let wrong_cofactor = Cover::try_new(2, vec![Cube::try_new(0b01, 0b01).expect("x0 cube")])
            .expect("well-formed candidate");
        let wrong_complement = Cover::try_new(
            2,
            vec![Cube::try_new(0b11, 0b01).expect("wrong-literal cube")],
        )
        .expect("well-formed candidate");
        assert_eq!(
            verify_interval(&original, &original, &wrong_cofactor),
            Ok(false)
        );
        assert_eq!(
            verify_interval(&original, &original, &wrong_complement),
            Ok(false)
        );
    }

    #[test]
    fn invalid_graphs_intervals_and_covers_are_refused() {
        assert_eq!(
            BooleanDag::try_new(1, vec![BoolNode::Not(0)], 0),
            Err(Error::ForwardReference)
        );
        let x = dag(1, vec![BoolNode::Atom(0)], 0);
        let not_x = dag(1, vec![BoolNode::Atom(0), BoolNode::Not(0)], 1);
        assert_eq!(
            optimize_interval(&x, &not_x, Limits::default()),
            Err(Error::InvalidInterval)
        );
        assert_eq!(Cube::try_new(0, 1), Err(Error::InvalidCube));
        assert!(Limits::with_caps(
            MAX_ATOMS + 1,
            MAX_DAG_NODES,
            MAX_MEMO_STATES,
            MAX_CUBES,
            MAX_WORK,
            MAX_ELAPSED,
        )
        .is_err());
    }

    #[test]
    fn every_construction_cap_falls_back_without_a_cover() {
        let two_atom = pair_and();
        let atom_cap = Limits::with_caps(
            1,
            MAX_DAG_NODES,
            MAX_MEMO_STATES,
            MAX_CUBES,
            MAX_WORK,
            MAX_ELAPSED,
        )
        .expect("lower cap");
        assert_eq!(
            optimize_interval(&two_atom, &two_atom, atom_cap),
            Ok(Optimization::Fallback(FallbackReason::AtomLimit))
        );
        let memo_cap = Limits::with_caps(
            MAX_ATOMS,
            MAX_DAG_NODES,
            0,
            MAX_CUBES,
            MAX_WORK,
            MAX_ELAPSED,
        )
        .expect("zero memo cap");
        assert_eq!(
            optimize_interval(&two_atom, &two_atom, memo_cap),
            Ok(Optimization::Fallback(FallbackReason::MemoLimit))
        );
        let one_memo_state = Limits::with_caps(
            MAX_ATOMS,
            MAX_DAG_NODES,
            1,
            MAX_CUBES,
            MAX_WORK,
            MAX_ELAPSED,
        )
        .expect("one-state memo cap");
        assert_eq!(
            optimize_interval(&two_atom, &two_atom, one_memo_state),
            Ok(Optimization::Fallback(FallbackReason::MemoLimit))
        );
        let node_cap = Limits::with_caps(
            MAX_ATOMS,
            0,
            MAX_MEMO_STATES,
            MAX_CUBES,
            MAX_WORK,
            MAX_ELAPSED,
        )
        .expect("zero node cap");
        assert_eq!(
            optimize_interval(&two_atom, &two_atom, node_cap),
            Ok(Optimization::Fallback(FallbackReason::NodeLimit))
        );
        let cube_cap = Limits::with_caps(
            MAX_ATOMS,
            MAX_DAG_NODES,
            MAX_MEMO_STATES,
            0,
            MAX_WORK,
            MAX_ELAPSED,
        )
        .expect("zero cube cap");
        assert_eq!(
            optimize_interval(&const_dag(true), &const_dag(true), cube_cap),
            Ok(Optimization::Fallback(FallbackReason::CubeLimit))
        );
        let work_cap = Limits::with_caps(
            MAX_ATOMS,
            MAX_DAG_NODES,
            MAX_MEMO_STATES,
            MAX_CUBES,
            0,
            MAX_ELAPSED,
        )
        .expect("zero work cap");
        assert_eq!(
            optimize_interval(&two_atom, &two_atom, work_cap),
            Ok(Optimization::Fallback(FallbackReason::WorkLimit))
        );
        let time_cap = Limits::with_caps(
            MAX_ATOMS,
            MAX_DAG_NODES,
            MAX_MEMO_STATES,
            MAX_CUBES,
            MAX_WORK,
            Duration::ZERO,
        )
        .expect("zero time cap");
        assert_eq!(
            optimize_interval(&two_atom, &two_atom, time_cap),
            Ok(Optimization::Fallback(FallbackReason::TimeLimit))
        );
    }
}
