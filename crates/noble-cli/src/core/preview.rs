//! A bounded, nonpublishing old/new Core replacement preview.
//!
//! The preview compares the installed definition with one explicitly selected
//! replacement file, using the existing preparation and dependent-rebuild checks.
//! It never stages a worker binary, publishes, executes either body or consumes a
//! self-edit grant, and it retains exactly one comparison reference that a later
//! explicit installation must still match.

use std::path::{Path, PathBuf};

pub(crate) const SCHEMA: &str = "noble-live-preview/v1";

/// The single retained comparison reference. A later explicit installation of the
/// same selected source must match these exact bytes and this base generation, or
/// it is refused as stale. The reference is not publication authority.
#[derive(Clone, Debug)]
pub(crate) struct Retained {
    pub(crate) path: PathBuf,
    pub(crate) digest: String,
    pub(crate) base_generation: u64,
}

/// A prospective replacement candidate: what the preview saw, not an installed
/// definition.
pub(crate) struct Candidate<'a> {
    pub(crate) path: &'a Path,
    pub(crate) digest: String,
    pub(crate) bytes: usize,
    pub(crate) definition: &'a str,
    pub(crate) prospective_identity: u64,
    pub(crate) interface: noble_contracts::source::DefinitionInterface,
}

/// Everything the report says, gathered where it was actually observed.
pub(crate) struct Observed<'a> {
    pub(crate) generation: u64,
    pub(crate) submissions: u64,
    pub(crate) installed_definition: Option<&'a str>,
    pub(crate) installed_identity: Option<u64>,
    pub(crate) dependents: &'a [String],
    pub(crate) discarded_previous: bool,
}

const NOT_COMPARED: &str = "behaviour is not compared by a preview, and passed structural checks \
                             establish neither equivalence nor termination";

/// A successful preview report. Every claim names its own status and subject: no
/// evidence badge, theorem, acceptance receipt or conformance claim is created or
/// copied from the installed subject.
pub(crate) fn previewed(observed: &Observed<'_>, candidate: &Candidate<'_>) -> serde_json::Value {
    serde_json::json!({
        "schema": SCHEMA,
        "stage": "core-preview",
        "outcome": "previewed",
        "base": {
            "generation": observed.generation,
            "submissions": observed.submissions,
            "path": candidate.path.display().to_string(),
            "installed_definition": observed.installed_definition,
            "installed_identity": observed.installed_identity,
            "installed_identity_scope": "unavailable when the session grant names no definition",
        },
        "candidate": {
            "path": candidate.path.display().to_string(),
            "sha256": candidate.digest,
            "bytes": candidate.bytes,
            "definition": candidate.definition,
            "prospective_identity": candidate.prospective_identity,
            "identity_scope": "prospective-not-installed",
        },
        "installed": { "identity_scope": "installed-at-base-generation" },
        "comparison": {
            "interface": "unavailable-not-exposed-by-this-preview",
            "interface_reason": "the preview checks replacement compatibility and exposes neither definition's ordered interface",
            "behaviour": "not-compared",
            "equivalence": "not-established",
            "reason": NOT_COMPARED,
        },
        "dependents": {
            "rebuild": observed.dependents,
            "scope": "current named dependents whose resolved calls reach the replaced definition",
        },
        "checks": {
            "source_preparation": "passed",
            "dependent_rebuild": "passed",
            "inert_submission": {
                "expectation": interface(&candidate.interface),
                "scope": "the empty-body submission this preview prepared; it is not either definition's interface",
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
            "sha256": candidate.digest,
            "base_generation": observed.generation,
            "authority": "comparison-reference-not-publication-authority",
        },
        "discarded_previous": observed.discarded_previous,
        "installed_state": {
            "staged_binaries": 0,
            "grants_consumed": 0,
            "guest_requests": 0,
            "protected_operations": 0,
            "namespace_changed": false,
        },
    })
}

/// A preview refusal keeps its own stage and reason: an incompatible interface or
/// effect growth, a dependency/rebind refusal and exhausted work stay distinct.
pub(crate) fn refused(
    generation: u64,
    path: &Path,
    stage: &str,
    reason: &str,
) -> serde_json::Value {
    serde_json::json!({
        "schema": SCHEMA,
        "stage": "core-preview",
        "outcome": "refused",
        "base": { "generation": generation, "path": path.display().to_string() },
        "refusal": { "stage": stage, "reason": reason },
        "installed_state": {
            "staged_binaries": 0,
            "grants_consumed": 0,
            "guest_requests": 0,
            "protected_operations": 0,
            "namespace_changed": false,
        },
    })
}

/// A discarded preview changes no installed state and establishes nothing.
pub(crate) fn cancelled(generation: u64, discarded: bool) -> serde_json::Value {
    serde_json::json!({
        "schema": SCHEMA,
        "stage": "core-preview",
        "outcome": if discarded { "cancelled" } else { "nothing-retained" },
        "base": { "generation": generation },
        "installed_state": {
            "staged_binaries": 0,
            "grants_consumed": 0,
            "guest_requests": 0,
            "protected_operations": 0,
            "namespace_changed": false,
            "selected_source_acknowledgement": "unchanged",
        },
    })
}

fn interface(described: &noble_contracts::source::DefinitionInterface) -> serde_json::Value {
    serde_json::json!({
        "stack_in": described.stack_in,
        "stack_out": described.stack_out,
        "effects": described.effects,
    })
}
