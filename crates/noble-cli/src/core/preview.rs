//! A bounded, nonpublishing old/new Core replacement preview.
//!
//! The preview compares the installed definition with one explicitly selected
//! replacement file, using the existing preparation and dependent-rebuild checks.
//! It never stages a worker binary, publishes, executes either body or consumes a
//! self-edit grant, and it retains exactly one comparison reference that a later
//! explicit installation must still match.
//!
//! Every report measures the installed session state before and after the
//! operation and derives its counters from those two observations, so "nothing
//! was installed" is a measured claim rather than a constant. Both ordered
//! inferred interfaces are printed side by side, each under its own identity:
//! the installed definition under its installed identity and the candidate under
//! a prospective identity that is not installed and not authority.

use noble_contracts::source::{DefinitionInterface, OrderedInterface};
use std::path::{Path, PathBuf};

pub(crate) const SCHEMA: &str = "noble-live-preview/v1";

/// The single retained comparison reference. A later explicit installation of the
/// same selected source must match these exact bytes and this base generation, or
/// it is refused as stale. The reference is not publication authority.
///
/// A refused installation marks the reference stale instead of discarding it, so
/// every later plain reload is refused until a new preview replaces it or
/// `:preview cancel` discards it explicitly.
#[derive(Clone, Debug)]
pub(crate) struct Retained {
    pub(crate) path: PathBuf,
    pub(crate) digest: String,
    pub(crate) base_generation: u64,
    pub(crate) stale: Option<String>,
}

/// Which contract refused a preview. The class never substitutes for the
/// diagnostic; the diagnostic is always carried unchanged beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RefusalKind {
    /// The selected path is missing, relative, or differs from the selected source.
    Selection,
    /// The selected file could not be read as one bounded snapshot.
    Snapshot,
    /// The candidate could not be prepared as one checked definition.
    Preparation,
    /// The session profile or candidate is not one replaceable checked definition.
    UnsupportedProfile,
    /// The replacement's ordered stack interface differs.
    InterfaceChange,
    /// The replacement admits an effect the installed definition does not.
    EffectGrowth,
    /// The replacement transitively depends on its own previous identity.
    SelfRebind,
    /// A retained dependent no longer rebuilds to its own name, effects or targets.
    DependentRebuild,
    /// A dependency walk, comparison or dependent replay exhausted its budget.
    WorkExhausted,
    /// A retained dependent's own retained source is missing or malformed.
    RetainedSource,
    /// A check returned a diagnostic this preview cannot classify.
    ComparisonFailed,
}

impl RefusalKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Selection => "preview-selection",
            Self::Snapshot => "source-snapshot",
            Self::Preparation => "source-preparation",
            Self::UnsupportedProfile => "unsupported-profile",
            Self::InterfaceChange => "interface-change",
            Self::EffectGrowth => "effect-growth",
            Self::SelfRebind => "self-rebind",
            Self::DependentRebuild => "dependent-rebuild",
            Self::WorkExhausted => "work-exhausted",
            Self::RetainedSource => "retained-source",
            Self::ComparisonFailed => "comparison-failed",
        }
    }
}

/// The typed class of a preview refusal from the contracts layer.
pub(crate) const fn kind_of(refusal: noble_contracts::source::PreviewRefusal) -> RefusalKind {
    use noble_contracts::source::PreviewRefusal;
    match refusal {
        PreviewRefusal::UnsupportedProfile => RefusalKind::UnsupportedProfile,
        PreviewRefusal::InterfaceChanged => RefusalKind::InterfaceChange,
        PreviewRefusal::EffectGrowth => RefusalKind::EffectGrowth,
        PreviewRefusal::SelfRebind => RefusalKind::SelfRebind,
        PreviewRefusal::DependentRebuild => RefusalKind::DependentRebuild,
        PreviewRefusal::WorkExhausted => RefusalKind::WorkExhausted,
        PreviewRefusal::RetainedSource => RefusalKind::RetainedSource,
        PreviewRefusal::ComparisonFailed => RefusalKind::ComparisonFailed,
    }
}

/// One refusal with the provenance of the diagnostic that decided it.
pub(crate) struct Refusal {
    pub(crate) kind: RefusalKind,
    /// The source stage the interrupted check stopped in, when one did. This is
    /// provenance beside the class, never a substitute for it.
    pub(crate) stage: Option<&'static str>,
    /// The diagnostic's own machine-facing category.
    pub(crate) category: Option<&'static str>,
    /// The unchanged diagnostic message.
    pub(crate) reason: String,
}

impl Refusal {
    /// A refusal of the local selection or snapshot: no source-stage provenance.
    pub(crate) fn local(kind: RefusalKind, reason: String) -> Self {
        Self {
            kind,
            stage: None,
            category: None,
            reason,
        }
    }

    /// A refusal decided on one `noble-contracts` source diagnostic. An
    /// exhausted budget is the same class wherever it was met.
    pub(crate) fn from_error(class: RefusalKind, error: &noble_contracts::source::Error) -> Self {
        let kind = if error.diagnostic().kind == noble_contracts::DiagnosticKind::Exhausted {
            RefusalKind::WorkExhausted
        } else {
            class
        };
        Self {
            kind,
            stage: Some(stage_spelling(error.stage())),
            category: Some(error.diagnostic().kind.category()),
            reason: error.diagnostic().message.clone(),
        }
    }

    /// A refusal decided on a typed preview refusal, keeping its own class and
    /// the unchanged diagnostic beside it.
    pub(crate) fn from_preview(error: &noble_contracts::source::PreviewError) -> Self {
        Self {
            kind: kind_of(error.kind()),
            stage: Some(stage_spelling(error.stage())),
            category: Some(error.diagnostic().kind.category()),
            reason: error.diagnostic().message.clone(),
        }
    }
}

/// Stable spelling of a source checking stage. It is reported as provenance
/// beside a refusal class and is never the class itself.
pub(crate) const fn stage_spelling(stage: noble_contracts::source::Stage) -> &'static str {
    use noble_contracts::source::Stage;
    match stage {
        Stage::Parse => "parse",
        Stage::Resolve => "resolve",
        Stage::Link => "link",
        Stage::Check => "check",
        Stage::Acceptance => "acceptance",
    }
}

/// One measured point of installed session state.
#[derive(Clone, Debug)]
pub(crate) struct Fingerprint {
    pub(crate) generation: u64,
    pub(crate) submissions: u64,
    pub(crate) grant: Option<String>,
    pub(crate) live_code_entries: usize,
    /// The selected-file acknowledgement the session currently holds.
    pub(crate) acknowledged_digest: Option<String>,
}

/// Everything a successful preview observed, gathered where it was observed.
pub(crate) struct Comparison {
    pub(crate) path: PathBuf,
    pub(crate) digest: String,
    pub(crate) bytes: usize,
    pub(crate) definition: String,
    pub(crate) prospective_identity: u64,
    pub(crate) candidate_interface: OrderedInterface,
    pub(crate) installed_definition: Option<String>,
    pub(crate) installed_identity: Option<u64>,
    pub(crate) installed_interface: Option<OrderedInterface>,
    pub(crate) installed_interface_reason: Option<String>,
    /// The empty-body submission this preview prepared against the successor.
    pub(crate) inert: DefinitionInterface,
    pub(crate) dependents: Vec<String>,
    pub(crate) discarded_previous: bool,
}

const NOT_COMPARED: &str = "behaviour is not compared by a preview, and passed structural checks \
                             establish neither equivalence nor termination";

const EQUALITY_SCOPE: &str = "the ordered inferred input/output types and admitted effects only; \
                              equal descriptions are not behavioural equivalence and not acceptance";

/// A successful preview report. Every claim names its own status and subject: no
/// evidence badge, theorem, acceptance receipt or conformance claim is created or
/// copied from the installed subject.
pub(crate) fn previewed(
    compared: &Comparison,
    before: &Fingerprint,
    after: &Fingerprint,
) -> serde_json::Value {
    let interface_equal = compared
        .installed_interface
        .as_ref()
        .map(|installed| installed == &compared.candidate_interface);
    serde_json::json!({
        "schema": SCHEMA,
        "stage": "core-preview",
        "outcome": "previewed",
        "base": {
            "generation": before.generation,
            "submissions": before.submissions,
            "path": compared.path.display().to_string(),
            "installed_definition": compared.installed_definition,
            "installed_identity": compared.installed_identity,
            "installed_identity_scope": "session-local identity from the installed namespace at the base generation; not portable or authority",
        },
        "candidate": {
            "path": compared.path.display().to_string(),
            "sha256": compared.digest,
            "bytes": compared.bytes,
            "definition": compared.definition,
            "prospective_identity": compared.prospective_identity,
            "identity_scope": "prospective-not-installed",
        },
        "comparison": {
            "installed": interface_subject(
                &compared.installed_interface,
                compared.installed_interface_reason.as_deref(),
                compared.installed_identity,
                "installed-at-base-generation",
                "installed",
            ),
            "candidate": {
                "subject": "prospective",
                "prospective_identity": compared.prospective_identity,
                "identity_scope": "prospective-not-installed",
                "interface": interface(&compared.candidate_interface),
            },
            "interface_equal": interface_equal,
            "equality_scope": EQUALITY_SCOPE,
            "behaviour": "not-compared",
            "equivalence": "not-established",
            "reason": NOT_COMPARED,
        },
        "dependents": {
            "rebuild": compared.dependents,
            "scope": "current named dependents whose resolved calls reach the replaced definition",
        },
        "checks": {
            "source_preparation": "passed",
            "dependent_rebuild": "passed",
            "inert_submission": {
                "expectation": inert_expectation(&compared.inert),
                "scope": "the empty-body submission this preview prepared against the current session stack; it describes that stack, not either definition's interface",
            },
            "backend_admission": "not-run",
            "publication": "not-run",
            "evidence": {
                "status": "not-checked",
                "subject": serde_json::Value::Null,
                "reason": "a preview binds no evidence to any subject; it copies no badge from the installed definition",
            },
        },
        "token": {
            "sha256": compared.digest,
            "base_generation": before.generation,
            "authority": "comparison-reference-not-publication-authority",
        },
        "discarded_previous": compared.discarded_previous,
        "installed_state": installed_state(before, after),
    })
}

/// A preview refusal keeps its own class, stage and unchanged reason. The
/// installed-state block is measured exactly as for a successful preview.
pub(crate) fn refused(
    generation: u64,
    path: &Path,
    refusal: &Refusal,
    before: &Fingerprint,
    after: &Fingerprint,
) -> serde_json::Value {
    serde_json::json!({
        "schema": SCHEMA,
        "stage": "core-preview",
        "outcome": "refused",
        "base": { "generation": generation, "path": path.display().to_string() },
        "refusal": {
            "kind": refusal.kind.as_str(),
            "stage": refusal.stage,
            "category": refusal.category,
            "reason": refusal.reason,
        },
        "installed_state": installed_state(before, after),
    })
}

/// A discarded preview changes no installed state and establishes nothing.
pub(crate) fn cancelled(
    generation: u64,
    discarded: bool,
    before: &Fingerprint,
    after: &Fingerprint,
) -> serde_json::Value {
    serde_json::json!({
        "schema": SCHEMA,
        "stage": "core-preview",
        "outcome": if discarded { "cancelled" } else { "nothing-retained" },
        "base": { "generation": generation },
        "installed_state": installed_state(before, after),
    })
}

/// The installed state block: every counter is derived from the two measured
/// points, and the two fields this path structurally cannot reach say so.
fn installed_state(before: &Fingerprint, after: &Fingerprint) -> serde_json::Value {
    serde_json::json!({
        "generation": { "before": before.generation, "after": after.generation },
        "submissions": { "before": before.submissions, "after": after.submissions },
        "grant": { "before": before.grant, "after": after.grant },
        "live_code_entries": {
            "before": before.live_code_entries,
            "after": after.live_code_entries,
        },
        "namespace_changed": before.generation != after.generation,
        "staged_binaries": after.live_code_entries.saturating_sub(before.live_code_entries),
        "grants_consumed": u8::from(before.grant.is_some() && after.grant.is_none()),
        "submissions_charged": after.submissions.saturating_sub(before.submissions),
        "selected_source_acknowledgement": if before.acknowledged_digest == after.acknowledged_digest {
            "unchanged"
        } else {
            "changed"
        },
        "guest_requests": 0,
        "protected_operations": 0,
        "unreachable": {
            "fields": ["guest_requests", "protected_operations"],
            "reason": "this path holds no worker and issues no guest request; those two counters are structural constants here",
        },
        "measurement_scope": "before and after the operation, from the session's own generation, submission count, grant selection, live-code registry and selected-file acknowledgement",
    })
}

fn interface_subject(
    described: &Option<OrderedInterface>,
    reason: Option<&str>,
    identity: Option<u64>,
    identity_scope: &str,
    subject: &str,
) -> serde_json::Value {
    match described {
        Some(described) => serde_json::json!({
            "subject": subject,
            "installed_identity": identity,
            "identity_scope": identity_scope,
            "interface": interface(described),
        }),
        None => serde_json::json!({
            "subject": subject,
            "installed_identity": identity,
            "identity_scope": identity_scope,
            "interface": serde_json::Value::Null,
            "interface_status": "unavailable",
            "interface_reason": reason.unwrap_or("no installed definition is selected in this session"),
        }),
    }
}

/// One ordered inferred interface in the inspection format: both sides as
/// bracketed ordered spellings, each side's tail stated, and the admitted effects.
fn interface(described: &OrderedInterface) -> serde_json::Value {
    serde_json::json!({
        "stack_in": {
            "values": described.stack_in,
            "depth": described.stack_in.len(),
        },
        "stack_in_tail": described.stack_in_tail.as_str(),
        "stack_out": {
            "values": described.stack_out,
            "depth": described.stack_out.len(),
        },
        "stack_out_tail": described.stack_out_tail.as_str(),
        "effects": described.effects.iter().map(|id| effect(*id)).collect::<Vec<_>>(),
        "effects_scope": "session-declared host-effect identities, by reserved spelling where one exists",
        "effects_unresolved": described.unresolved_effects,
        "source": "ordered static inference over the retained source; not a runtime observation",
    })
}

/// The reserved effect identities, spelled as the runtime Program interface
/// already spells them; every other identity stays numeric.
fn effect(id: u32) -> String {
    let name = match id {
        0 => "test.emit",
        1 => "test.abort",
        2 => "test.clock",
        3 => "live.propose",
        4 => "live.observe-generation",
        5 => "live.dispatch",
        6 => "test.code-export",
        7 => "test.send",
        other => return std::format!("effect:{other}"),
    };
    name.to_owned()
}

fn inert_expectation(described: &DefinitionInterface) -> serde_json::Value {
    serde_json::json!({
        "stack_in": described.stack_in,
        "stack_out": described.stack_out,
        "effects": described.effects,
    })
}
