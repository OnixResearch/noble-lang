//! Selected declared-module snapshots and trusted live policy publication.
//! The compiler and worker remain separate owners: no module source is treated
//! as an executable candidate, and old registered roots are never guessed dead.

use super::{encoding, fatal_failure, report_diagnostic, same_snapshot, Live, ReloadDecision, Snapshot};

const MAX_ROOTS: usize = 64;
const MAX_SAFE_GENERATION: u64 = 9_007_199_254_740_991;

#[derive(Clone)]
struct Root {
    identity: u64,
    module: String,
    version: u32,
    sha256: String,
}

#[derive(Clone)]
struct Alias {
    alias: String,
    identity: u64,
    slot: Option<u32>,
}

pub(super) struct State {
    pub(super) manifest: super::super::arguments::bindings::Manifest,
    roots: Vec<Root>,
    aliases: Vec<Alias>,
    selected_module_name: Option<String>,
}

impl State {
    pub(super) fn new(manifest: super::super::arguments::bindings::Manifest) -> Self {
        Self { manifest, roots: Vec::new(), aliases: Vec::new(), selected_module_name: None }
    }

    fn proposed(
        &self,
        prepared: &noble_contracts::source::ModulePrepared,
        source: &[u8],
    ) -> Result<(Vec<Root>, Vec<Alias>), String> {
        use noble_contracts::source::ModuleKind;
        let mut roots = self.roots.clone();
        let mut aliases = self.aliases.clone();
        match prepared.kind() {
            ModuleKind::Module => {
                let (identity, module, version) = prepared.resolved_module()
                    .ok_or("checked module omitted immutable identity")?;
                let sha256 = crate::workflow::intrinsic::sha256(source);
                if roots.iter().any(|root| root.identity == identity
                    || root.module == module && root.version == version) {
                    return Err("immutable module version or owner was already registered".into());
                }
                if roots.len() >= MAX_ROOTS {
                    return Err("live declared module root budget exhausted".into());
                }
                roots.push(Root { identity, module, version, sha256 });
            }
            ModuleKind::Import => {
                let alias = prepared.resolved_import_alias()
                    .ok_or("checked import omitted its resolved alias")?;
                let (identity, module, version) = prepared.resolved_module()
                    .ok_or("checked import omitted immutable module owner")?;
                if !roots.iter().any(|root| root.identity == identity
                    && root.module == module && root.version == version) {
                    return Err("import owner has no checked immutable source snapshot".into());
                }
                let linked = Alias { alias: alias.to_owned(), identity,
                    slot: prepared.linked_binding().map(|binding| binding.adapter_slot) };
                if let Some(index) = aliases.iter().position(|entry| entry.alias == alias) {
                    if prepared.rebuilt_import_definitions().is_none()
                        || aliases[index].identity == identity {
                        return Err("declared alias rebind lacks checked dependent replay".into());
                    }
                    // Keep the host policy's spelling order stable even when
                    // the checked frontend moves the selected alias last.
                    // Existing roots and their old adapter slots remain
                    // resident; only current selection changes atomically.
                    aliases[index] = linked;
                } else {
                    if prepared.rebuilt_import_definitions().is_some() {
                        return Err("checked rebind referred to an unknown live alias".into());
                    }
                    if aliases.len() >= MAX_ROOTS {
                        return Err("live declared import alias budget exhausted".into());
                    }
                    aliases.push(linked);
                }
            }
            ModuleKind::Definition | ModuleKind::Expression => {}
        }
        Ok((roots, aliases))
    }
}

fn state_payload(
    expected_generation: u64,
    roots: &[Root],
    aliases: &[Alias],
) -> serde_json::Value {
    serde_json::json!({
        "expected_generation": expected_generation,
        "generation": expected_generation + 1,
        "roots": roots.iter().map(|root| serde_json::json!({
            "identity": root.identity, "module": &root.module, "version": root.version,
            "sha256": &root.sha256,
        })).collect::<Vec<_>>(),
        "aliases": aliases.iter().map(|entry| serde_json::json!({
            "alias": &entry.alias, "identity": entry.identity, "slot": entry.slot,
        })).collect::<Vec<_>>(),
    })
}

impl Live {
    pub(super) fn reload_declared_candidate(
        &mut self,
        selected: std::path::PathBuf,
        candidate: Snapshot,
        watch_sequence: Option<u64>,
    ) -> Result<ReloadDecision, super::super::output::Failure> {
        use noble_contracts::source::ModuleKind;
        let frontend = self.session.declared.as_ref()
            .ok_or_else(|| fatal_failure("declared live frontend is unavailable"))?;
        let prepared = match frontend.prepare(&candidate.bytes, &self.session.stack,
            super::super::SOURCE_LIMITS) {
            Ok(prepared) if prepared.kind() == ModuleKind::Module => prepared,
            Ok(_) => return Ok(ReloadDecision::Refused(
                "selected declared source must be one checked module declaration".into())),
            Err(error) => return Ok(ReloadDecision::SelectedSourceRefused(error)),
        };
        let (_, module, _) = prepared.resolved_module()
            .ok_or_else(|| fatal_failure("declared module omitted checked identity"))?;
        if self.declared.as_ref().and_then(|state| state.selected_module_name.as_deref())
            .is_some_and(|prior| prior != module.as_str()) {
            return Ok(ReloadDecision::Refused("selected module name changed".into()));
        }
        let number = self.session.submissions.checked_add(1)
            .ok_or_else(|| fatal_failure("live submission count exhausted"))?;
        let decision = self.publish_declared(prepared, &candidate.bytes,
            Some((&selected, &candidate)), number, watch_sequence)?;
        if matches!(decision, ReloadDecision::Committed(_)) {
            self.selected = Some(selected);
            self.selected_file_hash = Some(candidate.digest);
            self.selected_file_identity = Some((candidate.identity, candidate.directory));
            if let Some(state) = self.declared.as_mut() {
                state.selected_module_name = Some(module);
            }
        }
        Ok(decision)
    }

    fn publish_declared(
        &mut self,
        prepared: noble_contracts::source::ModulePrepared,
        source: &[u8],
        file: Option<(&std::path::Path, &Snapshot)>,
        number: u64,
        watch_sequence: Option<u64>,
    ) -> Result<ReloadDecision, super::super::output::Failure> {
        use noble_contracts::source::ModuleKind;
        let kind = prepared.kind();
        if kind == ModuleKind::Expression {
            return Err(fatal_failure("expression cannot publish a declared namespace root"));
        }
        let rebuilt_import_definitions = prepared.rebuilt_import_definitions().map(|definitions|
            definitions.iter().map(|definition| serde_json::json!({
                "name": definition.name(),
                "previous_identity": definition.previous_identity(),
                "new_identity": definition.new_identity(),
            })).collect::<Vec<_>>());
        if let Some(problem) = self.retained_generation_budget_refusal() {
            return Ok(ReloadDecision::Refused(problem));
        }
        let proposed = self.declared.as_ref()
            .ok_or_else(|| fatal_failure("declared live state is unavailable"))?
            .proposed(&prepared, source);
        let (roots, aliases) = match proposed {
            Ok(proposed) => proposed,
            Err(problem) => return Ok(ReloadDecision::Refused(problem)),
        };
        if self.generation >= MAX_SAFE_GENERATION {
            return Ok(ReloadDecision::Refused("live generation budget exhausted".into()));
        }
        if file.is_some_and(|(path, candidate)| !same_snapshot(path, candidate)) {
            return Ok(ReloadDecision::Refused(
                "selected module source changed before declaration staging".into()));
        }
        let proposal = state_payload(self.generation, &roots, &aliases);
        let staged = self.worker()?.stage_live_declared(&proposal)?;
        if staged.outcome != "declared-state-staged" {
            return Ok(ReloadDecision::Refused(report_diagnostic(&staged)));
        }
        if let Some((path, candidate)) = file {
            if !self.candidate_is_current(path, candidate, watch_sequence)? {
                let discarded = self.worker()?.discard_live_declared()?;
                if discarded.outcome != "declared-state-discarded" {
                    return Err(fatal_failure("worker failed to discard changed module snapshot"));
                }
                return Ok(ReloadDecision::Refused(
                    "selected module source changed during declaration staging".into()));
            }
        } else if self.verify_source_before_execute {
            if let Some(report) = self.source_change_refusal() {
                let discarded = self.worker()?.discard_live_declared()?;
                if discarded.outcome != "declared-state-discarded" {
                    return Err(fatal_failure("worker failed to discard stale declaration"));
                }
                return Ok(ReloadDecision::SourceChanged(report));
            }
        }
        let (checked, proof_recheck) = if prepared.proof_obligations().is_some() {
            let (checked, proof) = self.session.commit_declared_proofs(
                prepared, source, super::super::SOURCE_LIMITS)?;
            let report = serde_json::from_str::<serde_json::Value>(&proof.encode())
                .map_err(|_| fatal_failure("checked module proof report is not JSON"))?;
            (checked, Some(report))
        } else {
            (self.session.commit_declared(prepared)?, None)
        };
        if let Err(error) = checked {
            let discarded = self.worker()?.discard_live_declared()?;
            if discarded.outcome != "declared-state-discarded" {
                return Err(fatal_failure("worker failed to discard rejected declaration"));
            }
            return Ok(ReloadDecision::SelectedSourceRefused(error));
        }
        // A publication failure after frontend commitment is terminal; it is
        // never misreported as a recoverable ACK for the old policy.
        let published = self.worker()?.publish_live_declared()?;
        if published.outcome != "declared-state-published" {
            return Err(fatal_failure("worker failed to publish checked declared policy"));
        }
        let state = self.declared.as_mut()
            .ok_or_else(|| fatal_failure("declared state vanished after publication"))?;
        state.roots = roots;
        state.aliases = aliases;
        self.generation += 1;
        self.session.submissions = number;
        let source_freshness = match file {
            Some((path, candidate)) if same_snapshot(path, candidate) =>
                "matched-at-postpublish-check",
            Some(_) => "changed-or-unverifiable-at-postpublish-check",
            None => "not-file-backed",
        };
        Ok(ReloadDecision::Committed(serde_json::json!({
            "schema": "noble-live-report/v1", "stage": "wasm-live",
            "outcome": "reload-committed", "generation": self.generation,
            "watch_sequence": watch_sequence,
            "source_sha256": crate::workflow::intrinsic::sha256(source),
            "source_version": if file.is_some() { "pinned-at-prepublish-check" }
                else { "checked-submission" },
            "source_freshness": source_freshness,
            "declared_kind": match kind {
                ModuleKind::Module => "module", ModuleKind::Import => "import",
                ModuleKind::Definition => "definition", ModuleKind::Expression => "expression",
            },
            "rebuilt_import_definitions": rebuilt_import_definitions,
            "proof_recheck": proof_recheck,
            "retained_module_roots": state.roots.len(),
            "code_retirement": "deferred-until-backend-owner-accounting",
            "stack": self.last_stack,
            "guest_requests": 0, "protected_operations": 0,
        })))
    }

    pub(super) fn submit_declared(
        &mut self,
        source: &[u8],
    ) -> Result<super::super::output::Report, super::super::output::Failure> {
        use noble_contracts::source::ModuleKind;
        self.session.submissions = self.session.submissions.checked_add(1)
            .ok_or_else(|| fatal_failure("live submission count exhausted"))?;
        let number = self.session.submissions;
        let frontend = self.session.declared.as_ref()
            .ok_or_else(|| fatal_failure("declared live frontend is unavailable"))?;
        let prepared = match frontend.prepare(source, &self.session.stack,
            super::super::SOURCE_LIMITS) {
            Ok(prepared) => prepared,
            Err(error) => return Ok(super::super::output::Report::declared_source_error(
                &error, number, false, false)),
        };
        if self.verify_source_before_execute {
            if let Some(refusal) = self.source_change_refusal() {
                return Ok(refusal);
            }
        }
        if prepared.kind() != ModuleKind::Expression {
            return match self.publish_declared(prepared, source, None, number, None)? {
                ReloadDecision::Committed(mut receipt) => {
                    if let Some(fields) = receipt.as_object_mut() {
                        fields.insert("stage".into(), serde_json::json!("declared"));
                        fields.insert("outcome".into(), serde_json::json!("namespace-committed"));
                    }
                    Ok(super::super::output::Report {
                        outcome: "namespace-committed".into(), json: receipt.to_string(),
                    })
                }
                ReloadDecision::Refused(problem) =>
                    Ok(self.admission_refusal(&problem, "declared-refused")),
                ReloadDecision::HostEffect(problem) =>
                    Ok(self.admission_refusal(&problem, "effect-refused")),
                ReloadDecision::SourceChanged(report) => Ok(report),
                ReloadDecision::SelectedSourceRefused(error) =>
                    Ok(super::super::output::Report::declared_source_error(
                        &error, number, false, false)),
            };
        }
        self.execute_declared(prepared, source, number)
    }

    fn execute_declared(
        &mut self,
        prepared: noble_contracts::source::ModulePrepared,
        source: &[u8],
        number: u64,
    ) -> Result<super::super::output::Report, super::super::output::Failure> {
        let submission = prepared.submission()
            .ok_or_else(|| fatal_failure("declared expression omitted checked submission"))?;
        let compiled = match self.session.compiler.prepare(submission) {
            Ok(compiled) => compiled,
            Err(error) => return Ok(super::super::output::Report::declared_backend_error(
                error, number, false, false)),
        };
        let binary = match encoding::encode_declared_checked(compiled.wat()) {
            Ok(binary) => binary,
            Err(encoding::EncodeError::HostEffect(problem)) =>
                return Ok(self.admission_refusal(&problem, "effect-refused")),
            Err(encoding::EncodeError::Other(problem)) =>
                return Ok(self.admission_refusal(&problem, "bytecode-refused")),
        };
        let ready = self.worker()?.prepare_binary(&binary, source, number)?;
        if ready.outcome != "ready" {
            return Ok(ready);
        }
        if self.verify_source_before_execute {
            if let Some(refusal) = self.source_change_refusal() {
                let discarded = self.worker()?.discard_prepared_live()?;
                if discarded.outcome != "discarded" {
                    return Err(fatal_failure("worker failed to discard stale declared expression"));
                }
                return Ok(refusal);
            }
        }
        let output = prepared.output().to_vec();
        self.session.compiler.commit(compiled)
            .map_err(super::super::output::Failure::backend)?;
        let committed = self.session.commit_declared(prepared)?;
        committed.map_err(super::super::output::Failure::source)?;
        let mut report = self.worker()?.execute()?;
        if report.outcome == "normal" {
            self.session.stack = output;
        }
        let mut value: serde_json::Value = serde_json::from_str(&report.json)
            .map_err(|_| fatal_failure("invalid declared live execution report"))?;
        if let Some(fields) = value.as_object_mut() {
            fields.insert("source_generation".into(), serde_json::json!(self.generation));
        }
        report.json = value.to_string();
        Ok(report)
    }
}
