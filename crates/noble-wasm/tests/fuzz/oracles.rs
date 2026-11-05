//! Property oracles shared by the libFuzzer targets in `crates/noble-wasm/fuzz`
//! and the workspace replay in `replay.rs`. Each oracle takes the raw fuzzer
//! input, so a committed crasher replays through exactly the code that found it.
//!
//! Input layout: a five-byte header, then the subject bytes unchanged (no
//! padding after the source, so a lexer that peeks ahead sees the real end).
//! Header byte 0's low two bits select the `Limits`: `0b11` is
//! `Limits::default()`, anything else is a deliberately tiny budget built from
//! bytes 0–3 (bytes 1–512, nodes 1–64, depth 1–8, work 1–4096), so the limit
//! and overflow branches are reached constantly rather than almost never. A
//! module session's retained-source cap is budgeted the same way (1–4081 bytes
//! from byte 2 when tiny, 65,536 otherwise).
//! Header byte 4 selects a frontend profile where a target has more than one.
//!
//! The oracles check statement shape only: they do not show that any accepted
//! program is correct, only that the frontend and backend never contradict
//! their own diagnostics.
#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::non_trait_imports,
    tigerstyle::compound_condition,
    tigerstyle::fragile_exhaustive_enum_match,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::function_length,
    tigerstyle::allocation_in_loop,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; these oracles compare whole frontend and backend outcomes on hostile input, so exhaustive outcome matches, owned comparison strings and long property legs are the readable form, and every collected module or report is bounded by the one input under test."
)]

/// Bytes before the subject.
pub(crate) const HEADER: usize = 5;

/// The header byte that selects a frontend profile.
pub(crate) const SELECT: usize = 4;

/// Header byte 0 selects `Limits::default()` when both low bits are set.
pub(crate) const DEFAULT_LIMITS: u8 = 0b11;

/// The retained-source cap of a module session under the default budget.
const RETAINED_BYTES: u32 = 65_536;

/// One decoded fuzzer input.
pub(crate) struct Case<'a> {
    pub(crate) limits: noble_contracts::Limits,
    pub(crate) retained: u32,
    pub(crate) select: u8,
    pub(crate) body: &'a [u8],
}

/// Decode the header. Inputs shorter than it carry no subject and are skipped.
pub(crate) fn case(data: &[u8]) -> Option<Case<'_>> {
    let (header, body) = data.split_at_checked(HEADER)?;
    let (limits, retained) = if header[0] & DEFAULT_LIMITS == DEFAULT_LIMITS {
        (noble_contracts::Limits::default(), RETAINED_BYTES)
    } else {
        (
            noble_contracts::Limits {
                bytes: 1 + u32::from(header[1]) * 2,
                nodes: 1 + u32::from(header[2] % 64),
                depth: 1 + u32::from(header[3] % 8),
                work: 1 + u32::from(header[0]) * 16,
            },
            1 + u32::from(header[2]) * 16,
        )
    };
    Some(Case {
        limits,
        retained,
        select: header[SELECT],
        body,
    })
}

/// Prefix a subject with a header, for seeds and committed regressions.
pub(crate) fn encode(header: [u8; HEADER], body: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(HEADER + body.len());
    data.extend_from_slice(&header);
    data.extend_from_slice(body);
    data
}

/// Whether `limits` is pointwise within the default budget, so a second run
/// under the default budget only ever adds headroom.
fn within_default(limits: noble_contracts::Limits) -> bool {
    let wide = noble_contracts::Limits::default();
    limits.bytes <= wide.bytes
        && limits.nodes <= wide.nodes
        && limits.depth <= wide.depth
        && limits.work <= wide.work
}

/// The comparable part of one diagnostic: its category, its span and its text.
fn diagnostic(value: &noble_contracts::Diagnostic) -> String {
    format!(
        "{}@{}..{}: {}",
        value.kind.category(),
        value.span.start,
        value.span.end,
        value.message
    )
}

/// A frontend outcome reduced to what a larger budget must reproduce.
enum Outcome {
    /// Accepted, with a limit-independent projection of what was accepted.
    Accepted(String),
    /// Refused for a reason other than the budget: the stage, category and span
    /// a larger budget must reproduce, then the full text for the report. The
    /// message itself is not compared, because its optional join provenance is
    /// rendered under whatever budget remains and is detail, not the outcome.
    Refused(String, String),
    /// Refused because the budget ran out.
    Exhausted(noble_contracts::Diagnostic),
}

impl Outcome {
    fn refusal(value: &noble_contracts::Diagnostic, stage: String) -> Result<Self, String> {
        match value.kind {
            noble_contracts::DiagnosticKind::Internal => Err(format!(
                "internal frontend diagnostic: {stage}{}",
                diagnostic(value)
            )),
            noble_contracts::DiagnosticKind::Exhausted => Ok(Self::Exhausted(value.clone())),
            noble_contracts::DiagnosticKind::Invalid
            | noble_contracts::DiagnosticKind::Unsupported => Ok(Self::Refused(
                format!(
                    "{stage}{}@{}..{}",
                    value.kind.category(),
                    value.span.start,
                    value.span.end
                ),
                format!("{stage}{}", diagnostic(value)),
            )),
        }
    }
}

/// Monotonicity: an outcome reached without running out of budget is reached
/// again, identically, under any pointwise larger budget, so a budget refusal is
/// never reported as an invalid or unsupported input. An exhausted run may end
/// in any outcome under a larger budget. In particular an invalid diagnostic
/// there may name an earlier construct than the place the budget ran out: each
/// stage reads the whole subject before the next one judges it, so a later
/// stage can refuse a construct an earlier stage already paid for.
fn monotone(small: &Outcome, wide: &Outcome) -> Result<(), String> {
    match (small, wide) {
        (Outcome::Accepted(left), Outcome::Accepted(right)) => {
            if left == right {
                Ok(())
            } else {
                Err(format!(
                    "accepted outcome changed under a larger budget:\n  small: {left}\n  wide:  {right}"
                ))
            }
        }
        (Outcome::Refused(left, left_text), Outcome::Refused(right, right_text)) => {
            if left == right {
                Ok(())
            } else {
                Err(format!(
                    "refusal changed under a larger budget:\n  small: {left_text}\n  wide:  {right_text}"
                ))
            }
        }
        (Outcome::Accepted(left), Outcome::Refused(..) | Outcome::Exhausted(_)) => Err(format!(
            "accepted under the small budget but not the wide one:\n  small: {left}\n  wide:  {}",
            describe(wide)
        )),
        (Outcome::Refused(_, left), Outcome::Accepted(_) | Outcome::Exhausted(_)) => Err(format!(
            "refused under the small budget but not identically under the wide one:\n  small: {left}\n  wide:  {}",
            describe(wide)
        )),
        (Outcome::Exhausted(_), _) => Ok(()),
    }
}

fn describe(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Accepted(text) | Outcome::Refused(_, text) => text.clone(),
        Outcome::Exhausted(value) => diagnostic(value),
    }
}

fn contract_outcome(
    result: &Result<noble_contracts::Prepared, noble_contracts::Diagnostic>,
) -> Result<Outcome, String> {
    match result {
        Ok(prepared) => Ok(Outcome::Accepted(format!(
            "{} {:?} {:?} {:?} {:?} {:?}",
            prepared.name(),
            prepared.candidate(),
            prepared.checked(),
            prepared.definitions(),
            prepared.expressions(),
            (prepared.requires(), prepared.ensures())
        ))),
        Err(value) => Outcome::refusal(value, String::new()),
    }
}

/// `noble_contracts::prepare`: never `Internal`, deterministic, and monotone in
/// its budget.
pub(crate) fn check_prepare(data: &[u8]) -> Result<(), String> {
    let Some(case) = case(data) else {
        return Ok(());
    };
    let first = noble_contracts::prepare(case.body, case.limits);
    let again = noble_contracts::prepare(case.body, case.limits);
    if format!("{first:?}") != format!("{again:?}") {
        return Err(String::from(
            "prepare is not deterministic on identical input",
        ));
    }
    let small = contract_outcome(&first)?;
    if case.limits == noble_contracts::Limits::default() || !within_default(case.limits) {
        return Ok(());
    }
    let wide = contract_outcome(&noble_contracts::prepare(
        case.body,
        noble_contracts::Limits::default(),
    ))?;
    monotone(&small, &wide)
}

/// The four source-frontend profiles a host can select without bindings.
#[derive(Clone, Copy)]
enum Profile {
    Core,
    CoreWithoutHosts,
    Module,
    Calculator,
}

impl Profile {
    fn select(byte: u8) -> Self {
        match byte % 4 {
            0 => Self::Core,
            1 => Self::CoreWithoutHosts,
            2 => Self::Module,
            _ => Self::Calculator,
        }
    }
}

fn source_error(value: &noble_contracts::source::Error) -> Result<Outcome, String> {
    Outcome::refusal(value.diagnostic(), format!("{:?} ", value.stage()))
}

/// Units a source input holds: NUL-separated, prepared and committed in order,
/// so declarations, imports and the retained-source cap meet across units.
const UNITS: usize = 4;

/// One source-frontend run: every unit is prepared and committed in order and
/// the first refusal ends the run. Returns the run's full text, for
/// determinism, and its outcome. A prepared value records the budget it was
/// prepared under, so an accepted outcome is compared across budgets by class.
fn source_run(
    profile: Profile,
    body: &[u8],
    limits: noble_contracts::Limits,
    retained: u32,
) -> Result<(String, Outcome), String> {
    let units = body.split(|byte| *byte == 0).take(UNITS);
    let mut text = String::new();
    match profile {
        Profile::Core | Profile::CoreWithoutHosts => {
            let mut session = match profile {
                Profile::Core => noble_contracts::source::Session::new(),
                _ => noble_contracts::source::Session::without_test_hosts(),
            };
            for unit in units {
                let result = session.prepare(unit, &[], limits);
                text.push_str(&format!("{result:?} / "));
                let prepared = match result {
                    Ok(prepared) => prepared,
                    Err(error) => return Ok((text, source_error(&error)?)),
                };
                let linked = session.commit(prepared);
                text.push_str(&format!("{linked:?} / "));
                if let Err(error) = linked {
                    return Ok((text, source_error(&error)?));
                }
            }
        }
        Profile::Module | Profile::Calculator => {
            let created = match profile {
                Profile::Module => noble_contracts::source::ModuleSession::new(
                    &[],
                    noble_contracts::source::RetainedSourceLimit::new(retained)
                        .ok_or("the retained source cap is above its maximum")?,
                ),
                _ => noble_contracts::source::ModuleSession::new_calculator(),
            };
            let mut session = created.map_err(|error| {
                format!(
                    "an empty module session was refused: {:?}",
                    error.diagnostic()
                )
            })?;
            for unit in units {
                let result = session.prepare(unit, &[], limits);
                text.push_str(&format!("{result:?} / "));
                let prepared = match result {
                    Ok(prepared) => prepared,
                    Err(error) => return Ok((text, source_error(&error)?)),
                };
                let (next, linked) = session.commit(prepared);
                session = next;
                text.push_str(&format!("{linked:?} / "));
                if let Err(error) = linked {
                    return Ok((text, source_error(&error)?));
                }
            }
        }
    }
    Ok((text, Outcome::Accepted(String::from("accepted"))))
}

/// The source frontend (lexer, parser, declared modules, linking): never
/// `Internal`, deterministic, and monotone in its budget.
pub(crate) fn check_source(data: &[u8]) -> Result<(), String> {
    let Some(case) = case(data) else {
        return Ok(());
    };
    let profile = Profile::select(case.select);
    let (first, small) = source_run(profile, case.body, case.limits, case.retained)?;
    let (again, _) = source_run(profile, case.body, case.limits, case.retained)?;
    if first != again {
        return Err(String::from(
            "source preparation is not deterministic on identical input",
        ));
    }
    if case.limits == noble_contracts::Limits::default()
        || !within_default(case.limits)
        || case.retained > RETAINED_BYTES
    {
        return Ok(());
    }
    let (_, wide) = source_run(
        profile,
        case.body,
        noble_contracts::Limits::default(),
        RETAINED_BYTES,
    )?;
    monotone(&small, &wide)
}

fn backend(error: noble_wasm::Diagnostic) -> &'static str {
    match error {
        noble_wasm::Diagnostic::Invalid => "invalid",
        noble_wasm::Diagnostic::Exhausted => "exhausted",
        noble_wasm::Diagnostic::Unsupported => "unsupported",
        noble_wasm::Diagnostic::Defective => "defective",
    }
}

/// The backend accepts what the frontend accepts: for every prepared contract,
/// both representations either emit WAT that parses, deterministically, or
/// refuse it as unsupported or exhausted. `Defective` is the backend's own
/// "this is a bug" outcome, and `Invalid` would mean its admission recheck
/// contradicts the frontend's acceptance. Returns every emitted binary module
/// so a caller can validate it further.
pub(crate) fn check_prepare_to_wasm(data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let Some(case) = case(data) else {
        return Ok(Vec::new());
    };
    let prepared = match noble_contracts::prepare(case.body, case.limits) {
        Ok(prepared) => prepared,
        Err(value) => {
            return match value.kind {
                noble_contracts::DiagnosticKind::Internal => Err(format!(
                    "internal frontend diagnostic: {}",
                    diagnostic(&value)
                )),
                noble_contracts::DiagnosticKind::Invalid
                | noble_contracts::DiagnosticKind::Unsupported
                | noble_contracts::DiagnosticKind::Exhausted => Ok(Vec::new()),
            };
        }
    };
    let mut modules = Vec::new();
    for representation in [
        noble_wasm::Representation::WasmGc,
        noble_wasm::Representation::ManagedLinearMemory,
    ] {
        let first = noble_wasm::compile(representation, prepared.request(), prepared.candidate());
        let again = noble_wasm::compile(representation, prepared.request(), prepared.candidate());
        if first != again {
            return Err(String::from("backend compilation is not deterministic"));
        }
        match first {
            Ok(text) => {
                let binary = wat::parse_bytes(&text)
                    .map_err(|error| format!("emitted WAT does not parse: {error}"))?;
                modules.push(binary.into_owned());
            }
            Err(error @ (noble_wasm::Diagnostic::Defective | noble_wasm::Diagnostic::Invalid)) => {
                return Err(format!(
                    "backend refused a frontend-accepted contract as {}",
                    backend(error)
                ));
            }
            Err(noble_wasm::Diagnostic::Unsupported | noble_wasm::Diagnostic::Exhausted) => {}
        }
    }
    Ok(modules)
}

/// The first `world NAME` the input names, so a mutated name stays selected.
pub(crate) fn world_name(wit: &[u8]) -> &str {
    let keyword = b"world ";
    let Some(start) = wit
        .windows(keyword.len())
        .position(|window| window == keyword)
    else {
        return "bounded";
    };
    let rest = &wit[start + keyword.len()..];
    let end = rest
        .iter()
        .position(|byte| !(byte.is_ascii_alphanumeric() || *byte == b'-'))
        .unwrap_or(rest.len());
    match core::str::from_utf8(&rest[..end]) {
        Ok(name) if !name.is_empty() => name,
        Ok(_) | Err(_) => "bounded",
    }
}

/// A parsed world records the budget it was parsed under, so the accepted
/// projection names everything else it selected.
fn world_outcome(
    result: &Result<noble_contracts::component::World, noble_contracts::component::Error>,
) -> Result<Outcome, String> {
    match result {
        Ok(world) => {
            if let Err(error) = world.environment() {
                if error.diagnostic.kind == noble_contracts::DiagnosticKind::Internal {
                    return Err(format!(
                        "internal diagnostic building a parsed world's environment: {}",
                        diagnostic(&error.diagnostic)
                    ));
                }
            }
            Ok(Outcome::Accepted(format!(
                "{} {} {:?} {:?} {:?} {} {}",
                world.identity(),
                world.name(),
                world.imports(),
                world.exports(),
                world.resources(),
                world.is_async(),
                world.checked_u64()
            )))
        }
        Err(error) => Outcome::refusal(&error.diagnostic, format!("{:?} ", error.stage)),
    }
}

fn parse_world(
    wit: &[u8],
    world: &str,
    limits: noble_contracts::Limits,
    checked_u64: bool,
) -> Result<noble_contracts::component::World, noble_contracts::component::Error> {
    if checked_u64 {
        noble_contracts::component::World::parse_checked_u64(wit, world, limits)
    } else {
        noble_contracts::component::World::parse(wit, world, limits)
    }
}

/// The WIT world parser: never `Internal`, deterministic, and monotone in its
/// budget, for both the plain and the checked-u64 boundary.
pub(crate) fn check_wit_world(data: &[u8]) -> Result<(), String> {
    let Some(case) = case(data) else {
        return Ok(());
    };
    let world = world_name(case.body);
    for checked_u64 in [false, true] {
        let first = parse_world(case.body, world, case.limits, checked_u64);
        let again = parse_world(case.body, world, case.limits, checked_u64);
        if format!("{first:?}") != format!("{again:?}") {
            return Err(String::from(
                "WIT parsing is not deterministic on identical input",
            ));
        }
        let small = world_outcome(&first)?;
        if case.limits == noble_contracts::Limits::default() || !within_default(case.limits) {
            continue;
        }
        let wide = world_outcome(&parse_world(
            case.body,
            world,
            noble_contracts::Limits::default(),
            checked_u64,
        ))?;
        monotone(&small, &wide)?;
    }
    Ok(())
}
