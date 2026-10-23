//! Source/session orchestration; execution occurs only in the selected Wasm engine.

mod arguments;
pub(crate) mod artifact;
pub(crate) mod companions;
mod declared;
pub(crate) mod editor;
pub(crate) mod entry;
pub(crate) mod framing;
pub(crate) mod live;
pub(crate) mod live_slot;
pub(crate) mod output;
mod report;
pub(crate) mod worker;
mod worker_candidate;
pub(crate) mod worker_package;
pub(crate) mod worker_program;
mod worker_program_guest;
mod worker_program_transition;
mod worker_reflect;
pub(crate) mod worker_scalar;
mod worker_send;
pub(crate) mod worker_shell;
pub(crate) mod worker_typed;
pub(crate) mod worker_value;

pub(crate) const SOURCE_LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

/// Evidence ingress has its own 512 KiB bound; source preparation stays 64 KiB.
pub(crate) const EVIDENCE_LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 524_288,
    ..SOURCE_LIMITS
};

pub const USAGE: &str = "usage:
  noble live repl [--source ABS_PATH] [--engine v8|interpreter] [--self-edit NAME --expect-generation N] [--verify-source-before-execute] [--watch] [--declared-modules --bindings MANIFEST] [--max-retained-generations N]
  live repl: newline-delimited source, :reload ABS_PATH, :grant-self-edit NAME N,
    or :release-observed-roots (relinquishes all previously reported raw cell handles);
    interpreter is unavailable
  noble live watch --source ABS_PATH [--engine v8] [--declared-modules --bindings MANIFEST] [--max-retained-generations N]
  live watch: Linux inotify complete-save rename + checked file ACK; in-place writes have no watch ACK
  noble live slot --engine v8 --authority ABS_PATH
  live slot: bounded JSONL define/install/candidate/publish/delete/rollback/invoke/reflect/trace/
    replay/policy/discard/release-program/release-replay/hold-checkpoint/resume-checkpoint;
    stdin must be an anonymous OS pipe whose launcher retains the write end exclusively and
    never delegates it to guest code; pipe metadata alone does not authenticate its writer;
    a checked saved Program owner may be published instead of its expression wrapper,
    but a dynamically composed owner without checked installed target metadata cannot;
    authority JSON separately selects owner, exact source SHA-256 allowlist, nominal resources,
    ordered slot contracts, effect ceiling and scoped grants. Proof-required publication refuses
    without independent target evidence; source text is never a control command.
  noble editor analyze|admit JSON_FILE [--emit NEW_DIR (admit only)]
  JSON_FILE: editor AST format 1, nodes integer/boolean/word/quotation/hole
  noble worker scalar SOURCE --state I64 --delta I64
  Scalar worker precursor: independently check I64 I64 -- I64 ! {}, run one
  bounded source-backed guest invocation, report its exact local outcome.
  noble worker program build SOURCE --name NAME --effect-ceiling pure
  Opted source-bound named Program builder: check and compile a module with two
  reachable, compatible named Programs once, then
  adapt, select, duplicate and compose typed handles under an explicit bound.
  This local builder does not execute a guest, import/export packages or grant effects.
  noble worker program check SOURCE --name NAME --effect-ceiling pure [--work N] [--schema SCHEMA]
  Admission only: check and compile a pure named Program under one
  host-selected request budget (default 50000000); every checking stage and
  meter draws from the same share, and nothing is installed or run.
  --schema selects an exact WorkerData@1 module, checked first under the same
  budget; NAME must then be State Event -- State List<Action> ! {}.
  noble worker program guest SOURCE --name NAME --operand I64 --effect-ceiling pure
  A pure Program may use signed, scoped self recursion (`signature` plus
  a self call); host-bound Programs may not.
  Selected pure WIT owner invokes the actual checked named Program cell in a
  fresh, bounded source Wasm instance. Guest source and component are rebuilt;
  the scoped Worker gates retain the execution evidence.
  noble worker program guest-package PACKAGE --operand I64 --effect-ceiling pure
  Freshly import a checked pure package graph and invoke its selected installed
  Program cell through the same WIT owner; package bytes are not executable.
  noble worker program prepare-transition SOURCE --name NAME --schema SCHEMA --out PACKAGE --effect-ceiling pure
  Explicit Worker Program v2 host preparation: check the schema, root, closure
  and transition interface, compile once under the interruptible profile,
  encode and re-admit the package, then publish only a new destination file.
  It starts no engine and reports a separate service trace.
  noble worker program guest-transition SOURCE --name NAME --schema SCHEMA --state STATE --event EVENT --effect-ceiling pure
  noble worker program guest-transition-package PACKAGE --schema SCHEMA --state STATE --event EVENT --effect-ceiling pure
  Run a schema-selected State Event -- State List<Action> transition through
  a fixed WIT guest. WIT carries canonical frame strings and a move-only owner;
  the host validates every nominal against its selected WorkerData@1 catalog,
  injects typed runtime cells, observes State and List<Action>, and publishes
  typed output only on normal return. Source and package routes are equivalent.
  noble worker program import PACKAGE --effect-ceiling pure [--work N] [--schema SCHEMA]
  Freshly recheck a finite local v2 package's source, complete named closure
  and checked recipe; Text aliases require fresh installed-cell rechecking.
  Import is inert and is not an executable guest. Export has no CLI command:
  only the test embedder's host code builds a grant, for its own fixture and
  destination; file JSON cannot grant it.
  A test.emit package/ceiling is refused: the host-only bound adapter and
  revocable effect policy require an explicit embedder, never a CLI argument.
  noble worker program test-host emit|emit-wrong|emit-ledger-denied|emit-request-denied|emit-panic|emit-revoked|wit-export|core-export|builders < FIXTURE_SOURCE
  noble worker program test-host candidate|candidate-fault < CANDIDATE_JSON
  Consumer acceptance of one untrusted noble-candidate/v1 document: the host
  selects the session, the expected interface (its own pure Program's), the
  bootstrap environment and the limits; the document carries only the
  candidate. The kernel checker runs at most once and only an accepted
  candidate is published. candidate-fault injects a host fault inside the
  acceptance stage.
  noble worker program test-host send-observe|send-observe-rows|send-observe-diagnostic < FIXTURE_SOURCE
  Run the built-in three-request test.send fixture once. The host authorizes
  each request against its own exact-plan grants, admits and starts it through
  the kernel authority table and hands it to a synthetic in-process peer; the
  guest learns no outcome. A lost acknowledgement is followed by the host's
  cancellation. The record keeps invocation and attempt correlation, counts
  and kernel outcomes; rows beyond the selected bound (6 for -rows) and a
  diagnostic beyond its bound (48 bytes for -diagnostic) are marked omitted.
  noble worker program test-host transition-normal|transition-cancel|transition-deadline|transition-trap|transition-exhausted < FIXTURE_SOURCE
  Run host-selected controls through the same transition WIT guest. Each
  reports source interruption checks (maximum interval eight), distinct
  primary outcomes, and no typed output for non-normal outcomes.
  noble worker program test-host export|export-denied|optimizer|reflect-prepare < FIXTURE_SOURCE
  Check the built-in recursive Program with a Text capture. export builds the
  grant for its own destination from the checked source and capture digests
  and prints the package with the experimental program-value identity;
  export-denied refuses the same subject and emits no bytes. optimizer checks
  the fixture twice and attests each under a different Cranelift level,
  reporting artifact digests and identities. reflect-prepare reflects every
  installed cell through the runtime's observation ABI, requires the decoded
  recipe to equal the checked recipe, renders it as a module with synthesized
  names and prepares it again. Nothing invokes the body.
  noble worker program test-host rebound < FIXTURE_SOURCE
  In one retained session commit Dep@1, Dep@2, import Dep@1 as dep and the
  fixture root, which calls dep.dec; admit the root, rebind dep to Dep@2,
  admit the root again, try it with only Dep@2 selected, and admit a control
  root registered after the rebind. Reports each identity.
  noble worker program test-host emit-import < PACKAGE
  Import an untrusted package under the same preselected test.emit binding;
  only that exact host-contract row can match, and importing issues nothing.
  builders adapts the fixture's pure Program to the bound Program's
  {test.emit} witness, then lists, selects, duplicates and composes both
  without compiling, invoking or issuing anything.
  Dedicated finite synthetic test embedder: built-in exact fixture source,
  checked owner, adapter, destination and epoch are chosen before bounded
  untrusted stdin; source mismatch denies before any issue. Only an installed
  allowlisted test.emit cell may be issued. Both export paths always deny.
  The ledger-byte-denial control retains four committed appends before the
  fifth request fails without delivering a result; a caught after-append host
  panic retains the prior append. The separate request-count control uses one
  checked source cell and 257 host-selected iterations of the existing WIT
  guest under one task, Store and issuer; the scoped real child observed 256
  protected appends, a denied 257th request and no delivered final I64.
  Canonical WORKER acceptance remains open.
  emit-revoked revokes an issued cell before invocation.
  It accepts no authority document or caller-selected export destination.
  noble worker typed PACKAGE --schema SCHEMA --state STATE_JSON --event EVENT_JSON
  Typed worker: host-select a source-checked WorkerData@1 module and two canonical
  nominal values; check one exact local no-capture package and exported transition
  before compiling and running one bounded State Event -- State List<Action> guest.
  noble worker prepare SOURCE --schema SCHEMA --out PACKAGE
  Explicit host preparation service: check candidate SOURCE against the host-selected
  WorkerData@1 schema and the exact State Event -- State List<Action> ! {} transition,
  compile it once, then write a new local package (never overwriting). Its JSON
  trace counts only service effects; it starts no engine and runs no candidate body.
  noble worker shell PACKAGE --schema SCHEMA --state STATE --submit EVENT --cancel EVENT --authority HOST_GRANT
  Local synthetic-send shell: authorize checked inert Send against a separately
  selected exact grant, then cancel a pinned native task before late completion.
  No network send or portable package/export authority is provided.
  noble admit-artifact WASM --effects CLAIMS_JSON [--source HOST_SOURCE] [--allow-effects test.emit,test.abort,test.clock] [--opt off|on]
  CLAIMS_JSON: {\"claimed_effects\":[]}; optional source/digest/allowed/trusted_correspondence are ignored
  --source and --allow-effects are selected by the host invoker, never by candidate claims.
  Admission rebuilds selected source and compares final Wasm bytes before fresh isolated execution.
  noble run SOURCE [--opt off|on] [--emit NEW_DIR]
  noble compile SOURCE [--text-byte-cursor] [--input-type I64|Bool|Text|Unit ...]
  noble compile SOURCE --declared-modules --bindings HOST_FILE [--module MODULE_FILE ...] [--input-type MODULE@VERSION.TYPE ...]
  noble compile SOURCE --declared-modules --signed-self-recursion --bindings HOST_FILE [--module MODULE_FILE ...]
  noble compile SOURCE --declared-modules --text-byte-cursor [--signed-self-recursion] [--module MODULE_FILE ...]
  noble session [--text-byte-cursor] [--framed] [--opt off|on] [--emit NEW_DIR]
  noble session --framed --declared-modules --bindings HOST_FILE [--opt off|on] [--emit NEW_DIR]
  noble session --framed --declared-modules --signed-self-recursion --bindings HOST_FILE [--opt off|on] [--emit NEW_DIR]
  noble session --framed --declared-modules --text-byte-cursor [--signed-self-recursion] [--opt off|on] [--emit NEW_DIR]
  Signed-Declared-Text-v1 framed control: :release-observed-roots STORE_UUID OBSERVATION_ID
  sends no source submission; after the caller has compared an observed aggregate,
  submit a checked `drop` expression first, then release that report's exact
  source_store_id/source_observation_id. The worker releases every nested
  observed handle in that receipt. A wrong, stale or foreign owner is refused.
  An idle ownerless epoch retires checked Program/Recipe cells and source code
  without restarting the session or changing the 4096-cell runtime maximum.
  HOST_FILE: bind MODULE@VERSION test.emit ADAPTER Text -- ! test.emit allow|deny
             bind MODULE@VERSION test.clock ADAPTER -- I64 ! test.clock allow|deny script I64[,I64...]
  Declared-Modules-v1 module/import/definition units link without running guest code.
  Preparation limits: --source-bytes N (0..=65536) --source-nodes N --source-depth N --source-work N
  --retained-source-bytes N (1..=262144) requires declared compile or session;
  sets the cumulative retained original-source budget independently of the per-input
  --source-bytes limit; without it the cumulative budget equals --source-bytes.

Core-Bootstrap uses the selected managed-linear-memory backend.
--text-byte-cursor alone selects Text-Byte-Cursor-v1 for ordinary compile/session;
it adds pure text.byte, retaining Text and returning unsigned byte 0..255,
EOF -1 at the byte length, negative-offset -2 or past-end -3. Increment
the offset only after a nonnegative byte. With --declared-modules it selects a
new pure declared-text profile with text.byte, signed I64 <, and no host
bindings or ambient test effects. --signed-self-recursion also requires
--declared-modules, an exported self word and an explicit fixed signature.
Existing ordinary declared modules retain their explicit --bindings file.
run checks one complete file before executing any of it.
session reads one source submission per line; --framed reads a decimal byte count,
newline, then exactly that many source bytes (permits multiline and invalid UTF-8).
Declared framed sessions drain a complete oversized frame up to 262144 bytes in
bounded chunks, refuse it without checking or executing it, then read the next
header; larger, malformed or incomplete frames terminate the input stream.
Reports are JSON lines, ordered bottom-to-top. A static refusal preserves the
session; a runtime trap or exhausted execution limit ends it with its effect prefix.
--emit retains source, WAT, Wasm, tool bindings and runtime observations in a new
directory. No file is overwritten. Missing or incompatible tools fail closed.
compile emits independently accepted WAT, not an execution report; input types
are ordered bottom-to-top and values are supplied only after module compilation.";

struct Session {
    frontend: noble_contracts::source::Session,
    declared: Option<noble_contracts::source::ModuleSession>,
    compiler: noble_wasm::source::Compiler,
    stack: std::vec::Vec<noble_kernel::types::Ty>,
    worker: Option<worker::Engine>,
    submissions: u64,
}

impl Session {
    fn new(options: &arguments::Options) -> Result<Self, output::Failure> {
        let declared = if options.declared_modules {
            let bindings = options
                .manifest
                .as_ref()
                .map(|manifest| manifest.operations())
                .unwrap_or_default();
            let retained = options.retained_source_limit();
            Some(attempt!(if options.declared_text() {
                noble_contracts::source::ModuleSession::new_declared_text(
                    &bindings,
                    options.signed_self_recursion,
                    retained,
                )
            } else if options.signed_self_recursion {
                noble_contracts::source::ModuleSession::new_signed_self(&bindings, retained)
            } else {
                noble_contracts::source::ModuleSession::new(&bindings, retained)
            }
            .map_err(output::Failure::source)))
        } else {
            None
        };
        Ok(Self {
            frontend: if options.text_byte_cursor {
                noble_contracts::source::Session::new_text_cursor()
            } else {
                noble_contracts::source::Session::new()
            },
            declared,
            compiler: if options.declared_text() {
                noble_wasm::source::Compiler::new_declared_text()
            } else if options.text_byte_cursor {
                noble_wasm::source::Compiler::new_text_cursor()
            } else {
                noble_wasm::source::Compiler::new()
            },
            stack: std::vec::Vec::new(),
            worker: None,
            submissions: 0,
        })
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; submit prepares and commits allocated source/compiler state and exchanges commands with the selected engine process. These runtime library and I/O operations cannot be const."
    )]
    fn submit(
        &mut self,
        source: &[u8],
        options: &arguments::Options,
    ) -> Result<output::Report, output::Failure> {
        attempt!(self.advance_submission());
        if self.declared.is_some() {
            return self.submit_declared(source, options);
        }
        let prepared = match self.frontend.prepare(source, &self.stack, options.limits) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Ok(output::Report::source_error_for_profile(
                    &error,
                    self.submissions,
                    options.text_byte_cursor,
                ))
            }
        };
        if prepared.is_definition() {
            attempt!(self
                .frontend
                .commit(prepared)
                .map_err(output::Failure::source));
            return Ok(output::Report::defined_for_profile(
                self.submissions,
                options.text_byte_cursor,
            ));
        }
        let submission = attempt!(prepared.submission().ok_or_else(|| {
            output::Failure::new(
                output::ErrorContext {
                    stage: "acceptance",
                    outcome: "internal-failure",
                },
                "missing expression candidate",
            )
        }));
        let compiled = match self.compiler.prepare(submission) {
            Ok(compiled) => compiled,
            Err(error) => {
                return Ok(output::Report::backend_error_for_profile(
                    error,
                    self.submissions,
                    options.text_byte_cursor,
                ))
            }
        };
        if self.worker.is_none() {
            self.worker = Some(attempt!(worker::Engine::start(options)));
        }
        let engine = attempt!(self.worker.as_mut().ok_or_else(|| {
            output::Failure::new(
                output::ErrorContext {
                    stage: "wasm",
                    outcome: "internal-failure",
                },
                "missing engine worker",
            )
        }));
        let prepared_report = attempt!(engine.prepare(compiled.wat(), source, self.submissions));
        if prepared_report.outcome != "ready" {
            return Ok(prepared_report);
        }
        let output_types = prepared.output().to_vec();
        attempt!(self
            .compiler
            .commit(compiled)
            .map_err(output::Failure::backend));
        attempt!(self
            .frontend
            .commit(prepared)
            .map_err(output::Failure::source));
        let report = attempt!(engine.execute());
        if report.outcome == "normal" {
            self.stack = output_types;
        }
        Ok(report)
    }

    fn advance_submission(&mut self) -> Result<u64, output::Failure> {
        self.submissions = attempt!(self.submissions.checked_add(1).ok_or_else(|| {
            output::Failure::new(
                output::ErrorContext {
                    stage: "session",
                    outcome: "exhausted",
                },
                "submission counter exhausted",
            )
        }));
        Ok(self.submissions)
    }
}
