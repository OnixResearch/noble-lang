#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; validation mutates only preparation-owned work accounting and fresh bounded scratch buffers; the borrowed submission and published compiler remain unchanged."
)]

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the fresh done vector matches the admitted definition count and every scan is charged; a no-progress round rejects cycles as Unsupported instead of asserting on dependency input."
)]
pub(super) fn check(
    submission: &noble_kernel::execution::Submission,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let mut done = alloc::vec![false; submission.definitions.len()];
    let mut remaining = done.len();
    let mut failure = None;
    while remaining != 0 {
        let before = remaining;
        match schedule_round(submission, &mut done, remaining, work) {
            Ok(count) => remaining = count,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
        if remaining == before {
            failure = Some(crate::Diagnostic::Unsupported);
            break;
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; remaining counts precisely the false slots in the private done table; each attempt is charged before readiness lookup, and the first diagnostic stops the round without changing its scan order."
)]
fn schedule_round(
    submission: &noble_kernel::execution::Submission,
    done: &mut [bool],
    mut remaining: usize,
    work: &mut super::super::Work,
) -> Result<usize, crate::Diagnostic> {
    let mut index = 0usize;
    let mut failure = None;
    while index < done.len() {
        match schedule_definition(submission, index, done, work) {
            Ok(true) => {
                done[index] = true;
                remaining = remaining.saturating_sub(1);
            }
            Ok(false) => {}
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
        index += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(remaining),
    }
}

fn schedule_definition(
    submission: &noble_kernel::execution::Submission,
    index: usize,
    done: &[bool],
    work: &mut super::super::Work,
) -> Result<bool, crate::Diagnostic> {
    attempt!(work.charge(1));
    if done[index] {
        Ok(false)
    } else {
        is_ready(submission, index, done, work)
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; exact environment admission bounds dependency rows, definition lookup rejects missing targets and the metered node scan builds a fresh seen set; invalid edges propagate diagnostics."
)]
fn is_ready(
    submission: &noble_kernel::execution::Submission,
    index: usize,
    done: &[bool],
    work: &mut super::super::Work,
) -> Result<bool, crate::Diagnostic> {
    let definition = &submission.definitions[index];
    let table_index = attempt!(crate::admission::index(noble_kernel::untrusted::NodeId(
        definition.definition.0
    )));
    let listed = &submission.environment.deps[table_index];
    let mut seen = alloc::vec::Vec::with_capacity(definition.body.candidate.nodes.len());
    let mut is_ready = true;
    let mut node = 0usize;
    let mut failure = None;
    while node < definition.body.candidate.nodes.len() {
        match inspect_node(
            submission,
            &definition.body.candidate.nodes[node],
            done,
            &mut seen,
            work,
        ) {
            Ok(ready) => {
                is_ready &= ready;
                node += 1;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    attempt!(edges(listed, &seen, work));
    Ok(is_ready)
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::borrowed_argument_types,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the caller reserves one slot per admitted node in this fresh seen table, whose insertion requires Vec growth; metered lookup rejects missing definitions without assertions while the submission and done slice remain immutable."
)]
fn inspect_node(
    submission: &noble_kernel::execution::Submission,
    node: &noble_kernel::untrusted::Node,
    done: &[bool],
    seen: &mut alloc::vec::Vec<noble_kernel::contracts::Definition>,
    work: &mut super::super::Work,
) -> Result<bool, crate::Diagnostic> {
    attempt!(work.entries(
        seen.len()
            .saturating_add(submission.definitions.len())
            .saturating_add(1)
    ));
    let def = match node {
        noble_kernel::untrusted::Node::Invocation { def, .. } => *def,
        noble_kernel::untrusted::Node::Literal { .. }
        | noble_kernel::untrusted::Node::Quotation { .. } => return Ok(true),
    };
    if !seen.contains(&def) {
        seen.push(def);
    }
    if def.0 >= 24 {
        let target = attempt!(super::definition_index(submission, def));
        Ok(done[target])
    } else {
        Ok(true)
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each bounded edge comparison is reserved in the shared meter; missing, invented and duplicate edges are expected rejection paths and must return Invalid rather than panic."
)]
fn edges(
    listed: &[noble_kernel::contracts::Definition],
    seen: &[noble_kernel::contracts::Definition],
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    // Builtins may be listed, but no named edge may be omitted or invented.
    let mut edge = 0usize;
    let mut failure = None;
    while edge < listed.len() {
        match listed_edge(listed, seen, edge, work) {
            Ok(()) => edge += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    edge = 0;
    while edge < seen.len() {
        match seen_edge(listed, seen[edge], work) {
            Ok(()) => edge += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

fn listed_edge(
    listed: &[noble_kernel::contracts::Definition],
    seen: &[noble_kernel::contracts::Definition],
    edge: usize,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    attempt!(work.entries(seen.len().saturating_add(edge).saturating_add(1)));
    if !seen.contains(&listed[edge]) {
        return Err(crate::Diagnostic::Invalid);
    }
    let mut previous = 0usize;
    while previous < edge && listed[edge] != listed[previous] {
        previous += 1;
    }
    if previous != edge {
        Err(crate::Diagnostic::Invalid)
    } else {
        Ok(())
    }
}

fn seen_edge(
    listed: &[noble_kernel::contracts::Definition],
    definition: noble_kernel::contracts::Definition,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    attempt!(work.entries(listed.len().saturating_add(1)));
    if definition.0 >= 24 && !listed.contains(&definition) {
        Err(crate::Diagnostic::Invalid)
    } else {
        Ok(())
    }
}
