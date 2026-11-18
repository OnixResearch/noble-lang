//! F3 contracts-queries, F4 contracts-commits and F5 contracts-modules: the source frontend's
//! queries, commit orders and declared-module definition orders.

use super::{Allowed, Case, Normalized, Tabulate, Tally, Verdict};

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

/// A rendered query result: exact for one session state, normalized across commit orders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Rendered {
    Prepared(String),
    Refused {
        stage: String,
        kind: String,
        diagnostic: String,
    },
}

fn refused(error: &noble_contracts::source::Error) -> Rendered {
    Rendered::Refused {
        stage: format!("{:?}", error.stage()),
        kind: format!("{:?}", error.diagnostic().kind),
        diagnostic: format!("{:?}", error.diagnostic()),
    }
}

/// Every field of a preparation that is independent of definition numbering.
fn normalized(prepared: &noble_contracts::source::Prepared) -> String {
    format!(
        "output={:?} definition={:?} interface={:?} checked={:?}",
        prepared.output(),
        prepared.definition_name(),
        prepared.submission_interface(),
        prepared.checked().map(|checked| &checked.interface),
    )
}

/// A core session with `definitions` committed in order; `None` when a commit is refused.
pub(super) fn session(definitions: &[&[u8]]) -> Option<noble_contracts::source::Session> {
    let mut session = noble_contracts::source::Session::new();
    for text in definitions {
        let prepared = session.prepare(text, &[], LIMITS).ok()?;
        session.commit(prepared).ok()?;
    }
    Some(session)
}

/// F3: a fixed query set against one session, in serial, reversed and seeded orders with repeats.
pub(super) struct Queries {
    pub(super) label: &'static str,
    pub(super) definitions: Vec<&'static [u8]>,
    pub(super) queries: Vec<&'static [u8]>,
}

/// A preparation's complete `Debug` rendering (every field, including generation and history),
/// or the rendered refusal.
#[derive(Debug)]
pub(super) enum QuerySeen {
    Prepared(String),
    Refused(Rendered),
}

impl Case for Queries {
    type Seen = QuerySeen;

    fn name(&self) -> String {
        format!("contracts-queries/{}", self.label)
    }

    fn items(&self) -> usize {
        self.queries.len()
    }

    fn is_query_case(&self) -> bool {
        true
    }

    fn run(&self, order: &[usize]) -> Vec<(usize, QuerySeen)> {
        let session = session(&self.definitions).expect("fixture definitions commit");
        order
            .iter()
            .map(|query| {
                let seen = match session.prepare(self.queries[*query], &[], LIMITS) {
                    Ok(prepared) => QuerySeen::Prepared(format!("{prepared:?}")),
                    Err(error) => QuerySeen::Refused(refused(&error)),
                };
                (*query, seen)
            })
            .collect()
    }

    fn compare(&self, _query: usize, base: &QuerySeen, other: &QuerySeen) -> Verdict {
        match (base, other) {
            (QuerySeen::Prepared(first), QuerySeen::Prepared(second)) => {
                Verdict::equal("preparation", first, second)
            }
            (QuerySeen::Refused(first), QuerySeen::Refused(second)) => {
                Verdict::equal("refusal", first, second)
            }
            _ => Verdict::Diverged(format!("verdict class: baseline {base:?}, other {other:?}")),
        }
    }

    fn orders(&self) -> Vec<(String, Vec<usize>)> {
        super::query_orders(self.items())
    }

    fn item_schedules(&self) -> Option<u64> {
        None
    }
}

impl Tabulate for Queries {
    fn input(&self, query: usize) -> String {
        format!("contracts-queries/{}/{:?}", self.label, self.queries[query])
    }

    fn normalized(&self, _query: usize, seen: &QuerySeen) -> Normalized {
        // Revision 1 compares the complete rendering exactly, so it is all normalized output.
        Normalized::Output(match seen {
            QuerySeen::Prepared(text) => format!("prepared:{text}"),
            QuerySeen::Refused(rendered) => format!("refused:{rendered:?}"),
        })
    }

    fn presentation(&self, _query: usize, _seen: &QuerySeen) -> String {
        String::new()
    }

    fn schedule(&self, _query: usize, label: &str, position: usize) -> String {
        super::schedule_id(&self.name(), label, position)
    }
}

/// F3 on a declared-module session whose queries share one request-work pool (A5): with an
/// ample pool the results agree, while the remaining balance depends on the query history.
pub(super) struct Pooled {
    pub(super) label: &'static str,
    pub(super) module: &'static [u8],
    pub(super) pool: u32,
    pub(super) queries: Vec<&'static [u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PooledSeen {
    result: Rendered,
    remaining: Option<u32>,
}

impl Case for Pooled {
    type Seen = PooledSeen;

    fn name(&self) -> String {
        format!("contracts-queries/{}", self.label)
    }

    fn items(&self) -> usize {
        self.queries.len()
    }

    fn is_query_case(&self) -> bool {
        true
    }

    fn run(&self, order: &[usize]) -> Vec<(usize, PooledSeen)> {
        let session =
            noble_contracts::source::ModuleSession::new(&[], retained()).expect("session");
        let prepared = session
            .prepare(self.module, &[], LIMITS)
            .expect("module prepares");
        let (mut session, committed) = session.commit(prepared);
        committed.expect("module commits");
        session.share_request_work(self.pool);
        order
            .iter()
            .map(|query| {
                let result = match session.prepare(self.queries[*query], &[], LIMITS) {
                    Ok(prepared) => Rendered::Prepared(format!("{:?}", prepared.output())),
                    Err(error) => refused(&error),
                };
                let remaining = session.remaining_request_work();
                (*query, PooledSeen { result, remaining })
            })
            .collect()
    }

    fn compare(&self, _query: usize, base: &PooledSeen, other: &PooledSeen) -> Verdict {
        Verdict::equal("pooled result", &base.result, &other.result).and(
            Verdict::allowed_if_differs(Allowed::SharedPool, &base.remaining, &other.remaining),
        )
    }

    fn orders(&self) -> Vec<(String, Vec<usize>)> {
        super::query_orders(self.items())
    }

    fn item_schedules(&self) -> Option<u64> {
        None
    }
}

impl Tabulate for Pooled {
    fn input(&self, query: usize) -> String {
        format!("contracts-queries/{}/{:?}", self.label, self.queries[query])
    }

    fn normalized(&self, _query: usize, seen: &PooledSeen) -> Normalized {
        Normalized::Output(format!("{:?}", seen.result))
    }

    fn presentation(&self, _query: usize, seen: &PooledSeen) -> String {
        // A5: the remaining balance of a shared request-work pool after the query history.
        format!("remaining-{:?}", seen.remaining)
    }

    fn schedule(&self, _query: usize, label: &str, position: usize) -> String {
        super::schedule_id(&self.name(), label, position)
    }
}

/// F4: every dependency-respecting commit order of named definitions, then a query set.
pub(super) struct Commits {
    pub(super) label: &'static str,
    /// `(name, text, indices of definitions it needs)`.
    pub(super) definitions: Vec<(&'static str, &'static [u8], Vec<usize>)>,
    pub(super) queries: Vec<&'static [u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CommitSeen {
    Query(Rendered),
    /// A definition's sorted current dependents and its session-local identity.
    Definition {
        dependents: Vec<String>,
        identity: Option<u64>,
    },
}

impl Commits {
    fn is_admitted(&self, order: &[usize]) -> bool {
        order.iter().enumerate().all(|(position, item)| {
            self.definitions[*item]
                .2
                .iter()
                .all(|need| order[..position].contains(need))
        })
    }
}

impl Case for Commits {
    type Seen = CommitSeen;

    fn name(&self) -> String {
        format!("contracts-commits/{}", self.label)
    }

    fn items(&self) -> usize {
        self.definitions.len()
    }

    fn is_query_case(&self) -> bool {
        false
    }

    fn run(&self, order: &[usize]) -> Vec<(usize, CommitSeen)> {
        if !self.is_admitted(order) {
            return Vec::new();
        }
        let texts = order
            .iter()
            .map(|item| self.definitions[*item].1)
            .collect::<Vec<_>>();
        let session = session(&texts).expect("an admitted commit order commits");
        let mut out = Vec::with_capacity(self.queries.len() + self.definitions.len());
        for (index, query) in self.queries.iter().enumerate() {
            let rendered = match session.prepare(query, &[], LIMITS) {
                Ok(prepared) => Rendered::Prepared(normalized(&prepared)),
                Err(error) => refused(&error),
            };
            out.push((index, CommitSeen::Query(rendered)));
        }
        for (index, (name, _, _)) in self.definitions.iter().enumerate() {
            let mut dependents = session.current_dependent_names(name);
            dependents.sort();
            let identity = session.installed_definition_identity(name);
            out.push((
                self.queries.len() + index,
                CommitSeen::Definition {
                    dependents,
                    identity,
                },
            ));
        }
        out
    }

    fn compare(&self, _query: usize, base: &CommitSeen, other: &CommitSeen) -> Verdict {
        match (base, other) {
            (
                CommitSeen::Definition {
                    dependents,
                    identity,
                },
                CommitSeen::Definition {
                    dependents: other_dependents,
                    identity: other_identity,
                },
            ) => Verdict::equal("dependents", dependents, other_dependents).and(
                Verdict::allowed_if_differs(Allowed::Numbering, identity, other_identity),
            ),
            _ => Verdict::equal("query", base, other),
        }
    }

    fn orders(&self) -> Vec<(String, Vec<usize>)> {
        super::permutations(self.items())
            .into_iter()
            .filter(|(_, order)| self.is_admitted(order))
            .collect()
    }

    fn item_schedules(&self) -> Option<u64> {
        None
    }
}

impl Tabulate for Commits {
    fn input(&self, query: usize) -> String {
        if query < self.queries.len() {
            return format!(
                "contracts-commits/{}/query-{query}/{:?}",
                self.label, self.queries[query]
            );
        }
        let (name, _, _) = &self.definitions[query - self.queries.len()];
        format!("contracts-commits/{}/definition-{name}", self.label)
    }

    fn normalized(&self, _query: usize, seen: &CommitSeen) -> Normalized {
        Normalized::Output(match seen {
            CommitSeen::Query(rendered) => format!("query:{rendered:?}"),
            CommitSeen::Definition { dependents, .. } => {
                format!("dependents:{dependents:?}")
            }
        })
    }

    fn presentation(&self, _query: usize, seen: &CommitSeen) -> String {
        match seen {
            // A1: the session-local identity follows commit order.
            CommitSeen::Definition { identity, .. } => format!("identity-{identity:?}"),
            CommitSeen::Query(_) => String::new(),
        }
    }

    fn schedule(&self, _query: usize, label: &str, position: usize) -> String {
        super::schedule_id(&self.name(), label, position)
    }
}

/// F5: one declared module whose local definitions are permuted; `variants` are the same
/// definitions with exactly one defect repaired (same byte lengths, so spans are comparable).
pub(super) struct Module {
    pub(super) label: &'static str,
    pub(super) definitions: Vec<&'static str>,
    pub(super) variants: Vec<Vec<&'static str>>,
    pub(super) invocations: Vec<&'static [u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ModuleSeen {
    main: Rendered,
    variants: Vec<Rendered>,
}

pub(super) fn retained() -> noble_contracts::source::RetainedSourceLimit {
    noble_contracts::source::RetainedSourceLimit::new(LIMITS.bytes).expect("retained source limit")
}

fn module_unit(definitions: &[&str], order: &[usize], invocations: &[&[u8]]) -> Rendered {
    let body = order
        .iter()
        .map(|item| definitions[*item])
        .collect::<Vec<_>>()
        .join(" ");
    let text = format!("module m@1 [ export top {body} ]");
    let session = noble_contracts::source::ModuleSession::new(&[], retained()).expect("session");
    let prepared = match session.prepare(text.as_bytes(), &[], LIMITS) {
        Ok(prepared) => prepared,
        Err(error) => return refused(&error),
    };
    let header = format!("kind={:?} output={:?}", prepared.kind(), prepared.output());
    let (session, committed) = session.commit(prepared);
    if let Err(error) = committed {
        return refused(&error);
    }
    let calls = invocations
        .iter()
        .map(|call| match session.prepare(call, &[], LIMITS) {
            Ok(prepared) => format!("{:?}", prepared.output()),
            Err(error) => format!("{:?}", refused(&error)),
        })
        .collect::<Vec<_>>();
    Rendered::Prepared(format!("{header} calls={calls:?}"))
}

impl Case for Module {
    type Seen = ModuleSeen;

    fn name(&self) -> String {
        format!("contracts-modules/{}", self.label)
    }

    fn items(&self) -> usize {
        self.definitions.len()
    }

    fn is_query_case(&self) -> bool {
        false
    }

    fn run(&self, order: &[usize]) -> Vec<(usize, ModuleSeen)> {
        let main = module_unit(&self.definitions, order, &self.invocations);
        let variants = self
            .variants
            .iter()
            .map(|variant| module_unit(variant, order, &self.invocations))
            .collect();
        vec![(0, ModuleSeen { main, variants })]
    }

    fn compare(&self, _query: usize, base: &ModuleSeen, other: &ModuleSeen) -> Verdict {
        let is_multi_defect = !other.variants.is_empty();
        let sound = match &other.main {
            Rendered::Refused { .. }
                if is_multi_defect && !other.variants.contains(&other.main) =>
            {
                Verdict::Diverged(format!("{:?} is no single-defect diagnostic", other.main))
            }
            _ => Verdict::Same,
        };
        let classes = match (&base.main, &other.main) {
            (
                Rendered::Refused { stage, kind, .. },
                Rendered::Refused {
                    stage: other_stage,
                    kind: other_kind,
                    ..
                },
            ) if is_multi_defect => Verdict::equal("stage", stage, other_stage)
                .and(Verdict::equal("kind", kind, other_kind))
                .and(Verdict::allowed_if_differs(
                    Allowed::FirstDefect,
                    &base.main,
                    &other.main,
                )),
            _ => Verdict::equal("module", &base.main, &other.main),
        };
        sound.and(classes)
    }

    fn orders(&self) -> Vec<(String, Vec<usize>)> {
        super::permutations(self.items())
    }

    fn item_schedules(&self) -> Option<u64> {
        None
    }
}

impl Tabulate for Module {
    fn input(&self, _query: usize) -> String {
        format!("contracts-modules/{}", self.label)
    }

    fn normalized(&self, _query: usize, seen: &ModuleSeen) -> Normalized {
        // With several independent defects only the refusal's stage and kind are semantic (A2).
        match (&seen.main, self.variants.is_empty()) {
            (Rendered::Refused { stage, kind, .. }, false) => {
                Normalized::Output(format!("refused:{stage}:{kind}"))
            }
            (main, _) => Normalized::Output(format!("{main:?}")),
        }
    }

    fn presentation(&self, _query: usize, seen: &ModuleSeen) -> String {
        match (&seen.main, self.variants.is_empty()) {
            (Rendered::Refused { .. }, false) => format!("reported-{:?}", seen.main),
            (Rendered::Refused { .. } | Rendered::Prepared(_), true)
            | (Rendered::Prepared(_), false) => String::new(),
        }
    }

    fn schedule(&self, _query: usize, label: &str, position: usize) -> String {
        super::schedule_id(&self.name(), label, position)
    }
}

fn query_cases() -> Vec<Queries> {
    vec![shared_queries()]
}

/// The shared-dependency query set (also issued concurrently by F9).
pub(super) fn shared_queries() -> Queries {
    Queries {
        label: "shared-base",
        definitions: vec![
            b"def base [ 2 ]",
            b"def left [ base 3 + ]",
            b"def right [ base 5 * ]",
        ],
        queries: vec![
            b"left right +",
            b"base",
            b"right",
            b"left 1 true +",
            b"missing",
            b"def fresh [ left right * ]",
            b"[ base ] run left +",
            b"left left left",
        ],
    }
}

fn pooled_cases() -> Vec<Pooled> {
    vec![Pooled {
        label: "module-pool",
        module: b"module m@1 [ export top def base [ 2 ] def left [ base 3 + ] def top [ left base * ] ]",
        pool: 1_000_000,
        queries: vec![b"m@1.top", b"m@1.top m@1.top +", b"m@1.top true +", b"m@1.missing"],
    }]
}

fn commit_cases() -> Vec<Commits> {
    vec![Commits {
        label: "diamond-and-solo",
        definitions: vec![
            ("base", b"def base [ 2 ]", vec![]),
            ("left", b"def left [ base 3 + ]", vec![0]),
            ("right", b"def right [ base 5 * ]", vec![0]),
            ("top", b"def top [ left right + ]", vec![1, 2]),
            ("solo", b"def solo [ 7 ]", vec![]),
        ],
        queries: vec![
            b"top",
            b"left solo +",
            b"def again [ top right - ]",
            b"top true +",
            b"nope",
        ],
    }]
}

fn module_cases() -> Vec<Module> {
    vec![
        Module {
            label: "shared-dependency",
            definitions: vec![
                "def base [ 2 ]",
                "def left [ base 3 + ]",
                "def right [ base 5 * ]",
                "def top [ left right + ]",
            ],
            variants: vec![],
            invocations: vec![b"m@1.top", b"m@1.top m@1.top +"],
        },
        Module {
            label: "cycle",
            definitions: vec!["def a [ b ]", "def b [ c ]", "def c [ a ]", "def top [ 1 ]"],
            variants: vec![],
            invocations: vec![b"m@1.top"],
        },
        Module {
            label: "two-defects",
            definitions: vec![
                "def base [ 2 ]",
                "def bad1 [ 1 true + ]",
                "def bad2 [ true 2 * ]",
                "def top [ base ]",
            ],
            variants: vec![
                vec![
                    "def base [ 2 ]",
                    "def bad1 [ 1 9999 + ]",
                    "def bad2 [ true 2 * ]",
                    "def top [ base ]",
                ],
                vec![
                    "def base [ 2 ]",
                    "def bad1 [ 1 true + ]",
                    "def bad2 [ 7777 2 * ]",
                    "def top [ base ]",
                ],
            ],
            invocations: vec![b"m@1.top"],
        },
    ]
}

/// Reruns one named schedule of these families, if the case belongs to them.
pub(super) fn replay(name: &str, order: &[usize]) -> Option<Result<(), String>> {
    if let Some(case) = query_cases().into_iter().find(|case| case.name() == name) {
        return Some(super::replay_case(&case, order));
    }
    if let Some(case) = pooled_cases().into_iter().find(|case| case.name() == name) {
        return Some(super::replay_case(&case, order));
    }
    if let Some(case) = commit_cases().into_iter().find(|case| case.name() == name) {
        return Some(super::replay_case(&case, order));
    }
    let case = module_cases()
        .into_iter()
        .find(|case| case.name() == name)?;
    Some(super::replay_case(&case, order))
}

#[test]
fn queries_agree_across_orders_and_repeats() {
    let mut tally = Tally::default();
    for case in query_cases() {
        tally.absorb(&super::check_case(&case));
    }
    for case in pooled_cases() {
        tally.absorb(&super::check_case(&case));
    }
    tally.emit("F3 contracts-queries");
}

#[test]
fn commit_orders_agree_after_mapping_names() {
    let mut tally = Tally::default();
    for case in commit_cases() {
        tally.absorb(&super::check_case(&case));
    }
    tally.emit("F4 contracts-commits");
}

#[test]
fn module_definition_orders_agree() {
    let mut tally = Tally::default();
    for case in module_cases() {
        tally.absorb(&super::check_case(&case));
    }
    tally.emit("F5 contracts-modules");
}
