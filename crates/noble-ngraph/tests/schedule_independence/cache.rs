//! F8 cache-sequences: specialization requests through one bounded cache session in serial,
//! reversed and seeded orders with repeats, under several cache limits.
//!
//! Every result must equal a cold run of the same request in a fresh session with the same
//! limits (generated artifacts byte for byte); cache status is opportunistic metadata (A4). The
//! session's memoization invariants are checked after every request.

use super::{Allowed, Case, Tally, Verdict};
use crate::common::IDS;
use noble_ngraph::specialize::cache::{Limits, Session, Status};

/// One request: original graph index, static map and residual budgets.
pub(super) struct Request {
    pub(super) graph: usize,
    pub(super) statics: Vec<Option<i64>>,
    pub(super) budgets: noble_ngraph::check::Budgets,
}

/// Every generated artifact and identity of one successful request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Artifacts {
    report: noble_ngraph::specialize::Report,
    residual_text: String,
    source: String,
    residual: noble_ngraph::ir::Digest,
    readmitted: noble_ngraph::ir::Digest,
    specialization_key: Option<noble_ngraph::ir::Digest>,
    program_key: Option<noble_ngraph::ir::Digest>,
}

/// Semantic fields of one result: the artifacts, or the refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Semantic {
    Specialized(Box<Artifacts>),
    Refused(noble_ngraph::diag::Diagnostic),
}

#[derive(Debug)]
pub(super) struct CacheSeen {
    semantic: Semantic,
    status: Option<Status>,
    /// A violated memoization invariant (criteria section 2 item 5).
    invariant: Option<String>,
}

/// One limits configuration over a fixed request list.
pub(super) struct Requests {
    pub(super) label: &'static str,
    pub(super) limits: Limits,
    pub(super) graphs: Vec<noble_ngraph::CheckedGraph>,
    pub(super) requests: Vec<Request>,
    cold: Vec<Semantic>,
}

type Outcome = Result<noble_ngraph::specialize::cache::Result, noble_ngraph::diag::Diagnostic>;

fn semantic(result: Outcome) -> (Semantic, Option<Status>) {
    match result {
        Ok(result) => (
            Semantic::Specialized(Box::new(Artifacts {
                report: result.residual.report.clone(),
                residual_text: noble_ngraph::format::graph::print(result.residual.graph()),
                source: result.source.clone(),
                residual: result.residual.checked.id(),
                readmitted: result.readmitted.id(),
                specialization_key: result.specialization_key,
                program_key: result.program_key,
            })),
            Some(result.status),
        ),
        Err(diagnostic) => (Semantic::Refused(diagnostic), None),
    }
}

/// One request through `session`; the recipe bytes are the original's canonical text.
fn specialize(
    session: &mut Session,
    original: &noble_ngraph::CheckedGraph,
    request: &Request,
) -> Outcome {
    let recipe = noble_ngraph::format::graph::print(original.graph());
    session.specialize(noble_ngraph::specialize::cache::Invocation::new(
        &IDS,
        original,
        recipe.as_bytes(),
        noble_ngraph::specialize::Request {
            static_inputs: &request.statics,
            budgets: &request.budgets,
        },
        noble_contracts::Limits::default(),
    ))
}

/// One request's semantic fields and status, for callers outside a scheduled case (F9).
pub(super) fn observe(session: &mut Session, request: &Request) -> (Semantic, Option<Status>) {
    let original = original(request.graph == 1);
    semantic(specialize(session, &original, request))
}

impl Requests {
    pub(super) fn new(
        label: &'static str,
        limits: Limits,
        graphs: Vec<noble_ngraph::CheckedGraph>,
        requests: Vec<Request>,
    ) -> Self {
        let cold = requests
            .iter()
            .map(|request| {
                let mut fresh = Session::new(limits).expect("valid cache limits");
                semantic(specialize(&mut fresh, &graphs[request.graph], request)).0
            })
            .collect();
        Self {
            label,
            limits,
            graphs,
            requests,
            cold,
        }
    }

    fn specialize(&self, session: &mut Session, index: usize) -> Outcome {
        let request = &self.requests[index];
        specialize(session, &self.graphs[request.graph], request)
    }

    /// The memoization invariants of one request, given the session before and after it.
    fn invariant(
        &self,
        before: (usize, usize),
        after: (usize, usize),
        status: Option<Status>,
        is_repeat: bool,
    ) -> Option<String> {
        if after.0 > self.limits.entries || after.1 > self.limits.retained_bytes {
            return Some(format!(
                "bounds exceeded: {after:?} under {:?}",
                self.limits
            ));
        }
        let is_unchanged = before == after;
        match status {
            None | Some(Status::Hit | Status::MissUncached | Status::BudgetBypass)
                if !is_unchanged =>
            {
                Some(format!(
                    "{status:?} changed the session from {before:?} to {after:?}"
                ))
            }
            Some(Status::Hit) if !is_repeat => {
                Some(String::from("hit before the request was made"))
            }
            Some(Status::CorruptRebuilt) => Some(String::from("an untampered entry was rebuilt")),
            _ => None,
        }
    }
}

impl Case for Requests {
    type Seen = CacheSeen;

    fn name(&self) -> String {
        format!("cache-sequences/{}", self.label)
    }

    fn items(&self) -> usize {
        self.requests.len()
    }

    fn is_query_case(&self) -> bool {
        true
    }

    fn run(&self, order: &[usize]) -> Vec<(usize, CacheSeen)> {
        let mut session = Session::new(self.limits).expect("valid cache limits");
        let mut out = Vec::with_capacity(order.len());
        for (position, index) in order.iter().enumerate() {
            let before = (session.len(), session.retained_bytes());
            let (semantic, status) = semantic(self.specialize(&mut session, *index));
            let after = (session.len(), session.retained_bytes());
            let is_repeat = order[..position].contains(index);
            let invariant = self.invariant(before, after, status, is_repeat);
            out.push((
                *index,
                CacheSeen {
                    semantic,
                    status,
                    invariant,
                },
            ));
        }
        out
    }

    fn compare(&self, query: usize, base: &CacheSeen, other: &CacheSeen) -> Verdict {
        if let Some(reason) = &other.invariant {
            return Verdict::Diverged(format!("memoization invariant: {reason}"));
        }
        Verdict::equal("cold result", &self.cold[query], &other.semantic)
            .and(Verdict::equal(
                "baseline result",
                &base.semantic,
                &other.semantic,
            ))
            .and(Verdict::allowed_if_differs(
                Allowed::Metadata,
                &base.status,
                &other.status,
            ))
    }

    fn orders(&self) -> Vec<(String, Vec<usize>)> {
        super::query_orders(self.items())
    }
}

/// `x * y + 7 + (x or y)`, an extracted-subject I64 graph the frontend can readmit.
pub(super) fn original(root_uses_y: bool) -> noble_ngraph::CheckedGraph {
    let subject = noble_ngraph::ir::Subject::derive(
        &IDS,
        noble_ngraph::ir::SubjectKind::Extracted,
        &[b"schedule-independence-cache"],
        vec![11, 12],
    );
    let mut builder =
        noble_ngraph::ir::GraphBuilder::new(noble_ngraph::ir::SubProfile::I64, subject);
    let x = builder.input(noble_ngraph::ir::ScalarType::I64);
    let y = builder.input(noble_ngraph::ir::ScalarType::I64);
    let product = builder.op(noble_ngraph::ir::Op::I64Mul(x, y));
    let seven = builder.i64_const(7);
    let captured = builder.op(noble_ngraph::ir::Op::I64Add(product, seven));
    let root = builder.op(noble_ngraph::ir::Op::I64Add(
        captured,
        if root_uses_y { y } else { x },
    ));
    builder.output(root);
    noble_ngraph::check::check(&IDS, builder.finish()).expect("cache original checks")
}

/// Shared original graphs, distinct static maps, a budget-only key change and two refusals.
pub(super) fn requests() -> Vec<Request> {
    let budgets = noble_ngraph::check::Budgets::default;
    let lowered = noble_ngraph::check::Budgets {
        residual_nodes: 64,
        ..budgets()
    };
    let starved = noble_ngraph::check::Budgets {
        residual_nodes: 1,
        ..budgets()
    };
    let request = |graph, statics: &[Option<i64>], budgets| Request {
        graph,
        statics: statics.to_vec(),
        budgets,
    };
    vec![
        request(0, &[Some(5), None], budgets()),
        request(0, &[Some(6_000_000_000), None], budgets()),
        request(0, &[None, Some(7)], budgets()),
        request(1, &[Some(5), None], budgets()),
        request(0, &[Some(5), None], lowered),
        request(0, &[Some(5)], budgets()),
        request(0, &[Some(5), None], starved),
    ]
}

fn limits() -> Vec<(&'static str, Limits)> {
    let base = Limits::default();
    let probe = Requests::new(
        "probe",
        base,
        vec![original(false), original(true)],
        requests(),
    );
    // Entry sizes of requests 0 and 3, measured in fresh sessions: a byte bound that holds both
    // but not request 3 beside the larger request 1 forces the planned two-step eviction.
    let measure = |index| {
        let mut alone = Session::default();
        probe
            .specialize(&mut alone, index)
            .expect("request specializes");
        alone.retained_bytes()
    };
    let tight = measure(0) + measure(3);
    vec![
        ("default", base),
        (
            "one-entry",
            Limits {
                entries: 1,
                variants_per_subject: 1,
                lookup_work: 1,
                eviction_work: 2,
                ..base
            },
        ),
        (
            "one-variant",
            Limits {
                variants_per_subject: 1,
                ..base
            },
        ),
        (
            "tight-eviction",
            Limits {
                entries: 2,
                variants_per_subject: 1,
                lookup_work: 2,
                eviction_work: 5,
                retained_bytes: tight,
                ..base
            },
        ),
        (
            "disabled",
            Limits {
                entries: 0,
                eviction_work: 0,
                ..base
            },
        ),
        (
            "manifest-bypass",
            Limits {
                manifest_bytes: 0,
                ..base
            },
        ),
    ]
}

pub(super) fn cases() -> Vec<Requests> {
    limits()
        .into_iter()
        .map(|(label, limits)| {
            Requests::new(
                label,
                limits,
                vec![original(false), original(true)],
                requests(),
            )
        })
        .collect()
}

/// Reruns one named schedule of this family, if the case belongs to it.
pub(super) fn replay(name: &str, order: &[usize]) -> Option<Result<(), String>> {
    let case = cases().into_iter().find(|case| case.name() == name)?;
    Some(super::replay_case(&case, order))
}

#[test]
fn cache_sequences_agree_with_cold_runs() {
    let mut tally = Tally::default();
    for case in cases() {
        let refusals = case
            .cold
            .iter()
            .filter(|seen| matches!(seen, Semantic::Refused(_)))
            .count();
        assert_eq!(
            refusals,
            2,
            "{}: the two refusing requests refuse cold",
            case.name()
        );
        tally.absorb(&super::check_case(&case));
    }
    tally.emit("F8 cache-sequences");
}
