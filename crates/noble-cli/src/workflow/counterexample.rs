//! Boundary-value counterexample candidates for unary `I64` MC1 contracts
//! (proof-automation profile PAUTO-P1).
//!
//! The candidate generator, the host evaluator and the instantiated refutation
//! text are untrusted producer output. A candidate becomes `disproved` only when
//! the pinned isolated Lean consumer accepts the reviewed refutation template
//! (`NobleContracts.exportedClaim_unaryI64_refutation`) instantiated at that
//! input against the separately regenerated claim. The compiled run of the
//! subject is test-class `observed-execution` evidence: it never decides the
//! outcome, and a disagreement with the semantic replay is reported as a
//! correspondence discrepancy that resolves nothing.

/// The P1 candidate producer and its revision.
pub(super) const PRODUCER: &str = "boundary-value";
pub(super) const PRODUCER_REVISION: u64 = 1;

/// The P1 boundary values, tried in this order; the producer budget is this list.
const BOUNDARY: [i64; 6] = [0, 1, -1, i64::MAX, i64::MIN, i64::MAX - 1];

/// The expression-graph work one predicate evaluation may spend.
const EVALUATION_BUDGET: u32 = 4096;

/// The producer's own budget pool, disjoint from the consumer's sandbox
/// limits and proof timeout: at most the boundary list, and a bounded walk
/// per predicate evaluation.
pub(super) fn budget_json() -> super::encoding::Json {
    super::encoding::object([
        (
            "candidate_inputs",
            super::encoding::Json::Number(BOUNDARY.len() as u64),
        ),
        (
            "predicate_work",
            super::encoding::Json::Number(u64::from(EVALUATION_BUDGET)),
        ),
        (
            "pool",
            super::encoding::string("producer; disjoint from the consumer's limits"),
        ),
    ])
}

/// A scalar value of the evaluated subset.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scalar {
    I64(i64),
    Bool(bool),
}

/// One straight-line MC1 operation of the P1 profile.
#[derive(Clone, Copy)]
enum Operation {
    Literal(Scalar),
    Dup,
    Drop,
    Swap,
    Add,
    Sub,
    Mul,
    Equals,
}

/// The accepted subject in the P1 profile: one `I64` input, one `I64` output,
/// no parameters, and a straight-line body of literals and pure builtins.
pub(super) struct Plan {
    operations: std::vec::Vec<Operation>,
}

/// A candidate input, its semantic output, and the source the compiled run uses.
pub(super) struct Candidate {
    pub(super) input: i64,
    pub(super) output: i64,
    /// Whether an arithmetic step wrapped around the signed 64-bit range.
    pub(super) has_wrapped: bool,
}

/// Select the P1 subject, or report the construct outside the profile. No
/// producer runs for an unsupported subject.
///
/// # Errors
/// An `unsupported` failure naming the first construct outside the profile.
pub(super) fn plan(prepared: &noble_contracts::Prepared) -> Result<Plan, super::output::Failure> {
    attempt!(unary_i64(prepared.inputs(), "input"));
    attempt!(unary_i64(prepared.outputs(), "output"));
    if !prepared.params().is_empty() {
        return Err(outside(
            "the boundary producer takes no ghost parameters".into(),
        ));
    }
    let subject = attempt!(noble_contracts::wire::lower_subject(prepared.candidate())
        .map_err(|_| outside("the program has no MC1 semantic projection".into())));
    let mut operations = std::vec::Vec::with_capacity(subject.body.len());
    for index in &subject.body {
        let node = attempt!(usize::try_from(*index)
            .ok()
            .and_then(|position| subject.nodes.get(position))
            .ok_or_else(|| outside(std::format!("program node {index} is missing"))));
        operations.push(attempt!(operation(node)));
    }
    attempt!(predicate_in_profile(prepared, prepared.requires()));
    attempt!(predicate_in_profile(prepared, prepared.ensures()));
    Ok(Plan { operations })
}

fn outside(message: std::string::String) -> super::output::Failure {
    super::output::Failure::unsupported("counterexample-outside-profile", message)
}

fn unary_i64(
    stack: &[noble_contracts::NamedType],
    role: &str,
) -> Result<(), super::output::Failure> {
    match stack {
        [single] if single.ty == noble_kernel::types::Ty::I64 => Ok(()),
        _ => Err(outside(std::format!(
            "the boundary producer takes exactly one I64 {role}; this contract declares {}",
            stack.len()
        ))),
    }
}

fn operation(
    node: &noble_contracts::wire::SemanticNode,
) -> Result<Operation, super::output::Failure> {
    match node {
        noble_contracts::wire::SemanticNode::I64(value) => {
            Ok(Operation::Literal(Scalar::I64(*value)))
        }
        noble_contracts::wire::SemanticNode::Boolean(value) => {
            Ok(Operation::Literal(Scalar::Bool(*value)))
        }
        noble_contracts::wire::SemanticNode::Word(0) => Ok(Operation::Dup),
        noble_contracts::wire::SemanticNode::Word(1) => Ok(Operation::Drop),
        noble_contracts::wire::SemanticNode::Word(2) => Ok(Operation::Swap),
        noble_contracts::wire::SemanticNode::Word(4) => Ok(Operation::Add),
        noble_contracts::wire::SemanticNode::Word(5) => Ok(Operation::Sub),
        noble_contracts::wire::SemanticNode::Word(6) => Ok(Operation::Mul),
        noble_contracts::wire::SemanticNode::Word(7) => Ok(Operation::Equals),
        noble_contracts::wire::SemanticNode::Word(word) => Err(outside(std::format!(
            "word {word} is outside the straight-line profile"
        ))),
        noble_contracts::wire::SemanticNode::UnitValue => {
            Err(outside("a unit literal is outside the profile".into()))
        }
        noble_contracts::wire::SemanticNode::Quotation(_) => Err(outside(
            "a quotation is outside the straight-line profile".into(),
        )),
    }
}

/// Walk a predicate and refuse the first node outside the scalar subset, with its span.
fn predicate_in_profile(
    prepared: &noble_contracts::Prepared,
    root: u32,
) -> Result<(), super::output::Failure> {
    let mut pending = std::vec::Vec::with_capacity(16);
    pending.push(root);
    let mut work = 0u32;
    while let Some(index) = pending.pop() {
        work = work.saturating_add(1);
        if work > EVALUATION_BUDGET {
            return Err(outside(
                "the predicate exceeds the producer's walk budget".into(),
            ));
        }
        let expression = attempt!(node(prepared, index));
        let operands: &[u32] = match &expression.kind {
            noble_contracts::ExprKind::I64(_)
            | noble_contracts::ExprKind::Bool(_)
            | noble_contracts::ExprKind::Input(0)
            | noble_contracts::ExprKind::Output(0) => &[],
            noble_contracts::ExprKind::Not(a) => core::slice::from_ref(a),
            noble_contracts::ExprKind::And(a, b)
            | noble_contracts::ExprKind::Or(a, b)
            | noble_contracts::ExprKind::Implies(a, b)
            | noble_contracts::ExprKind::Eq(a, b)
            | noble_contracts::ExprKind::Lt(a, b)
            | noble_contracts::ExprKind::Le(a, b)
            | noble_contracts::ExprKind::Add(a, b)
            | noble_contracts::ExprKind::Sub(a, b)
            | noble_contracts::ExprKind::Mul(a, b) => {
                pending.push(*a);
                pending.push(*b);
                &[]
            }
            noble_contracts::ExprKind::Unit
            | noble_contracts::ExprKind::Input(_)
            | noble_contracts::ExprKind::Output(_)
            | noble_contracts::ExprKind::Param(_)
            | noble_contracts::ExprKind::Definition(_)
            | noble_contracts::ExprKind::Pair(_, _)
            | noble_contracts::ExprKind::First(_)
            | noble_contracts::ExprKind::Second(_)
            | noble_contracts::ExprKind::Inl(_)
            | noble_contracts::ExprKind::Inr(_)
            | noble_contracts::ExprKind::IsLeft(_)
            | noble_contracts::ExprKind::Left(_)
            | noble_contracts::ExprKind::Right(_)
            | noble_contracts::ExprKind::Nil
            | noble_contracts::ExprKind::Cons(_, _)
            | noble_contracts::ExprKind::IsNil(_)
            | noble_contracts::ExprKind::Head(_)
            | noble_contracts::ExprKind::Tail(_)
            | noble_contracts::ExprKind::Length(_)
            | noble_contracts::ExprKind::Maps(_, _, _) => {
                let mut failure =
                    outside("this predicate construct is outside the scalar profile".into());
                failure.details = super::encoding::object([
                    (
                        "start",
                        super::encoding::Json::Number(u64::from(expression.span.start)),
                    ),
                    (
                        "end",
                        super::encoding::Json::Number(u64::from(expression.span.end)),
                    ),
                ]);
                return Err(failure);
            }
        };
        pending.extend_from_slice(operands);
    }
    Ok(())
}

fn node(
    prepared: &noble_contracts::Prepared,
    index: u32,
) -> Result<&noble_contracts::Expr, super::output::Failure> {
    usize::try_from(index)
        .ok()
        .and_then(|position| prepared.expressions().get(position))
        .ok_or_else(|| outside(std::format!("predicate node {index} is missing")))
}

/// Run the straight-line program on one input under the MC1 wrapping
/// semantics, and whether any step wrapped. `None` when the body does not
/// leave exactly one `I64`.
fn execute(plan: &Plan, input: i64) -> Option<(i64, bool)> {
    let mut stack = std::vec::Vec::with_capacity(plan.operations.len().saturating_add(1));
    stack.push(Scalar::I64(input));
    let mut has_wrapped = false;
    for operation in &plan.operations {
        match operation {
            Operation::Literal(value) => stack.push(*value),
            Operation::Dup => {
                let top = *stack.last()?;
                stack.push(top);
            }
            Operation::Drop => {
                stack.pop()?;
            }
            Operation::Swap => {
                let top = stack.pop()?;
                let below = stack.pop()?;
                stack.push(top);
                stack.push(below);
            }
            Operation::Add => {
                let (sum, overflowed) = arithmetic(&mut stack, i64::overflowing_add)?;
                has_wrapped |= overflowed;
                stack.push(Scalar::I64(sum));
            }
            Operation::Sub => {
                let (difference, overflowed) = arithmetic(&mut stack, i64::overflowing_sub)?;
                has_wrapped |= overflowed;
                stack.push(Scalar::I64(difference));
            }
            Operation::Mul => {
                let (product, overflowed) = arithmetic(&mut stack, i64::overflowing_mul)?;
                has_wrapped |= overflowed;
                stack.push(Scalar::I64(product));
            }
            Operation::Equals => {
                let (below, top) = operands(&mut stack)?;
                stack.push(Scalar::Bool(below == top));
            }
        }
    }
    match stack.as_slice() {
        [Scalar::I64(output)] => Some((*output, has_wrapped)),
        _ => None,
    }
}

/// Pop the top two `I64` values, the top last: MC1 `a b op` is `op a b`.
fn operands(stack: &mut std::vec::Vec<Scalar>) -> Option<(i64, i64)> {
    let Some(Scalar::I64(top)) = stack.pop() else {
        return None;
    };
    let Some(Scalar::I64(below)) = stack.pop() else {
        return None;
    };
    Some((below, top))
}

fn arithmetic(
    stack: &mut std::vec::Vec<Scalar>,
    apply: fn(i64, i64) -> (i64, bool),
) -> Option<(i64, bool)> {
    let (below, top) = operands(stack)?;
    Some(apply(below, top))
}

/// Evaluate a predicate node under MC1 `evaluate`, at one input and an
/// optional output. `None` is an undefined value, never `false`.
fn evaluate(
    prepared: &noble_contracts::Prepared,
    root: u32,
    input: i64,
    output: Option<i64>,
    budget: &mut u32,
) -> Option<Scalar> {
    *budget = budget.checked_sub(1)?;
    let expression = node(prepared, root).ok()?;
    let mut binary = |a: u32, b: u32| -> Option<(Scalar, Scalar)> {
        let left = evaluate(prepared, a, input, output, budget)?;
        let right = evaluate(prepared, b, input, output, budget)?;
        Some((left, right))
    };
    match &expression.kind {
        noble_contracts::ExprKind::I64(value) => Some(Scalar::I64(*value)),
        noble_contracts::ExprKind::Bool(value) => Some(Scalar::Bool(*value)),
        noble_contracts::ExprKind::Input(0) => Some(Scalar::I64(input)),
        noble_contracts::ExprKind::Output(0) => output.map(Scalar::I64),
        noble_contracts::ExprKind::Not(a) => match evaluate(prepared, *a, input, output, budget)? {
            Scalar::Bool(value) => Some(Scalar::Bool(!value)),
            Scalar::I64(_) => None,
        },
        noble_contracts::ExprKind::And(a, b) => booleans(binary(*a, *b)?, |x, y| x && y),
        noble_contracts::ExprKind::Or(a, b) => booleans(binary(*a, *b)?, |x, y| x || y),
        noble_contracts::ExprKind::Implies(a, b) => booleans(binary(*a, *b)?, |x, y| !x || y),
        noble_contracts::ExprKind::Eq(a, b) => {
            let (left, right) = binary(*a, *b)?;
            Some(Scalar::Bool(left == right))
        }
        noble_contracts::ExprKind::Lt(a, b) => compare(binary(*a, *b)?, |x, y| x < y),
        noble_contracts::ExprKind::Le(a, b) => compare(binary(*a, *b)?, |x, y| x <= y),
        noble_contracts::ExprKind::Add(a, b) => integers(binary(*a, *b)?, i64::wrapping_add),
        noble_contracts::ExprKind::Sub(a, b) => integers(binary(*a, *b)?, i64::wrapping_sub),
        noble_contracts::ExprKind::Mul(a, b) => integers(binary(*a, *b)?, i64::wrapping_mul),
        noble_contracts::ExprKind::Unit
        | noble_contracts::ExprKind::Input(_)
        | noble_contracts::ExprKind::Output(_)
        | noble_contracts::ExprKind::Param(_)
        | noble_contracts::ExprKind::Definition(_)
        | noble_contracts::ExprKind::Pair(_, _)
        | noble_contracts::ExprKind::First(_)
        | noble_contracts::ExprKind::Second(_)
        | noble_contracts::ExprKind::Inl(_)
        | noble_contracts::ExprKind::Inr(_)
        | noble_contracts::ExprKind::IsLeft(_)
        | noble_contracts::ExprKind::Left(_)
        | noble_contracts::ExprKind::Right(_)
        | noble_contracts::ExprKind::Nil
        | noble_contracts::ExprKind::Cons(_, _)
        | noble_contracts::ExprKind::IsNil(_)
        | noble_contracts::ExprKind::Head(_)
        | noble_contracts::ExprKind::Tail(_)
        | noble_contracts::ExprKind::Length(_)
        | noble_contracts::ExprKind::Maps(_, _, _) => None,
    }
}

fn booleans((left, right): (Scalar, Scalar), apply: fn(bool, bool) -> bool) -> Option<Scalar> {
    match (left, right) {
        (Scalar::Bool(x), Scalar::Bool(y)) => Some(Scalar::Bool(apply(x, y))),
        (Scalar::Bool(_) | Scalar::I64(_), Scalar::Bool(_) | Scalar::I64(_)) => None,
    }
}

fn compare((left, right): (Scalar, Scalar), apply: fn(i64, i64) -> bool) -> Option<Scalar> {
    match (left, right) {
        (Scalar::I64(x), Scalar::I64(y)) => Some(Scalar::Bool(apply(x, y))),
        (Scalar::Bool(_) | Scalar::I64(_), Scalar::Bool(_) | Scalar::I64(_)) => None,
    }
}

fn integers((left, right): (Scalar, Scalar), apply: fn(i64, i64) -> i64) -> Option<Scalar> {
    match (left, right) {
        (Scalar::I64(x), Scalar::I64(y)) => Some(Scalar::I64(apply(x, y))),
        (Scalar::Bool(_) | Scalar::I64(_), Scalar::Bool(_) | Scalar::I64(_)) => None,
    }
}

fn holds(prepared: &noble_contracts::Prepared, root: u32, input: i64, output: Option<i64>) -> bool {
    let mut budget = EVALUATION_BUDGET;
    evaluate(prepared, root, input, output, &mut budget) == Some(Scalar::Bool(true))
}

/// The host evaluator's reading of a predicate at one input and output:
/// `true`, `false`, or `undefined`. Descriptive only.
pub(super) fn evaluation(
    prepared: &noble_contracts::Prepared,
    root: u32,
    input: i64,
    output: Option<i64>,
) -> &'static str {
    let mut budget = EVALUATION_BUDGET;
    match evaluate(prepared, root, input, output, &mut budget) {
        Some(Scalar::Bool(true)) => "true",
        Some(Scalar::Bool(false)) => "false",
        Some(Scalar::I64(_)) | None => "undefined",
    }
}

/// The declared predicate's source span and text, read from the bounded
/// contract source when it is UTF-8.
pub(super) fn declared(
    prepared: &noble_contracts::Prepared,
    root: u32,
    source: Option<&str>,
) -> super::encoding::Json {
    let Ok(expression) = node(prepared, root) else {
        return super::encoding::Json::Null;
    };
    let text = usize::try_from(expression.span.start)
        .ok()
        .zip(usize::try_from(expression.span.end).ok())
        .and_then(|(start, end)| source.and_then(|text| text.get(start..end)));
    super::encoding::object([
        (
            "start",
            super::encoding::Json::Number(u64::from(expression.span.start)),
        ),
        (
            "end",
            super::encoding::Json::Number(u64::from(expression.span.end)),
        ),
        ("text", super::encoding::optional_string(text)),
    ])
}

/// The boundary candidates in producer order: each satisfies the precondition
/// and returns normally, and its semantic output falsifies the postcondition
/// under the host evaluator. The evaluator only selects candidates; it proves
/// nothing.
pub(super) fn boundary_candidates(
    plan: &Plan,
    prepared: &noble_contracts::Prepared,
) -> std::vec::Vec<Candidate> {
    let mut found = std::vec::Vec::with_capacity(BOUNDARY.len());
    for input in BOUNDARY {
        if !holds(prepared, prepared.requires(), input, None) {
            continue;
        }
        let Some((output, has_wrapped)) = execute(plan, input) else {
            continue;
        };
        if !holds(prepared, prepared.ensures(), input, Some(output)) {
            found.push(Candidate {
                input,
                output,
                has_wrapped,
            });
        }
    }
    found
}

/// A supplied candidate input (a model from an external producer): its
/// semantic output under the host evaluator, with no filtering by the
/// postcondition. `None` when the program does not return one `I64`.
pub(super) fn supplied_candidate(plan: &Plan, input: i64) -> Option<Candidate> {
    execute(plan, input).map(|(output, has_wrapped)| Candidate {
        input,
        output,
        has_wrapped,
    })
}

/// The instantiated reviewed refutation template for one candidate, as an
/// `MC1Proof.refutation` module over the regenerated `MC1Obligation`. The
/// execution witness is the chain of MC1 step constructors for the exact
/// program; the precondition and postcondition facts are left to the pinned
/// Lean consumer's `simp` over the generated definitions.
pub(super) fn refutation_module(
    plan: &Plan,
    prepared: &noble_contracts::Prepared,
    candidate: &Candidate,
) -> std::string::String {
    let mut steps = std::string::String::with_capacity(plan.operations.len().saturating_mul(24));
    for operation in &plan.operations {
        steps.push_str("(Run.cons Step.");
        steps.push_str(match operation {
            Operation::Literal(_) => "lit",
            Operation::Dup => "dup",
            Operation::Drop => "drop",
            Operation::Swap => "swap",
            Operation::Add => "add",
            Operation::Sub => "sub",
            Operation::Mul => "mul",
            Operation::Equals => "equals",
        });
        steps.push(' ');
    }
    steps.push_str("Run.nil");
    for _ in &plan.operations {
        steps.push(')');
    }
    let mut definitions =
        std::string::String::from("MC1Obligation.precondition, MC1Obligation.postcondition");
    for index in 0..prepared.expressions().len() {
        definitions.push_str(", MC1Obligation.expression_");
        definitions.push_str(&index.to_string());
    }
    std::format!(
        "import MC1Obligation\nimport NobleContracts.Refutation\n\nopen NobleContracts\n\nnamespace MC1Proof\n\n\
theorem refutation : Not MC1Obligation.claim :=\n  exportedClaim_unaryI64_refutation\n    \
(input := BitVec.ofInt 64 ({input}))\n    (output := BitVec.ofInt 64 ({output}))\n    {steps}\n    \
(by simp [{definitions}, Holds, evaluate])\n    (by simp [{definitions}, Holds, evaluate])\n\nend MC1Proof\n",
        input = candidate.input,
        output = candidate.output,
    )
}

/// The Noble source the compiled observation runs: the candidate input, then
/// the subject's straight-line body in canonical spelling.
pub(super) fn observation_source(plan: &Plan, candidate: &Candidate) -> std::string::String {
    let mut source = candidate.input.to_string();
    for operation in &plan.operations {
        source.push(' ');
        match operation {
            Operation::Literal(Scalar::I64(value)) => source.push_str(&value.to_string()),
            Operation::Literal(Scalar::Bool(true)) => source.push_str("true"),
            Operation::Literal(Scalar::Bool(false)) => source.push_str("false"),
            Operation::Dup => source.push_str("dup"),
            Operation::Drop => source.push_str("drop"),
            Operation::Swap => source.push_str("swap"),
            Operation::Add => source.push('+'),
            Operation::Sub => source.push('-'),
            Operation::Mul => source.push('*'),
            Operation::Equals => source.push('='),
        }
    }
    source.push('\n');
    source
}

/// The JSON record of one candidate's semantic replay.
pub(super) fn candidate_json(candidate: &Candidate) -> super::encoding::Json {
    super::encoding::object([
        (
            "input",
            super::encoding::string(candidate.input.to_string()),
        ),
        (
            "semantic_output",
            super::encoding::string(candidate.output.to_string()),
        ),
        (
            "wrapped",
            super::encoding::Json::Bool(candidate.has_wrapped),
        ),
    ])
}

/// The bound on the compiled run's report this shell reads.
const OBSERVATION_LIMIT: u64 = 1_048_576;

/// A compiled observation: test-class evidence, never acceptance.
pub(super) struct Observation {
    /// Whether the compiled run produced a report this shell could read.
    pub(super) completed: bool,
    /// The single `I64` the compiled run returned normally, if it did.
    pub(super) output: Option<i64>,
    pub(super) json: super::encoding::Json,
}

/// Run the canonical candidate source once through this executable's
/// ordinary isolated `noble run` path under its default quotas, and record
/// the returned stack or the refusal with the compiled artifact's identity.
pub(super) fn observe(source: &str) -> Observation {
    match run_compiled(source) {
        Ok((output, report)) => Observation {
            completed: true,
            output,
            json: super::encoding::object([
                (
                    "evidence_class",
                    super::encoding::string("observed-execution"),
                ),
                (
                    "route",
                    super::encoding::string("noble run (isolated, default quotas)"),
                ),
                ("source", super::encoding::string(source)),
                ("outcome", super::encoding::string(report.outcome)),
                (
                    "output",
                    match output {
                        Some(value) => super::encoding::string(value.to_string()),
                        None => super::encoding::Json::Null,
                    },
                ),
                (
                    "source_sha256",
                    super::encoding::optional_string(report.source_sha256.as_deref()),
                ),
                (
                    "wat_sha256",
                    super::encoding::optional_string(report.wat_sha256.as_deref()),
                ),
                (
                    "wasm_sha256",
                    super::encoding::optional_string(report.wasm_sha256.as_deref()),
                ),
            ]),
        },
        Err(message) => Observation {
            completed: false,
            output: None,
            json: super::encoding::object([
                (
                    "evidence_class",
                    super::encoding::string("observed-execution"),
                ),
                (
                    "route",
                    super::encoding::string("noble run (isolated, default quotas)"),
                ),
                ("source", super::encoding::string(source)),
                ("outcome", super::encoding::string("unavailable")),
                ("diagnostic", super::encoding::string(message)),
            ]),
        },
    }
}

/// The fields of a `noble-core-report/v1` this shell keeps.
struct RunReport {
    outcome: std::string::String,
    source_sha256: Option<std::string::String>,
    wat_sha256: Option<std::string::String>,
    wasm_sha256: Option<std::string::String>,
}

fn run_compiled(source: &str) -> Result<(Option<i64>, RunReport), std::string::String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let directory = std::env::temp_dir().join(std::format!(
        "noble-counterexample-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).map_err(|error| error.to_string())?;
    let path = directory.join("candidate.noble");
    let outcome = std::fs::write(&path, source)
        .map_err(|error| error.to_string())
        .and_then(|()| {
            std::process::Command::new(executable)
                .arg("run")
                .arg(&path)
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .map_err(|error| error.to_string())
        });
    let _removed = std::fs::remove_dir_all(&directory);
    let output = outcome?;
    if u64::try_from(output.stdout.len()).unwrap_or(u64::MAX) > OBSERVATION_LIMIT {
        return Err("the compiled run's report exceeds the bound this shell reads".into());
    }
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let text = |value: &serde_json::Value| value.as_str().map(std::string::String::from);
    let module = &report["module"];
    let kept = RunReport {
        outcome: text(&report["outcome"]).unwrap_or_else(|| "absent".into()),
        source_sha256: text(&module["source_sha256"]),
        wat_sha256: text(&module["wat_sha256"]),
        wasm_sha256: text(&module["wasm_sha256"]),
    };
    let returned = match report["stack"].as_array().map(std::vec::Vec::as_slice) {
        Some([cell]) if kept.outcome == "normal" && cell["type"] == "I64" => cell["value"]
            .as_str()
            .and_then(|value| value.parse::<i64>().ok()),
        Some(_) | None => None,
    };
    Ok((returned, kept))
}

#[cfg(test)]
#[path = "counterexample/tests.rs"]
mod tests;
