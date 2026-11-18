//! F1 kernel-dependencies: the kernel's B-CHECK-02 dependency walk under every renumbering of the
//! extra definitions and both orders of every dependency list.
//!
//! The observation maps a named definition back to its abstract item, so a renumbering changes
//! only presentation. Soundness is judged against an independent reachability reference: an
//! accepted input is acyclic, and a named definition lies on a cycle of the abstract graph.

use super::{Allowed, Case, Normalized, Tabulate, Tally, Verdict};

/// Definitions in the bootstrap table; extra definitions are numbered from here.
const BOOTSTRAP: u32 = 23;
/// A work limit no walk below can exhaust (every case has fewer than 16 edges).
pub(super) const AMPLE_WORK: u32 = 10_000;

/// One abstract dependency graph: `deps[i]` lists the items item `i` depends on.
pub(super) struct Dependencies {
    pub(super) label: &'static str,
    pub(super) deps: Vec<Vec<usize>>,
    pub(super) is_reversed: bool,
    pub(super) work: u32,
}

/// A family observation, with named definitions mapped back to abstract items.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Seen {
    Accepted,
    Cycle(usize),
    Exhausted,
    Other(String),
}

impl Dependencies {
    fn edges(&self) -> usize {
        self.deps.iter().map(Vec::len).sum()
    }

    /// Whether `item` reaches itself through at least one edge.
    pub(super) fn is_on_cycle(&self, item: usize) -> bool {
        let mut seen = vec![false; self.deps.len()];
        let mut stack = self.deps[item].clone();
        while let Some(next) = stack.pop() {
            if next == item {
                return true;
            }
            if !seen[next] {
                seen[next] = true;
                stack.extend(self.deps[next].iter().copied());
            }
        }
        false
    }

    fn is_acyclic(&self) -> bool {
        (0..self.deps.len()).all(|item| !self.is_on_cycle(item))
    }

    /// The environment where extra definition `BOOTSTRAP + k` is abstract item `order[k]`.
    pub(super) fn environment(&self, order: &[usize]) -> noble_kernel::contracts::Env {
        let mut env = noble_kernel::contracts::environment().expect("bootstrap table validates");
        let number = |item: usize| {
            let position = order.iter().position(|placed| *placed == item);
            noble_kernel::contracts::Definition(
                BOOTSTRAP + u32::try_from(position.expect("order is a permutation")).unwrap(),
            )
        };
        for item in order {
            let mut deps = self.deps[*item]
                .iter()
                .map(|dep| number(*dep))
                .collect::<Vec<_>>();
            if self.is_reversed {
                deps.reverse();
            }
            env.defs.push(unit_like());
            env.kinds.push(noble_kernel::contracts::Behavior::Named);
            env.deps.push(deps);
        }
        env
    }

    pub(super) fn observe(&self, order: &[usize]) -> Seen {
        let env = self.environment(order);
        let outcome = noble_kernel::acceptance::check(&env, &request(self.work), &empty_program());
        match outcome {
            noble_kernel::untrusted::Outcome::Accepted(_) => Seen::Accepted,
            noble_kernel::untrusted::Outcome::Unsupported(
                noble_kernel::untrusted::UnsupportedKind::RecursiveDependency(definition),
            ) => match definition.0.checked_sub(BOOTSTRAP) {
                Some(position) => Seen::Cycle(order[usize::try_from(position).unwrap()]),
                None => Seen::Other(format!("bootstrap definition {} named", definition.0)),
            },
            noble_kernel::untrusted::Outcome::Exhausted(
                noble_kernel::untrusted::LimitKind::Work,
            ) => Seen::Exhausted,
            other => Seen::Other(format!("{other:?}")),
        }
    }

    fn sound(&self, seen: &Seen) -> Verdict {
        match seen {
            Seen::Accepted if self.is_acyclic() => Verdict::Same,
            Seen::Accepted => Verdict::Diverged(String::from("accepted a cyclic dependency graph")),
            Seen::Cycle(item) if self.is_on_cycle(*item) => Verdict::Same,
            Seen::Cycle(item) => Verdict::Diverged(format!("named item {item} lies on no cycle")),
            Seen::Exhausted if usize::try_from(self.work).unwrap() < self.edges() => Verdict::Same,
            Seen::Exhausted => Verdict::Diverged(String::from("exhausted under sufficient work")),
            Seen::Other(text) => Verdict::Diverged(format!("unexpected outcome {text}")),
        }
    }
}

impl Case for Dependencies {
    type Seen = Seen;

    fn name(&self) -> String {
        let lists = if self.is_reversed {
            "reversed"
        } else {
            "forward"
        };
        format!(
            "kernel-dependencies/{}/{lists}/work-{}",
            self.label, self.work
        )
    }

    fn items(&self) -> usize {
        self.deps.len()
    }

    fn is_query_case(&self) -> bool {
        false
    }

    fn run(&self, order: &[usize]) -> Vec<(usize, Seen)> {
        vec![(0, self.observe(order))]
    }

    fn compare(&self, _query: usize, base: &Seen, other: &Seen) -> Verdict {
        let classes = match (base, other) {
            (Seen::Exhausted, Seen::Exhausted) => Verdict::Same,
            (Seen::Exhausted, _) | (_, Seen::Exhausted) => {
                Verdict::Allowed(vec![Allowed::Exhaustion])
            }
            (Seen::Cycle(first), Seen::Cycle(second)) => {
                Verdict::allowed_if_differs(Allowed::FirstDefect, first, second)
            }
            _ => Verdict::equal("verdict class", base, other),
        };
        self.sound(other).and(classes)
    }

    fn orders(&self) -> Vec<(String, Vec<usize>)> {
        super::permutations(self.items())
    }

    fn item_schedules(&self) -> Option<u64> {
        None
    }
}

impl Tabulate for Dependencies {
    fn input(&self, _query: usize) -> String {
        // The abstract graph and the work budget: both dependency-list orders of one graph share
        // one input, so their rows are compared (criteria-oracle section 1).
        format!("kernel-dependencies/{}/work-{}", self.label, self.work)
    }

    fn normalized(&self, _query: usize, seen: &Seen) -> Normalized {
        let edges = u64::try_from(self.edges()).expect("edge count fits u64");
        match seen {
            // A3: a budget below the edge count may give out where another order decides.
            Seen::Exhausted if u64::from(self.work) < edges => {
                Normalized::Exempt(Allowed::Exhaustion)
            }
            Seen::Accepted => Normalized::Output(String::from("accepted")),
            Seen::Cycle(_) => Normalized::Output(String::from("refused-cyclic")),
            Seen::Exhausted => Normalized::Output(String::from("exhausted")),
            Seen::Other(text) => Normalized::Output(format!("unexpected:{text}")),
        }
    }

    fn presentation(&self, _query: usize, seen: &Seen) -> String {
        match seen {
            // A2: which member of a cycle is named.
            Seen::Cycle(item) => format!("cycle-member-{item}"),
            Seen::Exhausted => String::from("exhausted"),
            Seen::Accepted | Seen::Other(_) => String::new(),
        }
    }

    fn schedule(&self, _query: usize, label: &str, position: usize) -> String {
        super::row_identifier(&self.name(), label, position)
    }
}

/// One extra named definition: `S -- S Unit`.
fn unit_like() -> noble_kernel::words::Scheme {
    noble_kernel::words::Scheme {
        var_kinds: vec![noble_kernel::words::VariableKind::Stack],
        stack_in: vec![noble_kernel::shapes::Pattern::StackVar(
            noble_kernel::words::Variable(0),
        )],
        stack_out: vec![
            noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0)),
            noble_kernel::shapes::Pattern::Unit,
        ],
        effects: vec![],
    }
}

pub(super) fn request(work: u32) -> noble_kernel::untrusted::Request {
    noble_kernel::untrusted::Request {
        input_bytes: 64,
        expected: noble_kernel::untrusted::Expected {
            stack_in: vec![],
            stack_out: vec![],
            allowed_effects: noble_kernel::types::EffSet::empty(),
        },
        limits: noble_kernel::untrusted::Limits {
            bytes: 1 << 16,
            nodes: 256,
            depth: 32,
            type_size: 64,
            stack_height: 16,
            work,
            diagnostics: 64,
        },
    }
}

pub(super) fn empty_program() -> noble_kernel::untrusted::Candidate {
    noble_kernel::untrusted::Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision: noble_kernel::untrusted::SEMANTIC_REVISION,
        nodes: vec![],
        body: vec![],
    }
}

/// The abstract graphs of the family: shared dependencies, cycles and partial defects.
pub(super) fn graphs() -> Vec<(&'static str, Vec<Vec<usize>>)> {
    vec![
        ("diamond", vec![vec![1, 2], vec![2], vec![]]),
        ("wide-diamond", vec![vec![1, 2], vec![3], vec![3], vec![]]),
        ("acyclic-chain", vec![vec![1], vec![2], vec![3], vec![]]),
        ("self-loop-and-entry", vec![vec![1], vec![1], vec![0]]),
        ("two-cycles", vec![vec![1], vec![0], vec![3], vec![2]]),
        ("chain-to-self-loop", vec![vec![1], vec![2], vec![2]]),
        (
            "shared-entry-cycle",
            vec![vec![2], vec![2], vec![3], vec![2]],
        ),
        (
            "cycle-and-shared-sink",
            vec![vec![1, 3], vec![0, 3], vec![3], vec![]],
        ),
    ]
}

fn cases() -> Vec<Dependencies> {
    const TIGHT_WORK: [u32; 3] = [1, 2, 3];
    let mut out = Vec::with_capacity(graphs().len() * (2 + TIGHT_WORK.len()));
    for (label, deps) in graphs() {
        for is_reversed in [false, true] {
            out.push(Dependencies {
                label,
                deps: deps.clone(),
                is_reversed,
                work: AMPLE_WORK,
            });
        }
    }
    // A3: a budget below the edge count may exhaust in some orders and decide in others.
    for (label, deps) in graphs() {
        for work in TIGHT_WORK {
            out.push(Dependencies {
                label,
                deps: deps.clone(),
                is_reversed: false,
                work,
            });
        }
    }
    out
}

/// Reruns one named schedule of this family, if the case belongs to it.
pub(super) fn replay(name: &str, order: &[usize]) -> Option<Result<(), String>> {
    let case = cases().into_iter().find(|case| case.name() == name)?;
    Some(super::replay_case(&case, order))
}

#[test]
fn dependencies_agree_across_renumberings() {
    let mut tally = Tally::default();
    for case in cases() {
        tally.absorb(&super::check_case(&case));
    }
    tally.emit("F1 kernel-dependencies");
}
