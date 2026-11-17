//! Order-variation harness for issue #134, governed by the frozen
//! `verification/schedule-independence/criteria.md` (revision 1).
//!
//! A *case* is a list of items plus a runner. Query cases run their queries in an order (repeats
//! allowed) and observe each query; renumbering cases rebuild one equivalent input from a
//! permutation of its items and observe the whole input. The serial order is the baseline; every
//! other admitted order is compared with it. A comparison either agrees, differs in a classified
//! allowed way (section 3 of the criteria, counted), or diverges. A divergence is minimized and
//! reported with a replay token; `NOBLE_SCHEDULE_REPLAY=<case>@<order>` reruns that schedule.

mod cache;
mod contracts;
mod graph;
mod kernel;
mod retained;
mod threads;

/// Base seed of every seeded order (criteria section 5).
pub(super) const BASE_SEED: u64 = 0x5eed_0134;
/// Seeded orders per query or graph case (criteria section 5).
pub(super) const SEEDED_ORDERS: u64 = 32;

/// Classified allowed differences (criteria section 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Allowed {
    /// A1: session-local numbering compared after mapping.
    Numbering,
    /// A2: which of several defects (or which closing member of one cycle) is reported.
    FirstDefect,
    /// A3: an exhausted run where another order reached a verdict.
    Exhaustion,
    /// A4: opportunistic memoization or scheduling metadata.
    Metadata,
    /// A5: a shared request-work pool's remaining balance.
    SharedPool,
}

impl Allowed {
    const ALL: [Self; 5] = [
        Self::Numbering,
        Self::FirstDefect,
        Self::Exhaustion,
        Self::Metadata,
        Self::SharedPool,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Numbering => "a1_numbering",
            Self::FirstDefect => "a2_first_defect",
            Self::Exhaustion => "a3_exhaustion",
            Self::Metadata => "a4_metadata",
            Self::SharedPool => "a5_shared_pool",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Numbering => 0,
            Self::FirstDefect => 1,
            Self::Exhaustion => 2,
            Self::Metadata => 3,
            Self::SharedPool => 4,
        }
    }
}

/// The outcome of comparing one observation with its baseline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    /// Equal in every compared field and every presentation field.
    Same,
    /// Equal in every compared field; the listed presentation classes differ.
    Allowed(Vec<Allowed>),
    /// A criteria section 2 failure.
    Diverged(String),
}

impl Verdict {
    /// Combines two verdicts about the same observation.
    pub(super) fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::Diverged(first), _) => Self::Diverged(first),
            (_, Self::Diverged(second)) => Self::Diverged(second),
            (Self::Same, Self::Same) => Self::Same,
            (Self::Allowed(classes), Self::Same) | (Self::Same, Self::Allowed(classes)) => {
                Self::Allowed(classes)
            }
            (Self::Allowed(mut first), Self::Allowed(second)) => {
                first.extend(second);
                Self::Allowed(first)
            }
        }
    }

    /// `Same` when the two values are equal, otherwise a divergence naming `what`.
    pub(super) fn equal<T: PartialEq + core::fmt::Debug>(what: &str, base: &T, other: &T) -> Self {
        if base == other {
            Self::Same
        } else {
            Self::Diverged(format!("{what}: baseline {base:?}, other {other:?}"))
        }
    }

    /// `Same` when equal, otherwise the allowed class.
    pub(super) fn allowed_if_differs<T: PartialEq>(class: Allowed, base: &T, other: &T) -> Self {
        if base == other {
            Self::Same
        } else {
            Self::Allowed(vec![class])
        }
    }
}

/// One family case driven by the scheduler.
pub(super) trait Case {
    /// The per-query (or per-input) observation.
    type Seen: core::fmt::Debug;
    /// Stable case name; the replay token prefix.
    fn name(&self) -> String;
    /// Number of items (queries, or permuted elements).
    fn items(&self) -> usize;
    /// Whether orders may repeat or drop queries (query cases) or must be permutations.
    fn is_query_case(&self) -> bool;
    /// Runs `order` and returns `(query or 0, observation)` in run order.
    fn run(&self, order: &[usize]) -> Vec<(usize, Self::Seen)>;
    /// Compares one observation with the baseline of the same query; also checks soundness.
    fn compare(&self, query: usize, base: &Self::Seen, other: &Self::Seen) -> Verdict;
    /// The admitted orders besides the serial baseline (criteria section 5).
    fn orders(&self) -> Vec<(String, Vec<usize>)>;
    /// Distinct admitted schedules a case covers when they are its items rather than its orders
    /// (F7 runs every pair-selection schedule as one query); `None` counts the orders run.
    fn item_schedules(&self) -> Option<u64> {
        None
    }
}

/// Counts for one family, emitted as one receipt line.
#[derive(Debug, Default)]
pub(super) struct Tally {
    cases: u64,
    schedules: u64,
    comparisons: u64,
    allowed: [u64; 5],
}

impl Tally {
    /// Adds another tally.
    pub(super) fn absorb(&mut self, other: &Self) {
        self.cases += other.cases;
        self.schedules += other.schedules;
        self.comparisons += other.comparisons;
        for (sum, add) in self.allowed.iter_mut().zip(other.allowed) {
            *sum += add;
        }
    }

    /// Records counts of a family whose comparisons are not driven by [`check_case`].
    pub(super) fn record(&mut self, cases: u64, schedules: u64, comparisons: u64) {
        self.cases += cases;
        self.schedules += schedules;
        self.comparisons += comparisons;
    }

    /// Counts one classified difference.
    pub(super) fn count(&mut self, class: Allowed) {
        self.allowed[class.index()] += 1;
    }

    /// The `NOBLE_SCHEDULE_FAMILY` receipt line of one family; zero divergences by construction,
    /// because a divergence fails the test before the line is printed.
    pub(super) fn emit(&self, family: &str) {
        assert!(self.cases > 0, "{family}: no case ran");
        assert!(self.comparisons > 0, "{family}: nothing was compared");
        let allowed = Allowed::ALL
            .iter()
            .map(|class| format!("\"{}\":{}", class.label(), self.allowed[class.index()]))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "NOBLE_SCHEDULE_FAMILY {{\"family\":\"{family}\",\"cases\":{},\"schedules\":{},\"comparisons\":{},\"divergences\":0,\"allowed\":{{{allowed}}}}}",
            self.cases, self.schedules, self.comparisons
        );
    }
}

/// The serial baseline order of a case.
fn serial(items: usize) -> Vec<usize> {
    (0..items).collect()
}

/// Seeded Fisher-Yates permutation of `items` (SplitMix64 from `seed`).
pub(super) fn shuffled(items: &[usize], seed: u64) -> Vec<usize> {
    let mut out = items.to_vec();
    let mut random = noble_ngraph::prng::SplitMix64::new(seed);
    let mut index = out.len();
    while index > 1 {
        let pick = random.below(index as u64) as usize;
        index -= 1;
        out.swap(index, pick);
    }
    out
}

/// The seed of the `round`-th seeded order of a case, derived from [`BASE_SEED`].
pub(super) fn seed(round: u64) -> u64 {
    let mut random = noble_ngraph::prng::SplitMix64::new(BASE_SEED ^ round);
    random.next_u64()
}

/// Serial-with-repeats, reversed-with-repeats and seeded orders of a query case: each query
/// appears twice, so every order includes an exact repeat of every query.
pub(super) fn query_orders(items: usize) -> Vec<(String, Vec<usize>)> {
    let twice = (0..items).chain(0..items).collect::<Vec<_>>();
    let mut orders = vec![
        (String::from("serial-repeated"), twice.clone()),
        (
            String::from("reversed-repeated"),
            twice.iter().rev().copied().collect(),
        ),
    ];
    for round in 0..SEEDED_ORDERS {
        let seed = seed(round);
        orders.push((format!("seed-{seed:#018x}"), shuffled(&twice, seed)));
    }
    orders
}

/// Every permutation of `items` elements (Heap's algorithm order), for renumbering cases.
pub(super) fn permutations(items: usize) -> Vec<(String, Vec<usize>)> {
    let mut current = serial(items);
    let mut out = vec![(String::from("perm-0"), current.clone())];
    let mut counters = vec![0usize; items];
    let mut index = 0usize;
    while index < items {
        if counters[index] < index {
            if index.is_multiple_of(2) {
                current.swap(0, index);
            } else {
                current.swap(counters[index], index);
            }
            out.push((format!("perm-{}", out.len()), current.clone()));
            counters[index] += 1;
            index = 0;
        } else {
            counters[index] = 0;
            index += 1;
        }
    }
    out
}

/// The first divergence of `order` against the case's serial baselines, if any.
fn divergence<C: Case>(case: &C, order: &[usize]) -> Option<String> {
    let base = baselines(case);
    let seen = case.run(order);
    for (query, observation) in &seen {
        let reference = base.iter().find(|(index, _)| index == query)?;
        if let Verdict::Diverged(reason) = case.compare(*query, &reference.1, observation) {
            return Some(reason);
        }
    }
    None
}

fn baselines<C: Case>(case: &C) -> Vec<(usize, C::Seen)> {
    case.run(&serial(case.items()))
}

/// Greedy minimization of a diverging order (criteria section 6): query cases drop entries,
/// renumbering cases move elements back to their serial position, while the divergence persists.
fn minimize<C: Case>(case: &C, order: &[usize]) -> Vec<usize> {
    let mut current = order.to_vec();
    let mut is_changed = true;
    while is_changed {
        is_changed = false;
        let mut index = 0usize;
        while index < current.len() {
            let candidate = if case.is_query_case() {
                let mut fewer = current.clone();
                fewer.remove(index);
                fewer
            } else {
                let mut closer = current.clone();
                let Some(home) = closer.iter().position(|item| *item == index) else {
                    index += 1;
                    continue;
                };
                closer.swap(index, home);
                closer
            };
            if candidate != current && divergence(case, &candidate).is_some() {
                current = candidate;
                is_changed = true;
            } else {
                index += 1;
            }
        }
    }
    current
}

fn token(name: &str, order: &[usize]) -> String {
    let positions = order
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!("{name}@{positions}")
}

/// Runs one case over its admitted orders; panics with a minimized replay token on divergence.
pub(super) fn check_case<C: Case>(case: &C) -> Tally {
    let base = baselines(case);
    let mut tally = Tally {
        cases: 1,
        schedules: 1,
        ..Tally::default()
    };
    for (query, observation) in &base {
        let schedule = ("serial", serial(case.items()));
        settle(
            case,
            &mut tally,
            (*query, observation, observation),
            (schedule.0, &schedule.1),
        );
    }
    for (label, order) in case.orders() {
        tally.schedules += 1;
        for (query, observation) in case.run(&order) {
            let reference = base
                .iter()
                .find(|(index, _)| *index == query)
                .unwrap_or_else(|| panic!("{}: no baseline for query {query}", case.name()));
            settle(
                case,
                &mut tally,
                (query, &reference.1, &observation),
                (&label, &order),
            );
        }
    }
    if let Some(schedules) = case.item_schedules() {
        tally.schedules = schedules;
    }
    tally
}

fn settle<C: Case>(
    case: &C,
    tally: &mut Tally,
    (query, base, other): (usize, &C::Seen, &C::Seen),
    (label, order): (&str, &[usize]),
) {
    tally.comparisons += 1;
    match case.compare(query, base, other) {
        Verdict::Same => {}
        Verdict::Allowed(classes) => {
            for class in classes {
                tally.count(class);
            }
        }
        Verdict::Diverged(reason) => {
            let minimal = minimize(case, order);
            println!(
                "NOBLE_SCHEDULE_DIVERGENCE {{\"token\":\"{}\",\"schedule\":\"{label}\",\"original\":\"{}\",\"reason\":{:?}}}",
                token(&case.name(), &minimal),
                token(&case.name(), order),
                reason
            );
            panic!(
                "{}: divergence ({reason}); replay with NOBLE_SCHEDULE_REPLAY={}",
                case.name(),
                token(&case.name(), &minimal)
            );
        }
    }
}

/// Reruns one schedule against the case's serial baseline: `Ok` when it agrees.
pub(super) fn replay_case<C: Case>(case: &C, order: &[usize]) -> Result<(), String> {
    if order.iter().any(|item| *item >= case.items()) {
        return Err(format!("{}: order names an unknown item", case.name()));
    }
    divergence(case, order).map_or(Ok(()), Err)
}

/// Parses `<case>@<comma separated positions>`.
pub(super) fn parse_token(token: &str) -> Option<(String, Vec<usize>)> {
    let (name, positions) = token.rsplit_once('@')?;
    let order = if positions.is_empty() {
        Vec::new()
    } else {
        positions
            .split(',')
            .map(|position| position.parse::<usize>().ok())
            .collect::<Option<Vec<_>>>()?
    };
    Some((name.to_string(), order))
}

#[test]
fn replay_requested_schedule() {
    let Ok(requested) = std::env::var("NOBLE_SCHEDULE_REPLAY") else {
        return;
    };
    let (name, order) = parse_token(&requested).expect("token is <case>@<positions>");
    let outcome = kernel::replay(&name, &order)
        .or_else(|| contracts::replay(&name, &order))
        .or_else(|| graph::replay(&name, &order))
        .or_else(|| cache::replay(&name, &order))
        .unwrap_or_else(|| panic!("no case named {name}"));
    if let Err(reason) = outcome {
        panic!("replayed divergence for {requested}: {reason}");
    }
    println!("NOBLE_SCHEDULE_REPLAYED {requested}: agrees with the serial baseline");
}

#[test]
fn scheduler_orders_are_reproducible_and_admitted() {
    assert_eq!(permutations(4).len(), 24);
    let mut sorted = permutations(4)
        .into_iter()
        .map(|(_, order)| order)
        .collect::<Vec<_>>();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        24,
        "Heap's algorithm visits every permutation once"
    );
    assert_eq!(
        query_orders(5),
        query_orders(5),
        "seeded orders are reproducible"
    );
    let orders = query_orders(5);
    assert_eq!(orders.len() as u64, 2 + SEEDED_ORDERS);
    for (_, order) in &orders {
        let mut counted = order.clone();
        counted.sort_unstable();
        assert_eq!(
            counted,
            [0, 0, 1, 1, 2, 2, 3, 3, 4, 4],
            "each query appears exactly twice"
        );
    }
    assert_eq!(
        parse_token("graph-orders/diamond@2,0,1"),
        Some((String::from("graph-orders/diamond"), vec![2, 0, 1]))
    );
}
