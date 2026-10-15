use noble_kernel::contracts::{Behavior, Definition};
use noble_kernel::untrusted::{Constraint, Node, Outcome};

fn limits() -> noble_contracts::Limits {
    noble_contracts::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        work: 2_000_000,
    }
}

#[test]
fn calculator_retains_sixty_four_long_source_programs_in_one_code_table() -> Result<(), String> {
    let mut body = String::new();
    for _ in 0..128 {
        body.push_str("1 drop ");
    }
    let mut session = noble_contracts::source::Session::new_calculator();
    let mut compiler = noble_wasm::source::Compiler::new_calculator();
    let mut carried = Vec::new();
    // The original 256 entries per repeated body exhaust the fixed table
    // during this history. Preserve the first quotation as an executable
    // input through every successful source/compiler commit.
    for generation in 0..66 {
        let source = if generation == 0 {
            "[ 1 + ]"
        } else if generation == 65 {
            "41 swap run"
        } else {
            &body
        };
        let checked = session.prepare(source.as_bytes(), &carried, limits())
            .map_err(|error| format!("generation {generation} source: {error:?}"))?;
        let next_carried = checked.output().to_vec();
        let prepared = compiler.prepare(checked.submission().ok_or("source omitted its submission")?)
            .map_err(|_| format!("generation {generation} compilation failed"))?;
        if !compiler.can_commit(&prepared) {
            return Err(format!("generation {generation} lost publication provenance"));
        }
        compiler.commit(prepared)
            .map_err(|_| format!("generation {generation} commit failed"))?;
        session.commit(checked)
            .map_err(|error| format!("generation {generation} source commit: {error:?}"))?;
        carried = next_carried;
    }
    assert_eq!(carried, [noble_kernel::types::Ty::I64]);
    Ok(())
}

#[test]
fn calculator_iteration_requires_its_exact_profile_and_private_prefix() -> Result<(), String> {
    let calculator = noble_contracts::source::Session::new_calculator();
    let mut ordinary_namespace = noble_contracts::source::Session::without_test_hosts();
    let pending = calculator.prepare(b"def callback [ 1 + ]", &[], limits())
        .map_err(|e| format!("{e:?}"))?;
    assert!(matches!(ordinary_namespace.commit(pending), Err(error)
        if error.stage() == noble_contracts::source::Stage::Acceptance));
    assert_eq!(ordinary_namespace.generation(), 0);
    let pending = ordinary_namespace.prepare(b"def ordinary [ 1 + ]", &[], limits())
        .map_err(|e| format!("{e:?}"))?;
    let mut calculator_namespace = noble_contracts::source::Session::new_calculator();
    assert!(matches!(calculator_namespace.commit(pending), Err(error)
        if error.stage() == noble_contracts::source::Stage::Acceptance));
    assert_eq!(calculator_namespace.generation(), 0);
    let length = calculator.prepare("\"Aé\" text.length".as_bytes(), &[], limits())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(length.output(), &[noble_kernel::types::Ty::Text, noble_kernel::types::Ty::I64]);
    let body = b"0 [ dup 3 i64.cmp 0 = [ false ] [ 1 + true ] if ] iterate";
    let prepared = calculator.prepare(body, &[], limits()).map_err(|e| format!("{e:?}"))?;
    let submission = prepared.submission().ok_or("no executable candidate")?;
    assert_eq!(prepared.output(), &[noble_kernel::types::Ty::I64]);
    noble_wasm::source::Compiler::new_calculator()
        .prepare(submission).map_err(|_| "calculator compiler rejected the guest iteration")?;
    if noble_wasm::source::Compiler::new().prepare(submission).is_ok()
        || noble_wasm::source::Compiler::new_text_cursor().prepare(submission).is_ok()
    {
        return Err("calculator submission crossed a different compiler profile".into());
    }
    for private in 22..=25 {
        let mut forged = submission.clone();
        let invocation = forged.body.candidate.nodes.iter_mut().find(|node|
            matches!(node, Node::Invocation { def, .. } if *def == Definition(27)))
            .ok_or("no checked guest iteration")?;
        if let Node::Invocation { def, .. } = invocation {
            *def = Definition(private);
        }
        match noble_kernel::acceptance::check(&forged.environment, &forged.request, &forged.body.candidate) {
            Outcome::Invalid(diagnostic) if matches!(diagnostic.constraint,
                Constraint::PrivateDefinition(def) if def == Definition(private)) => {}
            _ => return Err(format!("private definition {private} reached guest code")),
        }
        if noble_wasm::source::Compiler::new_calculator().prepare(&forged).is_ok() {
            return Err(format!("private definition {private} reached backend emission"));
        }
    }
    let mut forged = submission.clone();
    forged.environment.kinds[27] = Behavior::Reserved;
    if noble_wasm::source::Compiler::new_calculator().prepare(&forged).is_ok() {
        return Err("forged calculator prefix reached backend emission".into());
    }
    let mut forged = submission.clone();
    forged.environment.calculator = false;
    forged.environment.text_cursor = true;
    if noble_wasm::source::Compiler::new_text_cursor().prepare(&forged).is_ok() {
        return Err("calculator rows crossed the standalone cursor prefix".into());
    }
    let mut forged = submission.clone();
    forged.environment.calculator = false;
    if noble_wasm::source::Compiler::new().prepare(&forged).is_ok() {
        return Err("calculator rows crossed the ordinary prefix".into());
    }
    assert!(noble_contracts::source::Session::without_test_hosts().prepare(body, &[], limits()).is_err());
    assert!(noble_contracts::source::Session::new_text_cursor().prepare(body, &[], limits()).is_err());
    assert!(noble_contracts::source::Session::new_text_cursor()
        .prepare(b"\"abc\" text.length", &[], limits()).is_err());
    Ok(())
}

#[test]
fn calculator_modules_do_not_publish_into_ordinary_declared_sessions() -> Result<(), String> {
    let module = b"module digits@1 [ export count def count [ 0 [ dup 2 i64.cmp 0 = [ false ] [ 1 + true ] if ] iterate ] ]";
    let calculator = noble_contracts::source::ModuleSession::new_calculator().map_err(|e| format!("{e:?}"))?;
    let missing_adapter = calculator.prepare(
        b"module side@1 [ require emit Text -- ! test.emit export call def call [ \"audit\" emit ] ]",
        &[], limits(),
    ).expect_err("calculator module cannot acquire a host effect");
    assert_eq!(missing_adapter.stage(), noble_contracts::source::Stage::Link);
    let pending = calculator.prepare(module, &[], limits()).map_err(|e| format!("{e:?}"))?;
    let ordinary = noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    let (ordinary, result) = ordinary.commit(pending);
    assert!(matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance));
    assert_eq!(ordinary.generation(), 0);
    let pending = ordinary.prepare(
        b"module plain@1 [ export count def count [ 1 ] ]", &[], limits(),
    ).map_err(|e| format!("{e:?}"))?;
    let (calculator, result) = calculator.commit(pending);
    assert!(matches!(result, Err(error)
        if error.stage() == noble_contracts::source::Stage::Acceptance));
    assert_eq!(calculator.generation(), 0);
    assert!(ordinary.prepare(b"digits@1.count", &[], limits()).is_err());
    assert!(ordinary.prepare(module, &[], limits()).is_err());
    let pending = calculator.prepare(module, &[], limits()).map_err(|e| format!("{e:?}"))?;
    let (calculator, result) = calculator.commit(pending);
    result.map_err(|e| format!("{e:?}"))?;
    let candidate = calculator.prepare(b"digits@1.count", &[], limits()).map_err(|e| format!("{e:?}"))?;
    noble_wasm::source::Compiler::new_calculator().prepare(candidate.submission().ok_or("no module candidate")?)
        .map_err(|_| "calculator compiler rejected checked nominal module")?;
    Ok(())
}

#[test]
fn calculator_compiles_private_nominal_across_checked_phase_boundaries() -> Result<(), String> {
    let source = b"module capsule@1 [
        opaque Secret Pair<I64,I64> private
        export Secret
        signature make forall<S:stack> [ S -- S Secret ! pure ]
        export make
        def make [ 7 11 pair Secret.new ]
        signature sum forall<S:stack> [ S Secret -- S I64 ! pure ]
        export sum
        def sum [ Secret.into unpair + ]
    ]";
    let session = noble_contracts::source::ModuleSession::new_calculator()
        .map_err(|error| format!("{error:?}"))?;
    let pending = session.prepare(source, &[], limits())
        .map_err(|error| format!("{error:?}"))?;
    let (mut session, installed) = session.commit(pending);
    installed.map_err(|error| format!("{error:?}"))?;
    let mut compiler = noble_wasm::source::Compiler::new_calculator();
    let make = session.prepare(b"capsule@1.make", &[], limits())
        .map_err(|error| format!("{error:?}"))?;
    let carrier = make.output().to_vec();
    assert_eq!(carrier.len(), 1);
    assert!(matches!(carrier[0], noble_kernel::types::Ty::Nominal(_, _)));
    let stale = compiler.prepare(make.submission().ok_or("missing creator candidate")?)
        .map_err(|_| "calculator refused independent creator preparation")?;
    let ready = compiler.prepare(make.submission().ok_or("missing make phase")?)
        .map_err(|_| "calculator refused creator phase")?;
    assert!(compiler.can_commit(&ready));
    compiler.commit(ready).map_err(|_| "creator phase did not commit")?;
    assert!(!compiler.can_commit(&stale));
    assert!(compiler.commit(stale).is_err(), "stale compiler state was published");
    let (next, committed) = session.commit(make);
    committed.map_err(|error| format!("{error:?}"))?;
    session = next;
    let mut forged = carrier.clone();
    if let noble_kernel::types::Ty::Nominal(id, _) = &mut forged[0] {
        id.module += 100;
    }
    assert!(session.prepare(b"capsule@1.sum", &forged, limits()).is_err(),
        "another module's nominal identity entered the next checked phase");
    let sum = session.prepare(b"capsule@1.sum", &carrier, limits())
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(sum.output(), &[noble_kernel::types::Ty::I64]);
    let mut forged_submission = sum.submission().ok_or("no consumer candidate")?.clone();
    if let noble_kernel::types::Ty::Nominal(id, _) =
        &mut forged_submission.request.expected.stack_in[0]
    {
        id.module += 100;
    }
    assert!(compiler.prepare(&forged_submission).is_err(),
        "forged nominal identity reached backend emission");
    let ready = compiler.prepare(sum.submission().ok_or("missing consumer phase")?)
        .map_err(|_| "calculator refused consumer phase")?;
    assert!(compiler.can_commit(&ready));
    compiler.commit(ready).map_err(|_| "consumer phase did not commit")?;
    let private = session.prepare(b"capsule@1.Secret.into", &carrier, limits());
    assert!(private.is_err(), "root caller acquired private nominal payload");
    Ok(())
}

#[test]
fn calculator_preflight_admits_all_256_named_rows_without_widening_ordinary() -> Result<(), String> {
    let generous = noble_contracts::Limits {
        bytes: 262_144,
        nodes: 262_144,
        depth: 64,
        work: 512_000_000,
    };
    // The calculator owns 32 fixed rows; ordinary source admission owns 26.
    // Exercise both sides of the ordinary ceiling with real checked module
    // definitions rather than malformed spare rows. The separate selected
    // maximum is 512 named definitions.
    for named in 250..=256 {
        let mut source = String::from("module bulk@1 [");
        for index in 0..named {
            source.push_str(&format!(" export w{index} def w{index} [ 1 ]"));
        }
        source.push_str(" ]");
        let session = noble_contracts::source::ModuleSession::new_calculator()
            .map_err(|error| format!("{error:?}"))?;
        let pending = session.prepare(source.as_bytes(), &[], generous)
            .map_err(|error| format!("module with {named} definitions: {error:?}"))?;
        let (session, installed) = session.commit(pending);
        installed.map_err(|error| format!("module with {named} definitions: {error:?}"))?;
        let mut query = String::new();
        for index in 0..named {
            if index != 0 {
                query.push_str(" drop ");
            }
            query.push_str(&format!("bulk@1.w{index}"));
        }
        let checked = session.prepare(query.as_bytes(), &[], generous)
            .map_err(|error| format!("lookup with {named} definitions: {error:?}"))?;
        let submission = checked.submission().ok_or("no executable candidate")?;
        assert_eq!(submission.environment.defs.len(), 32 + named);
        assert_eq!(submission.definitions.len(), named);
        noble_wasm::source::Compiler::new_calculator().prepare(submission)
            .map_err(|_| format!("calculator rows {} rejected", 32 + named))?;
        // Even an Env bearing the genuine calculator marker cannot opt an
        // ordinary compiler into its larger preflight or canonical prefix.
        let ordinary = noble_wasm::source::Compiler::new().prepare(submission);
        if named > 250 {
            assert!(matches!(ordinary, Err(noble_wasm::Diagnostic::Exhausted)));
        } else {
            assert!(ordinary.is_err());
        }
    }
    Ok(())
}

#[test]
fn calculator_compiles_ten_module_parser_commit_without_spending_kernel_budget() -> Result<(), String> {
    use noble_kernel::types::Ty;

    let generous = noble_contracts::Limits {
        bytes: 262_144,
        nodes: 524_288,
        depth: 64,
        work: 512_000_000,
    };
    let fixtures = std::env::var("NOBLE_CALCULATOR_FIXTURE_DIR").unwrap_or_else(|_|
        format!("{}/../noble-contracts/src/fixtures", env!("CARGO_MANIFEST_DIR")));
    let mut session = noble_contracts::source::ModuleSession::new_calculator()
        .map_err(|error| format!("{error:?}"))?;
    for name in [
        "result.noble", "calc-errors.noble", "calculator-lexical.noble",
        "calculator-bits.noble", "calculator-bigint.noble", "calculator-decimal.noble",
        "calculator-numeric.noble", "calculator-bindings.noble", "calculator-display.noble",
        "calculator-expression.noble",
    ] {
        let source = std::fs::read(format!("{fixtures}/{name}"))
            .map_err(|error| format!("{name}: {error}"))?;
        let pending = session.prepare(&source, &[], generous)
            .map_err(|error| format!("{name}: {error:?}"))?;
        let (next, committed) = session.commit(pending);
        committed.map_err(|error| format!("{name} commit: {error:?}"))?;
        session = next;
    }
    let imported = session.prepare(b"import expr@1 as expr", &[], generous)
        .map_err(|error| format!("parser import: {error:?}"))?;
    let (next, committed) = session.commit(imported);
    committed.map_err(|error| format!("parser import commit: {error:?}"))?;
    session = next;
    let state = session.resolve_type("calc_env@1.State")
        .map_err(|error| format!("State: {error:?}"))?;
    let cursor = session.resolve_type("calc_lex@1.ScanCursor")
        .map_err(|error| format!("ScanCursor: {error:?}"))?;
    let guest = |root: &str, input: &[Ty]| -> Result<Vec<Ty>, String> {
        session.prepare(root.as_bytes(), input, generous)
            .map(|pending| pending.output().to_vec())
            .map_err(|error| format!("{root}: {error:?}"))
    };
    let finished = guest("calc_lex@1.scan_finish_begin", &[cursor])?;
    let tokens = guest("calc_lex@1.scan_finish_tokens", &finished)?;
    let mut stack = vec![state, Ty::Bool, Ty::Text];
    stack.extend(tokens);
    stack.extend(std::iter::repeat_n(Ty::I64, 7));
    let stack = guest("num@1.budget", &stack)?;
    let stack = guest("swap", &stack)?;
    let measured = guest("[ expr.preparse_input_measure ] dip", &stack)?;
    let checked = guest("[ expr.preparse_input_check ] dip", &measured)?;
    let pending = session.prepare(b"[ expr.preparse_input_commit ] dip", &checked, generous)
        .map_err(|error| format!("parser commit source: {error:?}"))?;
    let submission = pending.submission().ok_or("parser commit has no executable body")?;
    let mut compiler = noble_wasm::source::Compiler::new_calculator();
    let compiled = compiler.prepare(submission)
        .map_err(|_| "checked parser commit WAT was refused")?;
    let stale = compiler.prepare(submission)
        .map_err(|_| "second parser commit WAT was refused")?;
    compiler.commit(compiled)
        .map_err(|_| "checked parser commit publication was refused")?;
    assert!(!compiler.can_commit(&stale), "stale calculator compilation crossed a generation");
    assert!(noble_wasm::source::Compiler::new().prepare(submission).is_err());
    assert!(noble_wasm::source::Compiler::new_text_cursor().prepare(submission).is_err());

    let mut overdeep = submission.clone();
    let mut ty = Ty::I64;
    for _ in 0..65 {
        ty = Ty::List(Box::new(ty));
    }
    *overdeep.request.expected.stack_out.first_mut().ok_or("missing result")? = ty;
    assert!(matches!(
        noble_wasm::source::Compiler::new_calculator().prepare(&overdeep),
        Err(noble_wasm::Diagnostic::Exhausted),
    ), "selected borrowed allowance bypassed the independent physical type-depth bound");
    Ok(())
}

#[test]
fn calculator_checks_real_derived_rows_separately_from_named_bodies() -> Result<(), String> {
    use noble_kernel::types::Ty;

    let generous = noble_contracts::Limits {
        bytes: 262_144,
        nodes: 524_288,
        depth: 64,
        work: 512_000_000,
    };
    let fixtures = format!("{}/../noble-contracts/src/fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut session = noble_contracts::source::ModuleSession::new_calculator()
        .map_err(|error| format!("{error:?}"))?;
    for name in [
        "result.noble", "calc-errors.noble", "calculator-lexical.noble",
        "calculator-bits.noble", "calculator-bigint.noble", "calculator-decimal.noble",
        "calculator-numeric.noble", "calculator-bindings.noble",
        "calculator-display.noble", "calculator-expression.noble",
        "calculator-definitions.noble",
    ] {
        let source = std::fs::read(format!("{fixtures}/{name}"))
            .map_err(|error| format!("{name}: {error}"))?;
        let pending = session.prepare(&source, &[], generous)
            .map_err(|error| format!("{name}: {error:?}"))?;
        let (next, committed) = session.commit(pending);
        committed.map_err(|error| format!("{name} commit: {error:?}"))?;
        session = next;
    }
    let imported = session.prepare(b"import calc_lex@1 as lex", &[], generous)
        .map_err(|error| format!("lexical import: {error:?}"))?;
    let (next, committed) = session.commit(imported);
    committed.map_err(|error| format!("lexical import commit: {error:?}"))?;
    session = next;
    let input = [
        session.resolve_type("calc_env@1.State").map_err(|error| format!("{error:?}"))?,
        Ty::Bool,
        session.resolve_type("expr@1.FrameRun").map_err(|error| format!("{error:?}"))?,
        session.resolve_type("calc_lex@1.ScanCursor").map_err(|error| format!("{error:?}"))?,
    ];
    let checked = session.prepare(b"swap [ lex.scan_tick_integer_byte ] dip", &input, generous)
        .map_err(|error| format!("protected lexical phase: {error:?}"))?;
    let submission = checked.submission().ok_or("missing protected lexical submission")?;
    if submission.environment.defs.len() <= 544 || submission.environment.defs.len() > 896
        || submission.definitions.len() > 512
    {
        return Err("real protected lexical phase did not cross the old environment-row limit".into());
    }
    noble_wasm::source::Compiler::new_calculator().prepare(submission)
        .map_err(|_| "real derived environment rows refused independent selected backend")?;

    let ascii_input = [
        session.resolve_type("calc_env@1.State").map_err(|error| format!("{error:?}"))?,
        Ty::Bool,
        session.resolve_type("expr@1.FormatCounted").map_err(|error| format!("{error:?}"))?,
    ];
    let ascii = session.prepare(b"expr@1.format_ascii_fraction", &ascii_input, generous)
        .map_err(|error| format!("source-bound ASCII fraction: {error:?}"))?;
    let ascii = ascii.submission().ok_or("missing source-bound ASCII fraction")?;
    let bytes = ascii.environment.calculator_clone_budget()
        .ok_or("real ASCII fraction exceeded calculator clone budget")?;
    let copies = u64::try_from(ascii.definitions.len() + 1).map_err(|error| format!("{error}"))?;
    assert!(bytes.reserved_bytes.checked_mul(copies).is_some_and(|total|
        total <= noble_kernel::contracts::CALCULATOR_ENV_CLONE_FANOUT_LIMIT),
        "real ASCII fraction exceeds the borrowed preclone copy allowance");
    noble_wasm::source::Compiler::new_calculator().prepare(ascii)
        .map_err(|_| "current ASCII fraction refused selected backend")?;

    let mut shuffled_generic = submission.clone();
    let generic = shuffled_generic.environment.generic_variants.first_mut()
        .ok_or("real calculator environment omitted the generic Result declaration")?;
    generic.payload_params.swap(0, 1);
    assert!(matches!(noble_wasm::source::Compiler::new_calculator().prepare(&shuffled_generic),
        Err(noble_wasm::Diagnostic::Invalid)),
        "shuffled generic payload declaration bypassed complete environment replay");

    let mut wrong_operation = submission.clone();
    let family = wrong_operation.environment.generic_variants[0].id;
    let operation = wrong_operation.environment.kinds.iter_mut()
        .find(|kind| matches!(kind, Behavior::GenericLeft(id) if *id == family))
        .ok_or("real calculator environment omitted a generic arm operation")?;
    *operation = Behavior::GenericLeft(noble_kernel::types::NominalTypeId {
        module: family.module.saturating_add(1),
        ordinal: family.ordinal,
    });
    assert!(matches!(noble_wasm::source::Compiler::new_calculator().prepare(&wrong_operation),
        Err(noble_wasm::Diagnostic::Invalid)),
        "forged derived-operation identity bypassed complete environment replay");

    let mut missing_owner = submission.clone();
    let owner = missing_owner.environment.definition_owners.iter_mut()
        .find(|owner| owner.is_some()).ok_or("real calculator environment has no owned row")?;
    *owner = None;
    assert!(matches!(noble_wasm::source::Compiler::new_calculator().prepare(&missing_owner),
        Err(noble_wasm::Diagnostic::Invalid)),
        "forged derived-operation owner bypassed complete environment replay");

    let mut missing_dependency = submission.clone();
    let dependencies = missing_dependency.environment.deps.iter_mut().skip(32)
        .find(|dependencies| !dependencies.is_empty())
        .ok_or("real calculator environment has no dependent guest operation")?;
    dependencies.clear();
    assert!(matches!(noble_wasm::source::Compiler::new_calculator().prepare(&missing_dependency),
        Err(noble_wasm::Diagnostic::Invalid)),
        "forged guest dependency bypassed complete environment replay");

    let mut oversized = submission.clone();
    while oversized.environment.defs.len() <= 896 {
        oversized.environment.defs.push(submission.environment.defs[32].clone());
        oversized.environment.kinds.push(submission.environment.kinds[32]);
        oversized.environment.deps.push(submission.environment.deps[32].clone());
        oversized.environment.definition_owners.push(
            submission.environment.definition_owners[32]);
    }
    assert!(matches!(noble_wasm::source::Compiler::new_calculator().prepare(&oversized),
        Err(noble_wasm::Diagnostic::Exhausted)),
        "a hostile environment larger than 896 rows bypassed the selected bound");

    // Pad the retained source namespace, rather than forging an Env, so
    // exactly 896 rows cross both source snapshot and backend admission.
    let baseline = session.prepare(b"1", &[], generous)
        .map_err(|error| format!("baseline literal: {error:?}"))?;
    let baseline = baseline.submission().ok_or("baseline literal missing submission")?
        .environment.defs.len();
    let remaining = 896usize.checked_sub(baseline)
        .ok_or("real calculator namespace already exceeds the 896-row cap")?;
    if !(3..=256).contains(&remaining) {
        return Err(format!("padding fixture must fit one bounded source module: {remaining}"));
    }
    let padding = |name: &str, rows: usize| {
        let (variants, opaques) = if rows.is_multiple_of(2) {
            (0, rows / 2)
        } else {
            (1, (rows - 3) / 2)
        };
        let mut module = format!("module {name}@1 [\n");
        for index in 0..opaques {
            module.push_str(&format!("    opaque Padding{index} Unit private\n"));
        }
        if variants != 0 {
            module.push_str("    variant PaddingVariant Left Unit private Right Unit private\n");
        }
        module.push_str("]\n");
        module
    };
    let overflow = padding("calculator_row_overflow", remaining + 1);
    let refused = session.prepare(overflow.as_bytes(), &[], generous)
        .err().ok_or("source staged and published row 897")?;
    assert_eq!(refused.stage(), noble_contracts::source::Stage::Check);
    assert_eq!(refused.diagnostic().kind, noble_contracts::DiagnosticKind::Exhausted);
    let padding = padding("calculator_row_padding", remaining);
    let pending = session.prepare(padding.as_bytes(), &[], generous)
        .map_err(|error| format!("calculator row padding: {error:?}"))?;
    let (next, committed) = session.commit(pending);
    committed.map_err(|error| format!("calculator row padding commit: {error:?}"))?;
    session = next;
    let exact = session.prepare(b"1", &[], generous)
        .map_err(|error| format!("exact 896 source literal: {error:?}"))?;
    let selected = exact.submission().ok_or("exact 896 literal missing submission")?;
    assert_eq!(selected.environment.defs.len(), 896);
    noble_wasm::source::Compiler::new_calculator().prepare(selected)
        .map_err(|_| "exact 896 source-selected backend refused literal")?;
    let mut outside = selected.clone();
    outside.environment.defs.push(selected.environment.defs[32].clone());
    outside.environment.kinds.push(selected.environment.kinds[32]);
    outside.environment.deps.push(selected.environment.deps[32].clone());
    outside.environment.definition_owners.push(selected.environment.definition_owners[32]);
    assert_eq!(outside.environment.defs.len(), 897);
    assert!(matches!(noble_wasm::source::Compiler::new_calculator().prepare(&outside),
        Err(noble_wasm::Diagnostic::Exhausted)),
        "row 897 crossed compiler preflight before clone");
    Ok(())
}

#[test]
fn calculator_dynamic_text_carveout_does_not_reduce_cursor_static_text_space() -> Result<(), String> {
    let mut source = Vec::with_capacity(50_002);
    source.push(b'"');
    source.resize(50_001, b'a');
    source.push(b'"');
    let mut cursor = noble_contracts::source::Session::new_text_cursor();
    let mut cursor_compiler = noble_wasm::source::Compiler::new_text_cursor();
    let mut calculator = noble_contracts::source::Session::new_calculator();
    let mut calculator_compiler = noble_wasm::source::Compiler::new_calculator();

    for generation in 0..10 {
        let pending = cursor.prepare(&source, &[], limits()).map_err(|e| format!("{e:?}"))?;
        let compiled = cursor_compiler.prepare(pending.submission().ok_or("no cursor text")?)
            .map_err(|_| "cursor static Text reservation unexpectedly exhausted")?;
        cursor_compiler.commit(compiled).map_err(|_| "cursor compiler commit failed")?;
        cursor.commit(pending).map_err(|e| format!("{e:?}"))?;

        let pending = calculator.prepare(&source, &[], limits()).map_err(|e| format!("{e:?}"))?;
        if generation == 9 {
            assert!(matches!(calculator_compiler.prepare(pending.submission().ok_or("no calculator text")?),
                Err(noble_wasm::Diagnostic::Exhausted)),
                "calculator static Text must not overwrite its dynamic input pool");
            assert_eq!(calculator.generation(), generation);
        } else {
            let compiled = calculator_compiler.prepare(pending.submission().ok_or("no calculator text")?)
                .map_err(|_| "calculator static Text exhausted before reserved pool")?;
            calculator_compiler.commit(compiled)
                .map_err(|_| "calculator compiler commit failed")?;
            calculator.commit(pending).map_err(|e| format!("{e:?}"))?;
        }
    }
    Ok(())
}

fn balanced_i64_tree(depth: u8) -> noble_kernel::types::Ty {
    if depth == 0 {
        noble_kernel::types::Ty::I64
    } else {
        noble_kernel::types::Ty::Pair(
            Box::new(balanced_i64_tree(depth - 1)),
            Box::new(balanced_i64_tree(depth - 1)),
        )
    }
}

fn repeated_nominal_tree(
    depth: u8,
    shape: &noble_kernel::types::NominalShape,
) -> noble_kernel::shapes::Pattern {
    if depth == 0 {
        noble_kernel::shapes::Pattern::Nominal(
            noble_kernel::types::NominalTypeId { module: 77, ordinal: 0 },
            Box::new(shape.clone()),
        )
    } else {
        noble_kernel::shapes::Pattern::Pair(
            Box::new(repeated_nominal_tree(depth - 1, shape)),
            Box::new(repeated_nominal_tree(depth - 1, shape)),
        )
    }
}

#[test]
fn calculator_rejects_unmetered_nested_nominal_scheme_before_kernel_clone() -> Result<(), String> {
    let generous = noble_contracts::Limits {
        bytes: 262_144,
        nodes: 262_144,
        depth: 64,
        work: 32_000_000,
    };
    let session = noble_contracts::source::ModuleSession::new_calculator()
        .map_err(|error| format!("{error:?}"))?;
    let pending = session.prepare(b"module hostile@1 [ export x def x [ 1 ] ]", &[], generous)
        .map_err(|error| format!("{error:?}"))?;
    let (session, installed) = session.commit(pending);
    installed.map_err(|error| format!("{error:?}"))?;
    let checked = session.prepare(b"hostile@1.x", &[], generous)
        .map_err(|error| format!("{error:?}"))?;
    let mut forged = checked.submission().ok_or("no executable candidate")?.clone();
    let named = 32;
    assert_eq!(forged.environment.defs.len(), named + 1);
    // Each of 128 leaves embeds a fresh 256-node nominal descriptor; the
    // 255-node outer Pair pattern is still below the old 512-node preflight
    // bound. The old top-level-only allowance counted each entire row as
    // 512 before canonical replay or any owned kernel validation.
    let shape = noble_kernel::types::NominalShape::Opaque(Box::new(
        noble_kernel::types::Ty::List(Box::new(balanced_i64_tree(7))),
    ));
    forged.environment.defs[named].stack_out =
        vec![repeated_nominal_tree(7, &shape)];
    for _ in 0..3 {
        forged.environment.defs.push(forged.environment.defs[named].clone());
        forged.environment.kinds.push(Behavior::Named);
        forged.environment.deps.push(Vec::new());
        forged.environment.definition_owners.push(None);
    }
    assert!(matches!(
        noble_wasm::source::Compiler::new_calculator().prepare(&forged),
        Err(noble_wasm::Diagnostic::Exhausted),
    ));
    Ok(())
}

#[test]
fn calculator_prices_each_repeated_node_witness_before_kernel_clone() -> Result<(), String> {
    use noble_kernel::words::Binding;

    let mut requested = limits();
    requested.work = 32_000_000;
    let session = noble_contracts::source::Session::new_calculator();
    let prepared = session.prepare(b"1 2 swap", &[], requested)
        .map_err(|error| format!("{error:?}"))?;
    let mut forged = prepared.submission().ok_or("no swap candidate")?.clone();
    let swap = forged.body.candidate.nodes.iter_mut().find(|node| {
        matches!(node, Node::Invocation { def, .. } if *def == Definition(2))
    }).ok_or("no swap invocation")?;
    let Node::Invocation { inst, .. } = swap else {
        return Err("wrong swap node".into());
    };
    assert_eq!(inst.bindings.len(), 3);
    let large = balanced_i64_tree(8);
    inst.bindings[0] = Binding::Stack(vec![large.clone(); 126]);
    inst.bindings[1] = Binding::Value(large.clone());
    inst.bindings[2] = Binding::Value(large);
    let swap = forged.body.candidate.nodes.iter().find(|node|
        matches!(node, Node::Invocation { def, .. } if *def == Definition(2)))
        .ok_or("no swap node")?.clone();
    // With 128 identical 511-constructor types, every swap is genuinely
    // type-correct; remove the original literals so no unreachable arena
    // nodes could cause a later admission rejection instead of this guard.
    forged.body.candidate.nodes = vec![swap];
    let carried = vec![balanced_i64_tree(8); 128];
    forged.request.expected.stack_in = carried.clone();
    forged.request.expected.stack_out = carried;
    forged.request.limits.type_size = 512;
    forged.request.limits.stack_height = 128;
    forged.body.candidate.body = vec![noble_kernel::untrusted::NodeId(0); 4096];
    let mut one = forged.clone();
    one.body.candidate.body.truncate(1);
    let result = noble_kernel::acceptance::check(
        &one.environment, &one.request, &one.body.candidate
    );
    assert!(matches!(result, Outcome::Accepted(_)),
        "the repeated swap witness must be genuinely kernel-admissible: {result:?}");
    assert!(matches!(
        noble_wasm::source::Compiler::new_calculator().prepare(&forged),
        Err(noble_wasm::Diagnostic::Exhausted),
    ));
    Ok(())
}

#[test]
fn calculator_prices_observed_frame_height_not_unused_stack_capacity() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new_calculator()
        .map_err(|error| format!("{error:?}"))?;
    let pending = session.prepare(b"module frame@1 [ export noop def noop [ ] ]", &[], limits())
        .map_err(|error| format!("{error:?}"))?;
    let (session, installed) = session.commit(pending);
    installed.map_err(|error| format!("{error:?}"))?;
    let checked = session.prepare(b"frame@1.noop", &[], limits())
        .map_err(|error| format!("{error:?}"))?;
    let mut forged = checked.submission().ok_or("no no-op candidate")?.clone();
    let deep = balanced_i64_tree(8);
    forged.request.expected.stack_in = vec![deep.clone()];
    forged.request.expected.stack_out = vec![deep];
    forged.request.limits.type_size = 512;
    forged.request.limits.stack_height = 128;
    assert!(matches!(noble_kernel::acceptance::check(
        &forged.environment, &forged.request, &forged.body.candidate
    ), Outcome::Accepted(_)), "the single deep frame must remain kernel-admissible");
    noble_wasm::source::Compiler::new_calculator().prepare(&forged)
        .map_err(|_| "one deep value exhausted unused stack capacity")?;
    let mut full = forged;
    let carried = vec![full.request.expected.stack_in[0].clone(); 128];
    full.request.expected.stack_in = carried.clone();
    full.request.expected.stack_out = carried;
    full.request.limits.work = 4_000_000;
    assert!(matches!(noble_kernel::acceptance::check(
        &full.environment, &full.request, &full.body.candidate
    ), Outcome::Accepted(_)), "the full-height deep frame must be genuinely kernel-admissible");
    assert!(matches!(noble_wasm::source::Compiler::new_calculator().prepare(&full),
        Err(noble_wasm::Diagnostic::Exhausted)),
        "a genuinely full 128-element deep frame evaded the first-mismatch preclone reserve");
    Ok(())
}

#[test]
fn calculator_prices_only_a_reachable_mismatch_comparison_site() -> Result<(), String> {
    // A deep frame is consumed before a later twelve-element scalar frame.
    // No kernel mismatch can compare the earlier deep value at the later
    // height; successful per-appearance scans still debit the same Work.
    let deep = balanced_i64_tree(7);
    let mut source = String::from("drop");
    for _ in 0..12 {
        source.push_str(" 0");
    }
    for _ in 0..12 {
        source.push_str(" drop");
    }
    let checked = noble_contracts::source::Session::new_calculator()
        .prepare(source.as_bytes(), &[deep], limits())
        .map_err(|error| format!("checked split-frame expression: {error:?}"))?;
    let mut submission = checked.submission()
        .ok_or("split-frame expression produced no checked guest body")?.clone();
    submission.request.limits.work = 1_000_000;
    assert!(matches!(
        noble_kernel::acceptance::check(
            &submission.environment, &submission.request, &submission.body.candidate
        ),
        Outcome::Accepted(_),
    ), "split-frame expression must remain kernel-admissible within its Work");
    noble_wasm::source::Compiler::new_calculator().prepare(&submission)
        .map_err(|_| "independent mismatch sites exhausted their combined impossible maxima")?;
    Ok(())
}

#[test]
fn calculator_prices_carried_stack_on_each_noop_join() -> Result<(), String> {
    let mut requested = limits();
    requested.work = 32_000_000;
    let session = noble_contracts::source::ModuleSession::new_calculator()
        .map_err(|error| format!("{error:?}"))?;
    let pending = session.prepare(b"module frame@1 [ export noop def noop [ ] ]", &[], requested)
        .map_err(|error| format!("{error:?}"))?;
    let (session, installed) = session.commit(pending);
    installed.map_err(|error| format!("{error:?}"))?;
    let checked = session.prepare(b"frame@1.noop", &[], requested)
        .map_err(|error| format!("{error:?}"))?;
    let mut forged = checked.submission().ok_or("no no-op candidate")?.clone();
    let id = *forged.body.candidate.body.first().ok_or("no no-op node")?;
    assert!(matches!(forged.body.candidate.nodes.get(id.0 as usize),
        Some(Node::Invocation { def, inst }) if *def == Definition(32) && inst.bindings.is_empty()));
    let carried = vec![balanced_i64_tree(8); 128];
    forged.request.expected.stack_in = carried.clone();
    forged.request.expected.stack_out = carried;
    forged.body.candidate.body = vec![id; 4096];
    assert!(matches!(
        noble_wasm::source::Compiler::new_calculator().prepare(&forged),
        Err(noble_wasm::Diagnostic::Exhausted),
    ));
    Ok(())
}

#[test]
fn calculator_rejects_exponential_generic_substitution_before_kernel_clone() -> Result<(), String> {
    use noble_kernel::shapes::Pattern;
    use noble_kernel::types::NominalTypeId;

    let session = noble_contracts::source::ModuleSession::new_calculator()
        .map_err(|error| format!("{error:?}"))?;
    let pending = session.prepare(b"module finite@1 [ export x def x [ 1 ] ]", &[], limits())
        .map_err(|error| format!("{error:?}"))?;
    let (session, result) = session.commit(pending);
    result.map_err(|error| format!("{error:?}"))?;
    let prepared = session.prepare(b"finite@1.x", &[], limits())
        .map_err(|error| format!("{error:?}"))?;
    let mut forged = prepared.submission().ok_or("no named candidate")?.clone();
    let id = NominalTypeId { module: 77, ordinal: 0 };
    let mut expanded = Pattern::Unit;
    for _ in 0..32 {
        expanded = Pattern::GenericNominal(
            id,
            Box::new([expanded, Pattern::Unit]),
            [0, 1],
        );
    }
    forged.environment.defs[32].stack_out = vec![expanded];
    assert!(matches!(
        noble_wasm::source::Compiler::new_calculator().prepare(&forged),
        Err(noble_wasm::Diagnostic::Exhausted),
    ));
    Ok(())
}

#[test]
fn calculator_refuses_nested_environment_clone_payload_before_kernel_copy() -> Result<(), String> {
    use noble_kernel::{shapes::Pattern, words::Scheme};
    let source = noble_contracts::source::Session::new_calculator();
    let prepared = source.prepare(b"1", &[], limits())
        .map_err(|error| format!("{error:?}"))?;
    let mut hostile = prepared.submission().ok_or("no scalar submission")?.clone();
    let nested = Pattern::program(
        vec![Pattern::Unit; 128], vec![Pattern::Unit; 128], Vec::new(),
    );
    for _ in 0..8 {
        hostile.environment.defs.push(Scheme {
            var_kinds: Vec::new(),
            stack_in: Vec::new(),
            stack_out: vec![nested.clone(); 128],
            effects: Vec::new(),
        });
        hostile.environment.kinds.push(Behavior::Named);
        hostile.environment.deps.push(Vec::new());
        hostile.environment.definition_owners.push(None);
    }
    assert!(hostile.environment.calculator_clone_budget().is_none(),
        "nested program payload must exceed the per-clone bound before copying");
    assert!(matches!(
        noble_wasm::source::Compiler::new_calculator().prepare(&hostile),
        Err(noble_wasm::Diagnostic::Exhausted),
    ));
    Ok(())
}

#[test]
fn calculator_refuses_deep_pattern_and_type_in_896_row_environment_before_clone() -> Result<(), String> {
    use noble_kernel::{
        contracts::NominalDecl,
        shapes::Pattern,
        types::{NominalShape, NominalTypeId, Ty},
        words::Scheme,
    };
    let source = noble_contracts::source::Session::new_calculator();
    let prepared = source.prepare(b"1", &[], limits())
        .map_err(|error| format!("{error:?}"))?;
    let mut selected = prepared.submission().ok_or("no scalar submission")?.clone();
    while selected.environment.defs.len() < 896 {
        selected.environment.defs.push(Scheme {
            var_kinds: Vec::new(),
            stack_in: Vec::new(),
            stack_out: vec![Pattern::Unit],
            effects: Vec::new(),
        });
        selected.environment.kinds.push(Behavior::Named);
        selected.environment.deps.push(Vec::new());
        selected.environment.definition_owners.push(None);
    }
    assert_eq!(selected.environment.defs.len(), 896);
    assert!(selected.environment.calculator_clone_budget().is_some());
    let mut pattern = Pattern::Unit;
    for _ in 0..65 {
        pattern = Pattern::List(Box::new(pattern));
    }
    selected.environment.defs[895].stack_out[0] = pattern;
    assert!(selected.environment.calculator_clone_budget().is_none());
    assert!(matches!(
        noble_wasm::source::Compiler::new_calculator().prepare(&selected),
        Err(noble_wasm::Diagnostic::Exhausted),
    ));

    selected.environment.defs[895].stack_out[0] = Pattern::Unit;
    let mut ty = Ty::Unit;
    for _ in 0..65 {
        ty = Ty::List(Box::new(ty));
    }
    selected.environment.nominals.push(NominalDecl {
        id: NominalTypeId { module: 1, ordinal: 0 },
        shape: NominalShape::Opaque(Box::new(ty)),
        exported: false,
        public: [false, false],
    });
    assert!(selected.environment.calculator_clone_budget().is_none());
    assert!(matches!(
        noble_wasm::source::Compiler::new_calculator().prepare(&selected),
        Err(noble_wasm::Diagnostic::Exhausted),
    ));
    Ok(())
}

#[test]
fn calculator_refuses_maximum_named_clone_fanout_before_kernel_copy() -> Result<(), String> {
    use noble_kernel::{execution::Definition as Named, shapes::Pattern, words::Scheme};
    let source = noble_contracts::source::Session::new_calculator();
    let prepared = source.prepare(b"1", &[], limits())
        .map_err(|error| format!("{error:?}"))?;
    let mut hostile = prepared.submission().ok_or("no scalar submission")?.clone();
    let nested = Pattern::program(
        vec![Pattern::Unit; 128], vec![Pattern::Unit; 128], Vec::new(),
    );
    hostile.environment.defs.push(Scheme {
        var_kinds: Vec::new(),
        stack_in: Vec::new(),
        stack_out: vec![nested; 128],
        effects: Vec::new(),
    });
    hostile.environment.kinds.push(Behavior::Named);
    hostile.environment.deps.push(Vec::new());
    hostile.environment.definition_owners.push(None);
    // Stay inside the current row bound so compiler refusal exercises the
    // fanout guard, not the independent environment-row admission check.
    while hostile.environment.defs.len() < 896 {
        hostile.environment.defs.push(Scheme {
            var_kinds: Vec::new(),
            stack_in: Vec::new(),
            stack_out: vec![Pattern::Unit],
            effects: Vec::new(),
        });
        hostile.environment.kinds.push(Behavior::Named);
        hostile.environment.deps.push(Vec::new());
        hostile.environment.definition_owners.push(None);
    }
    let estimate = hostile.environment.calculator_clone_budget()
        .ok_or("one cloned environment unexpectedly exceeded its own bound")?;
    assert_eq!(hostile.environment.defs.len(), 896);
    assert!(estimate.reserved_bytes.checked_mul(513).is_some_and(|total|
        total > noble_kernel::contracts::CALCULATOR_ENV_CLONE_FANOUT_LIMIT));
    hostile.definitions = vec![Named {
        definition: Definition(32),
        identity: 1,
        body: hostile.body.clone(),
        expected: hostile.request.expected.clone(),
    }; 512];
    assert!(matches!(
        noble_wasm::source::Compiler::new_calculator().prepare(&hostile),
        Err(noble_wasm::Diagnostic::Exhausted),
    ));
    Ok(())
}
