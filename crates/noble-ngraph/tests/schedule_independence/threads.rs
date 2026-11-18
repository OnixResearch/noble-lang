//! F2 kernel-threads and F9 threads: concurrent requests for the same query.
//!
//! Every round starts four workers that pass a barrier before each query, so all four issue the
//! same query at the same time. The kernel checker shares one environment, request and candidate
//! between the workers; the frontend, graph and cache queries need non-`Send` state, so each
//! worker builds its own from the same plain inputs (no shared session exists to share).

use super::Tally;
use crate::common::IDS;

const WORKERS: usize = 4;
const ROUNDS: u64 = 8;

/// Runs `queries` serially, then `ROUNDS` times on `WORKERS` barrier-synchronized threads, and
/// requires every threaded observation to equal the serial one.
fn concurrently<S, F>(family: &str, queries: usize, observe: F) -> Tally
where
    S: PartialEq + core::fmt::Debug + Send,
    F: Fn(usize) -> S + Sync,
{
    let serial = (0..queries).map(&observe).collect::<Vec<_>>();
    let mut tally = Tally::default();
    for (query, observation) in serial.iter().enumerate() {
        tally.row(super::observed_row(
            &format!("{family}/{query}"),
            format!("{family}#serial"),
            &format!("{observation:?}"),
            query,
        ));
    }
    for round in 0..ROUNDS {
        let barrier = std::sync::Barrier::new(WORKERS);
        let seen = std::thread::scope(|scope| {
            let workers = (0..WORKERS)
                .map(|_| {
                    scope.spawn(|| {
                        (0..queries)
                            .map(|query| {
                                barrier.wait();
                                observe(query)
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>();
            workers
                .into_iter()
                .map(|worker| worker.join().expect("worker finished"))
                .collect::<Vec<_>>()
        });
        for (worker, observations) in seen.iter().enumerate() {
            for (query, observation) in observations.iter().enumerate() {
                assert_eq!(
                    observation, &serial[query],
                    "{family}: round {round}, worker {worker}, query {query} diverged from the serial run"
                );
                tally.row(super::observed_row(
                    &format!("{family}/{query}"),
                    format!("{family}#round-{round}/worker-{worker}"),
                    &format!("{observation:?}"),
                    query,
                ));
            }
        }
    }
    let runs = ROUNDS * WORKERS as u64;
    tally.record(queries as u64, runs, runs * queries as u64);
    tally
}

/// `41 1 add`, accepted with three derivations.
fn arithmetic() -> noble_kernel::untrusted::Candidate {
    let segment = |list| noble_kernel::words::Binding::Stack(list);
    let literal = |value, under| noble_kernel::untrusted::Node::Literal {
        lit: noble_kernel::untrusted::Lit::I64(value),
        inst: noble_kernel::words::Inst {
            bindings: vec![segment(under)],
        },
    };
    noble_kernel::untrusted::Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision: noble_kernel::untrusted::SEMANTIC_REVISION,
        nodes: vec![
            literal(41, vec![]),
            literal(1, vec![noble_kernel::types::Ty::I64]),
            noble_kernel::untrusted::Node::Invocation {
                def: noble_kernel::contracts::Definition(4),
                inst: noble_kernel::words::Inst {
                    bindings: vec![segment(vec![])],
                },
            },
        ],
        body: [0, 1, 2]
            .into_iter()
            .map(noble_kernel::untrusted::NodeId)
            .collect(),
    }
}

#[test]
fn kernel_checks_agree_under_concurrent_identical_requests() {
    let graphs = super::kernel::graphs();
    let environments = graphs
        .iter()
        .map(|(label, deps)| {
            let shape = super::kernel::Dependencies {
                label,
                deps: deps.clone(),
                is_reversed: false,
                work: super::kernel::AMPLE_WORK,
            };
            shape.environment(&(0..deps.len()).collect::<Vec<_>>())
        })
        .collect::<Vec<_>>();
    let mut request = super::kernel::request(super::kernel::AMPLE_WORK);
    request.expected.stack_out = vec![noble_kernel::types::Ty::I64];
    let program = arithmetic();
    let empty = super::kernel::empty_program();
    let empty_request = super::kernel::request(super::kernel::AMPLE_WORK);
    let tally = concurrently("F2 kernel-threads", 2 * environments.len(), |query| {
        let env = &environments[query / 2];
        let outcome = if query % 2 == 0 {
            noble_kernel::acceptance::check_with_work(env, &request, &program)
        } else {
            noble_kernel::acceptance::check_with_work(env, &empty_request, &empty)
        };
        format!("{outcome:?}")
    });
    tally.emit("F2 kernel-threads");
}

#[test]
fn frontend_graph_and_cache_queries_agree_under_concurrent_identical_requests() {
    let mut tally = Tally::default();
    let queries = super::contracts::shared_queries();
    tally.absorb(&concurrently(
        "F9 threads/contracts",
        queries.queries.len(),
        |query| {
            let session =
                super::contracts::session(&queries.definitions).expect("definitions commit");
            format!(
                "{:?}",
                session.prepare(
                    queries.queries[query],
                    &[],
                    noble_contracts::Limits::default()
                )
            )
        },
    ));
    let shapes = super::graph::shapes();
    tally.absorb(&concurrently("F9 threads/graphs", shapes.len(), |query| {
        let order = (0..shapes[query].nodes.len()).collect::<Vec<_>>();
        let graph = super::graph::build(&shapes[query], &shapes[query].nodes, &order);
        match noble_ngraph::check::check(&IDS, graph) {
            Ok(checked) => {
                let inputs = vec![noble_ngraph::sem::Value::I64(3); checked.graph().slots.len()];
                let control = &mut noble_ngraph::eval::Control::default();
                format!(
                    "{:?}",
                    noble_ngraph::eval::reference(&checked, &inputs, None, control)
                )
            }
            Err(diagnostic) => format!("{diagnostic:?}"),
        }
    }));
    let requests = super::cache::requests();
    tally.absorb(&concurrently("F9 threads/cache", requests.len(), |query| {
        let mut session = noble_ngraph::specialize::cache::Session::default();
        let first = super::cache::observe(&mut session, &requests[query]);
        let repeat = super::cache::observe(&mut session, &requests[query]);
        format!("{first:?} / {repeat:?}")
    }));
    tally.emit("F9 threads");
}
