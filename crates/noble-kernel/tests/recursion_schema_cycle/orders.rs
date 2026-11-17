//! Schedule-independence controls for the dependency walk (issue #134).
//!
//! The verdict must not depend on how definitions are numbered or how each
//! dependency list is ordered: the walk accepts exactly the acyclic graphs,
//! and its first failure ends it, so a rejection names the on-path definition
//! the first closing edge reaches, which lies on a cycle. The fixtures are
//! retained minimized counterexamples from the #134 inventory; the sweep
//! holds every renumbering of a fixed set of small graphs to a reachability
//! reference computed here, independently of the kernel's walk.

/// The graphs the sweep renumbers: entry `v` lists the extra definitions
/// that extra definition `v` depends on.
const GRAPHS: [&[&[usize]]; 10] = [
    &[&[1, 2], &[2], &[]],             // acyclic diamond
    &[&[1], &[2, 0], &[1]],            // cycles 0-1 and 1-2 sharing 1
    &[&[1], &[0], &[3], &[2]],         // two disjoint cycles
    &[&[1], &[1], &[0]],               // self-loop behind an entered chain
    &[&[1], &[2], &[3], &[]],          // acyclic chain
    &[&[2], &[2], &[3], &[3]],         // shared definition entering a self-loop
    &[&[2], &[2], &[3], &[2]],         // cycle 2-3 entered from two roots
    &[&[1, 2], &[3], &[3], &[]],       // acyclic four-definition diamond
    &[&[1, 2], &[2, 3], &[3], &[1]],   // cycles 1-2-3 and 1-3 entered from 0
    &[&[1, 2, 3], &[2, 3], &[3], &[]], // acyclic, two multi-entry lists
];

/// The sweep's running record.
struct Tally {
    cases: u32,
    accepted: u32,
    violations: Vec<String>,
}

/// Check the empty program against the bootstrap environment extended with
/// `deps` (extra definitions 23, 24, ...) under `work` declared work units.
fn outcome_of(deps: &[Vec<u32>], work: u32) -> Result<noble_kernel::untrusted::Outcome, String> {
    let env = crate::builders::env_with(deps)?;
    let mut request = crate::builders::request(vec![], vec![]);
    request.limits.work = work;
    Ok(noble_kernel::acceptance::check(
        &env,
        &request,
        &crate::builders::empty_program(),
    ))
}

/// Retained minimized schedule-independence counterexample for #134: 23
/// depends on 24, 24 on itself, and 25 on 23. The walk from root 23 closes
/// the self-loop at 24 first. Before the fix the root loop kept walking over
/// that aborted walk's stale on-path marks, and root 25's edge into 23
/// replaced the rejection with `RecursiveDependency(Definition(23))`,
/// naming a definition that lies on no cycle.
#[test]
fn self_loop_behind_a_chain_names_the_loop() -> Result<(), String> {
    let outcome = outcome_of(&[vec![24], vec![24], vec![23]], 10_000)?;
    assert_eq!(
        crate::builders::unsupported_of(outcome)?,
        noble_kernel::untrusted::UnsupportedKind::RecursiveDependency(
            noble_kernel::contracts::Definition(24)
        ),
        "the first closing edge is the self-loop at 24, the only definition on a cycle"
    );
    Ok(())
}

/// Retained minimized schedule-independence counterexample for #134: 23
/// depends on 24, 24 on 25 and 23, and 25 on 24 (cycles 23-24 and 24-25).
/// From root 23, definition 24 enqueues 25 and then closes the first cycle
/// with its edge back to the on-path 23. Before the fix the root loop went
/// on to root 25, enqueued but never reached by the aborted walk, whose edge
/// into the stale on-path 24 replaced the rejection with
/// `RecursiveDependency(Definition(24))`.
#[test]
fn overlapping_cycles_name_the_first_closing_edge() -> Result<(), String> {
    let outcome = outcome_of(&[vec![24], vec![25, 23], vec![24]], 10_000)?;
    assert_eq!(
        crate::builders::unsupported_of(outcome)?,
        noble_kernel::untrusted::UnsupportedKind::RecursiveDependency(
            noble_kernel::contracts::Definition(23)
        ),
        "24's edge back to the on-path 23 closes the first cycle, 23-24"
    );
    Ok(())
}

/// Retained minimized schedule-independence counterexample for #134: 23
/// depends on itself and 24 on 25, under one declared work unit. The walk
/// from root 23 charges that unit to the self-loop edge and rejects. Before
/// the fix the root loop went on to root 24, whose edge found the work spent,
/// and the outcome became `Exhausted(Work)`. With no work declared, the
/// self-loop edge cannot be charged and exhaustion is the first failure.
#[test]
fn exhaustion_after_a_rejection_keeps_the_rejection() -> Result<(), String> {
    let deps = [vec![23], vec![25], vec![]];
    assert_eq!(
        crate::builders::unsupported_of(outcome_of(&deps, 1)?)?,
        noble_kernel::untrusted::UnsupportedKind::RecursiveDependency(
            noble_kernel::contracts::Definition(23)
        ),
        "the self-loop rejection is the first failure and ends the walk"
    );
    let starved = outcome_of(&deps, 0)?;
    let noble_kernel::untrusted::Outcome::Exhausted(noble_kernel::untrusted::LimitKind::Work) =
        starved
    else {
        return Err(format!(
            "an uncharged self-loop edge must exhaust first, got {starved:?}"
        ));
    };
    Ok(())
}

/// Retained minimized schedule-independence counterexample for #134 (its
/// reference-model half): 23 depends on 24 and 25, and 24 on 25, an acyclic
/// diamond whose shared 25 finishes before 24 reaches it in the forward
/// order. Before the fix the reference model's walk kept finished
/// definitions on its path: it rejected the forward order as a cycle closing
/// on 25 and accepted the reversed one. The kernel accepted both orders
/// before its fix and still does, charging one unit per edge.
#[test]
fn shared_dependency_diamond_accepts_in_either_edge_order() -> Result<(), String> {
    for deps in [
        [vec![24, 25], vec![25], vec![]],
        [vec![25, 24], vec![25], vec![]],
    ] {
        let boundary = outcome_of(&deps, 3)?;
        let noble_kernel::untrusted::Outcome::Accepted(_) = boundary else {
            return Err(format!(
                "{deps:?}: the diamond must accept at three work units, got {boundary:?}"
            ));
        };
        let below = outcome_of(&deps, 2)?;
        let noble_kernel::untrusted::Outcome::Exhausted(noble_kernel::untrusted::LimitKind::Work) =
            below
        else {
            return Err(format!(
                "{deps:?}: two work units cannot charge three edges, got {below:?}"
            ));
        };
    }
    Ok(())
}

/// Every numbering of `count` definitions (the position each abstract
/// definition takes), built by inserting each definition into every slot.
fn all_numberings(count: usize) -> Result<Vec<Vec<usize>>, String> {
    let mut all: Vec<Vec<usize>> = vec![Vec::with_capacity(count)];
    let mut element: usize = 0;
    while element < count {
        let slots = element.checked_add(1).ok_or("numbering overflow")?;
        let total = all.len().checked_mul(slots).ok_or("numbering overflow")?;
        let mut grown: Vec<Vec<usize>> = Vec::with_capacity(total);
        for numbering in &all {
            for slot in 0..slots {
                let mut next = numbering.clone();
                next.insert(slot, element);
                grown.push(next);
            }
        }
        all = grown;
        element += 1;
    }
    Ok(all)
}

/// The renumbered graph: abstract definition `v` takes position
/// `numbering[v]`, its dependencies are renumbered alike, and bit `v` of
/// `mask` reverses its list.
fn renumber(graph: &[&[usize]], numbering: &[usize], mask: u32) -> Result<Vec<Vec<usize>>, String> {
    let mut renumbered: Vec<Vec<usize>> = vec![Vec::new(); graph.len()];
    for (node, list) in graph.iter().enumerate() {
        let mapped: Option<Vec<usize>> = list
            .iter()
            .map(|dep| numbering.get(*dep).copied())
            .collect();
        let mut mapped = mapped.ok_or("dependency outside the numbering")?;
        if ((mask >> node) & 1) == 1 {
            mapped.reverse();
        }
        let position = numbering.get(node).ok_or("unnumbered definition")?;
        *renumbered.get_mut(*position).ok_or("unplaced position")? = mapped;
    }
    Ok(renumbered)
}

/// Whether `start` reaches itself through at least one dependency edge: a
/// plain reachability search, independent of the kernel's walk.
fn is_on_cycle(graph: &[Vec<usize>], start: usize) -> bool {
    let mut visited = vec![false; graph.len()];
    let mut pending: Vec<usize> = Vec::with_capacity(graph.len());
    pending.push(start);
    let mut is_found = false;
    while let Some(node) = pending.pop() {
        for next in graph.get(node).into_iter().flatten() {
            is_found |= *next == start;
            if visited.get(*next) == Some(&false) {
                visited[*next] = true;
                pending.push(*next);
            }
        }
    }
    is_found
}

/// Check one renumbered graph and hold the outcome to the reference verdict:
/// acceptance exactly when no definition lies on a cycle, otherwise a
/// recursive-dependency rejection naming a definition that does.
fn sweep_case(base: u32, renumbered: &[Vec<usize>], tally: &mut Tally) -> Result<(), String> {
    let mut lists: Vec<Vec<u32>> = Vec::with_capacity(renumbered.len());
    for list in renumbered {
        let ids: Option<Vec<u32>> = list
            .iter()
            .map(|at| base.checked_add(u32::try_from(*at).ok()?))
            .collect();
        lists.push(ids.ok_or("definition identity overflow")?);
    }
    let env = crate::builders::env_with(&lists)?;
    let outcome = noble_kernel::acceptance::check(
        &env,
        &crate::builders::request(vec![], vec![]),
        &crate::builders::empty_program(),
    );
    let is_acyclic = (0..renumbered.len()).all(|node| !is_on_cycle(renumbered, node));
    let (is_accepted, is_kept) = match &outcome {
        noble_kernel::untrusted::Outcome::Accepted(_) => (true, is_acyclic),
        noble_kernel::untrusted::Outcome::Unsupported(
            noble_kernel::untrusted::UnsupportedKind::RecursiveDependency(named),
        ) => {
            let at = named
                .0
                .checked_sub(base)
                .and_then(|at| usize::try_from(at).ok());
            (
                false,
                !is_acyclic && at.is_some_and(|at| is_on_cycle(renumbered, at)),
            )
        }
        noble_kernel::untrusted::Outcome::Invalid(_)
        | noble_kernel::untrusted::Outcome::Unsupported(_)
        | noble_kernel::untrusted::Outcome::Exhausted(_)
        | noble_kernel::untrusted::Outcome::InternalFailure => (false, false),
    };
    tally.cases += 1;
    if is_accepted {
        tally.accepted += 1;
    }
    if !is_kept {
        let case = format!("{renumbered:?} (acyclic: {is_acyclic}): {outcome:?}");
        tally.violations.push(case);
    }
    Ok(())
}

/// Every numbering of every graph in `GRAPHS`, with every multi-entry
/// dependency list in both its forward and its reversed order, keeps the
/// reference verdict.
#[test]
fn every_numbering_and_edge_order_keeps_the_verdict() -> Result<(), String> {
    let bootstrap = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    let base = u32::try_from(bootstrap.defs.len()).map_err(|_| "bootstrap exceeds u32")?;
    let mut tally = Tally {
        cases: 0,
        accepted: 0,
        violations: Vec::with_capacity(GRAPHS.len()),
    };
    for graph in GRAPHS {
        // Bit `v` of a mask reverses list `v`; a list with fewer than two
        // entries reads the same both ways, so its bit stays clear.
        let short = graph.iter().enumerate().filter(|(_, list)| list.len() < 2);
        let fixed = short.fold(0_u32, |bits, (node, _)| bits | (1 << node));
        for numbering in &all_numberings(graph.len())? {
            for mask in (0..(1_u32 << graph.len())).filter(|mask| (mask & fixed) == 0) {
                sweep_case(base, &renumber(graph, numbering, mask)?, &mut tally)?;
            }
        }
    }
    if let Some(first) = tally.violations.first() {
        return Err(format!(
            "{} of {} cases broke the reference verdict; first: {first}",
            tally.violations.len(),
            tally.cases
        ));
    }
    // Numberings times orientations, graph by graph:
    // 3!*2 + 3!*2 + 4! + 3! + 4! + 4! + 4! + 4!*2 + 4!*4 + 4!*4 = 366,
    // of which the four acyclic graphs give 12 + 24 + 48 + 96 = 180.
    assert_eq!(tally.cases, 366, "every renumbered case was checked");
    assert_eq!(tally.accepted, 180, "exactly the acyclic cases accepted");
    Ok(())
}
