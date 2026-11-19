//! `noble ngraph eval --net`: the opt-in revision-3 interaction-net backend
//! (`docs/i64-net-contract-v3.md` §9) under explicit whole-request ceilings,
//! with a producer, contract and semantic-count receipt in its header line
//! (`.cairn/changes/net-request-limits-receipts`).
//!
//! Non-claims: the byte ceilings are accounted ceilings, not process RSS
//! bounds; the census check is one executed equality per run, not a
//! refinement proof of the reducer; the named Lean theorems are model
//! statements; the report names no digest of the running executable.

use super::arguments::{self, Parsed};
use super::files::{self, ReadStop};
use super::report::{self, Line};
use super::{Stop, IDS};
use crate::workflow::encoding::{decimal_len, object, quoted_len, string, strings, Json};
use noble_ngraph::{net, CheckedGraph, IdentityScheme, Value};

const C: &str = "eval";

/// Caps the graph and vector files together, before either is read.
pub(super) const INPUT_FLAG: &str = "--max-input-bytes";
/// Caps the accounted net storage of each row ([`net::Limits::max_retained_bytes`]).
pub(super) const RETAINED_FLAG: &str = "--max-retained-bytes";
/// Caps the encoded report, charged before each line is built.
pub(super) const OUTPUT_FLAG: &str = "--max-output-bytes";
/// The value flags only `--net` accepts.
pub(super) const FLAGS: [&str; 3] = [INPUT_FLAG, RETAINED_FLAG, OUTPUT_FLAG];

/// `--max-input-bytes` default: the graph and vector file bounds together.
pub(super) const DEFAULT_MAX_INPUT_BYTES: u64 =
    files::GRAPH_BYTES.saturating_add(files::VECTORS_BYTES);
/// `--max-output-bytes` default: 64 MiB of report text.
pub(super) const DEFAULT_MAX_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;

/// The count basis of every normal row: the census check passed.
const COUNT_BASIS: &str = "census-checked";
/// Fully qualified Lean declarations behind the semantic count.
const COUNT_THEOREMS: [&str; 3] = [
    "NobleFoundation.NetI64.i64_normal_form_unique",
    "NobleFoundation.NetI64.i64_equal_interaction_count",
    "NobleFoundation.NetI64.Certificate.accepted_exact_count",
];
/// The instantiated inets interaction system those theorems quantify over.
const LEAN_SYSTEM: &str = "NobleFoundation.NetI64.system";
const LEAN_MODULE: &str = "proofs/foundation/NobleFoundation/NetI64.lean";
const LEAN_MODULE_BLAKE3: &str = "a5d9ade10d41b2c0a7e2d209ce2f162918c7250805a13187c3428be357c5dc74";
const LEAN_MODULE_SHA256: &str = "c7706a132361014ee1b5eec5c724fe95d4a81b1a7024bc6d16e519f8c00b86cb";
const LEAN_TOOLCHAIN: &str = "leanprover/lean4:v4.35.0";
const INETS_MODULE: &str = "proofs/inets/InteractionNets.lean";
const INETS_COMMIT: &str = "98bbb01070664e4462a35efa5d62de6f27143572";
const INETS_BLAKE3: &str = "26f32cef1dae84c10443b7f42efe3ad78bb2fca2e4cfdb9c456fe7c49ec58d22";
const INETS_SHA256: &str = "c7a35dad49822583a9ee209b47d8ba5617d5abcb5a7f9f96df05e8502f362f54";
const RECEIPT: &str = "verification/foundation/receipts/518be7fd5/foundation-check.json";
const RECEIPT_BLAKE3: &str = "7f76a66321c597c8ac7a9aebf83a525a1d73bda73ca85f5298a0c29e843f4b1a";
const RECEIPT_SOURCE_COMMIT: &str = "518be7fd5691ac6925c99f2cae3b592ab8243ad1";
const MODEL_BOUNDARY: &str = "model-only: the named Lean theorems state that every complete \
reduction of a checked NetI64 translation that reaches a normal form has the same normal form \
and the same length, and that an accepted certificate fixes that length at 2*ops+dups+eras; \
they are not a refinement proof of this Rust lowering, reducer or read-back. census-checked \
means only that this run's interaction counter equaled the census of its validated net.";

/// The source revision a build declared; reported only when it is a full
/// lowercase Git commit identity.
const SOURCE_REVISION: Option<&str> = option_env!("NOBLE_SOURCE_REVISION");

/// The whole-request ceilings of one `eval --net`.
#[derive(Clone, Copy)]
pub(super) struct RequestLimits {
    input_bytes: u64,
    retained_bytes: u64,
    output_bytes: u64,
}

impl RequestLimits {
    /// Reads the three ceilings; a value above its default is a usage error.
    pub(super) fn parse(args: &Parsed) -> Result<Self, std::string::String> {
        Ok(Self {
            input_bytes: attempt!(ceiling(args, INPUT_FLAG, DEFAULT_MAX_INPUT_BYTES)),
            retained_bytes: attempt!(ceiling(
                args,
                RETAINED_FLAG,
                net::DEFAULT_MAX_RETAINED_BYTES
            )),
            output_bytes: attempt!(ceiling(args, OUTPUT_FLAG, DEFAULT_MAX_OUTPUT_BYTES)),
        })
    }
}

fn ceiling(args: &Parsed, flag: &str, default: u64) -> Result<u64, std::string::String> {
    let Some(text) = attempt!(args.text(flag)) else {
        return Ok(default);
    };
    let value = attempt!(arguments::unsigned(flag, &text));
    if value > default {
        return Err(std::format!(
            "{flag} may only lower its default of {default} bytes"
        ));
    }
    Ok(value)
}

/// Charges the request's input files against `--max-input-bytes` before they
/// are read: each read is bounded by the smaller of its file bound and the
/// bytes the request has left, so no byte past the ceiling is read.
pub(super) struct InputMeter {
    cap: u64,
    used: u64,
}

impl InputMeter {
    pub(super) const fn new(limits: &RequestLimits) -> Self {
        Self {
            cap: limits.input_bytes,
            used: 0,
        }
    }

    /// Bytes read so far.
    pub(super) const fn used(&self) -> u64 {
        self.used
    }

    pub(super) fn read(
        &mut self,
        path: &std::path::Path,
        file_bound: u64,
        what: &str,
    ) -> Result<std::vec::Vec<u8>, Line> {
        let remaining = self.cap.saturating_sub(self.used);
        let is_request_bound = remaining < file_bound;
        match files::read_bounded(path, remaining.min(file_bound), what) {
            Ok(bytes) => {
                let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                self.used = self.used.saturating_add(read);
                Ok(bytes)
            }
            Err(ReadStop::Host(message)) => Err(Line::host(C, "input", message)),
            Err(ReadStop::Over(observed)) if is_request_bound => Err(exhausted(
                "input",
                "input-bytes",
                self.cap,
                self.used.saturating_add(observed),
                std::format!("the net request's input files exceed {} bytes", self.cap),
            )),
            Err(ReadStop::Over(_)) => Err(Line::host(
                C,
                "input",
                std::format!("{what} exceeds {file_bound} bytes"),
            )),
        }
    }
}

/// Charges every report line, newline included, against `--max-output-bytes`
/// before the line is built. A refusal discards every buffered line: only
/// the one refusal line is written, never a truncated success.
struct OutputMeter {
    cap: u64,
    used: u64,
}

impl OutputMeter {
    fn charge(&mut self, encoded: usize) -> Result<(), Line> {
        let total = u64::try_from(encoded)
            .ok()
            .and_then(|bytes| bytes.checked_add(1))
            .and_then(|bytes| self.used.checked_add(bytes));
        match total {
            Some(total) if total <= self.cap => {
                self.used = total;
                Ok(())
            }
            total => Err(exhausted(
                "output",
                "output-bytes",
                self.cap,
                total.unwrap_or(u64::MAX),
                std::format!("the net report exceeds {} bytes", self.cap),
            )),
        }
    }
}

/// Runs each validated input row through the explicitly selected bounded net.
/// No output row is returned unless that row reached a complete normal form
/// whose interaction counter passed the census check, and no line is kept
/// unless the whole report fits `--max-output-bytes`.
pub(super) fn eval_net(
    graph: &CheckedGraph,
    rows: &[std::vec::Vec<Value>],
    fuel: Option<u64>,
    request: &RequestLimits,
    input_bytes: u64,
) -> Result<std::vec::Vec<Line>, Stop> {
    let defaults = net::Limits::default();
    let limits = net::Limits {
        interaction_fuel: fuel.map_or(defaults.interaction_fuel, |requested| {
            requested.min(defaults.interaction_fuel)
        }),
        max_retained_bytes: request.retained_bytes,
        ..defaults
    };
    debug_assert!(limits.interaction_fuel <= defaults.interaction_fuel);
    debug_assert!(limits.max_retained_bytes <= net::DEFAULT_MAX_RETAINED_BYTES);
    let mut output = OutputMeter {
        cap: request.output_bytes,
        used: 0,
    };
    let head = header(graph, &limits, request, input_bytes);
    attempt!(output.charge(head.encoded_len()));
    let mut lines = std::vec![head];
    for (index, row) in rows.iter().enumerate() {
        let candidate =
            attempt!(net::lower(&IDS, graph, row, &limits).map_err(|r| lowering_refusal(r, index)));
        let result = net::reduce(
            &IDS,
            net::ReduceRequest {
                checked: graph,
                candidate: &candidate,
                inputs: row,
                limits: &limits,
                plan: plan(),
            },
        );
        let net::Obs::Normal(values) = &result.obs else {
            return Err(row_failure(&result, index).into());
        };
        debug_assert_eq!(result.expected_interactions, Some(result.interactions));
        let row_line = normal_row(&result, index);
        attempt!(output.charge(
            row_line
                .encoded_len()
                .saturating_add(outputs_field_len(values))
        ));
        lines.push(
            row_line.with(
                "outputs",
                Json::Array(
                    values
                        .iter()
                        .map(|value| string(noble_ngraph::format::value::render(*value)))
                        .collect(),
                ),
            ),
        );
    }
    debug_assert!(output.used <= output.cap);
    Ok(lines)
}

/// The selected sequential schedule and storage policy, named in the header.
fn plan() -> net::Plan<'static> {
    net::Plan {
        schedule: Some(net::Schedule::Lifo),
        storage_policy: Some(net::StoragePolicy::Reuse),
        ..net::Plan::default()
    }
}

fn header(
    graph: &CheckedGraph,
    limits: &net::Limits,
    request: &RequestLimits,
    input_bytes: u64,
) -> Line {
    Line::normal(C, "evaluation")
        .with("graph", report::digest(&graph.id()))
        .with("snapshot", Json::Null)
        .with("backend", string("net"))
        .with("producer", producer())
        .with("net_contract", contract())
        .with("limits", effective(limits, request))
        .with("input_bytes", Json::Number(input_bytes))
        .with("semantic_count", semantic_count())
}

fn producer() -> Json {
    let revision = SOURCE_REVISION.filter(|revision| is_commit(revision));
    object([
        ("package", string(env!("CARGO_PKG_NAME"))),
        ("version", string(env!("CARGO_PKG_VERSION"))),
        ("source_revision", revision.map_or(Json::Null, string)),
        (
            "source_revision_status",
            string(if revision.is_some() {
                "build-declared"
            } else {
                "unavailable"
            }),
        ),
        ("executable_digest", Json::Null),
    ])
}

/// A full lowercase hexadecimal Git commit identity.
fn is_commit(text: &str) -> bool {
    text.len() == 40
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn contract() -> Json {
    object([
        ("contract", string(net::CONTRACT_REVISION)),
        ("lowering", string(net::LOWERING_REVISION)),
        ("rule_table", string(net::PLAN_RULES_REVISION)),
        ("plan", string(net::PLAN_REVISION)),
        ("representation", string(net::PLAN_REPRESENTATION_REVISION)),
        ("plan_checker", string(net::PLAN_CHECKER_REVISION)),
        ("plan_executor", string(net::PLAN_EXECUTOR_REVISION)),
        ("schedule", string("lifo")),
        ("storage_policy", string("reuse")),
        ("identity_scheme", string(IDS.name())),
    ])
}

fn effective(limits: &net::Limits, request: &RequestLimits) -> Json {
    object([
        ("max_input_bytes", Json::Number(request.input_bytes)),
        (
            "max_retained_bytes",
            Json::Number(limits.max_retained_bytes),
        ),
        ("max_output_bytes", Json::Number(request.output_bytes)),
        ("interaction_fuel", Json::Number(limits.interaction_fuel)),
        ("max_agent_slots", Json::Number(limits.max_agent_slots)),
        ("max_wires", Json::Number(limits.max_wires)),
        ("max_worklist_entries", Json::Number(limits.max_agent_slots)),
    ])
}

fn semantic_count() -> Json {
    object([
        ("unit", string("inets-principal-pair-interaction")),
        ("counter", string("noble-net-reducer-interactions")),
        (
            "boundary",
            string("complete-normal-form-and-ordered-read-back"),
        ),
        ("census", string("2*ops+dups+eras")),
        ("count_basis", string(COUNT_BASIS)),
        ("assurance", string("model-only")),
        ("theorems", strings(&COUNT_THEOREMS)),
        ("lean_system", string(LEAN_SYSTEM)),
        (
            "proof_source",
            object([
                ("module", string(LEAN_MODULE)),
                ("module_blake3", string(LEAN_MODULE_BLAKE3)),
                ("module_sha256", string(LEAN_MODULE_SHA256)),
                ("lean_toolchain", string(LEAN_TOOLCHAIN)),
                ("inets_module", string(INETS_MODULE)),
                ("inets_commit", string(INETS_COMMIT)),
                ("inets_blake3", string(INETS_BLAKE3)),
                ("inets_sha256", string(INETS_SHA256)),
                ("receipt", string(RECEIPT)),
                ("receipt_blake3", string(RECEIPT_BLAKE3)),
                ("receipt_source_commit", string(RECEIPT_SOURCE_COMMIT)),
            ]),
        ),
        ("model_boundary", string(MODEL_BOUNDARY)),
    ])
}

/// A normal row without its `outputs`, which [`outputs_field_len`] charges
/// before they are rendered.
fn normal_row(result: &net::Report, index: usize) -> Line {
    Line::normal(C, "evaluation")
        .with("vector", report::count(index))
        .with("backend", string("net"))
        .with("interactions", Json::Number(result.interactions))
        .with(
            "expected_interactions",
            optional(result.expected_interactions),
        )
        .with("count_basis", string(COUNT_BASIS))
        .with("peak_live_agents", Json::Number(result.peak_live_agents))
        .with("final_wires", optional(result.final_wires))
        .with("retained_bytes", Json::Number(result.retained_bytes))
        .with(
            "retained_bytes_high_water",
            Json::Number(result.retained_bytes_high_water),
        )
}

fn optional(value: Option<u64>) -> Json {
    value.map_or(Json::Null, Json::Number)
}

/// Encoded bytes that `.with("outputs", <rendered values>)` adds to a line
/// with earlier fields: the comma, the quoted key, the colon and the array.
fn outputs_field_len(values: &[Value]) -> usize {
    let items = values.iter().fold(0usize, |length, value| {
        length
            .saturating_add(rendered_len(*value))
            .saturating_add(2)
    });
    quoted_len("outputs")
        .saturating_add(4)
        .saturating_add(items)
        .saturating_add(values.len().saturating_sub(1))
}

/// Bytes of `noble_ngraph::format::value::render(value)`, which needs no
/// JSON escaping.
fn rendered_len(value: Value) -> usize {
    match value {
        Value::I64(x) => decimal_len(x.unsigned_abs())
            .saturating_add(usize::from(x < 0))
            .saturating_add(4),
        Value::Bool(x) => 10usize.saturating_sub(usize::from(x)),
        Value::F32(bits) if f32::from_bits(bits).is_nan() => 7,
        Value::F32(_) => 14,
    }
}

fn exhausted(
    stage: &'static str,
    limit: &'static str,
    cap: u64,
    observed: u64,
    detail: std::string::String,
) -> Line {
    Line::backend_failure(
        C,
        stage,
        noble_ngraph::Outcome::Exhausted,
        &std::format!("E-NET-BUDGET-{limit}"),
        detail,
    )
    .with("backend", string("net"))
    .with("limit", string(limit))
    .with("cap", Json::Number(cap))
    .with("observed", Json::Number(observed))
}

/// The single line that replaces the report when one row stops.
fn row_failure(result: &net::Report, index: usize) -> Line {
    let (outcome, code, label, detail) = match &result.obs {
        net::Obs::Refused(refusal) => (
            noble_ngraph::Outcome::Reject,
            "E-NET-REFUSED".to_owned(),
            "refused",
            std::format!("net reduction refused: {refusal:?}"),
        ),
        net::Obs::GuardFailed { node, fact } => (
            noble_ngraph::Outcome::Trap,
            "G-FACT-FALSE".to_owned(),
            "guard-failed",
            std::format!("%{} fails its {} fact", node.0, fact.kind_name()),
        ),
        net::Obs::Exhausted {
            limit,
            cap,
            observed,
        } => (
            noble_ngraph::Outcome::Exhausted,
            std::format!("E-NET-BUDGET-{}", limit.name()),
            "exhausted",
            std::format!(
                "net {} capacity {cap} exhausted at {observed}",
                limit.name()
            ),
        ),
        net::Obs::Cancelled => (
            noble_ngraph::Outcome::Trap,
            "C-CANCELLED".to_owned(),
            "cancelled",
            "net reduction cancelled by the host".to_owned(),
        ),
        net::Obs::InternalInvalid(invalid) => (
            noble_ngraph::Outcome::Reject,
            "E-NET-INTERNAL-INVALID".to_owned(),
            "internal-invalid",
            std::format!("net invariant failed: {}", invalid.name()),
        ),
        net::Obs::Normal(_) => (
            noble_ngraph::Outcome::Normal,
            std::string::String::new(),
            "normal",
            "unreachable normal observation".to_owned(),
        ),
    };
    let mut line = Line::backend_failure(C, "evaluation", outcome, &code, detail)
        .with("backend", string("net"))
        .with("observation", string(label))
        .with("vector", report::count(index))
        .with("interactions", Json::Number(result.interactions))
        .with(
            "expected_interactions",
            optional(result.expected_interactions),
        )
        .with("peak_live_agents", Json::Number(result.peak_live_agents))
        .with("retained_bytes", Json::Number(result.retained_bytes))
        .with(
            "retained_bytes_high_water",
            Json::Number(result.retained_bytes_high_water),
        );
    match &result.obs {
        net::Obs::Exhausted {
            limit,
            cap,
            observed,
        } => {
            line = line
                .with("limit", string(limit.name()))
                .with("cap", Json::Number(*cap))
                .with("observed", Json::Number(*observed));
        }
        net::Obs::InternalInvalid(invalid) => {
            line = line.with("invalid", string(invalid.name()));
        }
        _ => {}
    }
    line
}

/// The single line that replaces the report when lowering one row refuses.
fn lowering_refusal(refusal: net::Refusal, index: usize) -> Line {
    match refusal {
        net::Refusal::Profile
        | net::Refusal::Operation
        | net::Refusal::Facts
        | net::Refusal::Params => Line::backend_failure(
            C,
            "evaluation",
            noble_ngraph::Outcome::Unsupported,
            "E-NET-UNSUPPORTED",
            std::format!("the selected net backend refuses {}", refusal.name()),
        ),
        net::Refusal::Interface => Line::backend_failure(
            C,
            "evaluation",
            noble_ngraph::Outcome::Reject,
            "E-NET-INTERFACE",
            "net input values do not match the checked graph interface",
        ),
        net::Refusal::Exhausted {
            limit,
            cap,
            observed,
        } => exhausted(
            "evaluation",
            limit.name(),
            cap,
            observed,
            std::format!("net {} capacity {cap} requires {observed}", limit.name()),
        )
        .with("vector", report::count(index)),
        net::Refusal::InternalInvalid(invalid) => Line::backend_failure(
            C,
            "evaluation",
            noble_ngraph::Outcome::Reject,
            "E-NET-INTERNAL-INVALID",
            std::format!("net validation failed: {}", invalid.name()),
        )
        .with("invalid", string(invalid.name())),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_commit, outputs_field_len, rendered_len, RequestLimits, COUNT_THEOREMS, INETS_BLAKE3,
        INETS_COMMIT, INETS_MODULE, INETS_SHA256, LEAN_MODULE, LEAN_MODULE_BLAKE3,
        LEAN_MODULE_SHA256, RECEIPT, RECEIPT_BLAKE3, RECEIPT_SOURCE_COMMIT,
    };
    use crate::ngraph::arguments::{parse, Spec};
    use crate::ngraph::report::Line;
    use crate::workflow::encoding::{string, Json};
    use noble_ngraph::Value;

    type TestResult = Result<(), std::boxed::Box<dyn std::error::Error>>;

    fn repository(path: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }

    #[test]
    fn rendered_lengths_match_the_rendering() {
        let values = [
            Value::I64(0),
            Value::I64(-1),
            Value::I64(i64::MIN),
            Value::I64(i64::MAX),
            Value::Bool(true),
            Value::Bool(false),
            Value::F32(f32::NAN.to_bits()),
            Value::F32(1.5f32.to_bits()),
        ];
        for value in values {
            let rendered = noble_ngraph::format::value::render(value);
            assert_eq!(rendered_len(value), rendered.len(), "{rendered}");
        }
        for count in [0, 1, values.len()] {
            let slice = &values[..count];
            let bare = Line::normal("eval", "evaluation").with("vector", Json::Number(0));
            let skeleton = bare.encoded_len();
            let full = bare.with(
                "outputs",
                Json::Array(
                    slice
                        .iter()
                        .map(|value| string(noble_ngraph::format::value::render(*value)))
                        .collect(),
                ),
            );
            assert_eq!(skeleton + outputs_field_len(slice), full.encode().len());
        }
    }

    #[test]
    fn source_revisions_are_full_lowercase_commits() {
        assert!(is_commit(RECEIPT_SOURCE_COMMIT));
        assert!(is_commit(INETS_COMMIT));
        assert!(!is_commit(""));
        assert!(!is_commit(&RECEIPT_SOURCE_COMMIT[..39]));
        assert!(!is_commit(&RECEIPT_SOURCE_COMMIT.to_uppercase()));
        assert!(!is_commit(&std::format!("{RECEIPT_SOURCE_COMMIT}-dirty")));
    }

    #[test]
    fn ceilings_may_lower_but_never_raise_their_defaults() -> TestResult {
        let spec = Spec {
            positional: &[],
            values: &super::FLAGS,
            repeated: &[],
            switches: &[],
            budgets: &[],
        };
        let arguments = |pairs: &[(&str, &str)]| -> std::vec::Vec<std::ffi::OsString> {
            pairs
                .iter()
                .flat_map(|(flag, value)| {
                    [
                        std::ffi::OsString::from(*flag),
                        std::ffi::OsString::from(*value),
                    ]
                })
                .collect()
        };
        let defaults = RequestLimits::parse(&parse(&spec, &[])?)?;
        assert_eq!(defaults.input_bytes, super::DEFAULT_MAX_INPUT_BYTES);
        assert_eq!(
            defaults.retained_bytes,
            noble_ngraph::net::DEFAULT_MAX_RETAINED_BYTES
        );
        assert_eq!(defaults.output_bytes, super::DEFAULT_MAX_OUTPUT_BYTES);
        let at = std::format!("{}", super::DEFAULT_MAX_OUTPUT_BYTES);
        let above = std::format!("{}", super::DEFAULT_MAX_OUTPUT_BYTES + 1);
        let lowered = RequestLimits::parse(&parse(
            &spec,
            &arguments(&[(super::INPUT_FLAG, "0"), (super::OUTPUT_FLAG, at.as_str())]),
        )?)?;
        assert_eq!(lowered.input_bytes, 0);
        assert_eq!(lowered.output_bytes, super::DEFAULT_MAX_OUTPUT_BYTES);
        let raised = parse(&spec, &arguments(&[(super::OUTPUT_FLAG, above.as_str())]))?;
        assert!(RequestLimits::parse(&raised).is_err());
        let malformed = parse(&spec, &arguments(&[(super::RETAINED_FLAG, "-1")]))?;
        assert!(RequestLimits::parse(&malformed).is_err());
        Ok(())
    }

    #[test]
    fn the_reported_proof_identities_match_the_retained_receipt_and_sources() -> TestResult {
        let receipt_bytes = std::fs::read(repository(RECEIPT))?;
        assert_eq!(
            blake3::hash(&receipt_bytes).to_hex().as_str(),
            RECEIPT_BLAKE3
        );
        let receipt: serde_json::Value = serde_json::from_slice(&receipt_bytes)?;
        assert_eq!(receipt["source_commit"], RECEIPT_SOURCE_COMMIT);
        assert_eq!(receipt["sources_sha256"][LEAN_MODULE], LEAN_MODULE_SHA256);
        assert_eq!(receipt["sources_sha256"][INETS_MODULE], INETS_SHA256);
        let theorems = receipt["theorems"].as_array().ok_or("no theorem list")?;
        for name in COUNT_THEOREMS {
            assert!(
                theorems
                    .iter()
                    .any(|theorem| theorem["name"] == name && theorem["status"] == "proved"),
                "{name} is a proved receipt theorem"
            );
        }
        for (path, identity) in [
            (LEAN_MODULE, LEAN_MODULE_BLAKE3),
            (INETS_MODULE, INETS_BLAKE3),
        ] {
            let bytes = std::fs::read(repository(path))?;
            assert_eq!(
                blake3::hash(&bytes).to_hex().as_str(),
                identity,
                "{path} changed after its receipt; renew the receipt and these identities"
            );
        }
        let upstream: serde_json::Value =
            serde_json::from_slice(&std::fs::read(repository("proofs/inets/UPSTREAM.json"))?)?;
        assert_eq!(upstream["commit"], INETS_COMMIT);
        assert_eq!(upstream["sha256"], INETS_SHA256);
        Ok(())
    }
}
