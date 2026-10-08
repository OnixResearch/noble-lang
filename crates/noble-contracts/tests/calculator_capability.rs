use std::{fs, path::PathBuf};

use noble_contracts::source::{ModuleSession, RetainedSourceLimit, Stage};
use noble_kernel::types::Ty;

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

// Both probes feed an actual source-checked expression ending at the same
// calculator import admission word. The hostile input is a real, non-Data
// Noble Resource, not a JSON tag or a host-supplied rejection bit.
const ADMISSION: &[u8] = b"calc_session@1.empty swap quote drop nil 32768 1024 4096 64 256 5000000 8192 numeric_budget@1.make calc_session@1.stage_imported_budgeted";

fn calculator_session() -> Result<(ModuleSession, usize), String> {
    let root = std::env::var_os("NOBLE_CALCULATOR_SELECTED_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let numeric = std::env::var_os("NOBLE_CALCULATOR_NUMERIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("verification/calculator-exact-library"));
    let app = std::env::var_os("NOBLE_CALCULATOR_APP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("verification/calculator-app"));
    let mut sources = vec![
        root.join("crates/noble-contracts/src/fixtures/result.noble"),
        root.join("crates/noble-contracts/src/fixtures/calc-errors.noble"),
    ];
    for name in ["numeric_budget", "bitstep", "bitfull", "bittrim", "bitcompare",
        "bitadd", "bitsub", "bitmul", "bitdiv", "bitsigned", "bitdecode",
        "bigint", "rat", "decimal"] {
        sources.push(numeric.join(format!("{name}.noble")));
    }
    for name in ["calc_chars", "calc_token", "calc_diagnostic", "calc_code",
        "calc_lex", "calc_name", "calc_parse_state", "calc_parse", "calc_env",
        "calc_bind", "calc_eval", "calc_session"] {
        sources.push(app.join(format!("{name}.noble")));
    }
    let retained = RetainedSourceLimit::new(262_144).expect("selected retained-source cap");
    let mut session = ModuleSession::new_declared_text(&[], true, retained)
        .map_err(|error| format!("{error:?}"))?;
    let mut module_bytes = 0usize;
    for file in sources {
        let source = fs::read(&file).map_err(|error| format!("{}: {error}", file.display()))?;
        module_bytes += source.len();
        let prepared = session.prepare(&source, &[], LIMITS)
            .map_err(|error| format!("{}: {error:?}", file.display()))?;
        let (next, outcome) = session.commit(prepared);
        outcome.map_err(|error| format!("{}: {error:?}", file.display()))?;
        session = next;
    }
    Ok((session, module_bytes))
}

#[test]
fn checked_calculator_capture_and_rebind_retention() -> Result<(), String> {
    let (mut session, mut module_bytes) = calculator_session()?;
    let allowed = session.prepare(ADMISSION, &[Ty::I64], LIMITS)
        .map_err(|error| format!("ordinary capturable input rejected: {error:?}"))?;
    assert_eq!(allowed.output().len(), 1, "pure control must produce one calculator proposal");

    let resource = Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let rejected = session.prepare(ADMISSION, &[resource.clone()], LIMITS)
        .expect_err("a real Resource<test.counter> must not become an imported capture");
    assert_eq!(rejected.stage(), Stage::Acceptance);
    let word = rejected.diagnostic().join()
        .ok_or("missing source-bound Resource capture diagnostic")?;
    assert_eq!(word.word, "quote");
    assert_eq!(word.constraint, "eligibility:Data");
    assert!(word.actual_stack.contains("Resource"));
    let at = ADMISSION.windows(5).position(|text| text == b"quote")
        .ok_or("capture word absent from checked calculator admission source")?;
    assert_eq!(rejected.diagnostic().span.start as usize, at);
    assert_eq!(rejected.diagnostic().span.end as usize, at + 5);
    // Neither preparation above was committed. Real outside definitions,
    // unlike the calculator's expression-based commands, retain their exact
    // source and retain a SECOND framed copy on every accepted alias rebind.
    let first = b"module quota@1 [ signature value forall<S:stack> [ S -- S I64 ! pure ] export value def value [ 1 ] ]";
    let second = b"module quota@2 [ signature value forall<S:stack> [ S -- S I64 ! pure ] export value def value [ 2 ] ]";
    for source in [first.as_slice(), second.as_slice(), b"import quota@1 as quota"] {
        let prepared = session.prepare(source, &[], LIMITS)
            .map_err(|error| format!("quota fixture source refused: {error:?}"))?;
        let (next, outcome) = session.commit(prepared);
        outcome.map_err(|error| format!("quota fixture commit refused: {error:?}"))?;
        session = next;
    }
    module_bytes += first.len() + second.len();
    let available = 262_144usize.checked_sub(module_bytes)
        .ok_or("selected calculator modules exceed retained namespace cap")?;
    let body_bytes = (available / 2).checked_sub(4)
        .ok_or("selected calculator modules leave no outside-definition capacity")?;
    let prefix = b"def retained [ quota.value ]\n#";
    if body_bytes < prefix.len() || body_bytes > 65_536
        || 2 * (body_bytes + 4) > available || 3 * (body_bytes + 4) <= available {
        return Err("selected calculator modules leave no bounded replay boundary".into());
    }
    let mut definition = prefix.to_vec();
    definition.extend(std::iter::repeat_n(b'x', body_bytes - prefix.len()));
    let prepared = session.prepare(&definition, &[], LIMITS)
        .map_err(|error| format!("original outside definition refused: {error:?}"))?;
    let (next, outcome) = session.commit(prepared);
    outcome.map_err(|error| format!("original outside definition commit refused: {error:?}"))?;
    session = next;

    let prepared = session.prepare(b"import quota@2 as quota", &[], LIMITS)
        .map_err(|error| format!("first replay within bound refused: {error:?}"))?;
    let replayed = prepared.rebuilt_import_definitions()
        .ok_or("first rebind lacked a checked dependent replay")?;
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].name(), "retained");
    let (next, outcome) = session.commit(prepared);
    outcome.map_err(|error| format!("first replay commit refused: {error:?}"))?;
    session = next;

    let refused = session.prepare(b"import quota@1 as quota", &[], LIMITS)
        .expect_err("third framed definition source must exceed namespace quota");
    assert_eq!(refused.stage(), Stage::Check);
    assert!(refused.diagnostic().message.contains("retained"),
        "quota refusal must be a retained-source diagnostic: {refused:?}");
    session.prepare(b"retained", &[], LIMITS)
        .map_err(|error| format!("rejected replay damaged prior definition: {error:?}"))?;
    session.prepare(b"quota.value", &[], LIMITS)
        .map_err(|error| format!("rejected replay damaged published alias: {error:?}"))?;
    Ok(())
}
