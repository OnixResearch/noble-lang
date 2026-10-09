//! Opt-in Core/declared live session. Existing run/session paths do not enter here.
//! Core reload previews one checked replacement and its transitive dependents,
//! stages an inert binary, and publishes after a second selected-file snapshot.
//! Declared reload instead publishes immutable module roots and current aliases.
//! The resident engine retains old Program slots in both profiles.

mod encoding;
mod declared;
#[cfg(target_os = "linux")]
mod watch;

use std::os::unix::fs::MetadataExt;

const LIMIT: u32 = super::SOURCE_LIMITS.bytes;

type FileIdentity = (u64, u64, u64, u64, i64, i64, i64, i64);

struct Snapshot {
    bytes: std::vec::Vec<u8>,
    digest: std::string::String,
    identity: FileIdentity,
    directory: (u64, u64),
}

struct GrantSelection {
    name: std::string::String,
    expected_generation: u64,
}

struct LiveOptions {
    source: Option<std::path::PathBuf>,
    engine: &'static str,
    initial_grant: Option<GrantSelection>,
    verify_source_before_execute: bool,
    manifest: Option<super::arguments::bindings::Manifest>,
    watch: bool,
    retained_generation_limit: Option<u64>,
    guest_step_budget: Option<u32>,
}

enum ReloadDecision {
    Committed(serde_json::Value),
    Refused(std::string::String),
    SelectedSourceRefused(noble_contracts::source::Error),
    SourceChanged(super::output::Report),
    HostEffect(std::string::String),
}

#[derive(Clone, Copy)]
struct InstalledCode {
    span: noble_wasm::source::CodeSpan,
    owner: u64,
    source_generation: u64,
    source_sha256: [u8; 64],
    wat_sha256: [u8; 64],
    wasm_sha256: [u8; 64],
    bridge_wat_sha256: [u8; 64],
    bridge_wasm_sha256: [u8; 64],
    signature_catalogue_sha256: [u8; 64],
}

impl InstalledCode {
    fn matches_receipt(&self, receipt: &serde_json::Value) -> bool {
        receipt["code_span"] == serde_json::json!({
            "start": self.span.start, "length": self.span.length,
            "generation": self.span.generation,
        })
            && receipt["guest_instance_owner"] == self.owner
            && receipt["source_generation"].as_str()
                .and_then(|generation| generation.parse::<u64>().ok())
                == Some(self.source_generation)
            && [
                ("guest_source_sha256", &self.source_sha256),
                ("guest_wat_sha256", &self.wat_sha256),
                ("guest_wasm_sha256", &self.wasm_sha256),
                ("bridge_wat_sha256", &self.bridge_wat_sha256),
                ("bridge_wasm_sha256", &self.bridge_wasm_sha256),
                ("signature_catalogue_sha256", &self.signature_catalogue_sha256),
            ].iter().all(|(key, expected)| receipt[*key].as_str()
                .is_some_and(|hash| hash.as_bytes() == expected.as_slice()))
            && receipt["guest_table_entries_remaining"] == 0
            && receipt["guest_js_owner_edges_remaining"] == 0
            && receipt["core_dispatch_bridge_owned"] == true
            && receipt["guest_instance_releasable"] == true
    }
}

struct LiveBinaryBinding {
    source_sha256: std::string::String,
    wat_sha256: std::string::String,
    wasm_sha256: std::string::String,
    bridge_wat_sha256: std::string::String,
    bridge_wasm_sha256: std::string::String,
    signature_catalogue_sha256: std::string::String,
}

impl LiveBinaryBinding {
    fn new(
        compiled: &noble_wasm::source::Prepared,
        binary: &[u8],
        bridge: &[u8],
        bridge_wat_sha256: std::string::String,
        signatures: &[u8],
        source: &[u8],
    ) -> Self {
        Self {
            source_sha256: crate::workflow::intrinsic::sha256(source),
            wat_sha256: crate::workflow::intrinsic::sha256(compiled.wat()),
            wasm_sha256: crate::workflow::intrinsic::sha256(binary),
            bridge_wat_sha256,
            bridge_wasm_sha256: crate::workflow::intrinsic::sha256(bridge),
            signature_catalogue_sha256: crate::workflow::intrinsic::sha256(signatures),
        }
    }
}

struct Live {
    session: super::Session,
    declared: Option<declared::State>,
    verify_source_before_execute: bool,
    selected: Option<std::path::PathBuf>,
    selected_definition: Option<std::string::String>,
    selected_file_hash: Option<std::string::String>,
    selected_file_identity: Option<(FileIdentity, (u64, u64))>,
    grant_scope: Option<std::string::String>,
    grant: Option<GrantSelection>,
    last_stack: serde_json::Value,
    invocation_prior_stack: Option<serde_json::Value>,
    // Source/namespace publication generation, not the Wasm submission count.
    generation: u64,
    retained_generation_limit: Option<u64>,
    guest_step_budget: Option<u32>,
    live_module_installed: bool,
    live_code_spans: std::vec::Vec<InstalledCode>,
    #[cfg(target_os = "linux")]
    pending_watch_seal: Option<watch::SealedSnapshot>,
    #[cfg(target_os = "linux")]
    accepted_watch_seal: Option<watch::SealedSnapshot>,
    #[cfg(target_os = "linux")]
    watch: Option<watch::Watcher>,
}

pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let LiveOptions {
        source,
        engine,
        initial_grant,
        verify_source_before_execute,
        manifest,
        watch,
        retained_generation_limit,
        guest_step_budget,
    } = match parse(arguments) {
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
    };
    let selected_grant_name = initial_grant.as_ref().map(|grant| grant.name.as_str());
    let declared = match manifest.as_ref() {
        Some(manifest) => match noble_contracts::source::ModuleSession::new(
            &manifest.operations(),
            noble_contracts::source::RetainedSourceLimit::new(super::SOURCE_LIMITS.bytes)
                .expect("the fixed live source byte bound is valid"),
        ) {
            Ok(frontend) => Some(frontend),
            Err(error) => {
                emit(&refusal(0, &serde_json::json!([]), &error.diagnostic().message));
                return std::process::ExitCode::from(2);
            }
        },
        None => None,
    };
    #[cfg(target_os = "linux")]
    let watcher = if watch {
        match watch::Watcher::new(source.as_ref().expect("parsed watch requires a source")) {
            Ok(watcher) => Some(watcher),
            Err(problem) => {
                emit(&refusal(0, &serde_json::json!([]), &problem));
                return std::process::ExitCode::from(2);
            }
        }
    } else {
        None
    };
    #[cfg(not(target_os = "linux"))]
    if watch {
        emit(&refusal(0, &serde_json::json!([]), "live watch requires Linux inotify"));
        return std::process::ExitCode::from(2);
    }
    let mut live = Live {
        session: super::Session {
            // An empty selected spelling is not a source identifier: it keeps
            // the Core-live ABI while granting no body self-edit authority.
            frontend: noble_contracts::source::Session::new_live(
                selected_grant_name.unwrap_or(""),
            ),
            declared,
            compiler: if manifest.is_some() {
                noble_wasm::source::Compiler::new_declared_live()
            } else {
                noble_wasm::source::Compiler::new_live()
            },
            stack: std::vec::Vec::new(),
            worker: None,
            submissions: 0,
        },
        declared: manifest.map(declared::State::new),
        verify_source_before_execute,
        selected: None,
        selected_definition: None,
        selected_file_hash: None,
        selected_file_identity: None,
        grant_scope: selected_grant_name.map(str::to_owned),
        grant: initial_grant.as_ref().map(|grant| GrantSelection {
            name: grant.name.clone(),
            expected_generation: grant.expected_generation,
        }),
        last_stack: serde_json::json!([]),
        invocation_prior_stack: None,
        generation: 0,
        retained_generation_limit,
        guest_step_budget,
        live_module_installed: false,
        live_code_spans: std::vec::Vec::new(),
        #[cfg(target_os = "linux")]
        pending_watch_seal: None,
        #[cfg(target_os = "linux")]
        accepted_watch_seal: None,
        #[cfg(target_os = "linux")]
        watch: watcher,
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
    let mut input = std::io::BufReader::new(std::io::stdin());
    loop {
        #[cfg(target_os = "linux")]
        if live.watch.is_some() {
            // A BufReader may already hold more complete input lines than
            // poll can see. Drain pending save events before consuming its
            // next line, including a save that superseded the last commit.
            let watcher = live.watch.as_mut().expect("checked watch selection");
            if let Err(problem) = watcher.drain() {
                emit(&fatal(&problem));
                return std::process::ExitCode::from(2);
            }
            if watcher.has_pending() {
                if !live.respond_watch() {
                    return std::process::ExitCode::from(2);
                }
                continue;
            }
            if input.buffer().is_empty() {
                match live.watch.as_ref().expect("checked watch selection").wait(input.get_ref()) {
                    Ok(watch::Ready::Input) => {}
                    Ok(watch::Ready::Save) => {
                        if !live.respond_watch() {
                            return std::process::ExitCode::from(2);
                        }
                        continue;
                    }
                    Err(problem) => {
                        emit(&fatal(&problem));
                        return std::process::ExitCode::from(2);
                    }
                }
            }
        }
        let next = super::framing::read_line(&mut input, LIMIT)
            .map_err(|error| error.message);
        let bytes = match next {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return std::process::ExitCode::SUCCESS,
            Err(problem) => {
                emit(&fatal(&problem));
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
            #[cfg(all(target_os = "linux", feature = "live-test-barrier"))]
            if live.watch.is_some() {
                if let Err(problem) = watch::pause_explicit_reload_for_test(
                    "before-snapshot", path,
                ) {
                    emit(&fatal(&problem));
                    return std::process::ExitCode::from(2);
                }
            }
            if !live.respond_reload(std::path::Path::new(path)) {
                return std::process::ExitCode::from(2);
            }
            continue;
        }
        if line == ":release-observed-roots" {
            match live.release_observed_roots() {
                Ok(report) => emit(&report),
                Err(error) => {
                    emit(&fatal(&error.message));
                    return std::process::ExitCode::from(2);
                }
            }
            continue;
        }
        if line == ":generation" {
            emit(&serde_json::json!({
                "schema": "noble-live-report/v1",
                "stage": "inspection",
                "outcome": "generation-observed",
                "generation": live.generation,
                "engine": engine,
                "stack": &live.last_stack,
                "guest_requests": 0,
                "protected_operations": 0,
            }));
            continue;
        }
        if let Some(selection) = line.strip_prefix(":revoke-effect ") {
            match live.revoke_effect(selection) {
                Ok(report) => emit(&report),
                Err(error) => {
                    emit(&fatal(&error.message));
                    return std::process::ExitCode::from(2);
                }
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
                if let Err(error) = live.retire_unowned_live_code() {
                    emit(&fatal(&error.message));
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
) -> Result<LiveOptions, std::string::String> {
    let mut watch = match arguments.get(1).and_then(|argument| argument.to_str()) {
        Some("repl") => false,
        Some("watch") => true,
        _ => return Err("usage: noble live repl|watch --source ABS_PATH".into()),
    };
    let mut source = None;
    let mut engine = None;
    let mut grant_name = None;
    let mut grant_generation = None;
    let mut verify_source_before_execute = false;
    let mut declared_modules = false;
    let mut bindings = None;
    let mut retained_generation_limit = None;
    let mut guest_step_budget = None;
    let mut at = 2;
    while at < arguments.len() {
        let flag = arguments[at].to_str().ok_or("invalid live option")?;
        if flag == "--verify-source-before-execute" {
            if verify_source_before_execute {
                return Err("duplicate live source verification option".into());
            }
            verify_source_before_execute = true;
            at += 1;
            continue;
        }
        if flag == "--declared-modules" {
            if declared_modules {
                return Err("duplicate live declared modules option".into());
            }
            declared_modules = true;
            at += 1;
            continue;
        }
        if flag == "--watch" {
            if watch {
                return Err("duplicate live watch selection".into());
            }
            watch = true;
            at += 1;
            continue;
        }
        let value = arguments
            .get(at + 1)
            .and_then(|value| value.to_str())
            .ok_or("missing live option value")?;
        match flag {
            "--source" if source.is_none() => {
                source = Some(std::path::PathBuf::from(value))
            }
            "--bindings" if bindings.is_none() => bindings = Some(std::path::PathBuf::from(value)),
            "--max-retained-generations" if retained_generation_limit.is_none() => {
                let limit = value.parse::<u64>()
                    .map_err(|_| "invalid retained generation budget")?;
                if !(1..=64).contains(&limit) {
                    return Err("retained generation budget must be between 1 and 64".into());
                }
                retained_generation_limit = Some(limit);
            }
            "--max-guest-steps" if guest_step_budget.is_none() => {
                let limit = value.parse::<u32>()
                    .map_err(|_| "invalid guest execution step budget")?;
                if !(1..=100_000).contains(&limit) {
                    return Err("guest execution step budget must be between 1 and 100000".into());
                }
                guest_step_budget = Some(limit);
            }
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
    if (verify_source_before_execute || watch) && source.is_none() {
        return Err("source verification requires --source ABS_PATH".into());
    }
    // A watched complete-save ACK cannot permit execution after an observed
    // but unacknowledged file change, even before the rename is processed.
    verify_source_before_execute |= watch;
    if declared_modules != bindings.is_some() || declared_modules && source.is_none() {
        return Err("declared live modules require --source ABS_PATH --declared-modules --bindings MANIFEST".into());
    }
    if declared_modules && (grant_name.is_some() || grant_generation.is_some()) {
        return Err("declared live modules do not support guest self-edit grants".into());
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
    let manifest = match bindings {
        Some(path) => Some(
            super::arguments::load_live_manifest(&path).map_err(|error| error.message)?
        ),
        None => None,
    };
    Ok(LiveOptions {
        source,
        engine: engine.unwrap_or("v8"),
        initial_grant: grant,
        verify_source_before_execute,
        manifest,
        watch,
        retained_generation_limit,
        guest_step_budget,
    })
}

fn source_identity(metadata: &std::fs::Metadata) -> FileIdentity {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.nlink(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

impl Live {
    fn check_live_binary_binding(
        report: &super::output::Report,
        binding: &LiveBinaryBinding,
    ) -> Result<(), super::output::Failure> {
        let value: serde_json::Value = serde_json::from_str(&report.json)
            .map_err(|_| fatal_failure("invalid source-bound Core bridge receipt"))?;
        let module = &value["module"];
        for (field, expected) in [
            ("source_sha256", &binding.source_sha256),
            ("wat_sha256", &binding.wat_sha256),
            ("wasm_sha256", &binding.wasm_sha256),
            ("bridge_wat_sha256", &binding.bridge_wat_sha256),
            ("bridge_wasm_sha256", &binding.bridge_wasm_sha256),
            ("signature_catalogue_sha256", &binding.signature_catalogue_sha256),
        ] {
            if module[field].as_str() != Some(expected.as_str()) {
                return Err(fatal_failure(std::format!(
                    "Core worker did not bind checked source and {field}"
                )));
            }
        }
        Ok(())
    }

    fn runtime_bridge(
        compiled: &noble_wasm::source::Prepared,
    ) -> Result<(std::vec::Vec<u8>, std::vec::Vec<u8>, std::string::String), super::output::Failure> {
        let wat = compiled.runtime_bridge_wat().map_err(super::output::Failure::backend)?;
        let bridge_wat_sha256 = crate::workflow::intrinsic::sha256(&wat);
        let bridge = encoding::encode_runtime_bridge_checked(&wat)
            .map_err(|error| fatal_failure(std::format!(
                "trusted Core16 runtime bridge cannot be assembled: {error}"
            )))?;
        let signatures = compiled.runtime_signature_catalog()
            .map_err(super::output::Failure::backend)?;
        Ok((bridge, signatures, bridge_wat_sha256))
    }

    fn register_live_code(
        &mut self,
        span: noble_wasm::source::CodeSpan,
        execution: &super::output::Report,
    )
        -> Result<(), super::output::Failure> {
        let invocation: serde_json::Value = serde_json::from_str(&execution.json)
            .map_err(|_| fatal_failure("invalid installed live guest source receipt"))?;
        let registered = self.worker()?.register_live_code(span)?;
        let receipt: serde_json::Value = serde_json::from_str(&registered.json)
            .map_err(|_| fatal_failure("invalid installed live code owner receipt"))?;
        let exact = serde_json::json!({
            "start": span.start, "length": span.length, "generation": span.generation,
        });
        if registered.outcome != "code-registered" || receipt["code_span"] != exact
            || receipt["table_installed"] != true
            || receipt["guest_instance_owner"].as_u64().is_none_or(|id| id == 0)
            || receipt["guest_wasm_sha256"] != invocation["module"]["wasm_sha256"]
            || receipt["guest_wat_sha256"] != invocation["module"]["wat_sha256"]
            || receipt["guest_source_sha256"] != invocation["module"]["source_sha256"]
            || receipt["bridge_wat_sha256"] != invocation["module"]["bridge_wat_sha256"]
            || receipt["bridge_wasm_sha256"] != invocation["module"]["bridge_wasm_sha256"]
            || receipt["signature_catalogue_sha256"]
                != invocation["module"]["signature_catalogue_sha256"]
            || receipt["source_generation"].as_str() != Some(self.generation.to_string().as_str())
        {
            return Err(fatal_failure("worker did not register the exact checked installed code span"));
        }
        self.live_module_installed = true;
        self.live_code_spans.push(InstalledCode {
            span,
            owner: receipt["guest_instance_owner"].as_u64()
                .ok_or_else(|| fatal_failure("registered instance omitted owner"))?,
            source_generation: self.generation,
            source_sha256: receipt["guest_source_sha256"].as_str()
                .and_then(|value| value.as_bytes().try_into().ok())
                .ok_or_else(|| fatal_failure("registered instance omitted source hash"))?,
            wat_sha256: receipt["guest_wat_sha256"].as_str()
                .and_then(|value| value.as_bytes().try_into().ok())
                .ok_or_else(|| fatal_failure("registered instance omitted guest WAT hash"))?,
            wasm_sha256: receipt["guest_wasm_sha256"].as_str()
                .and_then(|value| value.as_bytes().try_into().ok())
                .ok_or_else(|| fatal_failure("registered instance omitted binary hash"))?,
            bridge_wat_sha256: receipt["bridge_wat_sha256"].as_str()
                .and_then(|value| value.as_bytes().try_into().ok())
                .ok_or_else(|| fatal_failure("registered instance omitted bridge WAT hash"))?,
            bridge_wasm_sha256: receipt["bridge_wasm_sha256"].as_str()
                .and_then(|value| value.as_bytes().try_into().ok())
                .ok_or_else(|| fatal_failure("registered instance omitted bridge binary hash"))?,
            signature_catalogue_sha256: receipt["signature_catalogue_sha256"].as_str()
                .and_then(|value| value.as_bytes().try_into().ok())
                .ok_or_else(|| fatal_failure("registered instance omitted catalogue hash"))?,
        });
        Ok(())
    }

    fn release_observed_roots(&mut self)
        -> Result<serde_json::Value, super::output::Failure> {
        let released = self.worker()?.release_observed_roots()?;
        let receipt: serde_json::Value = serde_json::from_str(&released.json)
            .map_err(|_| fatal_failure("invalid observed live root release receipt"))?;
        let count = receipt["released"].as_u64()
            .ok_or_else(|| fatal_failure("worker omitted released observed root count"))?;
        if released.outcome != "observed-roots-released"
            || receipt["outcome"] != "observed-roots-released"
            || count > 4096
        {
            return Err(fatal_failure("worker did not prove bounded observed root release"));
        }
        let retired_code_spans = self.retire_unowned_live_code()?;
        Ok(serde_json::json!({
            "schema": "noble-live-report/v1", "stage": "wasm-live",
            "outcome": "observed-roots-released", "generation": self.generation,
            "released_observed_roots": count,
            "retired_code_spans": retired_code_spans,
            "guest_requests": 0, "protected_operations": 0,
        }))
    }

    fn retire_unowned_live_code(
        &mut self,
    ) -> Result<std::vec::Vec<serde_json::Value>, super::output::Failure> {
        // Source declarations and host-only controls can precede the first
        // executable in either profile. An installed zero-length module
        // still has globals to release.
        if !self.live_module_installed {
            return Ok(std::vec::Vec::new());
        }
        let released = self.worker()?.release_superseded_live_module_roots()?;
        let receipt: serde_json::Value = serde_json::from_str(&released.json)
            .map_err(|_| fatal_failure("invalid superseded Core module root receipt"))?;
        if released.outcome != "module-roots-released"
            || receipt["released"].as_u64().is_none() {
            return Err(fatal_failure("worker did not release superseded Core module roots"));
        }
        let mut retired_receipts = std::vec::Vec::new();
        let mut at = 0;
        while let Some(&installed) = self.live_code_spans.get(at) {
            let span = installed.span;
            if span.length == 0 {
                // No backend CodeSpan exists to inspect or tombstone. The
                // worker observes only its exact module-root release state.
                let observed = self.worker()?.inspect_live_code(span)?;
                let state: serde_json::Value = serde_json::from_str(&observed.json)
                    .map_err(|_| fatal_failure("invalid empty guest module-root inventory"))?;
                let exact = serde_json::json!({
                    "start": span.start, "length": span.length, "generation": span.generation,
                });
                if state["code_span"] != exact
                    || state["guest_instance_owner"] != installed.owner
                    || state["no_code_interval"] != true
                    || state.get("code_owners").is_some()
                {
                    return Err(fatal_failure("worker returned inconsistent empty guest ownership"));
                }
                if observed.outcome == "instance-owned"
                    && state["module_roots_released"] == false
                {
                    at += 1;
                    continue;
                }
                if observed.outcome != "instance-unowned"
                    || state["module_roots_released"] != true
                {
                    return Err(fatal_failure("worker did not prove empty guest module-root release"));
                }
                // The distinct instance-only receipt proves that no table or
                // persistent JS edge retains this source-bound guest.
                let report = self.worker()?.retire_live_code(span)?;
                let receipt: serde_json::Value = serde_json::from_str(&report.json)
                    .map_err(|_| fatal_failure("invalid empty guest instance release receipt"))?;
                if report.outcome != "instance-retired"
                    || receipt["outcome"] != "instance-retired"
                    || receipt["no_code_interval"] != true
                    || receipt["module_roots_released"] != true
                    || receipt.get("code_owners").is_some()
                    || receipt.get("table_cleared").is_some()
                    || !installed.matches_receipt(&receipt)
                {
                    return Err(fatal_failure(
                        "worker did not prove exact empty guest instance release"));
                }
                retired_receipts.push(receipt);
                self.live_code_spans.remove(at);
                continue;
            }
            let inspected = self.worker()?.inspect_live_code(span)?;
            let receipt: serde_json::Value = serde_json::from_str(&inspected.json)
                .map_err(|_| fatal_failure("invalid live code owner inventory"))?;
            let exact = serde_json::json!({
                "start": span.start, "length": span.length, "generation": span.generation,
            });
            let owners = receipt["code_owners"]
                .as_u64().ok_or_else(|| fatal_failure("live code inventory omitted nonnegative owners"))?;
            if receipt["code_span"] != exact
                || (owners == 0 && inspected.outcome != "code-unowned")
                || (owners != 0 && inspected.outcome != "code-owned")
            {
                return Err(fatal_failure("worker returned inconsistent checked live code owners"));
            }
            if owners != 0 {
                at += 1;
                continue;
            }
            let mut problem = None;
            let mut retirement_receipt = None;
            let worker = self.session.worker.as_mut()
                .ok_or_else(|| fatal_failure("live worker disappeared before code retirement"))?;
            let retired = self.session.compiler.retire_code(span, |checked| {
                if checked != span {
                    problem = Some("compiler offered a different checked code interval".into());
                    return Err(noble_wasm::Diagnostic::Invalid);
                }
                let report = match worker.retire_live_code(checked) {
                    Ok(report) => report,
                    Err(error) => {
                        problem = Some(std::format!(
                            "worker live code retirement failed: {}", error.message));
                        return Err(noble_wasm::Diagnostic::Invalid);
                    }
                };
                let acknowledged = serde_json::from_str::<serde_json::Value>(&report.json).ok();
                if report.outcome != "code-retired"
                    || !acknowledged.as_ref().is_some_and(|value|
                        value["outcome"] == "code-retired"
                            && value["code_owners"] == 0
                            && value["table_cleared"] == true
                            && installed.matches_receipt(value))
                {
                    problem = Some("worker did not prove exact zero-owned code tombstones and table clearance".into());
                    return Err(noble_wasm::Diagnostic::Invalid);
                }
                retirement_receipt = acknowledged;
                Ok(())
            });
            if retired.is_err() {
                return Err(fatal_failure(problem.unwrap_or_else(||
                    "compiler refused exact installed live code retirement".into())));
            }
            retired_receipts.push(retirement_receipt
                .ok_or_else(|| fatal_failure("compiler retirement omitted worker clearance receipt"))?);
            self.live_code_spans.remove(at);
        }
        Ok(retired_receipts)
    }

    fn retained_generation_budget_refusal(&self) -> Option<String> {
        self.retained_generation_limit
            .filter(|limit| self.generation >= *limit)
            .map(|limit| format!(
                "retained generation budget of {limit} exhausted; prior generation remains active"
            ))
    }

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
            Ok(ReloadDecision::SourceChanged(report)) => self.proposal_receipt(
                "proposal-refused",
                &invocation,
                Some(&recipe_digest),
                Some(&proposal),
                Some(&report_diagnostic(&report)),
            ),
            Ok(ReloadDecision::SelectedSourceRefused(_)) => {
                return Err(fatal_failure("guest proposal reported a selected-file source error"));
            }
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
        // H-LIVE-04's bounded guest lane does not grant a dependent rebuild.
        // Host-selected file reload/direct definitions use the full batch
        // transaction; a guest proposal must refuse before candidate staging.
        if self.session.frontend.live_definition_has_dependents(selected) {
            return Ok(ReloadDecision::Refused(
                "bounded guest self-edit refuses dependent definitions".into(),
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
        self.install_definition(prepared, &source, number, None, None)
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

    /// Only this host command can remove a selected immutable binding's
    /// current permission. It never changes the source root, slot or grant.
    fn revoke_effect(&mut self, selection: &str)
        -> Result<serde_json::Value, super::output::Failure> {
        let Some(declared) = self.declared.as_ref() else {
            return Ok(refusal(
                self.generation, &self.last_stack,
                "host revocation requires selected declared modules",
            ));
        };
        if let Some(changed) = self.source_change_refusal() {
            return serde_json::from_str(&changed.json)
                .map_err(|_| fatal_failure("invalid selected-source refusal"));
        }
        let mut terms = selection.split_ascii_whitespace();
        let (Some(module_version), Some(adapter), Some(effect), None) =
            (terms.next(), terms.next(), terms.next(), terms.next()) else {
            return Ok(refusal(self.generation, &self.last_stack,
                "host revocation requires MODULE@VERSION ADAPTER EFFECT"));
        };
        let Some((module, version_text)) = module_version.rsplit_once('@') else {
            return Ok(refusal(self.generation, &self.last_stack,
                "host revocation requires a canonical module version"));
        };
        let Ok(version) = version_text.parse::<u32>() else {
            return Ok(refusal(self.generation, &self.last_stack,
                "host revocation requires a canonical module version"));
        };
        if version == 0 || version_text.starts_with('0')
            || !version_text.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Ok(refusal(self.generation, &self.last_stack,
                "host revocation requires a canonical module version"));
        }
        let (Some(slot), Some((identity, source_sha256))) = (
            declared.manifest.revocable_slot(module, version, adapter, effect),
            declared.revocation_root(module, version),
        ) else {
            return Ok(refusal(self.generation, &self.last_stack,
                "host revocation does not name a checked source binding"));
        };
        let checked = serde_json::json!({
            "generation": self.generation, "identity": identity,
            "module": module, "version": version, "source_sha256": source_sha256,
            "adapter": adapter, "effect": effect, "slot": slot,
        });
        let worker = self.worker()?.revoke_live_effect(&checked)?;
        if worker.outcome != "host-policy-revoked" && worker.outcome != "host-policy-refused" {
            return Err(fatal_failure("worker did not decide host revocation"));
        }
        let mut report: serde_json::Value = serde_json::from_str(&worker.json)
            .map_err(|_| fatal_failure("invalid host revocation receipt"))?;
        if report["outcome"] != worker.outcome
            || report["source_generation"].as_str() != Some(self.generation.to_string().as_str())
            || report["guest_requests"] != 0 || report["protected_operations"] != 0
        {
            return Err(fatal_failure("worker revocation receipt changed outcome, generation or host ledger"));
        }
        if worker.outcome == "host-policy-revoked"
            && (report["module"] != module || report["version"] != version
                || report["module_identity"].as_str() != Some(identity.to_string().as_str())
                || report["source_sha256"] != checked["source_sha256"]
                || report["adapter_identity"] != adapter || report["effect"] != effect
                || report["slot"] != slot || report["currently_authorized"] != false)
        {
            return Err(fatal_failure("worker revoked a different immutable binding"));
        }
        let fields = report.as_object_mut()
            .ok_or_else(|| fatal_failure("invalid host revocation receipt shape"))?;
        fields.insert("schema".into(), serde_json::json!("noble-live-report/v1"));
        fields.insert("stage".into(), serde_json::json!("host-policy"));
        fields.insert("generation".into(), serde_json::json!(self.generation));
        fields.insert("stack".into(), self.last_stack.clone());
        Ok(report)
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
            self.session.worker = Some(match &self.declared {
                Some(state) => super::worker::Engine::start_live_declared(&state.manifest)?,
                None => super::worker::Engine::start_live()?,
            });
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
        if self.verify_source_before_execute {
            if let Some(refusal) = self.source_change_refusal() {
                return Ok(refusal);
            }
        }
        if self.declared.is_some() {
            return self.submit_declared(source);
        }
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
        if self.verify_source_before_execute {
            if let Some(refusal) = self.source_change_refusal() {
                return Ok(refusal);
            }
        }
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
                ReloadDecision::SourceChanged(report) => Ok(report),
                ReloadDecision::SelectedSourceRefused(_) => {
                    Err(fatal_failure("direct definition reported a selected-file source error"))
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
        let encoded = encoding::encode_live_checked(compiled.wat());
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
        let (bridge, signatures, bridge_wat_sha256) = Self::runtime_bridge(&compiled)?;
        let binding = LiveBinaryBinding::new(
            &compiled, &binary, &bridge, bridge_wat_sha256, &signatures, source);
        let ready = self.worker()?.prepare_binary(
            &binary, &bridge, &signatures,
            &binding.wat_sha256, &binding.bridge_wat_sha256,
            compiled.code_span(), source, number)?;
        if ready.outcome != "ready" {
            return Ok(ready);
        }
        Self::check_live_binary_binding(&ready, &binding)?;
        if self.verify_source_before_execute {
            if let Some(refusal) = self.source_change_refusal() {
                let discarded = self.worker()?.discard_prepared_live()?;
                if discarded.outcome != "discarded" {
                    return Err(fatal_failure("live worker failed to discard a stale prepared submission"));
                }
                self.invocation_prior_stack = None;
                return Ok(refusal);
            }
        }
        let code_span = compiled.code_span();
        let output = prepared.output().to_vec();
        #[cfg(all(target_os = "linux", feature = "live-test-barrier"))]
        if let (Some(watch), Some(selected_source_sha256)) =
            (self.watch.as_ref(), self.selected_file_hash.as_deref())
        {
            // The selected bytes passed the final execution gate. A save
            // after this safepoint cannot change the already accepted frame.
            watch::pause_accepted_submission_for_test(
                source, self.generation, watch.sequence(), selected_source_sha256,
            ).map_err(fatal_failure)?;
        }
        self.session
            .compiler
            .commit(compiled)
            .map_err(super::output::Failure::backend)?;
        self.session
            .frontend
            .commit(prepared)
            .map_err(super::output::Failure::source)?;
        let guest_step_budget = self.guest_step_budget;
        let mut report = match guest_step_budget {
            Some(steps) => self.worker()?.execute_with_steps(steps)?,
            None => self.worker()?.execute()?,
        };
        if report.outcome == "normal" {
            Self::check_live_binary_binding(&report, &binding)?;
            self.session.stack = output;
            self.register_live_code(code_span, &report)?;
        }
        let mut invocation: serde_json::Value = serde_json::from_str(&report.json)
            .map_err(|_| fatal_failure("invalid live Wasm execution report"))?;
        let wasm_generation = invocation.get("generation").cloned();
        if let Some(fields) = invocation.as_object_mut() {
            fields.insert(
                "source_generation".into(),
                serde_json::json!(self.generation),
            );
            if report.outcome == "normal" {
                fields.insert(
                    "installed_code_span".into(),
                    serde_json::json!({
                        "start": code_span.start, "length": code_span.length,
                        "generation": code_span.generation,
                    }),
                );
            }
            if let Some(wasm_generation) = wasm_generation {
                fields.insert("wasm_submission_generation".into(), wasm_generation);
            }
        }
        report.json = invocation.to_string();
        Ok(report)
    }

    /// This is an execution gate, not an automatic watch or a claim that a
    /// filesystem pathname remains fresh after this observation. An external
    /// alias or writable mapping need not produce a selected-directory event.
    fn source_change_refusal(&self) -> Option<super::output::Report> {
        let selected = self.selected.as_ref()?;
        let expected = self.selected_file_hash.as_ref()?;
        let (observed, diagnostic) = match snapshot(selected) {
            Ok(snapshot) if snapshot.digest == *expected => {
                return None;
            }
            Ok(snapshot) => (
                Some(snapshot.digest),
                "selected source changed since its last explicit file ACK".to_owned(),
            ),
            Err(problem) => (
                None,
                std::format!("selected source cannot be verified before execution: {problem}"),
            ),
        };
        Some(super::output::Report {
            outcome: "source-change-refused".into(),
            json: serde_json::json!({
                "schema": "noble-live-report/v1",
                "stage": "admission",
                "outcome": "source-change-refused",
                "diagnostic": diagnostic,
                "generation": self.generation,
                "stack": self.last_stack,
                "selected_file_sha256": expected,
                "observed_source_sha256": observed,
                "source_freshness": "changed-or-unverifiable-before-execution",
                "guest_requests": 0,
                "protected_operations": 0,
                "candidate_prepare_requests": 0,
                "prior_stack": "unchanged",
                "prior_namespace": "unchanged",
            }).to_string(),
        })
    }

    fn respond_reload(&mut self, path: &std::path::Path) -> bool {
        let decision = self.reload(path);
        match self.reload_report(decision) {
            Ok(mut outcome) => {
                #[cfg(target_os = "linux")]
                if outcome["outcome"] == "reload-committed" && self.watch.is_some() {
                    // Explicit file selection is not an inotify save or a
                    // producer-fsync receipt. Prior automatic seals no longer
                    // describe this newly selected generation.
                    self.accepted_watch_seal = None;
                    let Some(selected) = self.selected.as_ref().cloned() else {
                        emit(&fatal("committed watch reload lost its selected file"));
                        return false;
                    };
                    if let Err(error) = self.reconcile_explicit_watch_commit(&outcome) {
                        emit(&fatal(&error.message));
                        return false;
                    }
                    if let Err(error) = self.finalize_watch_report(&mut outcome, &selected, None) {
                        emit(&fatal(&error.message));
                        return false;
                    }
                }
                emit(&outcome);
                true
            }
            Err(error) => {
                let report = if self.declared.is_some() {
                    fatal_declared_reload_failure(&error)
                } else {
                    fatal(&error.message)
                };
                emit(&report);
                false
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn reconcile_explicit_watch_commit(
        &mut self,
        report: &serde_json::Value,
    ) -> Result<(), super::output::Failure> {
        let sequence = report["watch_sequence"].as_u64()
            .ok_or_else(|| fatal_failure("explicit watch commit omitted its save sequence"))?;
        let source_hash = report["source_sha256"].as_str()
            .ok_or_else(|| fatal_failure("explicit watch commit omitted its source hash"))?;
        if self.selected_file_hash.as_deref() != Some(source_hash)
            || self.selected_file_identity.is_none()
            || report["generation"].as_u64() != Some(self.generation)
        {
            return Err(fatal_failure(
                "explicit watch commit differs from its selected source identity"));
        }
        #[cfg(feature = "live-test-barrier")]
        watch::pause_explicit_reload_for_test("before-reconcile", source_hash)
            .map_err(fatal_failure)?;
        // A committed A must never be automatically reloaded from a leftover
        // A notification, even if an unsupported in-place edit drifts after
        // its commit. B has a different sequence and stays pending.
        self.watch.as_mut().ok_or_else(||
            fatal_failure("explicit watch commit lost its directory watcher"))?
            .consume_published_pending(sequence).map_err(fatal_failure)
    }

    fn reload_report(
        &self,
        decision: Result<ReloadDecision, super::output::Failure>,
    ) -> Result<serde_json::Value, super::output::Failure> {
        Ok(match decision {
            Ok(ReloadDecision::Committed(report)) => report,
            Ok(ReloadDecision::Refused(problem)) => {
                refusal(self.generation, &self.last_stack, &problem)
            }
            Ok(ReloadDecision::SelectedSourceRefused(error)) => {
                self.selected_source_refusal(&error)?
            }
            Ok(ReloadDecision::HostEffect(problem)) => {
                refusal(self.generation, &self.last_stack, &problem)
            }
            Ok(ReloadDecision::SourceChanged(_)) => {
                return Err(fatal_failure("file-backed reload reported an unrelated source-change refusal"));
            }
            Err(error) => return Err(error),
        })
    }

    #[cfg(target_os = "linux")]
    fn respond_watch(&mut self) -> bool {
        let (selected, sequence, paired_rename) = {
            let watcher = self.watch.as_mut().expect("watch loop requires a watcher");
            if let Err(problem) = watcher.drain() {
                emit(&fatal(&problem));
                return false;
            }
            let Some((sequence, paired_rename)) = watcher.take_pending() else {
                return true;
            };
            (watcher.selected().to_path_buf(), sequence, paired_rename)
        };
        if !paired_rename {
            emit(&refusal(self.generation, &self.last_stack,
                "watched save requires a paired atomic rename of the selected .tmp file"));
            return true;
        }
        let candidate = match snapshot(&selected) {
            Ok(candidate) => candidate,
            Err(problem) => {
                emit(&refusal(self.generation, &self.last_stack, &problem));
                return true;
            }
        };
        let sealed = match self.watch.as_mut().expect("watch loop requires a watcher")
            .seal_snapshot(&candidate, sequence, self.generation)
        {
            Ok(sealed) => sealed,
            Err(problem) => {
                emit(&refusal(self.generation, &self.last_stack, &problem));
                return true;
            }
        };
        self.pending_watch_seal = Some(sealed);
        let decision = self.reload_candidate(selected.clone(), candidate);
        let mut report = match self.reload_report(decision) {
            Ok(report) => report,
            Err(error) => {
                self.pending_watch_seal = None;
                emit(&fatal(&error.message));
                return false;
            }
        };
        if report["outcome"] == "reload-committed" {
            self.accepted_watch_seal = self.pending_watch_seal.take();
            if let Err(error) = self.finalize_watch_report(&mut report, &selected, Some(sequence)) {
                emit(&fatal(&error.message));
                return false;
            }
        } else {
            self.pending_watch_seal = None;
        }
        emit(&report);
        true
    }

    #[cfg(target_os = "linux")]
    fn finalize_watch_report(
        &mut self,
        report: &mut serde_json::Value,
        selected: &std::path::Path,
        expected_sequence: Option<u64>,
    ) -> Result<(), super::output::Failure> {
        let selected_identity = self.selected_file_identity;
        let watcher = self.watch.as_mut()
            .ok_or_else(|| fatal_failure("watched source lost its watch"))?;
        watcher.drain().map_err(fatal_failure)?;
        let sequence = report["watch_sequence"].as_u64();
        let observed = snapshot(selected).ok();
        let sealed_current = expected_sequence.is_none_or(|event_sequence|
            self.accepted_watch_seal.as_ref().is_some_and(|seal|
                observed.as_ref().is_some_and(|after|
                    seal.committed_matches(after, event_sequence, self.generation))));
        let current = sequence == Some(watcher.sequence())
            && expected_sequence.is_none_or(|event_sequence| sequence == Some(event_sequence))
            && sealed_current
            && report["source_freshness"] == "matched-at-postpublish-check"
            && observed.as_ref().is_some_and(|after|
                report["source_sha256"].as_str() == Some(after.digest.as_str())
                    && selected_identity == Some((after.identity, after.directory)));
        let fields = report.as_object_mut()
            .ok_or_else(|| fatal_failure("committed reload report is not an object"))?;
        if expected_sequence.is_some() {
            fields.insert("watch_save_protocol".into(),
                serde_json::json!("paired-temp-rename-host-fsynced-snapshot"));
        }
        if current {
            fields.insert("source_freshness".into(),
                serde_json::json!("matched-at-watch-ack-check"));
        } else {
            // The committed generation is real and cannot be rolled back.
            // No success ACK is issued for bytes no longer selected; the
            // next queued complete save must be handled before next input.
            fields.insert("outcome".into(), serde_json::json!("watch-superseded"));
            fields.insert("source_freshness".into(),
                serde_json::json!("changed-or-unverifiable-at-watch-ack-check"));
            fields.insert("observed_source_sha256".into(),
                serde_json::json!(observed.as_ref().map(|after| after.digest.as_str())));
        }
        Ok(())
    }

    fn selected_source_refusal(
        &self,
        error: &noble_contracts::source::Error,
    ) -> Result<serde_json::Value, super::output::Failure> {
        let report = super::output::Report::source_error(error, 0);
        let mut source: serde_json::Value = serde_json::from_str(&report.json)
            .map_err(|_| fatal_failure("invalid checked source diagnostic"))?;
        let source_fields = source
            .as_object_mut()
            .ok_or_else(|| fatal_failure("checked source diagnostic is not an object"))?;
        let mut value = refusal(
            self.generation,
            &self.last_stack,
            &error.diagnostic().message,
        );
        let fields = value
            .as_object_mut()
            .ok_or_else(|| fatal_failure("live refusal is not an object"))?;
        for name in [
            "source_span",
            "source_location",
            "word_or_join",
            "required_stack",
            "actual_stack",
            "constraint",
            "value_origin_or_unavailable",
        ] {
            if let Some(field) = source_fields.remove(name) {
                fields.insert(name.into(), field);
            }
        }
        fields.insert(
            "source_span_basis".into(),
            serde_json::json!("refused-candidate-file-byte-offsets"),
        );
        Ok(value)
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
        self.reload_candidate(selected, candidate)
    }

    fn reload_candidate(
        &mut self,
        selected: std::path::PathBuf,
        candidate: Snapshot,
    ) -> Result<ReloadDecision, super::output::Failure> {
        if self.selected.as_ref().is_some_and(|prior| prior != &selected) {
            return Ok(ReloadDecision::Refused(
                "reload path differs from selected source".into(),
            ));
        }
        #[cfg(target_os = "linux")]
        if self.watch.is_some() && candidate.identity.2 != 1 {
            return Ok(ReloadDecision::Refused(
                "watched source has a hardlink alias, not an immutable complete-save file".into(),
            ));
        }
        #[cfg(target_os = "linux")]
        let watch_sequence = if let Some(watcher) = self.watch.as_mut() {
            watcher.drain().map_err(fatal_failure)?;
            Some(watcher.sequence())
        } else {
            None
        };
        #[cfg(not(target_os = "linux"))]
        let watch_sequence = None;
        #[cfg(target_os = "linux")]
        if !self.sealed_candidate_is_current(&candidate, watch_sequence) {
            return Ok(ReloadDecision::Refused(
                "watched source or host seal changed before reload preparation".into()));
        }
        if self.declared.is_some() {
            return self.reload_declared_candidate(selected, candidate, watch_sequence);
        }
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
            Err(error) => return Ok(ReloadDecision::SelectedSourceRefused(error)),
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
            watch_sequence,
        )?;
        if matches!(result, ReloadDecision::Committed(_)) {
            self.selected = Some(selected);
            self.selected_definition = Some(definition_name);
            self.selected_file_hash = Some(candidate.digest);
            self.selected_file_identity = Some((candidate.identity, candidate.directory));
        }
        Ok(result)
    }

    fn candidate_is_current(
        &mut self,
        selected: &std::path::Path,
        candidate: &Snapshot,
        watch_sequence: Option<u64>,
    ) -> Result<bool, super::output::Failure> {
        #[cfg(target_os = "linux")]
        if !self.sealed_candidate_is_current(candidate, watch_sequence) {
            return Ok(false);
        }
        if !same_snapshot(selected, candidate) {
            return Ok(false);
        }
        #[cfg(target_os = "linux")]
        if let Some(sequence) = watch_sequence {
            let watcher = self.watch.as_mut()
                .ok_or_else(|| fatal_failure("watched source lost its watch"))?;
            watcher.drain().map_err(fatal_failure)?;
            if watcher.sequence() != sequence {
                return Ok(false);
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = watch_sequence;
        let selected_current = same_snapshot(selected, candidate);
        #[cfg(target_os = "linux")]
        return Ok(selected_current
            && self.sealed_candidate_is_current(candidate, watch_sequence));
        #[cfg(not(target_os = "linux"))]
        Ok(selected_current)
    }

    #[cfg(target_os = "linux")]
    fn sealed_candidate_is_current(
        &self,
        candidate: &Snapshot,
        watch_sequence: Option<u64>,
    ) -> bool {
        self.pending_watch_seal.as_ref().is_none_or(|seal|
            watch_sequence.is_some_and(|sequence|
                seal.matches(candidate, sequence, self.generation)))
    }

    /// File reload, guest proposal and direct definition admission share one
    /// checked dependent-rebuild and binary-staged publication path. File
    /// reloads recheck their exact staged snapshot; opt-in direct definitions
    /// and proposals instead check the last explicitly ACKed selected bytes.
    fn install_definition(
        &mut self,
        definition: noble_contracts::source::Prepared,
        source: &[u8],
        number: u64,
        file: Option<(&std::path::Path, &Snapshot)>,
        watch_sequence: Option<u64>,
    ) -> Result<ReloadDecision, super::output::Failure> {
        if let Some(problem) = self.retained_generation_budget_refusal() {
            return Ok(ReloadDecision::Refused(problem));
        }
        if self
            .session
            .frontend
            .core_definition_requires_host_effects(&definition)
        {
            return Ok(ReloadDecision::HostEffect(
                "live profile has no host-effect grant for this definition".into(),
            ));
        }
        let preview = match self.session.frontend.preview_core_rebuild(&definition) {
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
        let encoded = encoding::encode_live_checked(compiled.wat());
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
        let (bridge, signatures, bridge_wat_sha256) = Self::runtime_bridge(&compiled)?;
        let binding = LiveBinaryBinding::new(
            &compiled, &binary, &bridge, bridge_wat_sha256, &signatures, source);
        let staged = self.worker()?.stage_binary(
            &binary, &bridge, &signatures,
            &binding.wat_sha256, &binding.bridge_wat_sha256,
            compiled.code_span(), source, number)?;
        if staged.outcome != "ready" {
            if staged.outcome == "reload-refused" {
                return Ok(ReloadDecision::Refused(report_diagnostic(&staged)));
            }
            return Err(fatal_failure(std::format!(
                "live binary staging failed: {}",
                staged.json
            )));
        }
        Self::check_live_binary_binding(&staged, &binding)?;
        if let Some((selected, candidate)) = file {
            if !self.candidate_is_current(selected, candidate, watch_sequence)? {
                let discarded = self.worker()?.discard_binary()?;
                if discarded.outcome != "discarded" {
                    return Err(fatal_failure(
                        "live worker did not discard superseded binary",
                    ));
                }
                return Ok(ReloadDecision::Refused(
                    "source changed or another complete save superseded reload staging".into(),
                ));
            }
        } else if self.verify_source_before_execute {
            if let Some(report) = self.source_change_refusal() {
                let discarded = self.worker()?.discard_binary()?;
                if discarded.outcome != "discarded" {
                    return Err(fatal_failure(
                        "live worker did not discard a stale staged definition",
                    ));
                }
                return Ok(ReloadDecision::SourceChanged(report));
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
        Self::check_live_binary_binding(&published, &binding)?;
        let publication: serde_json::Value = serde_json::from_str(&published.json)
            .map_err(|_| fatal_failure("invalid source-bound live publication receipt"))?;
        let installed_module = publication["module"].clone();
        let stack = report_stack(&published)
            .ok_or_else(|| fatal_failure("published reload omitted stack"))?;
        if stack != self.last_stack {
            return Err(fatal_failure("published reload changed the retained stack"));
        }
        let code_span = compiled.code_span();
        self.session
            .compiler
            .commit(compiled)
            .map_err(super::output::Failure::backend)?;
        self.session.frontend = preview;
        self.session.submissions = number;
        self.generation = next_generation;
        self.register_live_code(code_span, &published)?;
        let retired_code_spans = self.retire_unowned_live_code()?;
        // Publication is irreversible: a later rename cannot be reported as a
        // recoverable refusal. ACK the pinned bytes actually installed, and
        // disclose a bounded observation of the path after publication. Even
        // a match at this instant is not a filesystem watch or an ACK-time
        // guarantee that the path still names the installed version.
        let source_freshness = match file {
            Some((selected, candidate)) => match snapshot(selected) {
                Ok(after)
                    if after.digest == candidate.digest
                        && after.identity == candidate.identity
                        && after.directory == candidate.directory =>
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
            "watch_sequence": watch_sequence,
            "source_sha256": crate::workflow::intrinsic::sha256(source),
            "module": installed_module,
            "source_version": "pinned-at-prepublish-check",
            "source_freshness": source_freshness,
            "installed_code_span": {
                "start": code_span.start, "length": code_span.length,
                "generation": code_span.generation,
            },
            "retired_code_spans": retired_code_spans,
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

fn same_snapshot(path: &std::path::Path, candidate: &Snapshot) -> bool {
    snapshot(path).is_ok_and(|after| after.digest == candidate.digest
        && after.identity == candidate.identity && after.directory == candidate.directory)
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
    let directory_before = directory.metadata().map_err(|error| error.to_string())?;
    let parent_before = std::fs::symlink_metadata(parent).map_err(|error| error.to_string())?;
    let directory_identity = (directory_before.dev(), directory_before.ino());
    if !parent_before.is_dir()
        || directory_identity != (parent_before.dev(), parent_before.ino())
    {
        return Err("selected source directory changed before snapshot".into());
    }
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
    let directory_after = directory.metadata().map_err(|error| error.to_string())?;
    let parent_after = std::fs::symlink_metadata(parent).map_err(|error| error.to_string())?;
    if !after.is_file()
        || !opened_after.is_file()
        || !parent_after.is_dir()
        || directory_identity != (directory_after.dev(), directory_after.ino())
        || directory_identity != (parent_after.dev(), parent_after.ino())
        || source_identity(&before) != source_identity(&opened_before)
        || source_identity(&opened_before) != source_identity(&opened_after)
        || source_identity(&opened_after) != source_identity(&after)
    {
        return Err("source changed during bounded snapshot".into());
    }
    Ok(Snapshot {
        digest: crate::workflow::intrinsic::sha256(&bytes),
        bytes,
        identity: source_identity(&after),
        directory: directory_identity,
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

fn fatal_declared_reload_failure(error: &super::output::Failure) -> serde_json::Value {
    let mut report = fatal(&error.message);
    let phase = match error.context.stage {
        "declared-worker-start" => "worker-start",
        "declared-worker-stage" => "declared-stage",
        "declared-worker-publish" => "declared-publish",
        _ => "unknown",
    };
    report["startup_phase_code"] = serde_json::json!(phase);
    report
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

    #[test]
    fn discarded_prepared_live_binary_does_not_block_next_real_execution() {
        let source = b"21";
        let frontend = noble_contracts::source::Session::new_live("");
        let checked = frontend
            .prepare(source, &[], super::super::SOURCE_LIMITS)
            .unwrap_or_else(|_| panic!("accepted source"));
        let compiler = noble_wasm::source::Compiler::new_live();
        let compiled = compiler
            .prepare(checked.submission().expect("checked expression"))
            .unwrap_or_else(|_| panic!("compiled source"));
        let binary = super::encoding::encode_live_checked(compiled.wat())
            .unwrap_or_else(|_| panic!("checked live binary"));
        let (bridge, signatures, bridge_wat_sha256) = super::Live::runtime_bridge(&compiled)
            .unwrap_or_else(|_| panic!("checked source-free Core bridge"));
        let wat_sha256 = crate::workflow::intrinsic::sha256(compiled.wat());
        let mut worker = super::super::worker::Engine::start_live()
            .unwrap_or_else(|_| panic!("resident live worker"));
        let first = worker.prepare_binary(
            &binary, &bridge, &signatures, &wat_sha256, &bridge_wat_sha256,
            compiled.code_span(), source, 1)
            .unwrap_or_else(|_| panic!("first prepared binary"));
        assert_eq!(first.outcome, "ready");
        let discarded = worker.discard_prepared_live()
            .unwrap_or_else(|_| panic!("discarded prepared binary"));
        assert_eq!(discarded.outcome, "discarded");
        let report: serde_json::Value = serde_json::from_str(&discarded.json).expect("worker JSON");
        assert_eq!(report["guest_requests"], 0);
        assert_eq!(report["protected_operations"], 0);
        let second = worker.prepare_binary(
            &binary, &bridge, &signatures, &wat_sha256, &bridge_wat_sha256,
            compiled.code_span(), source, 2)
            .unwrap_or_else(|_| panic!("subsequent prepared binary"));
        assert_eq!(second.outcome, "ready");
        let executed = worker.execute().unwrap_or_else(|_| panic!("guest invocation"));
        assert_eq!(executed.outcome, "normal");
        let report: serde_json::Value = serde_json::from_str(&executed.json).expect("worker JSON");
        assert_eq!(report["stack"][0]["type"], "I64");
        assert_eq!(report["stack"][0]["value"], "21");
    }

    #[test]
    fn malformed_binary_stage_preserves_worker_for_real_execution() {
        let source = b"21";
        let frontend = noble_contracts::source::Session::new_live("");
        let checked = frontend
            .prepare(source, &[], super::super::SOURCE_LIMITS)
            .unwrap_or_else(|_| panic!("accepted source"));
        let compiler = noble_wasm::source::Compiler::new_live();
        let compiled = compiler
            .prepare(checked.submission().expect("checked expression"))
            .unwrap_or_else(|_| panic!("compiled expression"));
        let binary = super::encoding::encode_live_checked(compiled.wat())
            .unwrap_or_else(|_| panic!("checked live binary"));
        let (bridge, signatures, bridge_wat_sha256) = super::Live::runtime_bridge(&compiled)
            .unwrap_or_else(|_| panic!("checked source-free Core bridge"));
        let wat_sha256 = crate::workflow::intrinsic::sha256(compiled.wat());
        let mut worker = super::super::worker::Engine::start_live()
            .unwrap_or_else(|_| panic!("resident live worker"));
        let mut malformed = binary.clone();
        malformed[0] ^= 1;
        let refusal = worker.stage_binary(
            &malformed, &bridge, &signatures, &wat_sha256, &bridge_wat_sha256,
            compiled.code_span(), source, 1,
        ).unwrap_or_else(|_| panic!("malformed stage must refuse nonterminally"));
        assert_eq!(refusal.outcome, "reload-refused");
        let refused: serde_json::Value =
            serde_json::from_str(&refusal.json).expect("refusal JSON");
        assert_eq!(refused["generation"], 0);
        assert_eq!(refused["stack"], serde_json::json!([]));
        assert_eq!(refused["guest_requests"], 0);
        assert_eq!(refused["protected_operations"], 0);
        assert_eq!(refused["session_state"], "retained");

        let prepared = worker.prepare_binary(
            &binary, &bridge, &signatures, &wat_sha256, &bridge_wat_sha256,
            compiled.code_span(), source, 2,
        ).unwrap_or_else(|_| panic!("valid preparation after stage refusal"));
        assert_eq!(prepared.outcome, "ready");
        let published = worker.execute()
            .unwrap_or_else(|_| panic!("real guest execution after stage refusal"));
        assert_eq!(published.outcome, "normal");
        let result: serde_json::Value =
            serde_json::from_str(&published.json).expect("execution JSON");
        assert_eq!(result["generation"], 1);
        assert_eq!(result["stack"][0]["value"], "21");
        assert_eq!(result["guest_requests"], 0);
        assert_eq!(result["protected_operations"], 0);
        let registered = worker.register_live_code(compiled.code_span())
            .unwrap_or_else(|_| panic!("real installed guest registration"));
        assert_eq!(registered.outcome, "code-registered");
    }
}
