//! F6 graph-orders and F7 net-schedules: checking and evaluating one abstract graph under every
//! admitted node order, and reducing its interaction net under every pair-selection schedule.

use super::{Allowed, Case, Tally, Verdict};
use crate::common::IDS;
use noble_ngraph::ir::{GraphBuilder, Op, ScalarType, SlotKind, SubProfile, Subject};
use noble_ngraph::sem::Value;

/// One abstract node; operands are abstract node indices.
#[derive(Clone, Copy, Debug)]
pub(super) enum Spec {
    Input(u32),
    Const(i64),
    Add(usize, usize),
    Sub(usize, usize),
    Mul(usize, usize),
    Eq(usize, usize),
    /// `guard(lo <= value <= hi)`.
    Guard(usize, i64, i64),
}

impl Spec {
    #[expect(
        tigerstyle::fragile_exhaustive_enum_match,
        reason = "Owner: noble-maintainers; a new abstract node kind must name its operands here, so the harness fails to compile until it does."
    )]
    fn operands(self) -> Vec<usize> {
        match self {
            Self::Input(_) | Self::Const(_) => vec![],
            Self::Add(a, b) | Self::Sub(a, b) | Self::Mul(a, b) | Self::Eq(a, b) => vec![a, b],
            Self::Guard(value, _, _) => vec![value],
        }
    }

    #[expect(
        tigerstyle::fragile_exhaustive_enum_match,
        reason = "Owner: noble-maintainers; a new abstract node kind must choose its graph operation here, so the harness fails to compile until it does."
    )]
    fn op(self, at: &[noble_ngraph::ir::NodeId]) -> Op {
        match self {
            Self::Input(slot) => Op::Input(slot),
            Self::Const(value) => Op::Const(noble_ngraph::ir::Literal::I64(value)),
            Self::Add(a, b) => Op::I64Add(at[a], at[b]),
            Self::Sub(a, b) => Op::I64Sub(at[a], at[b]),
            Self::Mul(a, b) => Op::I64Mul(at[a], at[b]),
            Self::Eq(a, b) => Op::I64Eq(at[a], at[b]),
            Self::Guard(value, lo, hi) => Op::Guard {
                fact: noble_ngraph::ir::Fact::Range {
                    lo: noble_ngraph::ir::Literal::I64(lo),
                    hi: noble_ngraph::ir::Literal::I64(hi),
                },
                value: at[value],
            },
        }
    }
}

/// One input row; a row with `alternatives` violates several guards at once, and its observation
/// must equal the observation of one of the listed single-violation rows in the same order.
pub(super) struct Row {
    pub(super) inputs: Vec<i64>,
    pub(super) alternatives: Vec<usize>,
}

/// An abstract I64 graph. `back` lists `(consumer, operand)` edges that close a cycle; they are
/// excluded from the order constraints, so every admitted order keeps the cycle.
pub(super) struct Shape {
    pub(super) label: &'static str,
    pub(super) slots: u32,
    pub(super) nodes: Vec<Spec>,
    pub(super) outputs: Vec<usize>,
    pub(super) back: Vec<(usize, usize)>,
    pub(super) rows: Vec<Row>,
    /// The same graph with exactly one defect repaired, one entry per defect.
    pub(super) variants: Vec<Vec<Spec>>,
}

impl Shape {
    fn constraint(&self, consumer: usize, operand: usize) -> bool {
        !self.back.contains(&(consumer, operand))
    }

    /// Whether `order` (position to abstract node) places every constrained operand first.
    fn is_admitted(&self, order: &[usize]) -> bool {
        let mut position = vec![usize::MAX; self.nodes.len()];
        for (at, node) in order.iter().enumerate() {
            position[*node] = at;
        }
        (0..self.nodes.len()).all(|node| {
            self.nodes[node].operands().into_iter().all(|operand| {
                !self.constraint(node, operand) || position[operand] < position[node]
            })
        })
    }

    /// Kahn's algorithm over the constraint edges; `pick` selects among the sorted ready nodes.
    fn admitted_order(&self, mut pick: impl FnMut(usize) -> usize) -> Vec<usize> {
        let count = self.nodes.len();
        let mut waiting = (0..count)
            .map(|node| {
                let operands = self.nodes[node].operands();
                operands
                    .into_iter()
                    .filter(|operand| self.constraint(node, *operand))
                    .count()
            })
            .collect::<Vec<_>>();
        let mut ready = (0..count)
            .filter(|node| waiting[*node] == 0)
            .collect::<Vec<_>>();
        let mut order = Vec::with_capacity(count);
        while !ready.is_empty() {
            let chosen = ready.remove(pick(ready.len()));
            order.push(chosen);
            for (consumer, node) in self.nodes.iter().enumerate() {
                for operand in node.operands() {
                    if operand == chosen && self.constraint(consumer, operand) {
                        waiting[consumer] -= 1;
                        if waiting[consumer] == 0 {
                            ready.push(consumer);
                            ready.sort_unstable();
                        }
                    }
                }
            }
        }
        assert_eq!(
            order.len(),
            count,
            "{}: constraint edges are acyclic",
            self.label
        );
        order
    }

    /// Whether `node` reaches itself through operand edges, back edges included.
    fn is_on_cycle(&self, node: usize) -> bool {
        let mut seen = vec![false; self.nodes.len()];
        let mut stack = self.nodes[node].operands();
        while let Some(next) = stack.pop() {
            if next == node {
                return true;
            }
            if !seen[next] {
                seen[next] = true;
                stack.extend(self.nodes[next].operands());
            }
        }
        false
    }
}

/// A row observation with guard nodes mapped back to abstract nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RowSeen {
    Normal(Vec<Value>),
    GuardFailed(usize),
    Other(String),
}

/// A graph observation with node sites mapped back to abstract nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ShapeSeen {
    Refused { code: String, site: String },
    Checked(Vec<RowSeen>),
}

#[derive(Debug)]
pub(super) struct Observed {
    main: ShapeSeen,
    variants: Vec<ShapeSeen>,
    /// The compiled evaluator disagreed with the reference evaluator in this order.
    disagreement: Option<String>,
}

fn row_seen(obs: &noble_ngraph::eval::Obs, order: &[usize]) -> RowSeen {
    match obs {
        noble_ngraph::eval::Obs::Normal(values) => RowSeen::Normal(values.clone()),
        noble_ngraph::eval::Obs::GuardFailed { node, .. } => {
            RowSeen::GuardFailed(order[node.index()])
        }
        other => RowSeen::Other(format!("{other:?}")),
    }
}

fn site(site: &noble_ngraph::diag::Site, order: &[usize]) -> String {
    match site {
        noble_ngraph::diag::Site::Node(node) => format!("node:{}", order[node.index()]),
        other => format!("{other:?}"),
    }
}

/// Builds `nodes` in `order` (position to abstract node) with canonical input slots.
pub(super) fn build(shape: &Shape, nodes: &[Spec], order: &[usize]) -> noble_ngraph::Graph {
    let subject = Subject::authored(&IDS, &format!("schedule-independence/{}", shape.label));
    let mut builder = GraphBuilder::new(SubProfile::I64, subject);
    for _ in 0..shape.slots {
        builder.declare_slot(SlotKind::Input, ScalarType::I64);
    }
    let mut at = vec![noble_ngraph::ir::NodeId(0); nodes.len()];
    for (position, node) in order.iter().enumerate() {
        at[*node] = noble_ngraph::ir::NodeId(u32::try_from(position).unwrap());
    }
    for node in order {
        builder.op(nodes[*node].op(&at));
    }
    for output in &shape.outputs {
        builder.output(at[*output]);
    }
    builder.finish()
}

/// Checks and evaluates one build; also reports a compiled/reference disagreement.
fn observe(shape: &Shape, nodes: &[Spec], order: &[usize]) -> (ShapeSeen, Option<String>) {
    let checked = match noble_ngraph::check::check(&IDS, build(shape, nodes, order)) {
        Ok(checked) => checked,
        Err(diagnostic) => {
            let code = format!("{:?}", diagnostic.code);
            return (
                ShapeSeen::Refused {
                    code,
                    site: site(&diagnostic.site, order),
                },
                None,
            );
        }
    };
    let compiled = noble_ngraph::compiled::Compiled::new(&IDS, &checked);
    let mut rows = Vec::with_capacity(shape.rows.len());
    let mut disagreement = None;
    for row in &shape.rows {
        let inputs = row
            .inputs
            .iter()
            .copied()
            .map(Value::I64)
            .collect::<Vec<_>>();
        let control = &mut noble_ngraph::eval::Control::default();
        let reference = noble_ngraph::eval::reference(&checked, &inputs, None, control);
        let fast = compiled.evaluate(&inputs, None, &mut noble_ngraph::eval::Control::default());
        if !reference.obs.obs_eq(&fast.obs) {
            disagreement = Some(format!(
                "reference {:?} vs compiled {:?}",
                reference.obs, fast.obs
            ));
        }
        rows.push(row_seen(&reference.obs, order));
    }
    (ShapeSeen::Checked(rows), disagreement)
}

impl Case for Shape {
    type Seen = Observed;

    fn name(&self) -> String {
        format!("graph-orders/{}", self.label)
    }

    fn items(&self) -> usize {
        self.nodes.len()
    }

    fn is_query_case(&self) -> bool {
        false
    }

    fn run(&self, order: &[usize]) -> Vec<(usize, Observed)> {
        if !self.is_admitted(order) {
            return Vec::new();
        }
        let (main, disagreement) = observe(self, &self.nodes, order);
        let variants = self
            .variants
            .iter()
            .map(|variant| observe(self, variant, order).0)
            .collect();
        vec![(
            0,
            Observed {
                main,
                variants,
                disagreement,
            },
        )]
    }

    fn compare(&self, _query: usize, base: &Observed, other: &Observed) -> Verdict {
        if let Some(reason) = &other.disagreement {
            return Verdict::Diverged(format!("evaluators disagree: {reason}"));
        }
        match (&base.main, &other.main) {
            (
                ShapeSeen::Refused { code, site },
                ShapeSeen::Refused {
                    code: other_code,
                    site: other_site,
                },
            ) => self
                .refusal_sound(other)
                .and(Verdict::equal("refusal code", code, other_code))
                .and(self.site_difference(site, other_site)),
            (ShapeSeen::Checked(rows), ShapeSeen::Checked(other_rows)) => {
                self.compare_rows(rows, other_rows)
            }
            _ => Verdict::Diverged(format!(
                "verdict class: {:?} vs {:?}",
                base.main, other.main
            )),
        }
    }

    fn orders(&self) -> Vec<(String, Vec<usize>)> {
        let seeded = usize::try_from(super::SEEDED_ORDERS).expect("seeded order count fits usize");
        let mut orders = Vec::with_capacity(1 + seeded);
        orders.push((
            String::from("reversed"),
            self.admitted_order(|ready| ready - 1),
        ));
        for round in 0..super::SEEDED_ORDERS {
            let seed = super::seed(round);
            let mut random = noble_ngraph::prng::SplitMix64::new(seed);
            let order = self.admitted_order(|ready| {
                let bound = u64::try_from(ready).expect("ready count fits u64");
                usize::try_from(random.below(bound)).expect("a pick below a usize count")
            });
            orders.push((format!("seed-{seed:#018x}"), order));
        }
        orders
    }

    fn item_schedules(&self) -> Option<u64> {
        None
    }
}

impl Shape {
    /// A refusal naming a node must name a defect: a cycle member, or one single-defect variant's
    /// own refusal in the same order.
    fn refusal_sound(&self, other: &Observed) -> Verdict {
        let ShapeSeen::Refused { code, site } = &other.main else {
            return Verdict::Same;
        };
        if code.contains("Cycle") {
            let node = site
                .strip_prefix("node:")
                .and_then(|node| node.parse::<usize>().ok());
            return match node {
                Some(node) if self.is_on_cycle(node) => Verdict::Same,
                Some(_) | None => {
                    Verdict::Diverged(format!("cycle refusal names {site}, not a cycle member"))
                }
            };
        }
        if !self.variants.is_empty() && !other.variants.contains(&other.main) {
            return Verdict::Diverged(format!("{:?} is no single-defect refusal", other.main));
        }
        Verdict::Same
    }

    fn site_difference(&self, site: &str, other: &str) -> Verdict {
        if site == other {
            Verdict::Same
        } else if !self.variants.is_empty() || !self.back.is_empty() {
            Verdict::Allowed(vec![Allowed::FirstDefect])
        } else {
            Verdict::Diverged(format!("refusal site: baseline {site}, other {other}"))
        }
    }

    fn compare_rows(&self, rows: &[RowSeen], other: &[RowSeen]) -> Verdict {
        let mut verdict = Verdict::equal("row count", &rows.len(), &other.len());
        for (index, row) in self.rows.iter().enumerate() {
            if row.alternatives.is_empty() {
                verdict = verdict.and(Verdict::equal("row", &rows[index], &other[index]));
            } else if row
                .alternatives
                .iter()
                .any(|alt| other[*alt] == other[index])
            {
                verdict = verdict.and(Verdict::allowed_if_differs(
                    Allowed::FirstDefect,
                    &rows[index],
                    &other[index],
                ));
            } else {
                verdict = verdict.and(Verdict::Diverged(format!(
                    "row {index}: {:?} is no single-violation observation",
                    other[index]
                )));
            }
        }
        verdict
    }
}

fn row(inputs: &[i64]) -> Row {
    Row {
        inputs: inputs.to_vec(),
        alternatives: vec![],
    }
}

/// The family's shapes: shared dependencies, cycles and partial defects at check and run time.
pub(super) fn shapes() -> Vec<Shape> {
    use Spec::{Add, Const, Eq, Guard, Input, Mul, Sub};
    vec![
        Shape {
            label: "shared-diamond",
            slots: 2,
            nodes: vec![
                Input(0),
                Input(1),
                Add(0, 1),
                Mul(2, 0),
                Mul(2, 1),
                Sub(3, 4),
                Eq(5, 2),
            ],
            outputs: vec![5, 6, 2],
            back: vec![],
            rows: vec![row(&[3, 4]), row(&[i64::MAX, 2]), row(&[-7, 7])],
            variants: vec![],
        },
        Shape {
            label: "fanout",
            slots: 1,
            nodes: vec![
                Input(0),
                Mul(0, 0),
                Add(1, 0),
                Sub(1, 0),
                Mul(2, 3),
                Const(9),
                Add(4, 5),
            ],
            outputs: vec![6, 1],
            back: vec![],
            rows: vec![row(&[5]), row(&[-3]), row(&[i64::MIN])],
            variants: vec![],
        },
        Shape {
            label: "two-node-cycle",
            slots: 1,
            nodes: vec![Input(0), Add(0, 2), Add(0, 1), Mul(0, 0)],
            outputs: vec![1, 3],
            back: vec![(1, 2)],
            rows: vec![],
            variants: vec![],
        },
        Shape {
            label: "self-cycle",
            slots: 1,
            nodes: vec![Input(0), Add(1, 0), Mul(0, 0)],
            outputs: vec![1, 2],
            back: vec![(1, 1)],
            rows: vec![],
            variants: vec![],
        },
        Shape {
            label: "two-type-defects",
            slots: 2,
            nodes: vec![
                Input(0),
                Input(1),
                Eq(0, 1),
                Add(0, 2),
                Mul(2, 1),
                Add(3, 4),
            ],
            outputs: vec![5],
            back: vec![],
            rows: vec![],
            variants: vec![
                vec![
                    Input(0),
                    Input(1),
                    Eq(0, 1),
                    Add(0, 1),
                    Mul(2, 1),
                    Add(3, 4),
                ],
                vec![
                    Input(0),
                    Input(1),
                    Eq(0, 1),
                    Add(0, 2),
                    Mul(0, 1),
                    Add(3, 4),
                ],
            ],
        },
        Shape {
            label: "two-guards",
            slots: 2,
            nodes: vec![
                Input(0),
                Input(1),
                Guard(0, 0, 10),
                Guard(1, 0, 10),
                Add(0, 1),
            ],
            outputs: vec![4],
            back: vec![],
            rows: vec![
                row(&[3, 4]),
                row(&[20, 5]),
                row(&[5, 30]),
                Row {
                    inputs: vec![20, 30],
                    alternatives: vec![1, 2],
                },
            ],
            variants: vec![],
        },
    ]
}

/// F7: one net reduced under every pair-selection schedule, uncapped, with interaction fuel one
/// below its interaction count, and with a worklist cap at the smallest queue high-water mark
/// any schedule reaches.
pub(super) struct NetSchedules {
    label: &'static str,
    checked: noble_ngraph::CheckedGraph,
    net: noble_ngraph::net::InteractionGraph,
    inputs: Vec<Value>,
    lifo: NetSeen,
    lifo_fuel_capped: NetSeen,
    worklist_cap: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NetSeen {
    obs: noble_ngraph::net::Obs,
    interactions: u64,
    final_wires: Option<u64>,
    peak_live_agents: u64,
    queue_high_water: u64,
}

/// The resource variant of one F7 item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cap {
    None,
    Fuel,
    Worklist,
}

const CAPS: [Cap; 3] = [Cap::None, Cap::Fuel, Cap::Worklist];
/// Lifo, Fifo and the seeded schedules.
const NET_SCHEDULES: u64 = 2 + super::SEEDED_ORDERS;

/// Item `index` is schedule `index / 3` (Lifo, Fifo, then seeded SplitMix64) under cap
/// `index % 3`.
fn pair_selection(index: usize) -> (noble_ngraph::net::Schedule, Cap) {
    let selection = index.checked_div(CAPS.len()).expect("three caps");
    let cap = CAPS[index.checked_rem(CAPS.len()).expect("three caps")];
    let schedule = match selection {
        0 => noble_ngraph::net::Schedule::Lifo,
        1 => noble_ngraph::net::Schedule::Fifo,
        seeded => noble_ngraph::net::Schedule::SplitMix64 {
            seed: super::seed(u64::try_from(seeded - 2).expect("seed round fits u64")),
        },
    };
    (schedule, cap)
}

/// One reduction of `net` under `schedule`, optional interaction fuel and optional worklist cap.
fn reduce(
    checked: &noble_ngraph::CheckedGraph,
    net: &noble_ngraph::net::InteractionGraph,
    inputs: &[Value],
    (schedule, fuel, worklist): (noble_ngraph::net::Schedule, Option<u64>, Option<u64>),
) -> NetSeen {
    let limits = noble_ngraph::net::Limits::default();
    let report = noble_ngraph::net::reduce(
        &IDS,
        noble_ngraph::net::ReduceRequest {
            checked,
            candidate: net,
            inputs,
            limits: &limits,
            plan: noble_ngraph::net::Plan {
                schedule: Some(schedule),
                interaction_fuel: fuel,
                max_worklist_entries: worklist,
                ..noble_ngraph::net::Plan::default()
            },
        },
    );
    NetSeen {
        obs: report.obs,
        interactions: report.interactions,
        final_wires: report.final_wires,
        peak_live_agents: report.peak_live_agents,
        queue_high_water: report.plan_stats.queue_high_water,
    }
}

impl NetSchedules {
    fn new(shape: &Shape, row: &Row) -> Self {
        let order = (0..shape.nodes.len()).collect::<Vec<_>>();
        let checked = noble_ngraph::check::check(&IDS, build(shape, &shape.nodes, &order))
            .expect("net shapes check");
        let inputs = row
            .inputs
            .iter()
            .copied()
            .map(Value::I64)
            .collect::<Vec<_>>();
        let limits = noble_ngraph::net::Limits::default();
        let net = noble_ngraph::net::lower(&IDS, &checked, &inputs, &limits).expect("net lowers");
        let lifo = reduce(
            &checked,
            &net,
            &inputs,
            (noble_ngraph::net::Schedule::Lifo, None, None),
        );
        let cap = lifo.interactions.checked_sub(1).expect("the net fires");
        let lifo_fuel_capped = reduce(
            &checked,
            &net,
            &inputs,
            (noble_ngraph::net::Schedule::Lifo, Some(cap), None),
        );
        let schedules = usize::try_from(NET_SCHEDULES).expect("schedule count fits usize");
        let worklist_cap = (0..schedules)
            .map(|index| {
                reduce(
                    &checked,
                    &net,
                    &inputs,
                    (pair_selection(index * CAPS.len()).0, None, None),
                )
            })
            .map(|seen| seen.queue_high_water)
            .min()
            .expect("schedules exist");
        let control = &mut noble_ngraph::eval::Control::default();
        let reference = noble_ngraph::eval::reference(&checked, &inputs, None, control);
        let net_values = if let noble_ngraph::net::Obs::Normal(values) = &lifo.obs {
            Some(values.as_slice())
        } else {
            None
        };
        assert_eq!(
            reference.obs.outputs(),
            net_values,
            "{}: the net computes the reference outputs",
            shape.label
        );
        Self {
            label: shape.label,
            checked,
            net,
            inputs,
            lifo,
            lifo_fuel_capped,
            worklist_cap,
        }
    }
}

impl Case for NetSchedules {
    type Seen = NetSeen;

    fn name(&self) -> String {
        let row = self
            .inputs
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>();
        format!("net-schedules/{}/{}", self.label, row.join(","))
    }

    fn items(&self) -> usize {
        CAPS.len() * usize::try_from(NET_SCHEDULES).expect("schedule count fits usize")
    }

    fn is_query_case(&self) -> bool {
        true
    }

    fn run(&self, order: &[usize]) -> Vec<(usize, NetSeen)> {
        order
            .iter()
            .map(|index| {
                let (schedule, cap) = pair_selection(*index);
                let fuel = (cap == Cap::Fuel).then(|| self.lifo.interactions - 1);
                let worklist = (cap == Cap::Worklist).then_some(self.worklist_cap);
                let seen = reduce(
                    &self.checked,
                    &self.net,
                    &self.inputs,
                    (schedule, fuel, worklist),
                );
                (*index, seen)
            })
            .collect()
    }

    fn compare(&self, query: usize, _base: &NetSeen, other: &NetSeen) -> Verdict {
        let exhausted = if let noble_ngraph::net::Obs::Exhausted { limit, .. } = &other.obs {
            Some(*limit)
        } else {
            None
        };
        let cap = pair_selection(query).1;
        if cap == Cap::Fuel {
            // Every reduction fires the same interactions, so fuel one short refuses everywhere.
            return if exhausted == Some(noble_ngraph::net::Limit::Fuel) {
                Verdict::equal(
                    "fuel-capped observation",
                    &self.lifo_fuel_capped.obs,
                    &other.obs,
                )
            } else {
                Verdict::Diverged(format!("fuel one below the interactions: {other:?}"))
            };
        }
        if cap == Cap::Worklist && exhausted == Some(noble_ngraph::net::Limit::WorklistReservation)
        {
            return Verdict::Allowed(vec![Allowed::Exhaustion]);
        }
        let semantic = |seen: &NetSeen| {
            (
                seen.obs.clone(),
                seen.interactions,
                seen.final_wires,
                seen.peak_live_agents,
            )
        };
        Verdict::equal("net observation", &semantic(&self.lifo), &semantic(other)).and(
            Verdict::allowed_if_differs(
                Allowed::Metadata,
                &self.lifo.queue_high_water,
                &other.queue_high_water,
            ),
        )
    }

    fn orders(&self) -> Vec<(String, Vec<usize>)> {
        vec![(String::from("reversed"), (0..self.items()).rev().collect())]
    }

    fn item_schedules(&self) -> Option<u64> {
        Some(NET_SCHEDULES)
    }
}

fn net_cases() -> Vec<NetSchedules> {
    shapes()
        .iter()
        .filter(|shape| shape.label == "shared-diamond" || shape.label == "fanout")
        .flat_map(|shape| shape.rows.iter().map(|row| NetSchedules::new(shape, row)))
        .collect()
}

/// Reruns one named schedule of these families, if the case belongs to them.
pub(super) fn replay(name: &str, order: &[usize]) -> Option<Result<(), String>> {
    if let Some(case) = shapes().into_iter().find(|case| case.name() == name) {
        return Some(super::replay_case(&case, order));
    }
    let case = net_cases().into_iter().find(|case| case.name() == name)?;
    Some(super::replay_case(&case, order))
}

#[test]
fn node_orders_agree() {
    let mut tally = Tally::default();
    for case in shapes() {
        tally.absorb(&super::check_case(&case));
    }
    tally.emit("F6 graph-orders");
}

#[test]
fn net_schedules_agree_and_exhaust_together() {
    let mut tally = Tally::default();
    for case in net_cases() {
        tally.absorb(&super::check_case(&case));
    }
    tally.emit("F7 net-schedules");
}
