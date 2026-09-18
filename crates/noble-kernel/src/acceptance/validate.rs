//! External-environment-data validation walks (B-CHECK-02).
//!
//! Recursive definition dependencies and user-declared recursive schemas
//! are unsupported: both are rejected during preflight, before any
//! candidate body is checked, so external environment data never enters
//! checking unvalidated. Each walk is bounded, iterative, and charges the
//! declared work limit before every dependency edge and schema entry; no
//! step recurses and no loop body returns early.

/// The state one dependency walk threads through.
struct DepWalk {
    /// Per-definition color: 0 unvisited, 1 on the current path, 2 done.
    colors: alloc::vec::Vec<u8>,
    /// The depth-first stack of definitions with edges still pending.
    stack: alloc::vec::Vec<crate::contracts::Definition>,
    /// Work units spent on edges.
    spent: u32,
}

/// The dependency list of one definition, when it declares any.
fn deps_of(
    env: &crate::contracts::Env,
    def: crate::contracts::Definition,
) -> alloc::vec::Vec<crate::contracts::Definition> {
    match usize::try_from(def.0) {
        Ok(index) => match env.deps.get(index) {
            Some(list) => list.clone(),
            None => alloc::vec::Vec::new(),
        },
        Err(_) => alloc::vec::Vec::new(),
    }
}

/// Validate the environment's definition dependencies (B-CHECK-02).
///
/// A definition that transitively depends on itself — directly or through
/// a chain — is rejected as unsupported, naming the definition the cycle
/// closes on. Every followed edge charges the declared work limit before
/// it is taken; a walk that would exceed it fails closed as exhausted. The
/// bootstrap table declares no dependencies, so its walk charges nothing.
pub(super) fn dependencies(
    env: &crate::contracts::Env,
    limits: &crate::untrusted::Limits,
) -> Result<(), super::Fail> {
    let count = env.defs.len();
    let mut walk = DepWalk {
        colors: alloc::vec![0; count],
        stack: alloc::vec::Vec::with_capacity(crate::capacity::at_least(count, 4)),
        spent: 0,
    };
    let mut failure: Option<super::Fail> = None;
    let mut root: usize = 0;
    while root < count && failure.is_none() {
        if walk.colors[root] == 0 {
            let (next, step) =
                dep_root(env, crate::contracts::Definition(root as u32), walk, limits);
            walk = next;
            match step {
                Ok(()) => root += 1,
                Err(problem) => failure = Some(problem),
            }
        } else {
            root += 1;
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

/// Run one depth-first walk from an unvisited root to completion.
fn dep_root(
    env: &crate::contracts::Env,
    root: crate::contracts::Definition,
    walk: DepWalk,
    limits: &crate::untrusted::Limits,
) -> (DepWalk, Result<(), super::Fail>) {
    let mut walk = walk;
    walk.stack.push(root);
    let mut failure: Option<super::Fail> = None;
    while !walk.stack.is_empty() && failure.is_none() {
        let (next, step) = dep_step(env, walk, limits);
        walk = next;
        match step {
            Ok(()) => {}
            Err(problem) => failure = Some(problem),
        }
    }
    match failure {
        Some(problem) => (walk, Err(problem)),
        None => (walk, Ok(())),
    }
}

/// Take one depth-first step over the stack top.
///
/// An unvisited definition turns on-path and enqueues every unvisited
/// successor; an edge to an already on-path definition closes a cycle and
/// rejects, naming that definition; a resurfaced on-path definition has
/// finished its successors and turns done. Every arena position passes
/// through on-path at most once, so the walk itself terminates; the work
/// limit is its fail-closed bound, charged per followed edge.
fn dep_step(
    env: &crate::contracts::Env,
    mut walk: DepWalk,
    limits: &crate::untrusted::Limits,
) -> (DepWalk, Result<(), super::Fail>) {
    let top = match walk.stack.last() {
        Some(top) => *top,
        None => return (walk, Ok(())),
    };
    let index = usize::try_from(top.0).unwrap_or(usize::MAX);
    let color = walk.colors.get(index).copied().unwrap_or(2);
    if color == 1 {
        // Resurfaced: every successor enqueued below has finished.
        // The color read above returned `Some`, so the position is inside.
        walk.colors[index] = 2;
        walk.stack.pop();
        return (walk, Ok(()));
    }
    if color == 2 {
        walk.stack.pop();
        return (walk, Ok(()));
    }
    // Newly reached: turn on-path, then charge and follow each edge.
    walk.colors[index] = 1;
    let deps = deps_of(env, top);
    let mut failure: Option<super::Fail> = None;
    let mut dep_index: usize = 0;
    while dep_index < deps.len() && failure.is_none() {
        if walk.spent >= limits.work {
            failure = Some(super::Fail::Exhausted(crate::untrusted::LimitKind::Work));
            break;
        }
        walk.spent += 1;
        let dep = deps[dep_index];
        let position = usize::try_from(dep.0).unwrap_or(usize::MAX);
        let dep_color = walk.colors.get(position).copied().unwrap_or(2);
        if position >= walk.colors.len() {
            // An edge outside the arena names no definition; it is a sink.
            dep_index += 1;
            continue;
        }
        if dep_color == 1 {
            failure = Some(super::Fail::Unsupported(
                crate::untrusted::UnsupportedKind::RecursiveDependency(dep),
            ));
            break;
        }
        if dep_color == 0 {
            walk.stack.push(dep);
        }
        dep_index += 1;
    }
    let outcome = match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    };
    (walk, outcome)
}

/// Validate the environment's user-declared schemas (B-CHECK-02).
///
/// A declaration that names itself is a user-declared recursive schema and
/// is rejected as unsupported, naming the declaration; a non-recursive
/// declaration still has to be a well-formed scheme. Each entry charges
/// the declared work limit before it is inspected; the bootstrap
/// environment declares none, so its scan charges nothing.
pub(super) fn schemas(
    env: &crate::contracts::Env,
    limits: &crate::untrusted::Limits,
) -> Result<(), super::Fail> {
    let mut spent: u32 = 0;
    let mut failure: Option<super::Fail> = None;
    let mut index: usize = 0;
    while index < env.schemas.len() && failure.is_none() {
        if spent >= limits.work {
            failure = Some(super::Fail::Exhausted(crate::untrusted::LimitKind::Work));
            break;
        }
        spent += 1;
        let decl = &env.schemas[index];
        if decl.recursive {
            failure = Some(super::Fail::Unsupported(
                crate::untrusted::UnsupportedKind::RecursiveSchema(decl.id),
            ));
            break;
        }
        if decl.scheme.validate().is_err() {
            failure = Some(super::Fail::Unsupported(
                crate::untrusted::UnsupportedKind::SchemeForm,
            ));
            break;
        }
        index += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}
