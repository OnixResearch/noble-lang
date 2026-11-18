//! Order-variation harness for issue #134, governed by the frozen
//! `verification/schedule-independence/criteria.md` (revision 1).
//!
//! A *case* is a list of items plus a runner. Query cases run their queries in an order (repeats
//! allowed) and observe each query; renumbering cases rebuild one equivalent input from a
//! permutation of its items and observe the whole input. The serial order is the baseline; every
//! other admitted order is compared with it. A comparison either agrees, differs in a classified
//! allowed way (section 3 of the criteria, counted), or diverges. A divergence is minimized and
//! reported with a replay token; `NOBLE_SCHEDULE_REPLAY=<case>@<order>` reruns that schedule.
//!
//! Every observation also emits one row of the table oracle frozen in
//! `verification/schedule-independence/criteria-oracle.md` (revision 2): the input's digest, the
//! schedule, the normalized-output digest and the presentation digest. A family's table is checked
//! when its tally is emitted: the normalized output of an input must be a single function of it,
//! and presentation differences are counted as allowed.

mod cache;
mod contracts;
mod graph;
mod kernel;
mod retained;
mod table;
mod threads;

use std::collections::{BTreeMap, BTreeSet};

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
    /// Every class with its receipt label, in criteria order.
    const ALL: [(Self, &'static str); 5] = [
        (Self::Numbering, "a1_numbering"),
        (Self::FirstDefect, "a2_first_defect"),
        (Self::Exhaustion, "a3_exhaustion"),
        (Self::Metadata, "a4_metadata"),
        (Self::SharedPool, "a5_shared_pool"),
    ];

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|(class, _)| *class == self)
            .expect("every class is listed")
    }
}

/// The normalized output of one observation: the criteria section 2 projection, or the section 3
/// class that exempts the row from the functional-dependence check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Normalized {
    /// A row the table compares: the section 2 projection as text.
    Output(String),
    /// A row with no normalized output because the criteria permit it to differ (A3).
    Exempt(Allowed),
}

/// The table-level oracle projections of one case (criteria-oracle section 1).
pub(super) trait Tabulate: Case {
    /// The semantic identity of the observed input, independent of order and numbering.
    fn input(&self, query: usize) -> String;
    /// The section 2 projection, or the class that exempts the observation.
    fn normalized(&self, query: usize, seen: &Self::Seen) -> Normalized;
    /// The section 3 fields the criteria allow to vary.
    fn presentation(&self, query: usize, seen: &Self::Seen) -> String;
    /// The schedule identifier of one row: the case, the admitted order and the row's position.
    fn schedule(&self, _query: usize, label: &str, position: usize) -> String {
        format!("{}#{label}#{position}", self.name())
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
    fn item_schedules(&self) -> Option<u64>;
}

/// Counts for one family, emitted as one receipt line.
#[derive(Debug, Default)]
pub(super) struct Tally {
    cases: u64,
    schedules: u64,
    comparisons: u64,
    allowed: [u64; 5],
    rows: Vec<Row>,
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
        self.rows.extend(other.rows.iter().cloned());
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

    /// Records one table row.
    pub(super) fn row(&mut self, row: Row) {
        self.rows.push(row);
    }

    /// The oracle analysis of every row this family emitted (criteria-oracle section 2).
    fn analyse(&self) -> Analysis {
        analyse(&self.rows)
    }

    /// The `NOBLE_SCHEDULE_FAMILY` receipt line of one family; zero divergences by construction,
    /// because a divergence fails the test before the line is printed.
    pub(super) fn emit(&self, family: &str) {
        assert!(self.cases > 0, "{family}: no case ran");
        assert!(self.comparisons > 0, "{family}: nothing was compared");
        let allowed = Allowed::ALL
            .iter()
            .map(|(class, label)| format!("\"{label}\":{}", self.allowed[class.index()]))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "NOBLE_SCHEDULE_FAMILY {{\"family\":\"{family}\",\"cases\":{},\"schedules\":{},\"comparisons\":{},\"divergences\":0,\"allowed\":{{{allowed}}}}}",
            self.cases, self.schedules, self.comparisons
        );
        self.emit_oracle(family);
    }

    /// Prints and checks the family's table (criteria-oracle sections 2 and 3).
    fn emit_oracle(&self, family: &str) {
        assert!(!self.rows.is_empty(), "{family}: no oracle row was emitted");
        let analysis = self.analyse();
        let exempt = Allowed::ALL
            .iter()
            .map(|(class, label)| format!("\"{label}\":{}", analysis.exempt[class.index()]))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "NOBLE_SCHEDULE_ORACLE {{\"family\":\"{family}\",\"rows\":{},\"groups\":{},\"normalized_conflicts\":{},\"presentation_difference_groups\":{},\"exhaustion_difference_groups\":{},\"exempt\":{{{exempt}}}}}",
            self.rows.len(),
            analysis.groups,
            analysis.conflicts.len(),
            analysis.presentation_differences,
            analysis.exhaustion_differences
        );
        for pair in &analysis.conflicts {
            println!("NOBLE_SCHEDULE_ORACLE_CONFLICT {}", pair.json());
        }
        assert!(
            analysis.conflicts.is_empty(),
            "{family}: {} normalized-output conflict(s); the retained rows are printed above",
            analysis.conflicts.len()
        );
    }
}

/// One row of the table oracle (criteria-oracle section 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Row {
    /// Digest of the observed input's semantic identity.
    input: String,
    /// The schedule that produced this row.
    schedule: String,
    /// Digest of the normalized output; `None` when the row is exempt.
    normalized: Option<String>,
    /// The class that exempts the row from the functional-dependence check.
    exempt: Option<Allowed>,
    /// Digest of the presentation projection.
    presentation: String,
    /// The observed query or item index.
    query: usize,
    /// The replay token of the run that produced the row; `None` when the schedule is not an order.
    replay: Option<String>,
}

impl Row {
    fn json(&self) -> String {
        let normalized = self.normalized.as_deref().unwrap_or("");
        let replay = self.replay.as_deref().unwrap_or("");
        format!(
            "{{\"input\":\"{}\",\"schedule\":\"{}\",\"normalized\":\"{normalized}\",\"presentation\":\"{}\",\"query\":{},\"replay\":\"{replay}\"}}",
            self.input, self.schedule, self.presentation, self.query
        )
    }
}

/// Two rows of one input with different normalized outputs: the retained counterexample.
#[derive(Clone, Debug)]
pub(super) struct Pair {
    pub(super) input: String,
    pub(super) first: Row,
    pub(super) second: Row,
}

impl Pair {
    fn json(&self) -> String {
        format!(
            "{{\"input\":\"{}\",\"rows\":[{},{}]}}",
            self.input,
            self.first.json(),
            self.second.json()
        )
    }
}

/// The result of checking one family's table (criteria-oracle section 2).
#[derive(Debug, Default)]
pub(super) struct Analysis {
    pub(super) groups: u64,
    pub(super) presentation_differences: u64,
    pub(super) exhaustion_differences: u64,
    pub(super) exempt: [u64; 5],
    pub(super) conflicts: Vec<Pair>,
}

/// The table-level oracle's digest: 128 bits of FNV-1a over the domain and the text.
pub(super) fn digest(text: &str) -> String {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let stream = |basis: u64| {
        let mut hash = basis;
        for byte in b"noble.schedule.v1".iter().chain(text.as_bytes()) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
        hash
    };
    format!(
        "{:016x}{:016x}",
        stream(0xcbf2_9ce4_8422_2325),
        stream(0x8422_2325_cbf2_9ce4)
    )
}

/// Groups one family's rows by input and checks the functional dependence.
fn analyse(rows: &[Row]) -> Analysis {
    let mut by_input: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for row in rows {
        by_input.entry(row.input.as_str()).or_default().push(row);
    }
    let mut analysis = Analysis {
        groups: u64::try_from(by_input.len()).expect("group count fits u64"),
        ..Analysis::default()
    };
    for (input, group) in by_input {
        let compared = group
            .iter()
            .copied()
            .filter(|row| row.normalized.is_some())
            .collect::<Vec<_>>();
        for row in &group {
            if let Some(class) = row.exempt {
                analysis.exempt[class.index()] += 1;
            }
        }
        if compared.len() < group.len() && !compared.is_empty() {
            analysis.exhaustion_differences += 1;
        }
        let presentations = if compared.is_empty() {
            group.clone()
        } else {
            compared.clone()
        };
        let distinct = presentations
            .iter()
            .map(|row| row.presentation.as_str())
            .collect::<BTreeSet<_>>();
        if distinct.len() > 1 {
            analysis.presentation_differences += 1;
        }
        let outputs = compared
            .iter()
            .filter_map(|row| row.normalized.as_deref())
            .collect::<BTreeSet<_>>();
        if outputs.len() > 1 {
            let mut ordered = compared.clone();
            ordered.sort_by(|first, second| {
                (first.schedule.as_str(), first.normalized.as_deref())
                    .cmp(&(second.schedule.as_str(), second.normalized.as_deref()))
            });
            let first = ordered[0];
            let second = ordered
                .iter()
                .find(|row| row.normalized != first.normalized)
                .expect("the group has two normalized outputs");
            analysis.conflicts.push(Pair {
                input: input.to_string(),
                first: first.clone(),
                second: (**second).clone(),
            });
        }
    }
    analysis
}

/// A row for a schedule with no order token (the thread families).
pub(super) fn observed_row(input: &str, schedule: String, normalized: &str, query: usize) -> Row {
    Row {
        input: digest(input),
        schedule,
        normalized: Some(digest(normalized)),
        exempt: None,
        presentation: digest(""),
        query,
        replay: None,
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
        let bound = u64::try_from(index).expect("order length fits u64");
        let pick = usize::try_from(random.below(bound)).expect("a pick below a usize length");
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

/// The table rows of one run, in run order (criteria-oracle section 1).
fn rows<C: Tabulate>(
    case: &C,
    label: &str,
    order: &[usize],
    observed: &[(usize, C::Seen)],
) -> Vec<Row> {
    let replay = token(&case.name(), order);
    observed
        .iter()
        .enumerate()
        .map(|(position, (query, seen))| {
            let normalized = case.normalized(*query, seen);
            Row {
                input: digest(&case.input(*query)),
                schedule: case.schedule(*query, label, position),
                normalized: match &normalized {
                    Normalized::Output(text) => Some(digest(text)),
                    Normalized::Exempt(_) => None,
                },
                exempt: match normalized {
                    Normalized::Output(_) => None,
                    Normalized::Exempt(class) => Some(class),
                },
                presentation: digest(&case.presentation(*query, seen)),
                query: *query,
                replay: Some(replay.clone()),
            }
        })
        .collect()
}

/// Every row of one case: the serial baseline and every admitted order, without comparing.
pub(super) fn rows_of<C: Tabulate>(case: &C) -> Vec<Row> {
    let baseline = serial(case.items());
    let mut table = rows(case, "serial", &baseline, &case.run(&baseline));
    for (label, order) in case.orders() {
        table.extend(rows(case, &label, &order, &case.run(&order)));
    }
    table
}

/// Runs one case over its admitted orders; panics with a minimized replay token on divergence.
pub(super) fn check_case<C: Tabulate>(case: &C) -> Tally {
    let base = baselines(case);
    let mut tally = Tally {
        cases: 1,
        schedules: 1,
        ..Tally::default()
    };
    let serial_order = serial(case.items());
    for row in rows(case, "serial", &serial_order, &base) {
        tally.row(row);
    }
    for (query, observation) in &base {
        let schedule = ("serial", serial_order.clone());
        settle(
            case,
            &mut tally,
            (*query, observation, observation),
            (schedule.0, &schedule.1),
        );
    }
    for (label, order) in case.orders() {
        tally.schedules += 1;
        let observed = case.run(&order);
        for row in rows(case, &label, &order, &observed) {
            tally.row(row);
        }
        for (query, observation) in observed {
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
    let verdict = case.compare(query, base, other);
    if let Verdict::Diverged(reason) = &verdict {
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
    if let Verdict::Allowed(classes) = verdict {
        for class in classes {
            tally.count(class);
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
fn replay_requested_order() {
    let Ok(requested) = std::env::var("NOBLE_SCHEDULE_REPLAY") else {
        return;
    };
    let (name, order) = parse_token(&requested).expect("token is <case>@<positions>");
    let outcome = kernel::replay(&name, &order)
        .or_else(|| contracts::replay(&name, &order))
        .or_else(|| graph::replay(&name, &order))
        .or_else(|| cache::replay(&name, &order))
        .or_else(|| table::replay(&name, &order))
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
