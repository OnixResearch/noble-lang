//! Policy pin selection and observation (`noble-mc2-build/v4`).
//!
//! A `v1` report keeps its exact field set and a `v3` report keeps its exact
//! field set plus the caller-selected Lean toolchain binding. `--report-schema
//! v4` additionally names a reviewed pin document (`--pins FILE`) and binds
//! every identity the artifact-correspondence envelope names: the running CLI's
//! bytes, the embedded runtime helper set, the selected Lean release binary and
//! contract library, the selected node, wasm-tools and wasm-opt executables,
//! and the optimizer mode. [`select`] reads the selection and [`observe`]
//! digests the running build; [`verify`] refuses the report unless each
//! observed identity equals the selection, so a v4 receipt cannot be produced
//! by a different CLI, helper set or toolchain than the one it names.

use crate::workflow::encoding::Json;
use crate::workflow::output::Failure;

/// The pin document schema a v4 selection must name.
pub(super) const SCHEMA: &str = "noble-build-report-pins/v1";

/// The declared root every pin document must name for the helper set.
pub(super) const HELPER_ROOT: &str = "crates/noble-wasm/runtime";

/// The only optimizer mode this build lane produces: assembly is
/// `wasm-tools parse`/`validate` and no optimizer runs.
pub(super) const OPT_MODE: &str = "off";

const MAX_PIN_BYTES: u64 = 1_048_576;
// The shipped CLI is tens of megabytes; a debug test binary with all
// dependencies linked is larger, so the bound only refuses a pathological file.
const MAX_EXECUTABLE_BYTES: u64 = 1_073_741_824;
const MAX_WORKER_TOOLS_BYTES: u64 = 65_536;
const CLI_PATH: &str = "/proc/self/exe";
const CODE_SELECTION: &str = "build-pin-selection";
const CODE_MISMATCH: &str = "build-pin-mismatch";

fn selection(message: std::string::String) -> Failure {
    Failure::unsupported(CODE_SELECTION, message)
}

fn mismatch(identity: &str, selected: &str, observed: &str) -> Failure {
    Failure::unsupported(
        CODE_MISMATCH,
        std::format!(
            "{identity}: the selection names {selected} but this build observed {observed}"
        ),
    )
}

fn require_digest(identity: &str, value: &str) -> Result<(), Failure> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Ok(());
    }
    Err(selection(std::format!(
        "{identity} must be 64 lowercase hexadecimal digits"
    )))
}

/// One selected executable identity: the immutable path, its declared version
/// and the SHA-256 of its bytes.
pub(crate) struct ToolIdentity {
    pub(crate) path: std::string::String,
    pub(crate) version: std::string::String,
    pub(crate) sha256: std::string::String,
    /// The engine version the selected worker verified, for Node only.
    pub(crate) v8: Option<std::string::String>,
}

/// The three subprocess tools one assembly uses.
pub(crate) struct ToolSet {
    pub(crate) node: ToolIdentity,
    pub(crate) wasm_tools: ToolIdentity,
    pub(crate) wasm_opt: ToolIdentity,
}

/// One reviewed pin document, digested as read.
pub(crate) struct Selection {
    pub(crate) document_sha256: std::string::String,
    pub(super) cli_sha256: std::string::String,
    pub(super) helper_count: u64,
    pub(super) helper_set_sha256: std::string::String,
    pub(super) lean_binary_sha256: std::string::String,
    pub(super) lean_toolchain: std::string::String,
    pub(super) lean_commit: std::string::String,
    pub(super) library_blake3: std::string::String,
    pub(super) tools: ToolSet,
    pub(super) opt_mode: std::string::String,
}

/// The identities this build actually observed.
pub(crate) struct Observed {
    pub(super) cli_sha256: std::string::String,
    cli_path: std::string::String,
    helpers: std::vec::Vec<(std::string::String, std::string::String)>,
    helper_set_sha256: std::string::String,
    pub(super) lean_binary_sha256: std::string::String,
    lean_toolchain: &'static str,
    lean_commit: &'static str,
    pub(super) library_blake3: std::string::String,
    pub(crate) tools: Option<ToolSet>,
}

#[derive(serde::Deserialize)]
#[serde(crate = "::serde", deny_unknown_fields)]
struct Document {
    schema: std::string::String,
    opt_mode: std::string::String,
    cli: CliPin,
    helpers: HelperPin,
    lean: LeanPin,
    tools: ToolPins,
}

#[derive(serde::Deserialize)]
#[serde(crate = "::serde", deny_unknown_fields)]
struct CliPin {
    sha256: std::string::String,
}

#[derive(serde::Deserialize)]
#[serde(crate = "::serde", deny_unknown_fields)]
struct HelperPin {
    root: std::string::String,
    count: u64,
    set_sha256: std::string::String,
}

#[derive(serde::Deserialize)]
#[serde(crate = "::serde", deny_unknown_fields)]
struct LeanPin {
    binary_sha256: std::string::String,
    toolchain: std::string::String,
    commit: std::string::String,
    library_blake3: std::string::String,
}

#[derive(serde::Deserialize)]
#[serde(crate = "::serde", deny_unknown_fields)]
struct ToolPins {
    node: ToolPin,
    wasm_tools: ToolPin,
    wasm_opt: ToolPin,
}

#[derive(serde::Deserialize)]
#[serde(crate = "::serde", deny_unknown_fields)]
struct ToolPin {
    path: std::string::String,
    version: std::string::String,
    sha256: std::string::String,
}

#[derive(serde::Deserialize)]
#[serde(crate = "::serde", deny_unknown_fields)]
struct WorkerTools {
    node: WorkerTool,
    wasm_tools: WorkerTool,
    wasm_opt: WorkerTool,
}

#[derive(serde::Deserialize)]
#[serde(crate = "::serde", deny_unknown_fields)]
struct WorkerTool {
    path: std::string::String,
    version: std::string::String,
    sha256: std::string::String,
    #[serde(default)]
    v8: Option<std::string::String>,
}

impl ToolPin {
    fn bind(&self, identity: &str) -> Result<ToolIdentity, Failure> {
        if !self.path.starts_with("/nix/store/") {
            return Err(selection(std::format!(
                "{identity}.path must name an immutable /nix/store tool: {}",
                self.path
            )));
        }
        if self.version.is_empty() {
            return Err(selection(std::format!("{identity}.version is empty")));
        }
        attempt!(require_digest(&format!("{identity}.sha256"), &self.sha256));
        Ok(ToolIdentity {
            path: self.path.clone(),
            version: self.version.clone(),
            sha256: self.sha256.clone(),
            v8: None,
        })
    }
}

impl WorkerTool {
    fn bind(&self, identity: &str) -> Result<ToolIdentity, Failure> {
        if !self.path.starts_with("/nix/store/") {
            return Err(selection(std::format!(
                "{identity}.path is not an immutable tool: {}",
                self.path
            )));
        }
        attempt!(require_digest(&format!("{identity}.sha256"), &self.sha256));
        Ok(ToolIdentity {
            path: self.path.clone(),
            version: self.version.clone(),
            sha256: self.sha256.clone(),
            v8: self.v8.clone(),
        })
    }
}

/// Read one reviewed pin document and refuse a malformed, partial or
/// unsupported selection.
///
/// # Errors
/// `build-pin-selection` when the file is unreadable or oversized, is not a
/// strict [`SCHEMA`] document, names an unknown field, carries a malformed
/// digest, an empty version, a non-store tool path, an unexpected helper root,
/// a zero helper count or an optimizer mode other than `off` or `on`.
pub(crate) fn select(path: &std::path::Path) -> Result<Selection, Failure> {
    debug_assert!(!path.as_os_str().is_empty());
    let bytes = attempt!(super::toolchain::read_bounded(
        path,
        MAX_PIN_BYTES,
        CODE_SELECTION
    ));
    parse(&bytes, &path.display().to_string())
}

/// Decode and validate one pin document; `source` names it in diagnostics.
fn parse(bytes: &[u8], source: &str) -> Result<Selection, Failure> {
    debug_assert!(!bytes.is_empty());
    // The schema is checked before the strict shape so a document of another
    // revision reports that difference instead of a missing-field error.
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|error| {
        selection(std::format!(
            "--pins {source} is not a {SCHEMA} document: {error}"
        ))
    })?;
    let named = value.get("schema").and_then(serde_json::Value::as_str);
    let Some(named) = named else {
        return Err(selection(std::format!(
            "--pins {source} is not a {SCHEMA} document: no schema field"
        )));
    };
    if named != SCHEMA {
        return Err(selection(std::format!(
            "--pins {source} names schema {} but this build writes {SCHEMA}",
            named
        )));
    }
    let document: Document = serde_json::from_value(value).map_err(|error| {
        selection(std::format!(
            "--pins {source} is not a {SCHEMA} document: {error}"
        ))
    })?;
    debug_assert_eq!(document.schema, SCHEMA);
    if document.helpers.root != HELPER_ROOT {
        return Err(selection(std::format!(
            "helpers.root must be {HELPER_ROOT}, not {}",
            document.helpers.root
        )));
    }
    if document.helpers.count == 0 {
        return Err(selection(std::string::String::from(
            "helpers.count must cover the embedded helper set",
        )));
    }
    if !matches!(document.opt_mode.as_str(), "off" | "on") {
        return Err(selection(std::format!(
            "opt_mode must be `off` or `on`, not {}",
            document.opt_mode
        )));
    }
    attempt!(require_digest("cli.sha256", &document.cli.sha256));
    attempt!(require_digest(
        "helpers.set_sha256",
        &document.helpers.set_sha256
    ));
    attempt!(require_digest(
        "lean.binary_sha256",
        &document.lean.binary_sha256
    ));
    attempt!(require_digest(
        "lean.library_blake3",
        &document.lean.library_blake3
    ));
    if document.lean.toolchain.is_empty() || document.lean.commit.is_empty() {
        return Err(selection(std::string::String::from(
            "lean.toolchain and lean.commit must name the selected release",
        )));
    }
    Ok(Selection {
        document_sha256: crate::workflow::intrinsic::sha256(bytes),
        cli_sha256: document.cli.sha256,
        helper_count: document.helpers.count,
        helper_set_sha256: document.helpers.set_sha256,
        lean_binary_sha256: document.lean.binary_sha256,
        lean_toolchain: document.lean.toolchain,
        lean_commit: document.lean.commit,
        library_blake3: document.lean.library_blake3,
        tools: ToolSet {
            node: attempt!(document.tools.node.bind("tools.node")),
            wasm_tools: attempt!(document.tools.wasm_tools.bind("tools.wasm_tools")),
            wasm_opt: attempt!(document.tools.wasm_opt.bind("tools.wasm_opt")),
        },
        opt_mode: document.opt_mode,
    })
}

/// The helper set digest every pin and report uses: SHA-256 over each helper's
/// `path`, one space, its hex SHA-256 and a newline, in ascending path order.
fn helper_set_digest(rows: &[(std::string::String, std::string::String)]) -> std::string::String {
    let mut joined = std::string::String::new();
    for (path, digest) in rows {
        joined.push_str(path);
        joined.push(' ');
        joined.push_str(digest);
        joined.push('\n');
    }
    crate::workflow::intrinsic::sha256(joined.as_bytes())
}

/// The static identities of this build: its own executable bytes, the embedded
/// runtime helper set, the selected Lean release and library that
/// [`super::toolchain::select`] already verified, and the optimizer mode this
/// lane produces.
///
/// # Errors
/// `build-pin-selection` when the running executable cannot be resolved or read
/// or the embedded helper table is empty, duplicated or unsorted.
pub(crate) fn observe(selected: &super::toolchain::Selected) -> Result<Observed, Failure> {
    let cli_path = std::fs::read_link(CLI_PATH)
        .map_err(|error| selection(std::format!("cannot resolve {CLI_PATH}: {error}")))?;
    let executable = attempt!(super::toolchain::read_bounded(
        std::path::Path::new(CLI_PATH),
        MAX_EXECUTABLE_BYTES,
        CODE_SELECTION
    ));
    debug_assert!(!executable.is_empty());
    let mut helpers = std::vec::Vec::with_capacity(noble_wasm::assets::HELPERS.len());
    let mut previous: Option<&str> = None;
    for (path, contents) in noble_wasm::assets::HELPERS {
        if contents.is_empty() {
            return Err(selection(std::format!("embedded helper is empty: {path}")));
        }
        if previous.is_some_and(|previous| previous >= path) {
            return Err(selection(std::format!(
                "embedded helper table is not in ascending path order at {path}"
            )));
        }
        previous = Some(path);
        helpers.push((
            std::string::String::from(path),
            crate::workflow::intrinsic::sha256(contents.as_bytes()),
        ));
    }
    if helpers.is_empty() {
        return Err(selection(std::string::String::from(
            "this compiler embeds no runtime helper",
        )));
    }
    let set_sha256 = helper_set_digest(&helpers);
    Ok(Observed {
        cli_sha256: crate::workflow::intrinsic::sha256(&executable),
        cli_path: cli_path.to_string_lossy().into_owned(),
        helpers,
        helper_set_sha256: set_sha256,
        lean_binary_sha256: selected.lean_sha256.clone(),
        lean_toolchain: selected.lean_version,
        lean_commit: selected.lean_commit,
        library_blake3: selected.library_blake3.clone(),
        tools: None,
    })
}

/// Read the selected worker's own tool record, written after it checked the
/// Node/V8 path and the wasm-tools/wasm-opt versions against the selected
/// configuration.
///
/// # Errors
/// `build-pin-selection` when the record is oversized, is not the strict
/// `{node, wasm_tools, wasm_opt}` shape the worker writes, carries a malformed
/// digest, a non-store path, or a Node record without its V8 identity.
pub(crate) fn observe_tools(contents: &[u8]) -> Result<ToolSet, Failure> {
    debug_assert!(!contents.is_empty());
    if contents.len() as u64 > MAX_WORKER_TOOLS_BYTES {
        return Err(selection(std::string::String::from(
            "the selected worker's tool record exceeds its bound",
        )));
    }
    let record: WorkerTools = serde_json::from_slice(contents).map_err(|error| {
        selection(std::format!(
            "the selected worker's tool record is not the expected shape: {error}"
        ))
    })?;
    if record.node.v8.is_none() {
        return Err(selection(std::string::String::from(
            "the selected worker's Node record carries no V8 identity",
        )));
    }
    Ok(ToolSet {
        node: attempt!(record.node.bind("tools.node")),
        wasm_tools: attempt!(record.wasm_tools.bind("tools.wasm_tools")),
        wasm_opt: attempt!(record.wasm_opt.bind("tools.wasm_opt")),
    })
}

/// Refuse the report unless every static identity equals the selection.
///
/// # Errors
/// `build-pin-mismatch` naming the first differing identity.
pub(crate) fn verify(selection: &Selection, observed: &Observed) -> Result<(), Failure> {
    if selection.cli_sha256 != observed.cli_sha256 {
        return Err(mismatch(
            "cli.sha256",
            &selection.cli_sha256,
            &observed.cli_sha256,
        ));
    }
    if selection.helper_count != observed.helpers.len() as u64 {
        return Err(mismatch(
            "helpers.count",
            &std::format!("{}", selection.helper_count),
            &std::format!("{}", observed.helpers.len()),
        ));
    }
    if selection.helper_set_sha256 != observed.helper_set_sha256 {
        return Err(mismatch(
            "helpers.set_sha256",
            &selection.helper_set_sha256,
            &observed.helper_set_sha256,
        ));
    }
    if selection.lean_binary_sha256 != observed.lean_binary_sha256 {
        return Err(mismatch(
            "lean.binary_sha256",
            &selection.lean_binary_sha256,
            &observed.lean_binary_sha256,
        ));
    }
    if selection.lean_toolchain != observed.lean_toolchain {
        return Err(mismatch(
            "lean.toolchain",
            &selection.lean_toolchain,
            observed.lean_toolchain,
        ));
    }
    if selection.lean_commit != observed.lean_commit {
        return Err(mismatch(
            "lean.commit",
            &selection.lean_commit,
            observed.lean_commit,
        ));
    }
    if selection.library_blake3 != observed.library_blake3 {
        return Err(mismatch(
            "lean.library_blake3",
            &selection.library_blake3,
            &observed.library_blake3,
        ));
    }
    if selection.opt_mode != OPT_MODE {
        return Err(mismatch("opt_mode", &selection.opt_mode, OPT_MODE));
    }
    Ok(())
}

/// Refuse the report unless the assembly's observed tool identities equal the
/// selection.
///
/// # Errors
/// `build-pin-mismatch` naming the first differing path, version or digest.
pub(crate) fn verify_tools(selection: &Selection, observed: &ToolSet) -> Result<(), Failure> {
    for (identity, selected, actual) in [
        ("tools.node", &selection.tools.node, &observed.node),
        (
            "tools.wasm_tools",
            &selection.tools.wasm_tools,
            &observed.wasm_tools,
        ),
        (
            "tools.wasm_opt",
            &selection.tools.wasm_opt,
            &observed.wasm_opt,
        ),
    ] {
        if selected.path != actual.path {
            return Err(mismatch(
                &std::format!("{identity}.path"),
                &selected.path,
                &actual.path,
            ));
        }
        if selected.version != actual.version {
            return Err(mismatch(
                &std::format!("{identity}.version"),
                &selected.version,
                &actual.version,
            ));
        }
        if selected.sha256 != actual.sha256 {
            return Err(mismatch(
                &std::format!("{identity}.sha256"),
                &selected.sha256,
                &actual.sha256,
            ));
        }
    }
    Ok(())
}

impl Observed {
    /// Record the assembly's tool identities once it ran.
    pub(crate) fn record_tools(&mut self, tools: ToolSet) {
        self.tools = Some(tools);
    }

    /// The `pins` record. `status` is `complete` only when the assembly ran and
    /// every tool identity was observed; a report that never reached assembly
    /// says `partial` rather than omitting the field set.
    pub(super) fn json(&self, selection_sha256: &str) -> Json {
        let assets = self
            .helpers
            .iter()
            .map(|(path, sha256)| {
                crate::workflow::encoding::object([
                    ("path", crate::workflow::encoding::string(path)),
                    ("sha256", crate::workflow::encoding::string(sha256)),
                ])
            })
            .collect();
        let tools = match &self.tools {
            Some(tools) => crate::workflow::encoding::object([
                ("node", tool_json(&tools.node, true)),
                ("wasm_tools", tool_json(&tools.wasm_tools, false)),
                ("wasm_opt", tool_json(&tools.wasm_opt, false)),
            ]),
            None => Json::Null,
        };
        crate::workflow::encoding::object([
            (
                "schema",
                crate::workflow::encoding::string(SCHEMA),
            ),
            (
                "status",
                crate::workflow::encoding::string(if self.tools.is_some() {
                    "complete"
                } else {
                    "partial"
                }),
            ),
            (
                "selection_sha256",
                crate::workflow::encoding::string(selection_sha256),
            ),
            (
                "cli",
                crate::workflow::encoding::object([
                    (
                        "sha256",
                        crate::workflow::encoding::string(&self.cli_sha256),
                    ),
                    (
                        "path",
                        crate::workflow::encoding::string(&self.cli_path),
                    ),
                    (
                        "basis",
                        crate::workflow::encoding::string(
                            "SHA-256 of the running executable's bytes (/proc/self/exe), refused unless they equal the selection",
                        ),
                    ),
                ]),
            ),
            (
                "helpers",
                crate::workflow::encoding::object([
                    (
                        "root",
                        crate::workflow::encoding::string(HELPER_ROOT),
                    ),
                    (
                        "count",
                        Json::Number(self.helpers.len() as u64),
                    ),
                    (
                        "set_sha256",
                        crate::workflow::encoding::string(&self.helper_set_sha256),
                    ),
                    ("assets", Json::Array(assets)),
                    (
                        "basis",
                        crate::workflow::encoding::string(
                            "SHA-256 of each embedded crates/noble-wasm/runtime WAT; set_sha256 is SHA-256 over `<path> <sha256>` followed by a newline, for every helper in ascending path order, refused unless it equals the selection",
                        ),
                    ),
                ]),
            ),
            (
                "lean",
                crate::workflow::encoding::object([
                    (
                        "binary_sha256",
                        crate::workflow::encoding::string(&self.lean_binary_sha256),
                    ),
                    (
                        "toolchain",
                        crate::workflow::encoding::string(self.lean_toolchain),
                    ),
                    (
                        "commit",
                        crate::workflow::encoding::string(self.lean_commit),
                    ),
                    (
                        "library_blake3",
                        crate::workflow::encoding::string(&self.library_blake3),
                    ),
                    (
                        "basis",
                        crate::workflow::encoding::string(
                            "SHA-256 of the selected Lean release binary and BLAKE3 of the contract library tree this build compiles, both refused unless they equal the selection",
                        ),
                    ),
                ]),
            ),
            ("tools", tools),
            (
                "opt_mode",
                crate::workflow::encoding::object([
                    ("mode", crate::workflow::encoding::string(OPT_MODE)),
                    ("flags", crate::workflow::encoding::Json::Array(std::vec::Vec::new())),
                    (
                        "basis",
                        crate::workflow::encoding::string(
                            "the proof-required build assembles with wasm-tools parse/validate only; it never invokes the optimizer, so no optimizer flags apply and an `on` selection is refused",
                        ),
                    ),
                ]),
            ),
        ])
    }
}

fn tool_json(tool: &ToolIdentity, node: bool) -> Json {
    crate::workflow::encoding::object([
        ("path", crate::workflow::encoding::string(&tool.path)),
        ("version", crate::workflow::encoding::string(&tool.version)),
        ("sha256", crate::workflow::encoding::string(&tool.sha256)),
        (
            "sha256_basis",
            crate::workflow::encoding::string(if node {
                "SHA-256 of the Node executable the selected worker runs as, checked against the selected path and version before assembly"
            } else {
                "SHA-256 of the selected executable the worker invoked, checked against the selected version before assembly"
            }),
        ),
        (
            "v8",
            crate::workflow::encoding::optional_string(tool.v8.as_deref()),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::{
        helper_set_digest, observe, observe_tools, parse, verify, verify_tools, Observed,
        Selection, ToolIdentity, ToolSet, HELPER_ROOT,
    };

    const NODE_PATH: &str = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-nodejs/bin/node";
    const WASM_TOOLS_PATH: &str =
        "/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-wasm-tools/bin/wasm-tools";
    const WASM_OPT_PATH: &str = "/nix/store/cccccccccccccccccccccccccccccccc-binaryen/bin/wasm-opt";
    const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    /// The helper rows this build embeds, with synthetic per-file digests so a
    /// selected set digest is still exercised end to end.
    fn helper_rows() -> std::vec::Vec<(std::string::String, std::string::String)> {
        noble_wasm::assets::HELPERS
            .iter()
            .map(|(path, _)| (std::string::String::from(*path), ZERO.to_owned()))
            .collect()
    }

    fn document(root: &str) -> serde_json::Value {
        serde_json::json!({
            "schema": super::SCHEMA,
            "opt_mode": "off",
            "cli": { "sha256": ZERO },
            "helpers": { "root": root, "count": helper_rows().len(),
                "set_sha256": helper_set_digest(&helper_rows()) },
            "lean": { "binary_sha256": ZERO,
                "toolchain": "leanprover/lean4:v4.35.0",
                "commit": "eb6cafb7290e8bd5f2937522d8d804f58f02bafc", "library_blake3": ZERO },
            "tools": {
                "node": { "path": NODE_PATH, "version": "v24.13.0", "sha256": ZERO },
                "wasm_tools": { "path": WASM_TOOLS_PATH, "version": "wasm-tools 1.245.1",
                    "sha256": ZERO },
                "wasm_opt": { "path": WASM_OPT_PATH, "version": "wasm-opt version 125",
                    "sha256": ZERO },
            },
        })
    }

    /// Unwrap a pin result without requiring `Debug` on the failure carrier.
    fn require<T>(result: Result<T, super::Failure>) -> T {
        match result {
            Ok(value) => value,
            Err(failure) => std::panic!("{}: {}", failure.code, failure.message),
        }
    }

    fn require_refusal<T>(result: Result<T, super::Failure>) -> super::Failure {
        match result {
            Ok(_) => std::panic!("expected a refusal"),
            Err(failure) => failure,
        }
    }

    fn selected(value: &serde_json::Value) -> Result<Selection, super::Failure> {
        parse(value.to_string().as_bytes(), "test-pins")
    }

    fn observed() -> Observed {
        Observed {
            cli_sha256: ZERO.to_owned(),
            cli_path: "/tmp/noble".to_owned(),
            helper_set_sha256: helper_set_digest(&helper_rows()),
            helpers: helper_rows(),
            lean_binary_sha256: ZERO.to_owned(),
            lean_toolchain: crate::sandbox::PIN,
            lean_commit: crate::sandbox::COMMIT,
            library_blake3: ZERO.to_owned(),
            tools: None,
        }
    }

    fn tools() -> ToolSet {
        ToolSet {
            node: ToolIdentity {
                path: NODE_PATH.to_owned(),
                version: "v24.13.0".to_owned(),
                sha256: ZERO.to_owned(),
                v8: Some("13.6.233.17-node.37".to_owned()),
            },
            wasm_tools: ToolIdentity {
                path: WASM_TOOLS_PATH.to_owned(),
                version: "wasm-tools 1.245.1".to_owned(),
                sha256: ZERO.to_owned(),
                v8: None,
            },
            wasm_opt: ToolIdentity {
                path: WASM_OPT_PATH.to_owned(),
                version: "wasm-opt version 125".to_owned(),
                sha256: ZERO.to_owned(),
                v8: None,
            },
        }
    }

    fn refused(failure: super::Failure, identity: &str) {
        assert_eq!(failure.code, super::CODE_MISMATCH, "{}", failure.message);
        assert!(failure.message.contains(identity), "{}", failure.message);
    }

    #[test]
    fn a_well_formed_selection_binds_every_identity_and_verifies() {
        let selection = require(selected(&document(HELPER_ROOT)));
        let mut observed = observed();
        assert!(verify(&selection, &observed).is_ok());
        observed.record_tools(tools());
        assert!(verify_tools(&selection, &tools()).is_ok());
        assert_eq!(selection.helper_count, helper_rows().len() as u64);
        assert_eq!(selection.opt_mode, "off");
    }

    #[test]
    fn each_static_identity_mismatch_is_refused_by_name() {
        let base = document(HELPER_ROOT);
        let mutation = |path: &[&str], value: serde_json::Value| {
            let mut value_document = base.clone();
            let mut cursor = &mut value_document;
            for step in &path[..path.len() - 1] {
                cursor = &mut cursor[*step];
            }
            cursor[path[path.len() - 1]] = value;
            value_document
        };
        let other = "1111111111111111111111111111111111111111111111111111111111111111";
        for (path, value, identity) in [
            (
                vec!["cli", "sha256"],
                serde_json::json!(other),
                "cli.sha256",
            ),
            (
                vec!["helpers", "count"],
                serde_json::json!(1),
                "helpers.count",
            ),
            (
                vec!["helpers", "set_sha256"],
                serde_json::json!(other),
                "helpers.set_sha256",
            ),
            (
                vec!["lean", "binary_sha256"],
                serde_json::json!(other),
                "lean.binary_sha256",
            ),
            (
                vec!["lean", "toolchain"],
                serde_json::json!("leanprover/lean4:v9.9.9"),
                "lean.toolchain",
            ),
            (
                vec!["lean", "commit"],
                serde_json::json!("beef"),
                "lean.commit",
            ),
            (
                vec!["lean", "library_blake3"],
                serde_json::json!(other),
                "lean.library_blake3",
            ),
            (vec!["opt_mode"], serde_json::json!("on"), "opt_mode"),
        ] {
            let selection = require(selected(&mutation(&path, value)));
            let failure = require_refusal(verify(&selection, &observed()));
            refused(failure, identity);
        }
    }

    #[test]
    fn each_tool_identity_mismatch_is_refused_by_name() {
        let selection = require(selected(&document(HELPER_ROOT)));
        let other = "1111111111111111111111111111111111111111111111111111111111111111";
        for (identity, mutate) in [
            ("tools.node.path", 0usize),
            ("tools.wasm_tools.path", 1),
            ("tools.wasm_opt.path", 2),
        ] {
            let mut tools = tools();
            match mutate {
                0 => tools.node.path = "/ambient/node".to_owned(),
                1 => tools.wasm_tools.path = "/ambient/wasm-tools".to_owned(),
                _ => tools.wasm_opt.path = "/ambient/wasm-opt".to_owned(),
            }
            refused(require_refusal(verify_tools(&selection, &tools)), identity);
        }
        for (identity, mutate) in [
            ("tools.node.version", 0usize),
            ("tools.wasm_tools.version", 1),
            ("tools.wasm_opt.version", 2),
        ] {
            let mut tools = tools();
            match mutate {
                0 => tools.node.version = "v0.0.0".to_owned(),
                1 => tools.wasm_tools.version = "wasm-tools 0".to_owned(),
                _ => tools.wasm_opt.version = "wasm-opt version 0".to_owned(),
            }
            refused(require_refusal(verify_tools(&selection, &tools)), identity);
        }
        for (identity, mutate) in [
            ("tools.node.sha256", 0usize),
            ("tools.wasm_tools.sha256", 1),
            ("tools.wasm_opt.sha256", 2),
        ] {
            let mut tools = tools();
            match mutate {
                0 => tools.node.sha256 = other.to_owned(),
                1 => tools.wasm_tools.sha256 = other.to_owned(),
                _ => tools.wasm_opt.sha256 = other.to_owned(),
            }
            refused(require_refusal(verify_tools(&selection, &tools)), identity);
        }
    }

    #[test]
    fn a_malformed_or_unsupported_selection_is_refused_at_selection_time() {
        for (label, value, code) in [
            (
                "schema",
                serde_json::json!({ "schema": "noble-build-report-pins/v2" }),
                "names schema",
            ),
            (
                "unknown field",
                serde_json::json!({ "schema": super::SCHEMA, "extra": 1 }),
                "not a",
            ),
            (
                "helper root",
                document("/ambient/runtime").clone(),
                "helpers.root",
            ),
            (
                "zero count",
                {
                    let mut value = document(HELPER_ROOT);
                    value["helpers"]["count"] = serde_json::json!(0);
                    value
                },
                "helpers.count",
            ),
            (
                "opt mode",
                {
                    let mut value = document(HELPER_ROOT);
                    value["opt_mode"] = serde_json::json!("auto");
                    value
                },
                "opt_mode",
            ),
            (
                "digest",
                {
                    let mut value = document(HELPER_ROOT);
                    value["cli"]["sha256"] = serde_json::json!("ABC");
                    value
                },
                "cli.sha256",
            ),
            (
                "empty version",
                {
                    let mut value = document(HELPER_ROOT);
                    value["lean"]["toolchain"] = serde_json::json!("");
                    value
                },
                "lean.toolchain",
            ),
            (
                "tool path",
                {
                    let mut value = document(HELPER_ROOT);
                    value["tools"]["wasm_tools"]["path"] = serde_json::json!("/usr/bin/wasm-tools");
                    value
                },
                "tools.wasm_tools.path",
            ),
        ] {
            let failure = require_refusal(selected(&value));
            assert_eq!(
                failure.code,
                super::CODE_SELECTION,
                "{label}: {}",
                failure.message
            );
            assert!(
                failure.message.contains(code),
                "{label}: {}",
                failure.message
            );
        }
    }

    #[test]
    fn the_helper_set_digest_is_the_documented_line_digest() {
        let rows = [
            (std::format!("{HELPER_ROOT}/a.wat"), ZERO.to_owned()),
            (
                std::format!("{HELPER_ROOT}/b.wat"),
                "1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
            ),
        ];
        let expected = crate::workflow::intrinsic::sha256(
            std::format!(
                "{HELPER_ROOT}/a.wat {ZERO}\n{HELPER_ROOT}/b.wat {}\n",
                rows[1].1
            )
            .as_bytes(),
        );
        assert_eq!(helper_set_digest(&rows), expected);
    }

    fn tool_record(v8: bool) -> std::string::String {
        serde_json::json!({
            "node": { "path": NODE_PATH, "version": "v24.13.0", "sha256": ZERO,
                "v8": if v8 { serde_json::json!("13.6.233.17-node.37") } else { serde_json::Value::Null } },
            "wasm_tools": { "path": WASM_TOOLS_PATH, "version": "wasm-tools 1.245.1", "sha256": ZERO },
            "wasm_opt": { "path": WASM_OPT_PATH, "version": "wasm-opt version 125", "sha256": ZERO },
        })
        .to_string()
    }

    #[test]
    fn the_worker_tool_record_must_be_the_exact_verified_shape() {
        let tools = require(observe_tools(tool_record(true).as_bytes()));
        assert_eq!(tools.node.sha256, ZERO);
        let failure = require_refusal(observe_tools(tool_record(false).as_bytes()));
        assert!(failure.message.contains("V8"), "{}", failure.message);
        for (label, value) in [
            (
                "unknown field",
                serde_json::json!({ "node": {}, "extra": 1 }),
            ),
            ("malformed", serde_json::json!({ "node": {} })),
        ] {
            let failure = require_refusal(observe_tools(value.to_string().as_bytes()));
            assert_eq!(failure.code, super::CODE_SELECTION, "{label}");
        }
    }

    #[test]
    fn the_observed_cli_identity_is_the_running_executable() {
        let selected = super::super::toolchain::Selected {
            lean_sha256: ZERO.to_owned(),
            lean_version: crate::sandbox::PIN,
            lean_commit: crate::sandbox::COMMIT,
            library_blake3: ZERO.to_owned(),
            library_files: 1,
            library_bytes: 1,
        };
        let observed = require(observe(&selected));
        let executable = std::fs::read("/proc/self/exe").expect("current exe");
        assert_eq!(
            observed.cli_sha256,
            crate::workflow::intrinsic::sha256(&executable)
        );
        assert!(!observed.cli_path.is_empty());
        assert_eq!(observed.helpers.len(), noble_wasm::assets::HELPERS.len());
        assert_eq!(observed.lean_binary_sha256, ZERO);
        assert_eq!(observed.lean_toolchain, crate::sandbox::PIN);
    }
}
