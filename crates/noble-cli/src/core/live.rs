//! Opt-in, resource-free Core live session. Existing run/session paths do not enter here.
//! A reload previews one checked definition, compiles an inert empty expression
//! from that preview, and publishes only after isolated binary staging and an
//! exact second file snapshot. The resident engine retains old Program slots.

mod encoding;

use std::os::unix::fs::MetadataExt;

const LIMIT: u32 = super::SOURCE_LIMITS.bytes;

struct Snapshot {
    bytes: std::vec::Vec<u8>,
    digest: std::string::String,
    identity: (u64, u64, u64, i64, i64, i64, i64),
}

struct GrantSelection {
    name: std::string::String,
    expected_generation: u64,
}

enum ReloadDecision {
    Committed(serde_json::Value),
    Refused(std::string::String),
    HostEffect(std::string::String),
}

struct Live {
    session: super::Session,
    selected: Option<std::path::PathBuf>,
    selected_definition: Option<std::string::String>,
    selected_file_hash: Option<std::string::String>,
    grant_scope: Option<std::string::String>,
    grant: Option<GrantSelection>,
    last_stack: serde_json::Value,
    invocation_prior_stack: Option<serde_json::Value>,
    // Source/namespace publication generation, not the Wasm submission count.
    generation: u64,
}

pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let (source, engine, initial_grant) = match parse(arguments) {
        Ok(options) => options,
        Err(message) => {
            emit(&refusal(0, &serde_json::json!([]), &message));
            return std::process::ExitCode::from(2);
        }
    };
    if engine != "v8" {
        emit(&serde_json::json!({
            "schema": "noble-live-report/v1",
            "stage": "engine-selection",
            "outcome": "interpreter-unavailable",
            "diagnostic": "a pinned compatible no-JIT Wasm interpreter is unavailable",
            "guest_requests": 0,
            "protected_operations": 0,
        }));
        return std::process::ExitCode::from(2);
    }
    let selected_grant_name = initial_grant.as_ref().map(|grant| grant.name.as_str());
    let mut live = Live {
        session: super::Session {
            frontend: match selected_grant_name {
                Some(name) => noble_contracts::source::Session::new_live(name),
                None => noble_contracts::source::Session::new(),
            },
            declared: None,
            compiler: match selected_grant_name {
                Some(_) => noble_wasm::source::Compiler::new_live(),
                None => noble_wasm::source::Compiler::new(),
            },
            stack: std::vec::Vec::new(),
            worker: None,
            submissions: 0,
        },
        selected: None,
        selected_definition: None,
        selected_file_hash: None,
        grant_scope: selected_grant_name.map(str::to_owned),
        grant: initial_grant.as_ref().map(|grant| GrantSelection {
            name: grant.name.clone(),
            expected_generation: grant.expected_generation,
        }),
        last_stack: serde_json::json!([]),
        invocation_prior_stack: None,
        generation: 0,
    };
    if let Some(source) = source {
        if !live.respond_reload(&source) {
            return std::process::ExitCode::from(2);
        }
        if live.selected.is_none() {
            return std::process::ExitCode::from(2);
        }
    }
    if let Some(grant) = initial_grant {
        let report = match live.select_grant(grant) {
            Ok(report) => report,
            Err(error) => {
                emit(&fatal(&error.message));
                return std::process::ExitCode::from(2);
            }
        };
        emit(&report);
        if report["outcome"] != "grant-selected" {
            return std::process::ExitCode::from(2);
        }
    }
    let mut input = std::io::BufReader::new(std::io::stdin().lock());
    loop {
        let bytes = match super::framing::read_line(&mut input, LIMIT) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return std::process::ExitCode::SUCCESS,
            Err(error) => {
                emit(&fatal(&error.message));
                return std::process::ExitCode::from(2);
            }
        };
        // One complete physical line is one request; malformed UTF-8 cannot
        // become an alternate control protocol or a partial source frame.
        let line = match std::str::from_utf8(&bytes) {
            Ok(line) => line.trim_end_matches(['\r', '\n']),
            Err(_) => {
                emit(&refusal(
                    live.generation,
                    &live.last_stack,
                    "live input must be UTF-8",
                ));
                continue;
            }
        };
        if let Some(path) = line.strip_prefix(":reload ") {
            if !live.respond_reload(std::path::Path::new(path)) {
                return std::process::ExitCode::from(2);
            }
            continue;
        }
        if let Some(terms) = line.strip_prefix(":grant-self-edit ") {
            let mut words = terms.split_ascii_whitespace();
            let report = match (words.next(), words.next(), words.next()) {
                (Some(name), Some(generation), None) => generation
                    .parse::<u64>()
                    .map(|expected_generation| {
                        live.select_grant(GrantSelection {
                            name: name.to_owned(),
                            expected_generation,
                        })
                    })
                    .unwrap_or_else(|_| {
                        Ok(refusal(
                            live.generation,
                            &live.last_stack,
                            "invalid expected source generation",
                        ))
                    }),
                _ => Ok(refusal(
                    live.generation,
                    &live.last_stack,
                    "expected name and source generation",
                )),
            };
            match report {
                Ok(report) => emit(&report),
                Err(error) => {
                    emit(&fatal(&error.message));
                    return std::process::ExitCode::from(2);
                }
            }
            continue;
        }
        if line.starts_with(':') {
            emit(&refusal(
                live.generation,
                &live.last_stack,
                "unsupported live control command",
            ));
            continue;
        }
        match live.submit(line.as_bytes()) {
            Ok(report) => {
                if let Some(stack) = report_stack(&report) {
                    live.last_stack = stack;
                }
                if super::output::print(&report).is_err() {
                    return std::process::ExitCode::from(2);
                }
                if report.is_terminal() {
                    return std::process::ExitCode::from(report.exit().max(2));
                }
                if let Err(error) = live.finish_proposal(&report) {
                    if let Some(record) =
                        terminal_proposal_failure(&error, &live.last_stack, live.generation)
                    {
                        emit(&record);
                    } else {
                        emit(&fatal(&error.message));
                    }
                    return std::process::ExitCode::from(2);
                }
            }
            Err(error) => {
                emit(&fatal(&error.message));
                return std::process::ExitCode::from(2);
            }
        }
    }
}

fn parse(
    arguments: &[std::ffi::OsString],
) -> Result<
    (
        Option<std::path::PathBuf>,
        &'static str,
        Option<GrantSelection>,
    ),
    std::string::String,
> {
    if arguments.get(1).is_none_or(|arg| arg != "repl") {
        return Err("usage: noble live repl [--source ABS_PATH] [--engine v8|interpreter]".into());
    }
    let mut source = None;
    let mut engine = None;
    let mut grant_name = None;
    let mut grant_generation = None;
    let mut at = 2;
    while at < arguments.len() {
        let flag = arguments[at].to_str().ok_or("invalid live option")?;
        let value = arguments
            .get(at + 1)
            .and_then(|value| value.to_str())
            .ok_or("missing live option value")?;
        match flag {
            "--source" if source.is_none() => source = Some(std::path::PathBuf::from(value)),
            "--engine" if engine.is_none() => {
                engine = Some(match value {
                    "v8" => "v8",
                    "interpreter" => "interpreter",
                    _ => return Err("unsupported live engine".into()),
                })
            }
            "--self-edit" if grant_name.is_none() => grant_name = Some(value.to_owned()),
            "--expect-generation" if grant_generation.is_none() => {
                grant_generation = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| "invalid expected source generation")?,
                )
            }
            _ => return Err("unsupported or duplicate live option".into()),
        }
        at += 2;
    }
    let grant = match (grant_name, grant_generation) {
        (Some(name), Some(expected_generation)) if source.is_some() && !name.is_empty() => {
            Some(GrantSelection {
                name,
                expected_generation,
            })
        }
        (None, None) => None,
        _ => {
            return Err(
                "self-edit requires --source, --self-edit NAME, and --expect-generation N".into(),
            );
        }
    };
    Ok((source, engine.unwrap_or("v8"), grant))
}

impl Live {
    fn finish_proposal(
        &mut self,
        executed: &super::output::Report,
    ) -> Result<(), super::output::Failure> {
        let invocation: serde_json::Value = serde_json::from_str(&executed.json)
            .map_err(|_| fatal_failure("invalid guest execution report"))?;
        if executed.outcome != "normal" || invocation["proposal_queued"] != true {
            return Ok(());
        }
        let pending = self.worker()?.take_live_proposal()?;
        if pending.outcome == "proposal-refused" {
            let inspected: serde_json::Value = serde_json::from_str(&pending.json)
                .map_err(|_| fatal_failure("invalid proposal inspection refusal"))?;
            if inspected["session_state"]
                .as_str()
                .is_some_and(|state| state.starts_with("terminated"))
            {
                return Err(fatal_failure(
                    "proposal inspection poisoned the live session",
                ));
            }
            let problem = report_diagnostic(&pending);
            emit(&self.proposal_receipt(
                "proposal-pending",
                &invocation,
                None,
                Some(&inspected),
                None,
            ));
            emit(&self.proposal_receipt(
                "proposal-refused",
                &invocation,
                None,
                Some(&inspected),
                Some(&problem),
            ));
            self.invocation_prior_stack = None;
            return Ok(());
        }
        if pending.outcome != "proposal-pending" {
            return Err(fatal_failure(
                "guest proposal was not retained for post-return admission",
            ));
        }
        let proposal: serde_json::Value = serde_json::from_str(&pending.json)
            .map_err(|_| fatal_failure("invalid bounded proposal report"))?;
        let events = proposal
            .get("events")
            .ok_or_else(|| fatal_failure("guest proposal omitted recipe events"))?;
        let recipe_bytes = serde_json::to_vec(events)
            .map_err(|_| fatal_failure("guest proposal recipe cannot be encoded"))?;
        let recipe_digest = crate::workflow::intrinsic::sha256(&recipe_bytes);
        emit(&self.proposal_receipt(
            "proposal-pending",
            &invocation,
            Some(&recipe_digest),
            Some(&proposal),
            None,
        ));
        let outcome = self.admit_proposal(&proposal);
        let final_receipt = match outcome {
            Ok(ReloadDecision::Committed(mut published)) => {
                if let Some(fields) = published.as_object_mut() {
                    fields.insert("outcome".into(), serde_json::json!("proposal-committed"));
                    fields.insert("origin".into(), serde_json::json!("guest-program-recipe"));
                    fields.insert(
                        "observation_scope".into(),
                        serde_json::json!("bounded-live-tool-observation"),
                    );
                    fields.insert(
                        "source_version".into(),
                        serde_json::json!("guest-derived-checked-recipe"),
                    );
                    fields.insert(
                        "proposal_recipe_sha256".into(),
                        serde_json::json!(recipe_digest),
                    );
                    fields.insert("candidate_recipe_events".into(), proposal["events"].clone());
                    fields.insert(
                        "candidate_snapshot_sha256".into(),
                        proposal["candidate_snapshot_sha256"].clone(),
                    );
                    fields.insert(
                        "postreturn_snapshot_verified".into(),
                        proposal["postreturn_snapshot_verified"].clone(),
                    );
                    fields.insert("selected_name".into(), proposal["name"].clone());
                    fields.insert("checked_owner_identity".into(), proposal["owner"].clone());
                    fields.insert(
                        "grant_expected_generation".into(),
                        proposal["expected_generation"].clone(),
                    );
                    fields.insert(
                        "source_generation".into(),
                        serde_json::json!(self.generation),
                    );
                    fields.insert(
                        "candidate_program_interface".into(),
                        serde_json::json!({
                            "input": proposal["input_signature"],
                            "output": proposal["output_signature"],
                            "effects": proposal["effects"],
                        }),
                    );
                    fields.insert(
                        "pre_invocation_stack".into(),
                        serde_json::json!(self.invocation_prior_stack),
                    );
                    fields.insert(
                        "invocation_source_sha256".into(),
                        invocation["module"]["source_sha256"].clone(),
                    );
                    fields.insert(
                        "invocation_wasm_sha256".into(),
                        invocation["module"]["wasm_sha256"].clone(),
                    );
                    fields.insert(
                        "selected_file_sha256".into(),
                        serde_json::json!(self.selected_file_digest()),
                    );
                    fields.insert("request_trace".into(), invocation["request_trace"].clone());
                    fields.insert(
                        "guest_requests".into(),
                        invocation["guest_requests"].clone(),
                    );
                    fields.insert(
                        "protected_operations".into(),
                        invocation["protected_operations"].clone(),
                    );
                }
                self.grant = None;
                published
            }
            Ok(ReloadDecision::Refused(problem) | ReloadDecision::HostEffect(problem)) => self
                .proposal_receipt(
                    "proposal-refused",
                    &invocation,
                    Some(&recipe_digest),
                    Some(&proposal),
                    Some(&problem),
                ),
            Err(error) => return Err(error),
        };
        emit(&final_receipt);
        self.invocation_prior_stack = None;
        Ok(())
    }

    fn admit_proposal(
        &mut self,
        proposal: &serde_json::Value,
    ) -> Result<ReloadDecision, super::output::Failure> {
        let selected = match self.selected_definition.as_deref() {
            Some(name) => name,
            None => return Ok(ReloadDecision::Refused("no selected definition".into())),
        };
        let grant = match self.grant.as_ref() {
            Some(grant)
                if grant.name == selected && grant.expected_generation == self.generation =>
            {
                grant
            }
            _ => {
                return Ok(ReloadDecision::Refused(
                    "proposal has no current host-selected grant".into(),
                ));
            }
        };
        let identity = match self.session.frontend.selected_definition_identity(selected) {
            Some(identity) => identity,
            None => {
                return Ok(ReloadDecision::Refused(
                    "selected definition identity is unavailable".into(),
                ));
            }
        };
        if proposal["owner"]
            .as_str()
            .and_then(|owner| owner.parse::<u64>().ok())
            != Some(identity)
            || proposal["expected_generation"]
                .as_str()
                .and_then(|generation| generation.parse::<u64>().ok())
                != Some(grant.expected_generation)
            || proposal["source_generation"]
                .as_str()
                .and_then(|generation| generation.parse::<u64>().ok())
                != Some(self.generation)
        {
            return Ok(ReloadDecision::Refused(
                "stale or foreign checked proposal owner".into(),
            ));
        }
        let source = match candidate_source(selected, proposal, LIMIT as usize) {
            Ok(source) => source,
            Err(error) => return Ok(ReloadDecision::Refused(error)),
        };
        let prepared =
            match self
                .session
                .frontend
                .prepare(&source, &self.session.stack, super::SOURCE_LIMITS)
            {
                Ok(prepared)
                    if prepared.is_definition() && prepared.definition_name() == Some(selected) =>
                {
                    prepared
                }
                Ok(_) => {
                    return Ok(ReloadDecision::Refused(
                        "proposal changed the selected name".into(),
                    ));
                }
                Err(error) => {
                    return Ok(ReloadDecision::Refused(error.diagnostic().message.clone()));
                }
            };
        let number = self
            .session
            .submissions
            .checked_add(1)
            .ok_or_else(|| fatal_failure("live submission count exhausted"))?;
        self.install_definition(prepared, &source, number, None)
    }

    fn proposal_receipt(
        &self,
        outcome: &str,
        invocation: &serde_json::Value,
        recipe_digest: Option<&str>,
        proposal: Option<&serde_json::Value>,
        diagnostic: Option<&str>,
    ) -> serde_json::Value {
        serde_json::json!({
            "schema": "noble-live-report/v1",
            "stage": "self-edit",
            "outcome": outcome,
            "generation": self.generation,
            "source_generation": self.generation,
            "selected_name": self.selected_definition,
            "checked_owner_identity": self.selected_definition.as_deref()
                .and_then(|name| self.session.frontend.selected_definition_identity(name))
                .map(|identity| identity.to_string()),
            "grant_expected_generation": self.grant.as_ref().map(|grant| grant.expected_generation.to_string()),
            "proposal_name": proposal.map(|value| &value["name"]),
            "proposal_owner_identity": proposal.map(|value| &value["owner"]),
            "proposal_expected_generation": proposal.map(|value| &value["expected_generation"]),
            "proposal_source_generation": proposal.map(|value| &value["source_generation"]),
            "inspected_source_generation": proposal.map(|value| &value["inspected_source_generation"]),
            "candidate_program_interface": proposal.filter(|value|
                value["input_signature"].is_string()
                    && value["output_signature"].is_string()
                    && value["effects"].is_number()
            ).map(|value| serde_json::json!({
                "input": value["input_signature"],
                "output": value["output_signature"],
                "effects": value["effects"],
            })),
            "candidate_snapshot_sha256": proposal.map(|value| &value["candidate_snapshot_sha256"]),
            "postreturn_snapshot_verified": proposal.map(|value| &value["postreturn_snapshot_verified"]),
            "proposal_recipe_sha256": recipe_digest,
            "candidate_recipe_events": proposal
                .filter(|value| value["events"].as_array().is_some_and(|events| events.len() <= 512))
                .map(|value| &value["events"]),
            "origin": if proposal.and_then(|value| value["events"].as_array())
                .is_some_and(|events| events.len() <= 512)
            {
                "guest-program-recipe"
            } else {
                "guest-program-snapshot"
            },
            "observation_scope": "bounded-live-tool-observation",
            "selected_file_sha256": self.selected_file_digest(),
            "source_freshness": "not-file-backed",
            "stack": self.last_stack,
            "pre_invocation_stack": self.invocation_prior_stack,
            "request_trace": invocation["request_trace"],
            "guest_requests": invocation["guest_requests"],
            "protected_operations": invocation["protected_operations"],
            "invocation_source_sha256": invocation["module"]["source_sha256"],
            "invocation_wasm_sha256": invocation["module"]["wasm_sha256"],
            "candidate_prepare_requests": 0,
            "diagnostic": diagnostic,
        })
    }

    fn selected_file_digest(&self) -> Option<std::string::String> {
        self.selected_file_hash.clone()
    }

    fn select_grant(
        &mut self,
        selection: GrantSelection,
    ) -> Result<serde_json::Value, super::output::Failure> {
        if self.selected_definition.as_deref() != Some(selection.name.as_str())
            || self.grant_scope.as_deref() != Some(selection.name.as_str())
            || self.generation != selection.expected_generation
        {
            return Ok(refusal(
                self.generation,
                &self.last_stack,
                "self-edit grant requires selected definition and exact source generation",
            ));
        }
        let identity = match self
            .session
            .frontend
            .selected_definition_identity(&selection.name)
        {
            Some(identity) => identity,
            None => {
                return Ok(refusal(
                    self.generation,
                    &self.last_stack,
                    "selected definition lacks checked identity",
                ));
            }
        };
        let worker = self.worker()?.set_live_grant(
            &selection.name,
            identity,
            selection.expected_generation,
        )?;
        if worker.outcome != "grant-selected" {
            return Ok(refusal(
                self.generation,
                &self.last_stack,
                &report_diagnostic(&worker),
            ));
        }
        self.grant = Some(selection);
        Ok(serde_json::json!({
            "schema": "noble-live-report/v1",
            "stage": "grant",
            "outcome": "grant-selected",
            "name": self.selected_definition,
            "checked_owner_identity": identity,
            "source_generation": self.generation,
            "guest_requests": 0,
            "protected_operations": 0
        }))
    }

    fn admission_refusal(&self, diagnostic: &str, outcome: &'static str) -> super::output::Report {
        super::output::Report {
            outcome: outcome.into(),
            json: serde_json::json!({
                "schema": "noble-live-report/v1",
                "stage": "admission",
                "outcome": outcome,
                "diagnostic": diagnostic,
                "generation": self.generation,
                "stack": self.last_stack,
                "guest_requests": 0,
                "protected_operations": 0,
            })
            .to_string(),
        }
    }

    fn worker(&mut self) -> Result<&mut super::worker::Engine, super::output::Failure> {
        if self.session.worker.is_none() {
            self.session.worker = Some(super::worker::Engine::start_live()?);
        }
        self.session.worker.as_mut().ok_or_else(|| {
            super::output::Failure::new(
                super::output::ErrorContext {
                    stage: "wasm",
                    outcome: "internal-failure",
                },
                "missing live Node/V8 worker",
            )
        })
    }

    fn submit(&mut self, source: &[u8]) -> Result<super::output::Report, super::output::Failure> {
        self.session.submissions = self
            .session
            .submissions
            .checked_add(1)
            .ok_or_else(|| fatal_failure("live submission count exhausted"))?;
        let prepared =
            match self
                .session
                .frontend
                .prepare(source, &self.session.stack, super::SOURCE_LIMITS)
            {
                Ok(prepared) => prepared,
                Err(error) => {
                    return Ok(super::output::Report::source_error(
                        &error,
                        self.session.submissions,
                    ));
                }
            };
        if prepared.is_definition() {
            if self.selected_definition.as_deref() == prepared.definition_name() {
                return Ok(self.admission_refusal(
                    "selected file definition changes require :reload",
                    "definition-refused",
                ));
            }
            return match self.install_definition(
                prepared,
                source,
                self.session.submissions,
                None,
            )? {
                ReloadDecision::Committed(_) => {
                    Ok(super::output::Report::defined(self.session.submissions))
                }
                ReloadDecision::HostEffect(problem) => {
                    Ok(self.admission_refusal(&problem, "effect-refused"))
                }
                ReloadDecision::Refused(problem) => {
                    Ok(self.admission_refusal(&problem, "definition-refused"))
                }
            };
        }
        let submission = prepared
            .submission()
            .ok_or_else(|| fatal_failure("missing accepted live source submission"))?;
        let effects = submission.request.expected.allowed_effects.as_slice();
        if effects.iter().any(|effect| !matches!(effect.0, 3 | 4)) {
            return Ok(self.admission_refusal(
                "live profile has no grant for this host effect",
                "effect-refused",
            ));
        }
        if !effects.is_empty()
            && self.grant.as_ref().is_none_or(|grant| {
                grant.expected_generation != self.generation
                    || self.selected_definition.as_deref() != Some(grant.name.as_str())
            })
        {
            return Ok(self.admission_refusal(
                "live proposal requires a current host-selected grant",
                "effect-refused",
            ));
        }
        self.invocation_prior_stack = if effects.iter().any(|effect| effect.0 == 3) {
            Some(self.last_stack.clone())
        } else {
            None
        };
        let compiled = match self.session.compiler.prepare(submission) {
            Ok(compiled) => compiled,
            Err(error) => {
                return Ok(super::output::Report::backend_error(
                    error,
                    self.session.submissions,
                ));
            }
        };
        let encoded = if self.grant_scope.is_some() {
            encoding::encode_live_checked(compiled.wat())
        } else {
            encoding::encode_checked(compiled.wat())
        };
        let binary = match encoded {
            Ok(binary) => binary,
            Err(encoding::EncodeError::HostEffect(error)) => {
                return Ok(self.admission_refusal(&error, "effect-refused"));
            }
            Err(encoding::EncodeError::Other(error)) => {
                return Ok(self.admission_refusal(&error, "bytecode-refused"));
            }
        };
        let number = self.session.submissions;
        let ready = self.worker()?.prepare_binary(&binary, source, number)?;
        if ready.outcome != "ready" {
            return Ok(ready);
        }
        let output = prepared.output().to_vec();
        self.session
            .compiler
            .commit(compiled)
            .map_err(super::output::Failure::backend)?;
        self.session
            .frontend
            .commit(prepared)
            .map_err(super::output::Failure::source)?;
        let mut report = self.worker()?.execute()?;
        if report.outcome == "normal" {
            self.session.stack = output;
        }
        let mut invocation: serde_json::Value = serde_json::from_str(&report.json)
            .map_err(|_| fatal_failure("invalid live Wasm execution report"))?;
        let wasm_generation = invocation.get("generation").cloned();
        if let Some(fields) = invocation.as_object_mut() {
            fields.insert(
                "source_generation".into(),
                serde_json::json!(self.generation),
            );
            if let Some(wasm_generation) = wasm_generation {
                fields.insert("wasm_submission_generation".into(), wasm_generation);
            }
        }
        report.json = invocation.to_string();
        Ok(report)
    }

    fn respond_reload(&mut self, path: &std::path::Path) -> bool {
        let outcome = match self.reload(path) {
            Ok(ReloadDecision::Committed(report)) => report,
            Ok(ReloadDecision::Refused(problem)) => {
                refusal(self.generation, &self.last_stack, &problem)
            }
            Ok(ReloadDecision::HostEffect(problem)) => {
                refusal(self.generation, &self.last_stack, &problem)
            }
            Err(error) => {
                emit(&fatal(&error.message));
                return false;
            }
        };
        emit(&outcome);
        true
    }

    fn reload(&mut self, path: &std::path::Path) -> Result<ReloadDecision, super::output::Failure> {
        let selected = match selected_path(path) {
            Ok(path) => path,
            Err(error) => return Ok(ReloadDecision::Refused(error)),
        };
        if self
            .selected
            .as_ref()
            .is_some_and(|prior| prior != &selected)
        {
            return Ok(ReloadDecision::Refused(
                "reload path differs from selected source".into(),
            ));
        }
        let candidate = match snapshot(&selected) {
            Ok(candidate) => candidate,
            Err(error) => return Ok(ReloadDecision::Refused(error)),
        };
        let definition = match self.session.frontend.prepare(
            &candidate.bytes,
            &self.session.stack,
            super::SOURCE_LIMITS,
        ) {
            Ok(prepared) if prepared.is_definition() => prepared,
            Ok(_) => {
                return Ok(ReloadDecision::Refused(
                    "reload source must be one checked definition".into(),
                ));
            }
            Err(error) => return Ok(ReloadDecision::Refused(error.diagnostic().message.clone())),
        };
        let definition_name = definition
            .definition_name()
            .ok_or_else(|| fatal_failure("missing checked definition name"))?;
        if self
            .selected_definition
            .as_ref()
            .is_some_and(|previous| previous != definition_name)
        {
            return Ok(ReloadDecision::Refused(
                "reload changed the selected definition name".into(),
            ));
        }
        let definition_name = definition_name.to_owned();
        let number = self
            .session
            .submissions
            .checked_add(1)
            .ok_or_else(|| fatal_failure("live submission count exhausted"))?;
        let result = self.install_definition(
            definition,
            &candidate.bytes,
            number,
            Some((&selected, &candidate)),
        )?;
        if matches!(result, ReloadDecision::Committed(_)) {
            self.selected = Some(selected);
            self.selected_definition = Some(definition_name);
            self.selected_file_hash = Some(candidate.digest);
        }
        Ok(result)
    }

    /// File reload and direct definition admission share exactly one
    /// source-checked, binary-staged publication path. Only a file reload has
    /// an additional post-stage exact file snapshot requirement.
    fn install_definition(
        &mut self,
        definition: noble_contracts::source::Prepared,
        source: &[u8],
        number: u64,
        file: Option<(&std::path::Path, &Snapshot)>,
    ) -> Result<ReloadDecision, super::output::Failure> {
        if self
            .session
            .frontend
            .core_definition_requires_host_effects(&definition)
        {
            return Ok(ReloadDecision::HostEffect(
                "live profile has no host-effect grant for this definition".into(),
            ));
        }
        let preview = match self.session.frontend.preview_core_definition(&definition) {
            Ok(preview) => preview,
            Err(error) => return Ok(ReloadDecision::Refused(error.diagnostic().message.clone())),
        };
        let inert = match preview.prepare(b"", &self.session.stack, super::SOURCE_LIMITS) {
            Ok(prepared) => prepared,
            Err(error) => return Ok(ReloadDecision::Refused(error.diagnostic().message.clone())),
        };
        let checked = inert
            .submission()
            .ok_or_else(|| fatal_failure("missing inert reload submission"))?;
        let compiled = match self.session.compiler.prepare(checked) {
            Ok(compiled) => compiled,
            Err(error) => {
                let failure = super::output::Failure::backend(error);
                return Ok(ReloadDecision::Refused(std::format!(
                    "backend refused reload ({}): {}",
                    failure.context.outcome,
                    failure.message,
                )));
            }
        };
        if !self.session.compiler.can_commit(&compiled) {
            return Err(fatal_failure(
                "prospective compiler base changed before staging",
            ));
        }
        let encoded = if self.grant_scope.is_some() {
            encoding::encode_live_checked(compiled.wat())
        } else {
            encoding::encode_checked(compiled.wat())
        };
        let binary = match encoded {
            Ok(binary) => binary,
            Err(encoding::EncodeError::HostEffect(problem)) => {
                return Ok(ReloadDecision::HostEffect(problem));
            }
            Err(encoding::EncodeError::Other(problem)) => {
                return Ok(ReloadDecision::Refused(problem));
            }
        };
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| fatal_failure("live generation exhausted"))?;
        let staged = self.worker()?.stage_binary(&binary, source, number)?;
        if staged.outcome != "ready" {
            if staged.outcome == "reload-refused" {
                return Ok(ReloadDecision::Refused(report_diagnostic(&staged)));
            }
            return Err(fatal_failure(std::format!(
                "live binary staging failed: {}",
                staged.json
            )));
        }
        if let Some((selected, candidate)) = file {
            match snapshot(selected) {
                Ok(after)
                    if after.digest == candidate.digest && after.identity == candidate.identity => {
                }
                other => {
                    let discarded = self.worker()?.discard_binary()?;
                    if discarded.outcome != "discarded" {
                        return Err(fatal_failure(
                            "live worker did not discard superseded binary",
                        ));
                    }
                    let reason = match other {
                        Ok(_) => "source changed during reload staging".into(),
                        Err(error) => error,
                    };
                    return Ok(ReloadDecision::Refused(reason));
                }
            }
        }
        if !self.session.compiler.can_commit(&compiled) {
            let discarded = self.worker()?.discard_binary()?;
            if discarded.outcome != "discarded" {
                return Err(fatal_failure(
                    "live worker did not discard stale compiler binary",
                ));
            }
            return Err(fatal_failure(
                "prospective compiler base changed before publication",
            ));
        }
        let published = self.worker()?.publish_binary()?;
        if published.outcome == "reload-refused" {
            return Ok(ReloadDecision::Refused(report_diagnostic(&published)));
        }
        if published.outcome != "normal" {
            return Err(fatal_failure(std::format!(
                "live worker did not publish: {}",
                published.json
            )));
        }
        let stack = report_stack(&published)
            .ok_or_else(|| fatal_failure("published reload omitted stack"))?;
        if stack != self.last_stack {
            return Err(fatal_failure("published reload changed the retained stack"));
        }
        self.session
            .compiler
            .commit(compiled)
            .map_err(super::output::Failure::backend)?;
        self.session.frontend = preview;
        self.session.submissions = number;
        self.generation = next_generation;
        // Publication is irreversible: a later rename cannot be reported as a
        // recoverable refusal. ACK the pinned bytes actually installed, and
        // disclose a bounded observation of the path after publication. Even
        // a match at this instant is not a filesystem watch or an ACK-time
        // guarantee that the path still names the installed version.
        let source_freshness = match file {
            Some((selected, candidate)) => match snapshot(selected) {
                Ok(after)
                    if after.digest == candidate.digest && after.identity == candidate.identity =>
                {
                    "matched-at-postpublish-check"
                }
                _ => "changed-or-unverifiable-at-postpublish-check",
            },
            None => "not-file-backed",
        };
        Ok(ReloadDecision::Committed(serde_json::json!({
            "schema": "noble-live-report/v1", "stage": "wasm-live", "outcome": "reload-committed",
            "generation": self.generation, "stack": stack,
            "source_sha256": crate::workflow::intrinsic::sha256(source),
            "source_version": "pinned-at-prepublish-check",
            "source_freshness": source_freshness,
            "guest_requests": 0, "protected_operations": 0,
        })))
    }
}

fn selected_path(path: &std::path::Path) -> Result<std::path::PathBuf, std::string::String> {
    if !path.is_absolute() {
        return Err("reload requires an absolute selected file path".into());
    }
    let parent = path.parent().ok_or("reload path lacks a parent")?;
    let name = path.file_name().ok_or("reload path lacks a filename")?;
    let parent = std::fs::canonicalize(parent).map_err(|error| error.to_string())?;
    Ok(parent.join(name))
}

/// Reify only the closed, pure numeric recipe fragment. These bytes are a
/// derived source submission, not bytes from the selected file or a guest
/// supplied source string; all of them pass through ordinary source/kernel
/// acceptance again before staging.
fn candidate_source(
    name: &str,
    proposal: &serde_json::Value,
    limit: usize,
) -> Result<std::vec::Vec<u8>, std::string::String> {
    if proposal["input_signature"] != "[I64]"
        || proposal["output_signature"] != "[I64]"
        || proposal["effects"] != 0
    {
        return Err("candidate Program has an incompatible interface or effect".into());
    }
    let events = proposal["events"]
        .as_array()
        .ok_or("proposal lacks bounded recipe events")?;
    if events.len() < 4 || events.len() > 512 {
        return Err("candidate recipe event limit exceeded".into());
    }
    let signatures = proposal["signatures"]
        .as_array()
        .ok_or("proposal lacks checked signature descriptors")?;
    if signatures.len() > 64 {
        return Err("candidate signature limit exceeded".into());
    }
    let signature = |id: u64| -> Option<&str> {
        signatures
            .iter()
            .find(|entry| entry["id"].as_u64() == Some(id))
            .and_then(|entry| entry["descriptor"].as_str())
    };
    let signature_id =
        |event: &serde_json::Value| -> Option<u64> { event["value"].as_str()?.parse::<u64>().ok() };
    let input_id = proposal["input_signature_id"]
        .as_u64()
        .ok_or("missing candidate input signature ID")?;
    let output_id = proposal["output_signature_id"]
        .as_u64()
        .ok_or("missing candidate output signature ID")?;
    if signature(input_id) != Some("[I64]") || signature(output_id) != Some("[I64]") {
        return Err("candidate signature IDs do not match checked interface".into());
    }
    let mut source = std::format!("def {name} [ ").into_bytes();
    let mut cursor = 0usize;
    for (kind, value) in [(17, input_id), (18, output_id), (19, 0)] {
        if events[cursor]["kind"].as_u64() != Some(kind)
            || signature_id(&events[cursor]) != Some(value)
        {
            return Err("candidate recipe differs from checked Program interface".into());
        }
        cursor += 1;
    }
    let mut operations = 0usize;
    let mut stack_height = 1usize;
    while cursor < events.len() {
        let event = &events[cursor];
        let kind = event["kind"]
            .as_u64()
            .ok_or("invalid candidate recipe event")?;
        let value = event["value"]
            .as_str()
            .ok_or("invalid candidate recipe value")?;
        match kind {
            1 => {
                let number = value
                    .parse::<u64>()
                    .map_err(|_| "candidate I64 literal is out of range")?
                    as i64;
                source.extend_from_slice(number.to_string().as_bytes());
                source.push(b' ');
                stack_height = stack_height
                    .checked_add(1)
                    .ok_or("candidate stack height overflow")?;
                if stack_height > 128 {
                    return Err("candidate stack height limit exceeded".into());
                }
            }
            2 => {
                let word = match value {
                    "4" => "+",
                    "5" => "-",
                    "6" => "*",
                    _ => return Err("candidate recipe invokes an unsupported word".into()),
                };
                if stack_height < 2 {
                    return Err("candidate arithmetic lacks two checked I64 inputs".into());
                }
                source.extend_from_slice(word.as_bytes());
                source.push(b' ');
                for expected in [20, 21, 22] {
                    cursor += 1;
                    let metadata = events
                        .get(cursor)
                        .ok_or("incomplete invocation signature")?;
                    if metadata["kind"].as_u64() != Some(expected) {
                        return Err("invalid invocation signature metadata".into());
                    }
                    let actual = signature_id(metadata).ok_or("invalid invocation signature ID")?;
                    if expected == 20
                        && signature(actual).and_then(numeric_stack_height) != Some(stack_height)
                        || expected == 21
                            && signature(actual).and_then(numeric_stack_height)
                                != Some(stack_height - 1)
                    {
                        return Err(
                            "invocation signatures disagree with candidate operations".into()
                        );
                    }
                    if expected == 22 && actual != 0 {
                        return Err("candidate invocation carries an effect".into());
                    }
                }
                stack_height -= 1;
            }
            _ => return Err("candidate recipe uses unsupported captures or syntax".into()),
        }
        operations += 1;
        if source.len().saturating_add(1) > limit {
            return Err("derived source exceeds bounded input".into());
        }
        cursor += 1;
    }
    if operations == 0 {
        return Err("candidate recipe contains no body".into());
    }
    if stack_height != 1 {
        return Err("candidate recipe does not preserve its I64 interface".into());
    }
    source.push(b']');
    Ok(source)
}

fn numeric_stack_height(descriptor: &str) -> Option<usize> {
    let inside = descriptor.strip_prefix('[')?.strip_suffix(']')?;
    if inside.is_empty() {
        return Some(0);
    }
    let mut height = 0usize;
    for ty in inside.split(' ') {
        if ty != "I64" {
            return None;
        }
        height = height.checked_add(1)?;
    }
    Some(height)
}

fn snapshot(path: &std::path::Path) -> Result<Snapshot, std::string::String> {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;

    let parent = path.parent().ok_or("selected source lacks a parent")?;
    let name = path.file_name().ok_or("selected source lacks a filename")?;
    let directory = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(parent)
        .map_err(|error| error.to_string())?;
    let before = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !before.is_file() || before.len() > u64::from(LIMIT) {
        return Err("reload requires a bounded regular file, not a symlink".into());
    }
    // /proc/self/fd pins the selected directory while we resolve only its
    // final filename. O_NOFOLLOW prevents a symlink swapped in for this open
    // from redirecting source bytes; O_NONBLOCK refuses an attacker-swapped
    // FIFO rather than hanging. No unsafe openat is needed in this crate.
    let pinned = std::path::PathBuf::from(std::format!("/proc/self/fd/{}", directory.as_raw_fd()))
        .join(name);
    let mut opened = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(pinned)
        .map_err(|error| error.to_string())?;
    let opened_before = opened.metadata().map_err(|error| error.to_string())?;
    if !opened_before.is_file() || opened_before.len() > u64::from(LIMIT) {
        return Err("reload opened a nonregular or oversized source".into());
    }
    let mut bytes = Vec::new();
    opened
        .by_ref()
        .take(u64::from(LIMIT) + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > LIMIT as usize {
        return Err("reload source exceeds bounded input".into());
    }
    let opened_after = opened.metadata().map_err(|error| error.to_string())?;
    let after = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    let identity = |metadata: &std::fs::Metadata| {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        )
    };
    if !after.is_file()
        || !opened_after.is_file()
        || identity(&before) != identity(&opened_before)
        || identity(&opened_before) != identity(&opened_after)
        || identity(&opened_after) != identity(&after)
    {
        return Err("source changed during bounded snapshot".into());
    }
    Ok(Snapshot {
        digest: crate::workflow::intrinsic::sha256(&bytes),
        bytes,
        identity: identity(&after),
    })
}

fn report_stack(report: &super::output::Report) -> Option<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(&report.json)
        .ok()?
        .get("stack")
        .cloned()
}

fn report_diagnostic(report: &super::output::Report) -> std::string::String {
    serde_json::from_str::<serde_json::Value>(&report.json)
        .ok()
        .and_then(|value| value.get("diagnostic")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| report.json.clone())
}

fn refusal(generation: u64, stack: &serde_json::Value, diagnostic: &str) -> serde_json::Value {
    serde_json::json!({"schema":"noble-live-report/v1", "stage":"admission", "outcome":"reload-refused",
        "generation":generation, "stack":stack, "diagnostic":diagnostic,
        "guest_requests":0, "protected_operations":0})
}

fn fatal(diagnostic: &str) -> serde_json::Value {
    serde_json::json!({"schema":"noble-live-report/v1", "stage":"wasm-live", "outcome":"internal-failure", "diagnostic":diagnostic})
}

fn fatal_failure(message: impl Into<std::string::String>) -> super::output::Failure {
    super::output::Failure::new(
        super::output::ErrorContext {
            stage: "wasm-live",
            outcome: "internal-failure",
        },
        message,
    )
}

fn terminal_proposal_failure(
    error: &super::output::Failure,
    completed_stack: &serde_json::Value,
    source_generation: u64,
) -> Option<serde_json::Value> {
    let mut report = serde_json::from_str::<serde_json::Value>(&error.message).ok()?;
    if report["outcome"] != "internal-failure"
        || !report["session_state"]
            .as_str()
            .is_some_and(|state| state.starts_with("terminated"))
    {
        return None;
    }
    let fields = report.as_object_mut()?;
    fields.insert("completed_invocation_stack".into(), completed_stack.clone());
    fields.insert(
        "source_generation".into(),
        serde_json::json!(source_generation),
    );
    Some(report)
}

fn emit(value: &serde_json::Value) {
    let report = super::output::Report {
        outcome: value
            .get("outcome")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("internal-failure")
            .into(),
        json: value.to_string(),
    };
    if let Err(error) = super::output::print(&report) {
        eprintln!("noble: live reply transport failed: {}", error.message);
    }
}

#[cfg(test)]
mod tests {
    use super::{candidate_source, terminal_proposal_failure};

    #[test]
    fn reifier_refuses_inconsistent_checked_invocation_metadata() {
        let mut proposal = serde_json::json!({
            "input_signature": "[I64]",
            "output_signature": "[I64]",
            "input_signature_id": 1,
            "output_signature_id": 1,
            "effects": 0,
            "signatures": [
                {"id": 1, "descriptor": "[I64]"},
                {"id": 2, "descriptor": "[I64 I64]"}
            ],
            "events": [
                {"kind":17,"value":"1"}, {"kind":18,"value":"1"},
                {"kind":19,"value":"0"}, {"kind":1,"value":"2"},
                {"kind":2,"value":"4"}, {"kind":20,"value":"2"},
                {"kind":21,"value":"1"}, {"kind":22,"value":"0"}
            ]
        });
        assert_eq!(
            candidate_source("evolve", &proposal, 65_536).unwrap(),
            b"def evolve [ 2 + ]"
        );
        proposal["events"][5]["value"] = serde_json::json!("1");
        assert!(
            candidate_source("evolve", &proposal, 65_536)
                .unwrap_err()
                .contains("invocation signatures disagree")
        );
        proposal["events"][5]["value"] = serde_json::json!("2");
        proposal["events"][1]["value"] = serde_json::json!("2");
        assert!(
            candidate_source("evolve", &proposal, 65_536)
                .unwrap_err()
                .contains("differs from checked Program interface")
        );
    }

    #[test]
    fn poisoned_inspection_preserves_worker_trace_and_terminates() {
        let original = serde_json::json!({
            "outcome": "internal-failure",
            "diagnostic": "immutable live proposal changed",
            "session_state": "terminated-after-proposal-inspection-failure",
            "request_trace": ["live.observe-generation:1", "live.propose:evolve:queued"],
            "guest_requests": 2,
        });
        let failure = super::super::output::Failure::new(
            super::super::output::ErrorContext {
                stage: "wasm",
                outcome: "internal-failure",
            },
            original.to_string(),
        );
        let terminal = terminal_proposal_failure(
            &failure,
            &serde_json::json!([{"type":"I64","value":"21"}]),
            1,
        )
        .expect("terminal worker failure must remain terminal");
        assert_eq!(terminal["diagnostic"], original["diagnostic"]);
        assert_eq!(terminal["request_trace"], original["request_trace"]);
        assert_eq!(terminal["guest_requests"], 2);
        assert_eq!(terminal["completed_invocation_stack"][0]["value"], "21");
        assert_eq!(
            terminal["session_state"],
            "terminated-after-proposal-inspection-failure"
        );
    }
}
