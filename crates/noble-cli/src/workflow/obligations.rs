//! Read-only obligation map and goal view (PAUTO-MAP-01, PAUTO-GOAL-01).
//!
//! Both reports are projections of prepared source data. They never run the
//! pinned Lean consumer, so no row can be acceptance evidence: every entry
//! carries `acceptance: false` and the goal view's chip says which checker
//! result, if any, produced it.
//!
//! The map's authoritative statement is the trusted generator's own text
//! (`export_lean` for a revision-1 contract, `prepare_named_contract` for the
//! named v2 family, and the exact declared source span for a pure proposition,
//! whose Lean rendering only the checker produces). `statement_digest` is the
//! SHA-256 of exactly that text.

use noble_contracts::intrinsic::{
    ContractGoal, Form, FormKind, PendingGoal, ProofBatch, ProofObligation, SubjectBinding,
};
use noble_contracts::source::{ModulePrepared, ModuleSession};
use noble_contracts::{Limits, Span};

use super::{encoding, intrinsic, output};

pub(crate) const MAP_SCHEMA: &str = "noble-obligation-map/v1";
pub(crate) const VIEW_SCHEMA: &str = "noble-goal-view/v1";

/// The Lean release the pinned consumer accepts; `workflow::USAGE` states it.
const LEAN_RELEASE: &str = "4.35.0";
/// The consumer's allowed axioms; live-slot selected-target evidence declares
/// exactly these as its assumptions.
pub(crate) const ALLOWED_AXIOMS: [&str; 3] = ["propext", "Classical.choice", "Quot.sound"];
const KEY_FORMAT: &str = "<module>@<version>/<declaration>/<revision>[/<site-path>]";
const STACK_ORDER: &str = "bottom-first; the last entry is top";
const PARTIAL_CORRECTNESS: &str = "partial-correctness";
const INCOMPLETE_NOTICE: &str = "no independent checker result is bound to this statement digest; partial correctness is never shown as checked";
const NO_VIEW_RESULT: &str = "status chips come only from independent checker output; this command runs no consumer, so every site is incomplete";

/// Which read-only report one invocation prints.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Report {
    Map,
    View,
}

impl Report {
    const fn schema(self) -> &'static str {
        match self {
            Self::Map => MAP_SCHEMA,
            Self::View => VIEW_SCHEMA,
        }
    }

    const fn command(self) -> &'static str {
        match self {
            Self::Map => "obligations",
            Self::View => "goal-view",
        }
    }

    const fn role(self) -> &'static str {
        match self {
            Self::Map => "read-only-diagnostic",
            Self::View => "read-only-projection",
        }
    }
}

/// The revisions and consumer policy a future checker result must bind. A
/// contract file has no intrinsic revision, so those fields stay null.
struct Versions {
    model: Option<std::string::String>,
    checker: Option<std::string::String>,
    named_model: Option<std::string::String>,
    named_checker: Option<std::string::String>,
}

fn module_versions(named: bool) -> Versions {
    let checker = intrinsic::checker_revision();
    let named_checker = if named {
        Some(intrinsic::named_checker_revision(&checker))
    } else {
        None
    };
    let named_model = if named {
        Some(intrinsic::named_model_revision())
    } else {
        None
    };
    Versions {
        model: Some(intrinsic::model_revision()),
        checker: Some(checker),
        named_model,
        named_checker,
    }
}

fn contract_file_versions() -> Versions {
    Versions {
        model: None,
        checker: None,
        named_model: None,
        named_checker: None,
    }
}

/// One obligation row, rendered either in full (map) or as the goal view.
struct Entry {
    key: std::string::String,
    kind: &'static str,
    declaration: std::string::String,
    revision: u32,
    semantics: &'static str,
    statement_source: &'static str,
    statement_ref: &'static str,
    statement: Option<std::string::String>,
    statement_digest: Option<std::string::String>,
    refusal: Option<std::string::String>,
    subject: encoding::Json,
    proof: Option<std::string::String>,
    predicates: encoding::Json,
    stacks: encoding::Json,
    premises: encoding::Json,
    sites: encoding::Json,
    /// The inserted-arguments row: what the `implicit-args/v1` elaborator
    /// filled into this site's proof, from its elaboration record.
    inserted: encoding::Json,
    assumptions: encoding::Json,
    vacuity: &'static str,
    staleness: encoding::Json,
}

impl Entry {
    fn map_json(&self) -> encoding::Json {
        let mut fields = std::vec::Vec::from([
            ("key", encoding::string(&self.key)),
            ("kind", encoding::string(self.kind)),
            ("declaration", encoding::string(&self.declaration)),
            ("revision", encoding::Json::Number(u64::from(self.revision))),
            ("semantics", encoding::string(self.semantics)),
            ("statement_source", encoding::string(self.statement_source)),
            ("statement_ref", encoding::string(self.statement_ref)),
            (
                "statement_digest",
                encoding::optional_string(self.statement_digest.as_deref()),
            ),
            (
                "statement",
                encoding::optional_string(self.statement.as_deref()),
            ),
            ("subject", self.subject.clone()),
            ("proof", encoding::optional_string(self.proof.as_deref())),
            ("predicates", self.predicates.clone()),
            ("stacks", self.stacks.clone()),
            ("premises", self.premises.clone()),
            ("sites", self.sites.clone()),
            ("inserted", self.inserted.clone()),
            ("arithmetic", arithmetic()),
            ("assumptions", self.assumptions.clone()),
            ("vacuity", encoding::string(self.vacuity)),
            ("acceptance", encoding::Json::Bool(false)),
            ("staleness", self.staleness.clone()),
        ]);
        if let Some(refusal) = &self.refusal {
            fields.push(("refusal", encoding::string(refusal)));
        }
        encoding::Json::Object(fields)
    }

    fn view_json(&self) -> encoding::Json {
        encoding::Json::Object(std::vec::Vec::from([
            ("key", encoding::string(&self.key)),
            ("declaration", encoding::string(&self.declaration)),
            ("revision", encoding::Json::Number(u64::from(self.revision))),
            ("statement_semantics", encoding::string(self.semantics)),
            ("statement_source", encoding::string(self.statement_source)),
            ("statement_ref", encoding::string(self.statement_ref)),
            (
                "statement_digest",
                encoding::optional_string(self.statement_digest.as_deref()),
            ),
            (
                "statement",
                encoding::optional_string(self.statement.as_deref()),
            ),
            ("chip", encoding::string("incomplete")),
            ("chip_source", encoding::string("no-checker-result")),
            ("outcome", encoding::string("not-run")),
            ("category", encoding::string("explanation-only")),
            ("checker_result", encoding::Json::Null),
            ("notice", encoding::string(INCOMPLETE_NOTICE)),
            ("predicates", self.predicates.clone()),
            ("stacks", self.stacks.clone()),
            ("premises", self.premises.clone()),
            ("sites", self.sites.clone()),
            ("inserted", self.inserted.clone()),
            ("assumptions", self.assumptions.clone()),
            ("acceptance", encoding::Json::Bool(false)),
            ("staleness", self.staleness.clone()),
        ]))
    }
}

/// The inserted-arguments row for `proof`: every argument the
/// `implicit-args/v1` elaborator filled in, from the batch's elaboration
/// record. A proof written fully explicitly has no record and an empty row.
/// The row restates an untrusted producer's record; it is never acceptance
/// evidence, which comes only from checking the explicit term.
fn inserted_arguments(batch: Option<&ProofBatch>, proof: Option<&str>) -> encoding::Json {
    let record = batch.zip(proof).and_then(|(batch, proof)| {
        batch
            .elaborations
            .iter()
            .find(|record| record.proof == proof)
    });
    let Some(record) = record else {
        return encoding::Json::Array(std::vec::Vec::new());
    };
    encoding::Json::Array(
        record
            .inserted
            .iter()
            .map(|argument| {
                encoding::object([
                    ("revision", encoding::string(&record.revision)),
                    ("binder", encoding::string(&argument.binder)),
                    (
                        "position",
                        encoding::Json::Number(u64::from(argument.position)),
                    ),
                    ("head", encoding::string(&argument.head)),
                    ("solution", encoding::string(&argument.solution)),
                    ("span", span_json(argument.span)),
                ])
            })
            .collect(),
    )
}

/// One assembled report before encoding.
struct Document {
    report: Report,
    profile: &'static str,
    source_path: std::string::String,
    source_sha256: std::string::String,
    module: encoding::Json,
    versions: Versions,
    dependencies: std::vec::Vec<encoding::Json>,
    limits: Limits,
    entries: std::vec::Vec<Entry>,
}

impl Document {
    fn json(&self) -> encoding::Json {
        let mut fields = std::vec::Vec::from([
            ("schema", encoding::string(self.report.schema())),
            ("command", encoding::string(self.report.command())),
            ("role", encoding::string(self.report.role())),
            ("acceptance", encoding::Json::Bool(false)),
            ("source_path", encoding::string(&self.source_path)),
            ("source_sha256", encoding::string(&self.source_sha256)),
            ("module", self.module.clone()),
            ("profile", encoding::string(self.profile)),
            ("key_format", encoding::string(KEY_FORMAT)),
            ("revisions", self.revisions_json()),
            ("consumer_policy", consumer_policy()),
            ("limits", limits_json(self.limits)),
            ("arithmetic", arithmetic()),
            (
                "dependencies",
                encoding::Json::Array(self.dependencies.clone()),
            ),
        ]);
        match self.report {
            Report::Map => fields.push((
                "obligations",
                encoding::Json::Array(self.entries.iter().map(Entry::map_json).collect()),
            )),
            Report::View => {
                fields.push(("checker_result", encoding::Json::Null));
                fields.push(("notice", encoding::string(NO_VIEW_RESULT)));
                fields.push((
                    "sites",
                    encoding::Json::Array(self.entries.iter().map(Entry::view_json).collect()),
                ));
            }
        }
        encoding::Json::Object(fields)
    }

    fn revisions_json(&self) -> encoding::Json {
        encoding::object([
            (
                "model",
                encoding::optional_string(self.versions.model.as_deref()),
            ),
            (
                "checker",
                encoding::optional_string(self.versions.checker.as_deref()),
            ),
            (
                "named_model",
                encoding::optional_string(self.versions.named_model.as_deref()),
            ),
            (
                "named_checker",
                encoding::optional_string(self.versions.named_checker.as_deref()),
            ),
        ])
    }
}

/// Pointer to the obligation map; the editor or a later tool re-reads it.
fn arithmetic() -> encoding::Json {
    encoding::object([
        ("i64", encoding::string("two's-complement 64-bit wrapping")),
        ("comparisons", encoding::string("signed")),
    ])
}

/// The pinned Lean consumer policy every checker result is bound to.
pub(super) fn consumer_policy() -> encoding::Json {
    encoding::object([
        ("lean_release", encoding::string(LEAN_RELEASE)),
        ("allowed_axioms", encoding::strings(&ALLOWED_AXIOMS)),
        (
            "axiom_policy",
            encoding::string("exact: only the listed axioms, and at most once each"),
        ),
        ("revision", encoding::string("noble-intrinsic-consumer/v1")),
    ])
}

fn limits_json(limits: Limits) -> encoding::Json {
    encoding::object([
        ("bytes", encoding::Json::Number(u64::from(limits.bytes))),
        ("nodes", encoding::Json::Number(u64::from(limits.nodes))),
        ("depth", encoding::Json::Number(u64::from(limits.depth))),
        ("work", encoding::Json::Number(u64::from(limits.work))),
    ])
}

fn span_json(span: Span) -> encoding::Json {
    encoding::object([
        ("start", encoding::Json::Number(u64::from(span.start))),
        ("end", encoding::Json::Number(u64::from(span.end))),
    ])
}

fn optional_span(span: Option<Span>) -> encoding::Json {
    span.map_or(encoding::Json::Null, span_json)
}

fn atom(form: &Form) -> Option<&str> {
    match &form.kind {
        FormKind::Atom(word) => Some(word),
        FormKind::List(_) => None,
    }
}

fn list(form: &Form) -> Option<&[Form]> {
    match &form.kind {
        FormKind::List(items) => Some(items),
        FormKind::Atom(_) => None,
    }
}

fn head(form: &Form) -> Option<&str> {
    list(form)?.first().and_then(atom)
}

fn source_slice(source: &[u8], span: Span) -> Option<&str> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    std::str::from_utf8(source.get(start..end)?).ok()
}

fn bindings(values: &[(std::string::String, std::string::String)]) -> encoding::Json {
    encoding::Json::Array(
        values
            .iter()
            .map(|(name, ty)| {
                encoding::object([
                    ("name", encoding::string(name)),
                    ("type", encoding::string(ty)),
                ])
            })
            .collect(),
    )
}

/// The Γ/Δ display of one obligation: declared stacks with the preserved
/// bottom tail. The accepted skeleton names each slice stack; this read-only
/// map renders no stack it cannot derive from prepared data.
fn stacks(
    inputs: &[(std::string::String, std::string::String)],
    outputs: &[(std::string::String, std::string::String)],
    reviewed: encoding::Json,
) -> encoding::Json {
    encoding::object([
        ("order", encoding::string(STACK_ORDER)),
        ("input", bindings(inputs)),
        ("output", bindings(outputs)),
        (
            "bottom_tail",
            encoding::object([
                ("sort", encoding::string("Stack")),
                ("preserved", encoding::Json::Bool(true)),
                ("binder", encoding::Json::Null),
                (
                    "binder_source",
                    encoding::string("the declared proof skeleton names the tail binder"),
                ),
            ]),
        ),
        ("intermediate", encoding::Json::Null),
        (
            "intermediate_source",
            encoding::string("the declared proof skeleton names each slice stack"),
        ),
        ("reviewed_slices", reviewed),
    ])
}

fn empty_stacks() -> encoding::Json {
    encoding::object([
        ("order", encoding::string(STACK_ORDER)),
        ("input", encoding::Json::Array(std::vec::Vec::new())),
        ("output", encoding::Json::Array(std::vec::Vec::new())),
        ("bottom_tail", encoding::Json::Null),
        ("intermediate", encoding::Json::Null),
        (
            "intermediate_source",
            encoding::string("a pure proposition has no contract stacks"),
        ),
        ("reviewed_slices", encoding::Json::Null),
    ])
}

fn module_assumptions(versions: &Versions, limits: Limits) -> encoding::Json {
    encoding::object([
        ("kind", encoding::string("partial-correctness")),
        ("allowed_axioms", encoding::strings(&ALLOWED_AXIOMS)),
        ("consumer_policy", consumer_policy()),
        (
            "model_revision",
            encoding::optional_string(versions.model.as_deref()),
        ),
        // The reviewed MC1 rules ride in the pinned model closure, so their
        // rule revision is exactly the model revision.
        (
            "rule_revision",
            encoding::optional_string(versions.model.as_deref()),
        ),
        (
            "checker_revision",
            encoding::optional_string(versions.checker.as_deref()),
        ),
        (
            "named_model_revision",
            encoding::optional_string(versions.named_model.as_deref()),
        ),
        (
            "named_checker_revision",
            encoding::optional_string(versions.named_checker.as_deref()),
        ),
        ("limits", limits_json(limits)),
    ])
}

fn contract_file_assumptions(limits: Limits) -> encoding::Json {
    encoding::object([
        ("kind", encoding::string("partial-correctness")),
        ("allowed_axioms", encoding::strings(&ALLOWED_AXIOMS)),
        ("consumer_policy", consumer_policy()),
        ("model_revision", encoding::Json::Null),
        ("rule_revision", encoding::Json::Null),
        ("checker_revision", encoding::Json::Null),
        ("named_model_revision", encoding::Json::Null),
        ("named_checker_revision", encoding::Json::Null),
        ("limits", limits_json(limits)),
    ])
}

fn subject_evidence(subject: &SubjectBinding) -> encoding::Json {
    encoding::object([
        ("module", encoding::string(&subject.module)),
        (
            "version",
            encoding::Json::Number(u64::from(subject.version)),
        ),
        ("definition", encoding::string(&subject.definition)),
        (
            "definition_identity_session_local",
            encoding::string(subject.definition_identity.to_string()),
        ),
        (
            "definition_owner_session_local",
            encoding::string(subject.definition_owner.to_string()),
        ),
        (
            "module_source_sha256",
            encoding::string(intrinsic::sha256(&subject.module_source)),
        ),
        (
            "definition_source_sha256",
            encoding::string(intrinsic::sha256(&subject.definition_source)),
        ),
        (
            "definition_ordinal",
            encoding::Json::Number(u64::from(subject.definition_ordinal)),
        ),
        ("source_span", span_json(subject.source_span)),
        (
            "source_dependencies",
            encoding::Json::Array(
                subject
                    .source_dependencies
                    .iter()
                    .map(|module| {
                        encoding::object([
                            ("module", encoding::string(&module.module)),
                            ("version", encoding::Json::Number(u64::from(module.version))),
                            (
                                "source_sha256",
                                encoding::string(intrinsic::sha256(&module.full_source)),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn pure_subject(
    module: &str,
    version: u32,
    proof_span: Span,
    proposition_span: Span,
) -> encoding::Json {
    encoding::object([
        ("module", encoding::string(module)),
        ("version", encoding::Json::Number(u64::from(version))),
        ("definition", encoding::Json::Null),
        ("definition_identity_session_local", encoding::Json::Null),
        ("declaration_span", span_json(proof_span)),
        ("proposition_span", span_json(proposition_span)),
    ])
}

fn module_staleness(
    subject: &SubjectBinding,
    revision: u32,
    statement_digest: Option<&str>,
    dependency_digest: Option<&str>,
    versions: &Versions,
) -> encoding::Json {
    encoding::object([
        (
            "source_sha256",
            encoding::string(intrinsic::sha256(&subject.module_source)),
        ),
        (
            "definition_source_sha256",
            encoding::string(intrinsic::sha256(&subject.definition_source)),
        ),
        (
            "definition_identity_session_local",
            encoding::string(subject.definition_identity.to_string()),
        ),
        (
            "definition_owner_session_local",
            encoding::string(subject.definition_owner.to_string()),
        ),
        (
            "contract_revision",
            encoding::Json::Number(u64::from(revision)),
        ),
        (
            "statement_digest",
            encoding::optional_string(statement_digest),
        ),
        (
            "dependency_closure_digest",
            encoding::optional_string(dependency_digest),
        ),
        (
            "model_revision",
            encoding::optional_string(versions.model.as_deref()),
        ),
        (
            "checker_revision",
            encoding::optional_string(versions.checker.as_deref()),
        ),
        (
            "named_model_revision",
            encoding::optional_string(versions.named_model.as_deref()),
        ),
        (
            "named_checker_revision",
            encoding::optional_string(versions.named_checker.as_deref()),
        ),
        (
            "consumer_policy",
            encoding::string("noble-intrinsic-consumer/v1"),
        ),
    ])
}

fn pure_staleness(
    source_sha256: &str,
    revision: u32,
    statement_digest: Option<&str>,
    versions: &Versions,
) -> encoding::Json {
    encoding::object([
        ("source_sha256", encoding::string(source_sha256)),
        ("definition_source_sha256", encoding::Json::Null),
        ("definition_identity_session_local", encoding::Json::Null),
        ("definition_owner_session_local", encoding::Json::Null),
        (
            "contract_revision",
            encoding::Json::Number(u64::from(revision)),
        ),
        (
            "statement_digest",
            encoding::optional_string(statement_digest),
        ),
        ("dependency_closure_digest", encoding::Json::Null),
        (
            "model_revision",
            encoding::optional_string(versions.model.as_deref()),
        ),
        (
            "checker_revision",
            encoding::optional_string(versions.checker.as_deref()),
        ),
        (
            "named_model_revision",
            encoding::optional_string(versions.named_model.as_deref()),
        ),
        (
            "named_checker_revision",
            encoding::optional_string(versions.named_checker.as_deref()),
        ),
        (
            "consumer_policy",
            encoding::string("noble-intrinsic-consumer/v1"),
        ),
    ])
}

fn contract_file_staleness(source_sha256: &str, statement_digest: Option<&str>) -> encoding::Json {
    encoding::object([
        ("source_sha256", encoding::string(source_sha256)),
        ("definition_source_sha256", encoding::Json::Null),
        ("definition_identity_session_local", encoding::Json::Null),
        ("definition_owner_session_local", encoding::Json::Null),
        ("contract_revision", encoding::Json::Number(1)),
        (
            "statement_digest",
            encoding::optional_string(statement_digest),
        ),
        ("dependency_closure_digest", encoding::Json::Null),
        ("model_revision", encoding::Json::Null),
        ("checker_revision", encoding::Json::Null),
        ("named_model_revision", encoding::Json::Null),
        ("named_checker_revision", encoding::Json::Null),
        ("consumer_policy", encoding::string("noble-mc1-consumer/v1")),
    ])
}

fn site_row(key: &str, name: &str, reference: &str, span: Option<Span>) -> encoding::Json {
    encoding::object([
        ("key", encoding::string(std::format!("{key}/{name}"))),
        ("site", encoding::string(name)),
        ("statement_ref", encoding::string(reference)),
        ("source_span", optional_span(span)),
    ])
}

/// One predicate root: its generated-definition reference, its exact source
/// span and the source bytes at that span. `(in x)` and `(out y)` stay in
/// their own namespaces because the declared text is shown unchanged.
fn predicate_row(reference: &str, span: Option<Span>, source: &[u8]) -> encoding::Json {
    encoding::object([
        ("statement_ref", encoding::string(reference)),
        ("source_span", optional_span(span)),
        (
            "source_text",
            encoding::optional_string(span.and_then(|span| source_slice(source, span))),
        ),
    ])
}

fn contract_predicates(goal: &ContractGoal, source: &[u8]) -> encoding::Json {
    // The named v2 claim inlines both predicates, so both roots cite the claim;
    // the revision-1 export names each predicate definition.
    let (pre, post) = match goal.revision {
        2 => ("NamedV2Obligation.claim", "NamedV2Obligation.claim"),
        _ => ("MC1Obligation.precondition", "MC1Obligation.postcondition"),
    };
    encoding::object([
        ("pre", predicate_row(pre, Some(goal.requires.span), source)),
        ("post", predicate_row(post, Some(goal.ensures.span), source)),
    ])
}

/// Constructors whose first argument is a nullary reviewed rule word.
const RULE_SITES: [&str; 3] = ["bridge", "join", "pc-exact"];

fn discharged_by<'a>(items: &'a [Form], constructor: &str) -> Option<&'a str> {
    if !RULE_SITES.contains(&constructor) {
        return None;
    }
    head(items.get(1)?)
}

/// One declared proof skeleton: one row per list constructor with its exact
/// source span, in declaration order. The walk is bounded by the parsed
/// depth and node limits and never asserts well-formedness.
fn declared_skeleton(
    key: &str,
    term: &Form,
) -> (std::vec::Vec<encoding::Json>, std::vec::Vec<encoding::Json>) {
    let mut sites = std::vec::Vec::new();
    let mut premises = std::vec::Vec::new();
    let mut pending = std::vec::Vec::new();
    if let Some(root) = head(term) {
        pending.push((std::string::String::from(root), term));
    }
    while let Some((path, form)) = pending.pop() {
        let Some(items) = list(form) else {
            continue;
        };
        let constructor = items.first().and_then(atom).unwrap_or("");
        let mut row = std::vec::Vec::from([
            ("key", encoding::string(std::format!("{key}/{path}"))),
            ("site", encoding::string(&path)),
            ("constructor", encoding::string(constructor)),
            ("source_span", span_json(form.span)),
        ]);
        if let Some(rule) = discharged_by(items, constructor) {
            row.push(("discharged_by", encoding::string(rule)));
        }
        sites.push(encoding::Json::Object(row));
        if constructor == "intro" {
            if let Some(binder) = items.get(1).and_then(list) {
                let name = binder.first().and_then(atom).unwrap_or("");
                let sort = binder.get(1).and_then(atom).unwrap_or("");
                premises.push(encoding::object([
                    ("name", encoding::string(name)),
                    ("sort", encoding::string(sort)),
                    ("site", encoding::string(&path)),
                    ("source_span", span_json(form.span)),
                ]));
            }
        }
        for (position, child) in items.iter().enumerate().rev() {
            if position == 0 {
                continue;
            }
            if let Some(child_head) = head(child) {
                // Site path indices count the constructor's arguments, so the
                // leading constructor atom is not position 0.
                pending.push((std::format!("{path}.{}.{child_head}", position - 1), child));
            }
        }
    }
    (sites, premises)
}

/// The tail binder and literal a declared revision-1 skeleton names.
struct SkeletonFacts {
    tail: Option<std::string::String>,
    literal: Option<std::string::String>,
}

fn skeleton_facts(term: &Form) -> SkeletonFacts {
    let mut facts = SkeletonFacts {
        tail: None,
        literal: None,
    };
    let mut pending = std::vec::Vec::from([term]);
    while let Some(form) = pending.pop() {
        let Some(items) = list(form) else {
            continue;
        };
        match items.first().and_then(atom) {
            Some("intro") if facts.tail.is_none() => {
                if let Some(binder) = items.get(1).and_then(list) {
                    if binder.get(1).and_then(atom) == Some("Stack") {
                        facts.tail = binder.first().and_then(atom).map(std::string::String::from);
                    }
                }
            }
            Some("exec-literal") if facts.literal.is_none() => {
                facts.literal = items.get(1).and_then(atom).map(std::string::String::from);
            }
            _ => {}
        }
        for child in items.iter().skip(1).rev() {
            if list(child).is_some() {
                pending.push(child);
            }
        }
    }
    facts
}

/// The reviewed unary-I64 slices the declared skeleton names, as display text.
/// They are derived from the declaration, never from a checker result.
fn reviewed_slices(goal: &ContractGoal, proof: Option<&ProofObligation>) -> encoding::Json {
    let Some(proof) = proof else {
        return encoding::Json::Null;
    };
    let unary = goal.revision == 1
        && goal.inputs.len() == 1
        && goal.inputs[0].1 == "I64"
        && goal.outputs.len() == 1
        && goal.outputs[0].1 == "I64";
    if !unary || head(&proof.term) != Some("export-unary-I64") {
        return encoding::Json::Null;
    }
    let facts = skeleton_facts(&proof.term);
    let (Some(tail), Some(literal)) = (facts.tail, facts.literal) else {
        return encoding::Json::Null;
    };
    let input = &goal.inputs[0].0;
    let row = |step: &'static str, text: std::string::String| {
        encoding::object([
            ("step", encoding::string(step)),
            ("text", encoding::string(text)),
            ("source", encoding::string("declared-proof-skeleton")),
        ])
    };
    encoding::Json::Array(std::vec::Vec::from([
        row("input", std::format!("{tail} ++ [{input}]")),
        row(
            "after-exec-literal",
            std::format!("{tail} ++ [{input}, {literal}]"),
        ),
        row(
            "after-exec-add",
            std::format!("{tail} ++ [wrap64({input} + {literal})]"),
        ),
    ]))
}

fn contract_sites(
    key: &str,
    goal: &ContractGoal,
    proof: Option<&ProofObligation>,
) -> (encoding::Json, encoding::Json) {
    let mut sites = std::vec::Vec::new();
    match goal.revision {
        2 => {
            sites.push(site_row(
                key,
                "claim",
                "NamedV2Obligation.claim",
                Some(goal.contract_span),
            ));
            sites.push(site_row(
                key,
                "subject",
                "NamedV2Obligation.subject",
                Some(goal.contract_span),
            ));
            for (index, origin) in goal.subject.named_uses.iter().enumerate() {
                let name = match index {
                    0 => "useA",
                    _ => "useB",
                };
                sites.push(site_row(
                    key,
                    name,
                    "NamedV2Obligation.use",
                    Some(origin.definition_span),
                ));
            }
        }
        _ => {
            sites.push(site_row(
                key,
                "precondition",
                "MC1Obligation.precondition",
                Some(goal.requires.span),
            ));
            sites.push(site_row(
                key,
                "postcondition",
                "MC1Obligation.postcondition",
                Some(goal.ensures.span),
            ));
            sites.push(site_row(
                key,
                "claim",
                "MC1Obligation.claim",
                Some(goal.contract_span),
            ));
        }
    }
    let premises = match proof {
        Some(proof) => {
            let (declared, premises) = declared_skeleton(key, &proof.term);
            sites.extend(declared);
            premises
        }
        None => std::vec::Vec::new(),
    };
    (
        encoding::Json::Array(sites),
        encoding::Json::Array(premises),
    )
}

fn contract_entry(
    goal: &ContractGoal,
    proof: Option<&ProofObligation>,
    source: &[u8],
    versions: &Versions,
    dependency_digest: Option<&str>,
    limits: Limits,
) -> Entry {
    let key = std::format!(
        "{}@{}/{}/{}",
        goal.subject.module,
        goal.subject.version,
        goal.contract_name,
        goal.revision
    );
    let named = goal.revision >= 2;
    let prepared: Result<(std::string::String, &'static str, &'static str), String> =
        match goal.revision {
            2 => noble_contracts::intrinsic::prepare_named_contract(goal, limits)
                .map(|text| (text, "prepare_named_contract", "NamedV2Obligation.claim"))
                .map_err(|problem| problem.message),
            // Revision 3 reports its regenerated canonical claim; its reviewed
            // Lean lowering is not implemented yet, so the projection is not a
            // Lean statement and says so at `statement_source`.
            3 => noble_contracts::intrinsic::claim_shape(goal, limits)
                .map(|text| (text, "claim_shape", "NamedV3Obligation.claim"))
                .map_err(|problem| problem.message),
            _ => noble_contracts::intrinsic::prepare_contract(goal, limits)
                .map(|prepared| {
                    (
                        noble_contracts::export_lean(&prepared),
                        "export_lean",
                        "MC1Obligation.claim",
                    )
                })
                .map_err(|problem| problem.message),
        };
    let (statement, statement_source, statement_ref, refusal) = match prepared {
        Ok((text, source, reference)) => (Some(text), source, reference, None),
        Err(message) => (
            None,
            if named {
                "prepare_named_contract"
            } else {
                "export_lean"
            },
            if named {
                "NamedV2Obligation.claim"
            } else {
                "MC1Obligation.claim"
            },
            Some(message),
        ),
    };
    let statement_digest = statement
        .as_deref()
        .map(|text| intrinsic::sha256(text.as_bytes()));
    let (sites, premises) = contract_sites(&key, goal, proof);
    Entry {
        key,
        kind: "contract",
        declaration: goal.contract_name.clone(),
        revision: goal.revision,
        semantics: PARTIAL_CORRECTNESS,
        statement_source,
        statement_ref,
        statement,
        statement_digest: statement_digest.clone(),
        refusal,
        subject: subject_evidence(&goal.subject),
        proof: proof.map(|proof| proof.name.clone()),
        predicates: contract_predicates(goal, source),
        stacks: stacks(&goal.inputs, &goal.outputs, reviewed_slices(goal, proof)),
        premises,
        sites,
        inserted: encoding::Json::Array(std::vec::Vec::new()),
        assumptions: module_assumptions(versions, limits),
        vacuity: match &goal.requires.kind {
            FormKind::Atom(word) if word == "true" => "none",
            FormKind::Atom(word) if word == "false" => "suspected",
            _ => "not-assessed",
        },
        staleness: module_staleness(
            &goal.subject,
            goal.revision,
            statement_digest.as_deref(),
            dependency_digest,
            versions,
        ),
    }
}

/// The target module a pure proposition belongs to.
struct PureContext<'a> {
    name: &'a str,
    version: u32,
    source: &'a [u8],
    source_sha256: &'a str,
}

fn pure_entry(
    context: &PureContext<'_>,
    proof: &ProofObligation,
    proposition: &Form,
    versions: &Versions,
    limits: Limits,
) -> Entry {
    let key = std::format!(
        "{}@{}/{}/{}",
        context.name,
        context.version,
        proof.name,
        proof.revision
    );
    let statement = source_slice(context.source, proposition.span).map(std::string::String::from);
    let statement_digest = statement
        .as_deref()
        .map(|text| intrinsic::sha256(text.as_bytes()));
    let (declared, premises) = declared_skeleton(&key, &proof.term);
    let mut sites = std::vec::Vec::from([site_row(
        &key,
        "proposition",
        "declared-proposition",
        Some(proposition.span),
    )]);
    sites.extend(declared);
    Entry {
        key,
        kind: "pure",
        declaration: proof.name.clone(),
        revision: proof.revision,
        semantics: "proposition",
        statement_source: "source-span",
        statement_ref: "declared-proposition",
        statement,
        statement_digest: statement_digest.clone(),
        refusal: None,
        subject: pure_subject(context.name, context.version, proof.span, proposition.span),
        proof: Some(proof.name.clone()),
        predicates: encoding::Json::Null,
        stacks: empty_stacks(),
        premises: encoding::Json::Array(premises),
        sites: encoding::Json::Array(sites),
        inserted: encoding::Json::Array(std::vec::Vec::new()),
        assumptions: module_assumptions(versions, limits),
        vacuity: "not-assessed",
        staleness: pure_staleness(
            context.source_sha256,
            proof.revision,
            statement_digest.as_deref(),
            versions,
        ),
    }
}

fn dependency_digest(dependencies: &[encoding::Json]) -> std::string::String {
    let mut text = std::string::String::new();
    for dependency in dependencies {
        text.push_str(&dependency.encode());
        text.push('\n');
    }
    intrinsic::sha256(text.as_bytes())
}

fn module_report(
    prepared: &ModulePrepared,
    source: &[u8],
    source_path: &str,
    source_sha256: &str,
    report: Report,
) -> encoding::Json {
    let limits = Limits::default();
    let batch = prepared.proof_obligations();
    let named = prepared.contract_goals().any(|goal| goal.revision >= 2)
        || batch.is_some_and(|batch| {
            batch
                .obligations
                .iter()
                .any(|obligation| obligation.revision == 2)
        });
    let versions = module_versions(named);
    let dependencies: std::vec::Vec<encoding::Json> = batch
        .map(|batch| {
            batch
                .dependencies
                .iter()
                .map(intrinsic::dependency_evidence)
                .collect()
        })
        .unwrap_or_default();
    let digest = (!dependencies.is_empty()).then(|| dependency_digest(&dependencies));
    let module = prepared.resolved_module();
    let (module_name, module_version) = match &module {
        Some((_, name, version)) => (name.as_str(), *version),
        None => ("", 0),
    };
    let mut entries = std::vec::Vec::new();
    for goal in prepared.contract_goals() {
        let proof = batch.and_then(|batch| contract_proof(batch, goal));
        let mut entry = contract_entry(goal, proof, source, &versions, digest.as_deref(), limits);
        entry.inserted = inserted_arguments(batch, entry.proof.as_deref());
        entries.push(entry);
    }
    if let Some(batch) = batch {
        for obligation in &batch.obligations {
            if let PendingGoal::Pure { proposition } = &obligation.goal {
                let mut entry = pure_entry(
                    &PureContext {
                        name: module_name,
                        version: module_version,
                        source,
                        source_sha256,
                    },
                    obligation,
                    proposition,
                    &versions,
                    limits,
                );
                entry.inserted = inserted_arguments(Some(batch), Some(&obligation.name));
                entries.push(entry);
            }
        }
    }
    Document {
        report,
        profile: if named {
            "Named-Contracts-v2"
        } else {
            "Intrinsic-Proofs-Draft"
        },
        source_path: std::string::String::from(source_path),
        source_sha256: std::string::String::from(source_sha256),
        module: match &module {
            Some((identity, name, version)) => encoding::object([
                (
                    "identity_session_local",
                    encoding::string(identity.to_string()),
                ),
                ("name", encoding::string(name)),
                ("version", encoding::Json::Number(u64::from(*version))),
            ]),
            None => encoding::Json::Null,
        },
        versions,
        dependencies,
        limits,
        entries,
    }
    .json()
}

fn contract_proof<'a>(batch: &'a ProofBatch, goal: &ContractGoal) -> Option<&'a ProofObligation> {
    batch.obligations.iter().find(|obligation| {
        matches!(
            &obligation.goal,
            PendingGoal::Contract { contract }
                if contract.contract_name == goal.contract_name
                    && contract.revision == goal.revision
        )
    })
}

fn expression_span(prepared: &noble_contracts::Prepared, index: u32) -> Option<Span> {
    usize::try_from(index)
        .ok()
        .and_then(|at| prepared.expressions().get(at))
        .map(|expression| expression.span)
}

fn contract_file_entry(
    prepared: &noble_contracts::Prepared,
    source: &[u8],
    source_sha256: &str,
    statement: std::string::String,
    limits: Limits,
) -> Entry {
    let semantic_revision = prepared.candidate().revision;
    let key = std::format!("{}@{semantic_revision}/claim/1", prepared.name());
    let statement_digest = intrinsic::sha256(statement.as_bytes());
    let sites = encoding::Json::Array(std::vec::Vec::from([
        site_row(
            &key,
            "precondition",
            "MC1Obligation.precondition",
            expression_span(prepared, prepared.requires()),
        ),
        site_row(
            &key,
            "postcondition",
            "MC1Obligation.postcondition",
            expression_span(prepared, prepared.ensures()),
        ),
        site_row(&key, "claim", "MC1Obligation.claim", None),
    ]));
    Entry {
        key,
        kind: "contract",
        declaration: std::string::String::from(prepared.name()),
        revision: 1,
        semantics: PARTIAL_CORRECTNESS,
        statement_source: "export_lean",
        statement_ref: "MC1Obligation.claim",
        statement: Some(statement),
        statement_digest: Some(statement_digest.clone()),
        refusal: None,
        subject: super::subject_json(prepared, source),
        proof: None,
        predicates: encoding::object([
            (
                "pre",
                predicate_row(
                    "MC1Obligation.precondition",
                    expression_span(prepared, prepared.requires()),
                    source,
                ),
            ),
            (
                "post",
                predicate_row(
                    "MC1Obligation.postcondition",
                    expression_span(prepared, prepared.ensures()),
                    source,
                ),
            ),
        ]),
        stacks: encoding::object([
            ("order", encoding::string(STACK_ORDER)),
            ("input", super::subject::bindings(prepared.inputs())),
            ("output", super::subject::bindings(prepared.outputs())),
            ("parameters", super::subject::bindings(prepared.params())),
            ("bottom_tail", encoding::Json::Null),
            ("intermediate", encoding::Json::Null),
            (
                "intermediate_source",
                encoding::string(
                    "the MC1 claim names the execution suffix; this map renders no slice stacks",
                ),
            ),
            ("reviewed_slices", encoding::Json::Null),
        ]),
        premises: encoding::Json::Array(std::vec::Vec::new()),
        sites,
        inserted: encoding::Json::Array(std::vec::Vec::new()),
        assumptions: contract_file_assumptions(limits),
        vacuity: match usize::try_from(prepared.requires())
            .ok()
            .and_then(|index| prepared.expressions().get(index))
            .map(|expression| &expression.kind)
        {
            Some(noble_contracts::ExprKind::Bool(true)) => "none",
            Some(noble_contracts::ExprKind::Bool(false)) => "suspected",
            _ => "not-assessed",
        },
        staleness: contract_file_staleness(source_sha256, Some(&statement_digest)),
    }
}

fn contract_file_report(
    source: &[u8],
    source_path: &str,
    source_sha256: &str,
    report: Report,
) -> Result<encoding::Json, output::Failure> {
    let limits = Limits::default();
    let prepared = noble_contracts::prepare(source, limits).map_err(|problem| {
        let code = match problem.kind {
            noble_contracts::DiagnosticKind::Exhausted => "obligation-map-exhausted",
            noble_contracts::DiagnosticKind::Unsupported => "obligation-map-unsupported",
            noble_contracts::DiagnosticKind::Internal => "obligation-map-internal",
            noble_contracts::DiagnosticKind::Invalid => "obligation-map-invalid",
        };
        match problem.kind {
            noble_contracts::DiagnosticKind::Unsupported => {
                output::Failure::unsupported(code, problem.message)
            }
            _ => output::Failure::error(code, problem.message),
        }
    })?;
    let statement = noble_contracts::export_lean(&prepared);
    let semantic_revision = prepared.candidate().revision;
    let entry = contract_file_entry(&prepared, source, source_sha256, statement, limits);
    Ok(Document {
        report,
        profile: "MC1",
        source_path: std::string::String::from(source_path),
        source_sha256: std::string::String::from(source_sha256),
        module: encoding::object([
            ("identity_session_local", encoding::Json::Null),
            ("name", encoding::string(prepared.name())),
            (
                "version",
                encoding::Json::Number(u64::from(semantic_revision)),
            ),
        ]),
        versions: contract_file_versions(),
        dependencies: std::vec::Vec::new(),
        limits,
        entries: std::vec::Vec::from([entry]),
    }
    .json())
}

struct Options {
    source: std::path::PathBuf,
    modules: std::vec::Vec<std::path::PathBuf>,
}

fn usage(message: &str) -> output::Failure {
    output::Failure::error("usage", std::string::String::from(message))
}

fn options(arguments: &[std::ffi::OsString], report: Report) -> Result<Options, output::Failure> {
    let source = arguments.get(1).ok_or_else(|| {
        usage(match report {
            Report::Map => "obligations SOURCE [--module FILE ...]",
            Report::View => "goal-view SOURCE [--module FILE ...]",
        })
    })?;
    let mut modules = std::vec::Vec::new();
    let mut at = 2usize;
    while at < arguments.len() {
        let option = arguments[at]
            .to_str()
            .ok_or_else(|| usage("argument must be UTF-8"))?;
        at += 1;
        let value = arguments
            .get(at)
            .ok_or_else(|| usage("missing option value"))?;
        match option {
            "--module" => {
                if modules.len() >= super::MODULE_LIMIT {
                    return Err(output::Failure::error(
                        "module-limit",
                        "too many prior module units".into(),
                    ));
                }
                modules.push(std::path::PathBuf::from(value));
            }
            _ => return Err(usage("unknown obligation-map option")),
        }
        at += 1;
    }
    Ok(Options {
        source: std::path::PathBuf::from(source),
        modules,
    })
}

fn document(
    options: &Options,
    report: Report,
    source_path: &str,
) -> Result<encoding::Json, output::Failure> {
    let source = super::read_bounded(&options.source, super::SOURCE_LIMIT, "module-source")?;
    let source_sha256 = intrinsic::sha256(&source);
    if contract_source(&source) {
        return contract_file_report(&source, source_path, &source_sha256, report);
    }
    module_document(options, &source, source_path, &source_sha256, report)
}

/// Whether the file is an MC1 `.noble-contract` rather than a `.noble` module.
fn contract_source(source: &[u8]) -> bool {
    let mut at = 0usize;
    while source
        .get(at)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        at += 1;
    }
    source
        .get(at..)
        .is_some_and(|rest| rest.starts_with(b"(contract"))
}

fn module_document(
    options: &Options,
    source: &[u8],
    source_path: &str,
    source_sha256: &str,
    report: Report,
) -> Result<encoding::Json, output::Failure> {
    let limits = Limits::default();
    let retained = noble_contracts::source::RetainedSourceLimit::new(65_536)
        .expect("obligation map retained source budget");
    let mut session =
        ModuleSession::new(&[], retained).map_err(|error| intrinsic::source_error(&error))?;
    let target = &options.source;
    for module in options.modules.iter().chain(core::iter::once(target)) {
        let owned;
        let bytes: &[u8] = if module == target {
            source
        } else {
            owned = super::read_bounded(module, super::SOURCE_LIMIT, "module-source")?;
            &owned
        };
        let prepared = session
            .prepare(bytes, &[], limits)
            .map_err(|error| intrinsic::source_error(&error))?;
        if module == target {
            return Ok(module_report(
                &prepared,
                source,
                source_path,
                source_sha256,
                report,
            ));
        }
        let (next, committed) = session.commit(prepared);
        committed.map_err(|error| intrinsic::source_error(&error))?;
        session = next;
    }
    Err(output::Failure::error(
        "obligation-map-internal",
        "no target module was selected".into(),
    ))
}

fn emit(arguments: &[std::ffi::OsString], report: Report) -> std::process::ExitCode {
    let source_path = arguments
        .get(1)
        .map_or_else(std::string::String::new, |path| {
            path.to_string_lossy().into_owned()
        });
    let result =
        options(arguments, report).and_then(|options| document(&options, report, &source_path));
    match result {
        Ok(document) => {
            std::println!("{}", document.encode());
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            let failure = encoding::object([
                ("schema", encoding::string(report.schema())),
                ("command", encoding::string(report.command())),
                ("role", encoding::string(report.role())),
                ("acceptance", encoding::Json::Bool(false)),
                ("source_path", encoding::string(&source_path)),
                ("outcome", encoding::string(error.outcome.name())),
                ("diagnostic", error.json()),
            ]);
            std::println!("{}", failure.encode());
            std::process::ExitCode::from(error.outcome.exit())
        }
    }
}

/// Print the read-only obligation map.
pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    emit(arguments, Report::Map)
}

/// Print the read-only goal view over the same prepared data.
pub(crate) fn goal_view(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    emit(arguments, Report::View)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INCREMENT: &[u8] = b"(contract 1 increment (input (x I64)) (output (y I64)) (program [ 1 + ]) (requires true) (ensures (eq (out y) (add (in x) 1))))";

    const MODULE: &[u8] = b"module arithmetic@1 [ export increment def increment [ 1 + ] contract 1 increment-law [ subject increment input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] proof 1 increment-correct for increment-law [ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ] ]";

    fn versions() -> Versions {
        Versions {
            model: Some(std::string::String::from("model")),
            checker: Some(std::string::String::from("checker")),
            named_model: None,
            named_checker: None,
        }
    }

    #[test]
    fn contract_file_entry_digests_the_export_text() {
        let limits = Limits::default();
        let prepared = noble_contracts::prepare(INCREMENT, limits).expect("prepares");
        let statement = noble_contracts::export_lean(&prepared);
        let entry = contract_file_entry(&prepared, INCREMENT, "source", statement.clone(), limits);
        assert_eq!(entry.statement.as_deref(), Some(statement.as_str()));
        assert_eq!(
            entry.statement_digest.as_deref(),
            Some(intrinsic::sha256(statement.as_bytes()).as_str())
        );
        assert_eq!(entry.statement_source, "export_lean");
        assert_eq!(entry.statement_ref, "MC1Obligation.claim");
        assert_eq!(entry.kind, "contract");
    }

    #[test]
    fn contract_entry_digests_the_prepared_statement() {
        let limits = Limits::default();
        let retained = noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("retained source budget");
        let session = ModuleSession::new(&[], retained).expect("session");
        let prepared = session.prepare(MODULE, &[], limits).expect("prepares");
        let goals: std::vec::Vec<_> = prepared.contract_goals().collect();
        assert_eq!(goals.len(), 1);
        let goal = goals[0];
        let proof = prepared
            .proof_obligations()
            .and_then(|batch| contract_proof(batch, goal));
        assert!(proof.is_some(), "the declared proof must be visible");
        let entry = contract_entry(goal, proof, MODULE, &versions(), None, limits);
        let statement = entry.statement.as_deref().expect("prepared statement");
        assert!(statement.starts_with("import NobleContracts"));
        assert_eq!(
            entry.statement_digest.as_deref(),
            Some(intrinsic::sha256(statement.as_bytes()).as_str())
        );
        assert_eq!(entry.declaration, "increment-law");
        assert_eq!(entry.proof.as_deref(), Some("increment-correct"));
        assert_eq!(entry.vacuity, "none");
        let sites = entry.sites.encode();
        assert!(
            sites.contains("export-unary-I64.0.intro.1.intro.1.pc-sequence.2.bridge"),
            "{sites}"
        );
        assert!(
            sites.contains("\"discharged_by\":\"by-exact-append-assoc\""),
            "the bridge names its reviewed rule: {sites}"
        );
        let premises = entry.premises.encode();
        assert!(premises.contains("\"name\":\"tail\""), "{premises}");
        assert!(premises.contains("\"sort\":\"I64\""), "{premises}");
        let slices = entry.stacks.encode();
        assert!(
            slices.contains("wrap64(x + 1)"),
            "the reviewed slices: {slices}"
        );
        let predicates = entry.predicates.encode();
        assert!(
            predicates.contains("\"MC1Obligation.precondition\""),
            "the pre root references its generated definition: {predicates}"
        );
        assert!(
            predicates.contains("\"source_text\":\"true\""),
            "the declared precondition text: {predicates}"
        );
    }

    #[test]
    fn pure_obligation_digests_its_exact_source_span() {
        let source: &[u8] = b"module m@1 [ proof 1 refl : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ] ]";
        let limits = Limits::default();
        let retained = noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("retained source budget");
        let session = ModuleSession::new(&[], retained).expect("session");
        let prepared = session.prepare(source, &[], limits).expect("prepares");
        let digest = intrinsic::sha256(source);
        let batch = prepared.proof_obligations().expect("proof obligations");
        let obligation = &batch.obligations[0];
        let PendingGoal::Pure { proposition } = &obligation.goal else {
            panic!("expected a pure obligation");
        };
        let entry = pure_entry(
            &PureContext {
                name: "m",
                version: 1,
                source,
                source_sha256: &digest,
            },
            obligation,
            proposition,
            &versions(),
            limits,
        );
        let statement = entry.statement.as_deref().expect("declared proposition");
        assert_eq!(statement, "(Pi (x I64) (Eq I64 x x))");
        assert_eq!(
            entry.statement_digest.as_deref(),
            Some(intrinsic::sha256(statement_store(statement).as_bytes()).as_str())
        );
        assert_eq!(entry.statement_source, "source-span");
        assert_eq!(entry.kind, "pure");
    }

    fn statement_store(statement: &str) -> std::string::String {
        std::string::String::from(statement)
    }

    #[test]
    fn goal_view_lists_the_arguments_the_elaborator_inserted() {
        let source: &[u8] = b"module logic@1 [ proof 1 poly-refl : [ (Pi (A Type0) (Pi (x A) (Eq A x x))) ] [ (intro (A Type0) (intro (x A) (refl x))) ] proof 1 refl-type infer : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (apply (use poly-refl) (infer)) (infer))) ] ]";
        let limits = Limits::default();
        let retained = noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("retained source budget");
        let session = ModuleSession::new(&[], retained).expect("session");
        let prepared = session.prepare(source, &[], limits).expect("prepares");
        let digest = intrinsic::sha256(source);
        let view = module_report(&prepared, source, "logic.noble", &digest, Report::View).encode();
        let parsed: serde_json::Value = serde_json::from_str(&view).expect("goal view JSON");
        let sites = parsed["sites"].as_array().expect("goal view rows");
        let row = |declaration: &str| {
            sites
                .iter()
                .find(|site| site["declaration"] == declaration)
                .unwrap_or_else(|| panic!("no row for {declaration}: {view}"))
        };
        assert_eq!(
            row("poly-refl")["inserted"],
            serde_json::json!([]),
            "an explicit proof has an empty inserted-arguments row"
        );
        let inserted = row("refl-type")["inserted"]
            .as_array()
            .expect("inserted-arguments row");
        let pairs: std::vec::Vec<(&str, &str)> = inserted
            .iter()
            .map(|argument| {
                (
                    argument["binder"].as_str().unwrap_or_default(),
                    argument["solution"].as_str().unwrap_or_default(),
                )
            })
            .collect();
        assert_eq!(pairs, [("A", "I64"), ("x", "x")], "{view}");
        assert!(
            inserted
                .iter()
                .all(|argument| argument["revision"] == "implicit-args/v1"
                    && argument["head"] == "logic@1/poly-refl"),
            "every row names the elaborator revision and the exact head: {view}"
        );
        assert_eq!(parsed["acceptance"], false, "the view stays a projection");
    }

    #[test]
    fn published_lean_release_matches_the_usage_text() {
        assert!(
            crate::workflow::USAGE.contains(LEAN_RELEASE),
            "the usage text must state the pinned release"
        );
    }

    #[test]
    fn contract_detection_reads_the_declared_top_level_form() {
        assert!(contract_source(INCREMENT));
        assert!(contract_source(b"\n  (contract 1 c)"));
        assert!(!contract_source(MODULE));
        assert!(!contract_source(b""));
    }
}
