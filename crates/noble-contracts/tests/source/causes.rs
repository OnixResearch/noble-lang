//! Issue #96 prototype: finite relational explanations of why a checked Program carries an effect.
//!
//! Facts come from the kernel's own checked structure. A body is the root or one specialization in
//! `Submission::definitions`, and only the root's `Checked` survives preparation, so every body is
//! re-checked with `noble_kernel::acceptance::check`. Each node of a body is joined with its
//! candidate node:
//! - a word whose scheme fixes an effect is a host operation and contributes it directly;
//! - a named row with no body whose scheme fixes an effect is a declared contribution;
//! - a word whose scheme carries an effect variable runs a Program operand, so its instantiated
//!   effects are a latent bound whose originating construction is not tracked;
//! - a named word with a specialization is a call into that body;
//! - a quotation literal is construction only, so its own context's effects stay latent.
//!
//! A bounded fixed point over the contexts yields each context's effects with the first reason for
//! each. The kernel's per-body and per-node interfaces are the comparison, and resubmission with a
//! narrower bound is the ceiling control. The analysis cannot accept a Program or narrow a bound.

/// The relation and witness revision these facts and rules state.
const RULES: &str = "noble-effect-causes/v1";

/// The work budget of one fixed point, counted in edge visits.
const BUDGET: u32 = 100_000;

/// Where a node occurs: its body (0 is the root, `i + 1` is `Submission::definitions[i]`) and, when
/// it sits inside a quotation literal, that literal's node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Context {
    body: usize,
    quotation: Option<u32>,
}

impl Context {
    const fn of_body(body: usize) -> Self {
        Self {
            body,
            quotation: None,
        }
    }

    const fn in_quotation(body: usize, node: u32) -> Self {
        Self {
            body,
            quotation: Some(node),
        }
    }
}

/// Why a node contributes effects itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Origin {
    /// A host operation whose scheme fixes the effect; the contract-table row.
    Host(u32),
    /// A named row with no body in the submission whose declared scheme fixes the effect, such as
    /// a plain session's `test.abort`. The declaration is the contribution; no host operation is
    /// claimed for it.
    Declared(u32),
    /// A word that runs a Program operand. Its instantiated effects are that Program's latent
    /// bound, and the construction that supplied the Program is not tracked.
    Latent(u32),
}

/// One fact about one node occurrence.
#[derive(Clone, Debug)]
enum Edge {
    Direct {
        at: Context,
        node: u32,
        origin: Origin,
        effects: Vec<u32>,
    },
    Call {
        at: Context,
        node: u32,
        callee: usize,
    },
    Quote {
        at: Context,
        node: u32,
    },
    Unknown {
        at: Context,
        node: u32,
        row: u32,
    },
}

/// One checked body's identity and the kernel's view of it.
#[derive(Clone, Debug, Default)]
struct Body {
    /// The specialization's contract-table row; `None` for the root.
    row: Option<u32>,
    identity: Option<u64>,
    source_identity: Option<u64>,
    /// The kernel's checked interface effects.
    checked: std::collections::BTreeSet<u32>,
    /// The kernel's derived effects per node occurrence.
    derived: std::collections::BTreeMap<u32, std::collections::BTreeSet<u32>>,
    /// Each quotation literal's latent bound, when its instantiation binds it directly.
    latent: std::collections::BTreeMap<u32, std::collections::BTreeSet<u32>>,
}

#[derive(Clone, Debug, Default)]
struct Facts {
    bodies: Vec<Body>,
    edges: Vec<Edge>,
    unsupported: Vec<String>,
}

fn ids(set: &noble_kernel::types::EffSet) -> std::collections::BTreeSet<u32> {
    set.as_slice().iter().map(|id| id.0).collect()
}

/// Re-check one body against its own expected interface, with the submission's limits.
fn recheck(
    submission: &noble_kernel::execution::Submission,
    expected: &noble_kernel::untrusted::Expected,
    candidate: &noble_kernel::untrusted::Candidate,
) -> noble_kernel::untrusted::Outcome {
    let request = noble_kernel::untrusted::Request {
        input_bytes: submission.request.input_bytes,
        expected: expected.clone(),
        limits: submission.request.limits,
    };
    noble_kernel::acceptance::check(&submission.environment, &request, candidate)
}

fn accepted(
    outcome: noble_kernel::untrusted::Outcome,
) -> Result<noble_kernel::untrusted::Checked, String> {
    match outcome {
        noble_kernel::untrusted::Outcome::Accepted(checked) => Ok(checked),
        noble_kernel::untrusted::Outcome::Invalid(diagnostic) => {
            Err(format!("invalid: {:?}", diagnostic.constraint))
        }
        noble_kernel::untrusted::Outcome::Unsupported(kind) => {
            Err(format!("unsupported: {kind:?}"))
        }
        noble_kernel::untrusted::Outcome::Exhausted(limit) => Err(format!("exhausted: {limit:?}")),
        noble_kernel::untrusted::Outcome::InternalFailure => Err(String::from("internal failure")),
    }
}

/// One body to extract facts from.
struct Source<'a> {
    submission: &'a noble_kernel::execution::Submission,
    index: usize,
    candidate: &'a noble_kernel::untrusted::Candidate,
}

impl Source<'_> {
    /// Walk the body and every quotation literal inside it with an explicit work stack. Each
    /// quotation is pushed once, so the stack never holds more entries than the arena has nodes.
    fn walk(&self, facts: &mut Facts) -> Result<(), String> {
        let node_count = self.candidate.nodes.len();
        let mut pending: Vec<(Context, &[noble_kernel::untrusted::NodeId])> =
            Vec::with_capacity(node_count.saturating_add(1));
        pending.push((Context::of_body(self.index), self.candidate.body.as_slice()));
        while let Some((at, nodes)) = pending.pop() {
            for id in nodes {
                let position = usize::try_from(id.0).map_err(|error| error.to_string())?;
                let node = self
                    .candidate
                    .nodes
                    .get(position)
                    .ok_or_else(|| format!("body {}: dangling node {}", self.index, id.0))?;
                if let Some(inner) = self.node(facts, at, id.0, node)? {
                    assert!(pending.len() < node_count, "a quotation was pushed twice");
                    pending.push(inner);
                }
            }
        }
        assert!(pending.is_empty());
        Ok(())
    }

    /// Record one node's fact. A quotation literal returns its own body to walk.
    fn node<'a>(
        &self,
        facts: &mut Facts,
        at: Context,
        id: u32,
        node: &'a noble_kernel::untrusted::Node,
    ) -> Result<Option<(Context, &'a [noble_kernel::untrusted::NodeId])>, String> {
        match node {
            noble_kernel::untrusted::Node::Literal { .. } => Ok(None),
            noble_kernel::untrusted::Node::Invocation { def, .. } => {
                self.invocation(facts, at, id, *def)?;
                Ok(None)
            }
            noble_kernel::untrusted::Node::Quotation { body, inst } => {
                facts.edges.push(Edge::Quote { at, node: id });
                let bound = match inst.bindings.get(3) {
                    Some(noble_kernel::words::Binding::Effect(set)) => Some(ids(set)),
                    Some(
                        noble_kernel::words::Binding::Stack(_)
                        | noble_kernel::words::Binding::Value(_)
                        | noble_kernel::words::Binding::Ref(_),
                    )
                    | None => None,
                };
                if let (Some(bound), Some(target)) = (bound, facts.bodies.get_mut(self.index)) {
                    target.latent.insert(id, bound);
                }
                Ok(Some((
                    Context::in_quotation(self.index, id),
                    body.as_slice(),
                )))
            }
            noble_kernel::untrusted::Node::SignedSelf { .. } => {
                facts
                    .unsupported
                    .push(format!("body {} node {id}: signed self", self.index));
                Ok(None)
            }
            noble_kernel::untrusted::Node::SlotInvoke { .. } => {
                facts
                    .unsupported
                    .push(format!("body {} node {id}: live dispatch", self.index));
                Ok(None)
            }
        }
    }

    /// Classify one invocation by its callee. A named word with a specialization is a call.
    /// Otherwise the scheme decides: its fixed effects are a host contribution, or a declared one
    /// for a named row without a body, and an effect variable is a latent contribution. A row with
    /// no scheme is an unknown target.
    fn invocation(
        &self,
        facts: &mut Facts,
        at: Context,
        node: u32,
        def: noble_kernel::contracts::Definition,
    ) -> Result<(), String> {
        let environment = &self.submission.environment;
        let row = usize::try_from(def.0).map_err(|error| error.to_string())?;
        let is_named =
            environment.kinds.get(row) == Some(&noble_kernel::contracts::Behavior::Named);
        let callee = self
            .submission
            .definitions
            .iter()
            .position(|specialization| specialization.definition.0 == def.0)
            .and_then(|position| position.checked_add(1));
        if let (true, Some(callee)) = (is_named, callee) {
            facts.edges.push(Edge::Call { at, node, callee });
            return Ok(());
        }
        let Some(scheme) = environment.defs.get(row) else {
            facts.edges.push(Edge::Unknown {
                at,
                node,
                row: def.0,
            });
            return Ok(());
        };
        let mut fixed = Vec::with_capacity(scheme.effects.len());
        let mut has_variable = false;
        for slot in &scheme.effects {
            match slot {
                noble_kernel::shapes::EffectSlot::Effect(id) => fixed.push(id.0),
                noble_kernel::shapes::EffectSlot::Var(_) => has_variable = true,
            }
        }
        if !fixed.is_empty() {
            let origin = if is_named {
                Origin::Declared(def.0)
            } else {
                Origin::Host(def.0)
            };
            facts.edges.push(Edge::Direct {
                at,
                node,
                origin,
                effects: fixed,
            });
        }
        if has_variable {
            let instantiated = facts
                .bodies
                .get(self.index)
                .and_then(|body| body.derived.get(&node))
                .map(|set| set.iter().copied().collect())
                .unwrap_or_default();
            let origin = Origin::Latent(def.0);
            facts.edges.push(Edge::Direct {
                at,
                node,
                origin,
                effects: instantiated,
            });
        }
        Ok(())
    }
}

/// Re-check one body, record its identity and the kernel's view, then walk its nodes.
fn body_facts(
    facts: &mut Facts,
    source: &Source<'_>,
    specialization: Option<&noble_kernel::execution::Definition>,
    expected: &noble_kernel::untrusted::Expected,
) -> Result<(), String> {
    let checked = accepted(recheck(source.submission, expected, source.candidate))
        .map_err(|error| format!("body {}: {error}", source.index))?;
    facts.bodies.push(Body {
        row: specialization.map(|definition| definition.definition.0),
        identity: specialization.map(|definition| definition.identity),
        source_identity: specialization.map(|definition| definition.source_identity),
        checked: ids(&checked.interface.effects),
        derived: checked
            .derivations
            .iter()
            .map(|derivation| (derivation.node.0, ids(&derivation.interface.effects)))
            .collect(),
        latent: std::collections::BTreeMap::new(),
    });
    source.walk(facts)
}

fn extract(prepared: &noble_contracts::source::Prepared) -> Result<Facts, String> {
    let submission = prepared
        .submission()
        .ok_or("expression has no submission")?;
    let mut facts = Facts::default();
    let root = Source {
        submission,
        index: 0,
        candidate: &submission.body.candidate,
    };
    body_facts(&mut facts, &root, None, &submission.request.expected)?;
    for (offset, definition) in submission.definitions.iter().enumerate() {
        let index = offset
            .checked_add(1)
            .ok_or("specialization index overflows")?;
        let source = Source {
            submission,
            index,
            candidate: &definition.body.candidate,
        };
        body_facts(&mut facts, &source, Some(definition), &definition.expected)?;
    }
    assert!(facts.bodies.len() > submission.definitions.len());
    assert!(facts
        .edges
        .iter()
        .all(|edge| edge_context(edge).body < facts.bodies.len()));
    Ok(facts)
}

const fn edge_context(edge: &Edge) -> Context {
    match edge {
        Edge::Direct { at, .. }
        | Edge::Call { at, .. }
        | Edge::Quote { at, .. }
        | Edge::Unknown { at, .. } => *at,
    }
}

/// The first reason an effect reached a context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reason {
    Direct { node: u32, origin: Origin },
    Call { node: u32, callee: usize },
}

#[derive(Clone, Debug, Default)]
struct Closure {
    effects: std::collections::BTreeMap<Context, std::collections::BTreeSet<u32>>,
    reasons: std::collections::BTreeMap<(Context, u32), Reason>,
    /// Contexts that reach an unknown target: their effect sets are lower bounds only.
    incomplete: std::collections::BTreeSet<Context>,
    work: u32,
    rounds: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Exhausted {
    work: u32,
}

impl Closure {
    fn add(&mut self, at: Context, effect: u32, reason: Reason) -> bool {
        let is_new = self.effects.entry(at).or_default().insert(effect);
        if is_new {
            self.reasons.insert((at, effect), reason);
        }
        is_new
    }

    fn apply(&mut self, edge: &Edge) -> bool {
        let mut is_changed = false;
        match edge {
            Edge::Direct {
                at,
                node,
                origin,
                effects,
            } => {
                let reason = Reason::Direct {
                    node: *node,
                    origin: *origin,
                };
                for effect in effects {
                    is_changed |= self.add(*at, *effect, reason);
                }
            }
            Edge::Call { at, node, callee } => {
                let from = Context::of_body(*callee);
                let reached = self.effects.get(&from).cloned().unwrap_or_default();
                let reason = Reason::Call {
                    node: *node,
                    callee: *callee,
                };
                for effect in &reached {
                    is_changed |= self.add(*at, *effect, reason);
                }
                if self.incomplete.contains(&from) {
                    is_changed |= self.incomplete.insert(*at);
                }
            }
            Edge::Quote { .. } => {}
            Edge::Unknown { at, .. } => is_changed = self.incomplete.insert(*at),
        }
        is_changed
    }

    /// The witness path for `effect` from `at`, through call sites down to the contributing node.
    /// Reasons are recorded at first insertion, so the path cannot revisit a context; it is bounded
    /// by the number of bodies.
    fn path(&self, at: Context, effect: u32, bodies: usize) -> Vec<Reason> {
        let mut steps = Vec::with_capacity(bodies.saturating_add(1));
        let mut current = at;
        while steps.len() <= bodies {
            match self.reasons.get(&(current, effect)) {
                Some(reason @ Reason::Call { callee, .. }) => {
                    steps.push(*reason);
                    current = Context::of_body(*callee);
                }
                Some(reason @ Reason::Direct { .. }) => {
                    steps.push(*reason);
                    break;
                }
                None => break,
            }
        }
        steps
    }
}

/// Evaluate the facts to a fixed point within `budget` edge visits.
fn closure(facts: &Facts, budget: u32) -> Result<Closure, Exhausted> {
    let mut state = Closure::default();
    let mut is_changed = true;
    while is_changed {
        is_changed = false;
        state.rounds = state.rounds.saturating_add(1);
        for edge in &facts.edges {
            state.work = state.work.saturating_add(1);
            if state.work > budget {
                return Err(Exhausted { work: state.work });
            }
            is_changed |= state.apply(edge);
        }
    }
    Ok(state)
}

#[derive(Clone, Debug, Default)]
struct Comparison {
    bodies: u32,
    nodes: u32,
    mismatches: Vec<String>,
    /// Effects a quotation's latent bound admits and its body does not contribute.
    bound_only: Vec<(Context, std::collections::BTreeSet<u32>)>,
}

fn effects_at(state: &Closure, at: Context) -> std::collections::BTreeSet<u32> {
    state.effects.get(&at).cloned().unwrap_or_default()
}

/// Compare the closure with the kernel: every body's checked effects, every call and contributing
/// node's derived effects, and every quotation: construction contributes nothing, and the body's
/// effects fit the latent bound.
fn compare(facts: &Facts, state: &Closure) -> Comparison {
    let mut comparison = Comparison::default();
    for (index, body) in facts.bodies.iter().enumerate() {
        comparison.bodies = comparison.bodies.saturating_add(1);
        let analysis = effects_at(state, Context::of_body(index));
        if analysis != body.checked {
            comparison.mismatches.push(format!(
                "body {index}: analysis {analysis:?}, kernel {:?}",
                body.checked
            ));
        }
    }
    for edge in &facts.edges {
        compare_node(facts, state, edge, &mut comparison);
    }
    comparison
}

/// Check a quotation's body against its latent bound and label any bound-only effects.
fn compare_quotation(facts: &Facts, state: &Closure, inner: Context, comparison: &mut Comparison) {
    assert!(
        inner.quotation.is_some(),
        "only a quotation context has a latent bound"
    );
    assert!(
        inner.body < facts.bodies.len(),
        "a quotation outside the facts"
    );
    let analysis = effects_at(state, inner);
    let bound = inner.quotation.and_then(|node| {
        facts
            .bodies
            .get(inner.body)
            .and_then(|body| body.latent.get(&node))
    });
    if let Some(bound) = bound {
        if !analysis.is_subset(bound) {
            comparison.mismatches.push(format!(
                "{inner:?}: body {analysis:?} exceeds bound {bound:?}"
            ));
        }
        let extra: std::collections::BTreeSet<u32> = bound.difference(&analysis).copied().collect();
        if !extra.is_empty() {
            comparison.bound_only.push((inner, extra));
        }
    }
}

fn compare_node(facts: &Facts, state: &Closure, edge: &Edge, comparison: &mut Comparison) {
    let (at, node, analysis) = match edge {
        Edge::Call { at, node, callee } => {
            (*at, *node, effects_at(state, Context::of_body(*callee)))
        }
        Edge::Direct {
            at,
            node,
            origin: Origin::Host(_) | Origin::Declared(_),
            effects,
        } => (*at, *node, effects.iter().copied().collect()),
        Edge::Quote { at, node } => {
            compare_quotation(
                facts,
                state,
                Context::in_quotation(at.body, *node),
                comparison,
            );
            (*at, *node, std::collections::BTreeSet::new())
        }
        Edge::Direct {
            origin: Origin::Latent(_),
            ..
        }
        | Edge::Unknown { .. } => return,
    };
    let body = facts.bodies.get(at.body);
    assert!(body.is_some(), "an edge names a body outside the facts");
    comparison.nodes = comparison.nodes.saturating_add(1);
    let kernel = body
        .and_then(|body| body.derived.get(&node))
        .cloned()
        .unwrap_or_default();
    assert!(comparison.nodes > 0);
    if analysis != kernel {
        comparison.mismatches.push(format!(
            "{at:?} node {node}: analysis {analysis:?}, kernel {kernel:?}"
        ));
    }
}

/// The kernel's verdict on the root resubmitted with `allowed` as its effect bound.
fn resubmit(
    prepared: &noble_contracts::source::Prepared,
    allowed: &std::collections::BTreeSet<u32>,
) -> Result<String, String> {
    let submission = prepared
        .submission()
        .ok_or("expression has no submission")?;
    let bound: Vec<noble_kernel::types::EffId> = allowed
        .iter()
        .map(|id| noble_kernel::types::EffId(*id))
        .collect();
    let expected = noble_kernel::untrusted::Expected {
        allowed_effects: noble_kernel::types::EffSet::from_ids(&bound),
        ..submission.request.expected.clone()
    };
    assert_eq!(expected.stack_out, submission.request.expected.stack_out);
    assert_eq!(expected.allowed_effects.as_slice().len(), allowed.len());
    Ok(
        match recheck(submission, &expected, &submission.body.candidate) {
            noble_kernel::untrusted::Outcome::Accepted(_) => String::from("accepted"),
            noble_kernel::untrusted::Outcome::Invalid(diagnostic) => {
                if let noble_kernel::untrusted::Constraint::EffectInclusion(id) =
                    diagnostic.constraint
                {
                    format!("refused: effect {}", id.0)
                } else {
                    format!("invalid: {:?}", diagnostic.constraint)
                }
            }
            noble_kernel::untrusted::Outcome::Unsupported(kind) => format!("unsupported: {kind:?}"),
            noble_kernel::untrusted::Outcome::Exhausted(limit) => format!("exhausted: {limit:?}"),
            noble_kernel::untrusted::Outcome::InternalFailure => String::from("internal failure"),
        },
    )
}

/// One fixture's analysis, comparison and ceiling controls.
#[derive(Clone, Debug, Default)]
struct Run {
    facts: Facts,
    state: Closure,
    comparison: Comparison,
    /// For each root effect: the kernel's verdict without it, and the witness path.
    ceilings: Vec<(u32, String, Vec<Reason>)>,
}

fn analyze(session: &noble_contracts::source::Session, source: &[u8]) -> Result<Run, String> {
    let prepared = super::prepare(session, source)?;
    let facts = extract(&prepared)?;
    let state = closure(&facts, BUDGET).map_err(|stop| format!("{stop:?}"))?;
    let comparison = compare(&facts, &state);
    let root = effects_at(&state, Context::of_body(0));
    let mut ceilings = Vec::with_capacity(root.len());
    for effect in &root {
        let narrower = root.iter().copied().filter(|id| id != effect).collect();
        let verdict = resubmit(&prepared, &narrower)?;
        let path = state.path(Context::of_body(0), *effect, facts.bodies.len());
        ceilings.push((*effect, verdict, path));
    }
    assert!(state.work <= BUDGET);
    assert_eq!(ceilings.len(), root.len());
    Ok(Run {
        facts,
        state,
        comparison,
        ceilings,
    })
}

/// The call sites in the root that carry `effect`, each with the rest of its path.
fn root_sites(run: &Run, effect: u32) -> Vec<Vec<Reason>> {
    let mut sites = Vec::with_capacity(run.facts.edges.len());
    for edge in &run.facts.edges {
        if let Edge::Call { at, node, callee } = edge {
            let is_carried = effects_at(&run.state, Context::of_body(*callee)).contains(&effect);
            if *at == Context::of_body(0) && is_carried {
                let mut steps = vec![Reason::Call {
                    node: *node,
                    callee: *callee,
                }];
                steps.extend(run.state.path(
                    Context::of_body(*callee),
                    effect,
                    run.facts.bodies.len(),
                ));
                sites.push(steps);
            }
        }
    }
    assert!(sites.len() <= run.facts.edges.len());
    assert!(
        sites.iter().all(|steps| !steps.is_empty()),
        "every site starts with its call"
    );
    sites
}

fn session_with(definitions: &[&[u8]]) -> Result<noble_contracts::source::Session, String> {
    let mut session = noble_contracts::source::Session::new();
    for definition in definitions {
        super::define(&mut session, definition)?;
    }
    Ok(session)
}

const EMIT: u32 = 0;
const ABORT: u32 = 1;
/// `test.emit`'s contract-table row in a plain session.
const EMIT_ROW: u32 = 22;
/// `test.abort`'s contract-table row in a plain session.
const ABORT_ROW: u32 = 23;

fn set(effects: &[u32]) -> std::collections::BTreeSet<u32> {
    effects.iter().copied().collect()
}

fn agreed(run: &Run) -> Result<(), String> {
    if run.comparison.mismatches.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "comparison mismatches: {:?}",
            run.comparison.mismatches
        ))
    }
}

#[test]
fn a_chain_explains_its_effect_through_each_call_site() -> Result<(), String> {
    let session = session_with(&[b"def b [ \"x\" test.emit ]", b"def a [ b ]"])?;
    let run = analyze(&session, b"a")?;
    agreed(&run)?;
    assert_eq!(effects_at(&run.state, Context::of_body(0)), set(&[EMIT]));
    let [(effect, verdict, path)] = run.ceilings.as_slice() else {
        return Err(format!("one ceiling control expected: {:?}", run.ceilings));
    };
    assert_eq!((*effect, verdict.as_str()), (EMIT, "refused: effect 0"));
    match path.as_slice() {
        [Reason::Call { .. }, Reason::Call { .. }, Reason::Direct {
            origin: Origin::Host(EMIT_ROW),
            ..
        }] => Ok(()),
        steps => Err(format!(
            "root -> a -> b -> test.emit expected, got {steps:?}"
        )),
    }
}

#[test]
fn a_diamond_keeps_both_call_sites_and_distinct_callee_instances() -> Result<(), String> {
    let session = session_with(&[
        b"def shared [ \"x\" test.emit ]",
        b"def left [ shared ]",
        b"def right [ shared ]",
    ])?;
    let run = analyze(&session, b"left right")?;
    agreed(&run)?;
    let sites = root_sites(&run, EMIT);
    assert_eq!(sites.len(), 2, "{sites:?}");
    let shared: Vec<&Body> = run
        .facts
        .bodies
        .iter()
        .skip(1)
        .filter(|body| body.checked == set(&[EMIT]) && body.derived.len() == 2)
        .collect();
    let [first, second] = shared.as_slice() else {
        return Err(format!("two instances of `shared`: {:?}", run.facts.bodies));
    };
    assert_eq!(first.source_identity, second.source_identity);
    assert_ne!(first.row, second.row);
    Ok(())
}

#[test]
fn constructing_a_quotation_is_not_running_it() -> Result<(), String> {
    let session = noble_contracts::source::Session::new();
    for (source, root) in [
        (&b"[ \"a\" test.emit ] drop"[..], set(&[])),
        (&b"[ \"a\" test.emit ] run"[..], set(&[EMIT])),
        (&b"[ [ \"a\" test.emit ] run ] run"[..], set(&[EMIT])),
        (&b"[ [ \"a\" test.emit ] ] run drop"[..], set(&[])),
    ] {
        let run = analyze(&session, source)?;
        agreed(&run)?;
        assert_eq!(
            effects_at(&run.state, Context::of_body(0)),
            root,
            "{}",
            String::from_utf8_lossy(source)
        );
        let has_quoted_emit = run
            .state
            .effects
            .iter()
            .any(|(at, effects)| at.quotation.is_some() && effects.contains(&EMIT));
        assert!(
            has_quoted_emit,
            "the innermost quotation's context keeps its effect"
        );
    }
    Ok(())
}

#[test]
fn distinct_instantiations_of_one_callee_keep_their_own_contexts() -> Result<(), String> {
    let session = session_with(&[b"def apply [ run ]"])?;
    let run = analyze(&session, b"[ \"a\" test.emit ] apply [ ] apply")?;
    agreed(&run)?;
    assert_eq!(
        run.facts.bodies.len(),
        3,
        "the root and two instances of `apply`"
    );
    let applied: Vec<std::collections::BTreeSet<u32>> = run
        .facts
        .bodies
        .iter()
        .skip(1)
        .map(|body| body.checked.clone())
        .collect();
    assert_eq!(applied, vec![set(&[EMIT]), set(&[])]);
    let sites = root_sites(&run, EMIT);
    let [only] = sites.as_slice() else {
        return Err(format!(
            "only the first instance carries test.emit: {sites:?}"
        ));
    };
    match only.as_slice() {
        [Reason::Call { callee: 1, .. }, Reason::Direct {
            origin: Origin::Latent(_),
            ..
        }] => Ok(()),
        steps => Err(format!(
            "a call into the first instance, then a latent bound: {steps:?}"
        )),
    }
}

#[test]
fn a_declared_row_is_not_reported_as_a_host_operation() -> Result<(), String> {
    let session = noble_contracts::source::Session::new();
    let run = analyze(
        &session,
        b"[ \"first\" test.emit test.abort ] [ \"second\" test.emit ] compose run",
    )?;
    agreed(&run)?;
    assert_eq!(
        effects_at(&run.state, Context::of_body(0)),
        set(&[EMIT, ABORT])
    );
    let mut origins: Vec<Origin> = run
        .facts
        .edges
        .iter()
        .filter_map(|edge| match edge {
            Edge::Direct { origin, .. } => Some(*origin),
            Edge::Call { .. } | Edge::Quote { .. } | Edge::Unknown { .. } => None,
        })
        .collect();
    origins.sort();
    assert_eq!(
        origins,
        vec![
            Origin::Host(EMIT_ROW),
            Origin::Host(EMIT_ROW),
            Origin::Declared(ABORT_ROW),
            Origin::Latent(10)
        ],
        "the two test.emit host calls, test.abort's declared row and run's latent bound"
    );
    Ok(())
}

/// A declared bound wider than what the body contributes. Inferred bounds are exact in a plain
/// session: `if`'s branches and `compose`'s operands each keep their own latent bound, so the
/// quotation comparison finds nothing wider. A request's `allowed_effects` is a declared bound, so
/// a pure root resubmitted with `test.emit` declared is accepted. The analysis labels that effect
/// bound-only and finds no contributing node for it.
#[test]
fn a_bound_wider_than_the_body_is_labelled_as_a_bound() -> Result<(), String> {
    let session = noble_contracts::source::Session::new();
    let branches = analyze(&session, b"true [ \"a\" test.emit ] [ ] if")?;
    agreed(&branches)?;
    assert!(
        branches.comparison.bound_only.is_empty(),
        "inferred bounds are exact"
    );
    let control = declared_bound(&session, b"[ \"a\" test.emit ] drop", &set(&[EMIT]))?;
    assert_eq!(control.verdict, "accepted");
    assert_eq!(control.bound_only, set(&[EMIT]));
    assert!(
        control.contributors.is_empty(),
        "no contributor is invented for a bound"
    );
    Ok(())
}

/// The kernel's verdict on a root resubmitted with `declared` as its bound, the declared effects
/// the analysis finds no contribution for, and the witness steps it finds for them.
struct DeclaredBound {
    verdict: String,
    bound_only: std::collections::BTreeSet<u32>,
    contributors: Vec<Reason>,
}

fn declared_bound(
    session: &noble_contracts::source::Session,
    source: &[u8],
    declared: &std::collections::BTreeSet<u32>,
) -> Result<DeclaredBound, String> {
    let prepared = super::prepare(session, source)?;
    let facts = extract(&prepared)?;
    let state = closure(&facts, BUDGET).map_err(|stop| format!("{stop:?}"))?;
    let derived = effects_at(&state, Context::of_body(0));
    let bound_only: std::collections::BTreeSet<u32> =
        declared.difference(&derived).copied().collect();
    let contributors = bound_only
        .iter()
        .flat_map(|effect| state.path(Context::of_body(0), *effect, facts.bodies.len()))
        .collect();
    Ok(DeclaredBound {
        verdict: resubmit(&prepared, declared)?,
        bound_only,
        contributors,
    })
}

#[test]
fn saved_programs_keep_their_old_dependencies_after_rebinding() -> Result<(), String> {
    let session = session_with(&[
        b"def g [ \"x\" test.emit ]",
        b"def f [ g ]",
        b"def g [ ]",
        b"def h [ g ]",
    ])?;
    let run = analyze(&session, b"f h")?;
    agreed(&run)?;
    assert_eq!(effects_at(&run.state, Context::of_body(0)), set(&[EMIT]));
    let sites = root_sites(&run, EMIT);
    let [first] = sites.as_slice() else {
        return Err(format!("only `f` carries the old effect: {sites:?}"));
    };
    let identities: std::collections::BTreeSet<Option<u64>> = run
        .facts
        .bodies
        .iter()
        .skip(1)
        .map(|body| body.identity)
        .collect();
    assert_eq!(
        identities.len(),
        4,
        "f, old g, h and new g stay distinct: {:?}",
        run.facts.bodies
    );
    match first.as_slice() {
        [Reason::Call { .. }, Reason::Call { .. }, Reason::Direct { .. }] => Ok(()),
        steps => Err(format!("f -> old g -> test.emit expected: {steps:?}")),
    }
}

/// Synthetic facts with a call cycle between bodies 1 and 2, entered from the root. These are
/// analysis fixtures only: source and admission keep refusing recursion.
fn cycle(is_seeded: bool) -> Facts {
    let mut edges = Vec::with_capacity(4);
    edges.push(Edge::Call {
        at: Context::of_body(0),
        node: 0,
        callee: 1,
    });
    edges.push(Edge::Call {
        at: Context::of_body(1),
        node: 0,
        callee: 2,
    });
    edges.push(Edge::Call {
        at: Context::of_body(2),
        node: 0,
        callee: 1,
    });
    if is_seeded {
        edges.push(Edge::Direct {
            at: Context::of_body(2),
            node: 1,
            origin: Origin::Host(EMIT_ROW),
            effects: vec![EMIT],
        });
    }
    assert!(edges.len() <= 4);
    assert_eq!(edges.len() == 4, is_seeded);
    Facts {
        bodies: vec![Body::default(); 3],
        edges,
        unsupported: Vec::new(),
    }
}

#[test]
fn synthetic_cycles_reach_a_fixed_point() -> Result<(), String> {
    for is_seeded in [true, false] {
        let state = closure(&cycle(is_seeded), BUDGET).map_err(|stop| format!("{stop:?}"))?;
        let expected = if is_seeded { set(&[EMIT]) } else { set(&[]) };
        for body in 0..3 {
            assert_eq!(
                effects_at(&state, Context::of_body(body)),
                expected,
                "seeded {is_seeded}"
            );
        }
        assert!(state.rounds <= 4, "{} rounds", state.rounds);
    }
    Ok(())
}

#[test]
fn unknown_targets_and_exhaustion_stay_explicit() -> Result<(), String> {
    let mut facts = cycle(false);
    facts.edges.push(Edge::Unknown {
        at: Context::of_body(2),
        node: 2,
        row: 99,
    });
    let state = closure(&facts, BUDGET).map_err(|stop| format!("{stop:?}"))?;
    assert!(effects_at(&state, Context::of_body(0)).is_empty());
    assert!(
        state.incomplete.contains(&Context::of_body(0)),
        "an empty set under an unknown target is not purity"
    );
    assert_eq!(
        closure(&cycle(true), 3).map(|state| state.work),
        Err(Exhausted { work: 4 })
    );
    Ok(())
}

/// A lost call edge leaves the analysis short of the kernel; the comparison reports it.
fn lose_first_call(facts: &Facts) -> Facts {
    let mut faulty = facts.clone();
    if let Some(position) = faulty
        .edges
        .iter()
        .position(|edge| matches!(edge, Edge::Call { .. }))
    {
        faulty.edges.remove(position);
    }
    faulty
}

/// Treating construction as running leaks a quotation's effects into its context.
fn leak_quotations(facts: &Facts) -> Facts {
    let mut faulty = facts.clone();
    faulty.edges.reserve(facts.edges.len());
    for edge in &facts.edges {
        let Edge::Direct {
            at,
            node,
            origin,
            effects,
        } = edge
        else {
            continue;
        };
        let Some(quoted) = at.quotation else {
            continue;
        };
        let enclosing = facts.edges.iter().find_map(|candidate| match candidate {
            Edge::Quote { at: outer, node } if outer.body == at.body && *node == quoted => {
                Some(*outer)
            }
            Edge::Direct { .. } | Edge::Call { .. } | Edge::Quote { .. } | Edge::Unknown { .. } => {
                None
            }
        });
        if let Some(outer) = enclosing {
            faulty.edges.push(Edge::Direct {
                at: outer,
                node: *node,
                origin: *origin,
                effects: effects.clone(),
            });
        }
    }
    assert!(faulty.edges.len() >= facts.edges.len());
    assert_eq!(faulty.bodies.len(), facts.bodies.len());
    faulty
}

#[test]
fn seeded_faults_are_detected_by_the_kernel_comparison() -> Result<(), String> {
    let chain = session_with(&[b"def b [ \"x\" test.emit ]", b"def a [ b ]"])?;
    let run = analyze(&chain, b"a")?;
    let lost = lose_first_call(&run.facts);
    let state = closure(&lost, BUDGET).map_err(|stop| format!("{stop:?}"))?;
    assert!(
        !compare(&lost, &state).mismatches.is_empty(),
        "a lost edge goes unnoticed"
    );
    let plain = noble_contracts::source::Session::new();
    let run = analyze(&plain, b"[ \"a\" test.emit ] drop")?;
    let leaked = leak_quotations(&run.facts);
    let state = closure(&leaked, BUDGET).map_err(|stop| format!("{stop:?}"))?;
    assert!(
        !compare(&leaked, &state).mismatches.is_empty(),
        "a quotation leak goes unnoticed"
    );
    Ok(())
}

/// A fixture: its name, the definitions committed first, and the root expression.
type Fixture = (&'static str, &'static [&'static [u8]], &'static [u8]);

/// The checked fixtures the report covers.
const FIXTURES: [Fixture; 10] = [
    (
        "chain",
        &[b"def b [ \"x\" test.emit ]", b"def a [ b ]"],
        b"a",
    ),
    (
        "diamond",
        &[
            b"def shared [ \"x\" test.emit ]",
            b"def left [ shared ]",
            b"def right [ shared ]",
        ],
        b"left right",
    ),
    ("quotation-constructed", &[], b"[ \"a\" test.emit ] drop"),
    ("quotation-run", &[], b"[ \"a\" test.emit ] run"),
    (
        "quotation-nested-run",
        &[],
        b"[ [ \"a\" test.emit ] run ] run",
    ),
    (
        "quotation-nested-constructed",
        &[],
        b"[ [ \"a\" test.emit ] ] run drop",
    ),
    (
        "instantiations",
        &[b"def apply [ run ]"],
        b"[ \"a\" test.emit ] apply [ ] apply",
    ),
    (
        "composed-declared",
        &[],
        b"[ \"first\" test.emit test.abort ] [ \"second\" test.emit ] compose run",
    ),
    ("branch", &[], b"true [ \"a\" test.emit ] [ ] if"),
    (
        "rebinding",
        &[
            b"def g [ \"x\" test.emit ]",
            b"def f [ g ]",
            b"def g [ ]",
            b"def h [ g ]",
        ],
        b"f h",
    ),
];

fn json_set(effects: &std::collections::BTreeSet<u32>) -> String {
    let items: Vec<String> = effects.iter().map(u32::to_string).collect();
    format!("[{}]", items.join(","))
}

fn json_reason(reason: &Reason) -> String {
    match reason {
        Reason::Call { node, callee } => {
            format!("{{\"call\":{{\"node\":{node},\"body\":{callee}}}}}")
        }
        Reason::Direct {
            node,
            origin: Origin::Host(row),
        } => format!("{{\"host\":{{\"node\":{node},\"row\":{row}}}}}"),
        Reason::Direct {
            node,
            origin: Origin::Declared(row),
        } => format!("{{\"declared\":{{\"node\":{node},\"row\":{row}}}}}"),
        Reason::Direct {
            node,
            origin: Origin::Latent(row),
        } => format!("{{\"latent\":{{\"node\":{node},\"row\":{row},\"origin\":\"unavailable\"}}}}"),
    }
}

fn json_steps(steps: &[Reason]) -> String {
    let items: Vec<String> = steps.iter().map(json_reason).collect();
    format!("[{}]", items.join(","))
}

/// Each root effect with its witness path, its root call sites and the kernel's verdict without it.
fn json_explained(run: &Run) -> String {
    let root = effects_at(&run.state, Context::of_body(0));
    let rows: Vec<String> = root
        .iter()
        .map(|effect| {
            let sites: Vec<String> = root_sites(run, *effect)
                .iter()
                .map(|steps| json_steps(steps))
                .collect();
            let (verdict, path) = run
                .ceilings
                .iter()
                .find(|(id, ..)| id == effect)
                .map(|(_, verdict, path)| (verdict.clone(), json_steps(path)))
                .unwrap_or_default();
            format!(
                "{{\"effect\":{effect},\"path\":{path},\"call_sites\":[{}],\"kernel_without_it\":\"{verdict}\"}}",
                sites.join(",")
            )
        })
        .collect();
    assert_eq!(rows.len(), root.len());
    assert!(
        run.ceilings.len() <= root.len(),
        "one ceiling control per root effect"
    );
    format!("[{}]", rows.join(","))
}

/// The bodies with their rows, source identities and checked effects; the bound-only labels; the
/// unknown targets.
fn json_structure(run: &Run) -> (String, String, String) {
    let bodies: Vec<String> = run
        .facts
        .bodies
        .iter()
        .map(|body| {
            let row = body
                .row
                .map_or_else(|| String::from("null"), |row| row.to_string());
            let identity = body
                .source_identity
                .map_or_else(|| String::from("null"), |identity| identity.to_string());
            let checked = json_set(&body.checked);
            format!("{{\"row\":{row},\"source_identity\":{identity},\"checked\":{checked}}}")
        })
        .collect();
    let bounds: Vec<String> = run
        .comparison
        .bound_only
        .iter()
        .map(|(at, extra)| {
            format!(
                "{{\"context\":\"{at:?}\",\"bound_only\":{}}}",
                json_set(extra)
            )
        })
        .collect();
    let unknown: Vec<String> = run
        .facts
        .edges
        .iter()
        .filter_map(|edge| match edge {
            Edge::Unknown { at, node, row } => Some(format!(
                "{{\"context\":\"{at:?}\",\"node\":{node},\"row\":{row}}}"
            )),
            Edge::Direct { .. } | Edge::Call { .. } | Edge::Quote { .. } => None,
        })
        .collect();
    assert_eq!(bodies.len(), run.facts.bodies.len());
    assert_eq!(bounds.len(), run.comparison.bound_only.len());
    (bodies.join(","), bounds.join(","), unknown.join(","))
}

fn json_run(name: &str, source: &[u8], run: &Run) -> String {
    assert!(!name.is_empty(), "a fixture has a name");
    assert!(!source.is_empty(), "a fixture has a root expression");
    let (bodies, bounds, unknown) = json_structure(run);
    let compared = format!(
        "{{\"bodies\":{},\"nodes\":{},\"mismatches\":{}}}",
        run.comparison.bodies,
        run.comparison.nodes,
        run.comparison.mismatches.len()
    );
    let cost = format!(
        "{{\"facts\":{},\"work\":{},\"rounds\":{}}}",
        run.facts.edges.len(),
        run.state.work,
        run.state.rounds
    );
    format!(
        "{{\"fixture\":\"{name}\",\"source\":{:?},\"flat\":{},\"explained\":{},\"bounds\":[{bounds}],\
         \"bodies\":[{bodies}],\"compared\":{compared},\"unsupported\":{},\"unknown\":[{unknown}],\
         \"incomplete\":{},\"cost\":{cost}}}",
        String::from_utf8_lossy(source),
        json_set(&effects_at(&run.state, Context::of_body(0))),
        json_explained(run),
        run.facts.unsupported.len(),
        run.state.incomplete.len()
    )
}

/// The synthetic cycles, the seeded faults and the declared-bound control.
fn json_controls() -> Result<String, String> {
    let seeded = closure(&cycle(true), BUDGET).map_err(|stop| format!("{stop:?}"))?;
    let unseeded = closure(&cycle(false), BUDGET).map_err(|stop| format!("{stop:?}"))?;
    let chain = analyze(&session_with(FIXTURES[0].1)?, FIXTURES[0].2)?;
    let lost = lose_first_call(&chain.facts);
    let lost_state = closure(&lost, BUDGET).map_err(|stop| format!("{stop:?}"))?;
    let session = noble_contracts::source::Session::new();
    let constructed = analyze(&session, FIXTURES[2].2)?;
    let leaked = leak_quotations(&constructed.facts);
    let leaked_state = closure(&leaked, BUDGET).map_err(|stop| format!("{stop:?}"))?;
    let lost_mismatches = compare(&lost, &lost_state).mismatches.len();
    let leak_mismatches = compare(&leaked, &leaked_state).mismatches.len();
    assert!(lost_mismatches > 0, "a lost edge went unseen");
    assert!(leak_mismatches > 0, "a quotation leak went unseen");
    let declared = set(&[EMIT]);
    let bound = declared_bound(&session, FIXTURES[2].2, &declared)?;
    Ok(format!(
        "\"cycles\":{{\"seeded\":{{\"effects\":{},\"rounds\":{}}},\"unseeded\":{{\"effects\":{},\"rounds\":{}}}}},\
         \"faults\":{{\"lost_edge_mismatches\":{lost_mismatches},\"quotation_leak_mismatches\":{leak_mismatches}}},\
         \"declared_bound\":{{\"source\":{:?},\"declared\":{},\"kernel\":\"{}\",\"bound_only\":{},\"contributors\":{}}}",
        json_set(&effects_at(&seeded, Context::of_body(0))),
        seeded.rounds,
        json_set(&effects_at(&unseeded, Context::of_body(0))),
        unseeded.rounds,
        String::from_utf8_lossy(FIXTURES[2].2),
        json_set(&declared),
        bound.verdict,
        json_set(&bound.bound_only),
        bound.contributors.len()
    ))
}

/// Every fixture, the synthetic controls and the seeded faults, printed as one JSON line for the
/// lane to retain. The test fails if any fixture disagrees with the kernel or a fault goes unseen.
#[test]
fn report() -> Result<(), String> {
    let mut rows = Vec::with_capacity(FIXTURES.len());
    for (name, definitions, source) in FIXTURES {
        let run = analyze(&session_with(definitions)?, source)?;
        agreed(&run)?;
        rows.push(json_run(name, source, &run));
    }
    assert_eq!(rows.len(), FIXTURES.len());
    let controls = json_controls()?;
    assert!(!controls.is_empty());
    println!(
        "NOBLE-EFFECT-CAUSES-REPORT {{\"schema\":\"{RULES}\",\"budget\":{BUDGET},\"fixtures\":[{}],{controls}}}",
        rows.join(",")
    );
    Ok(())
}
