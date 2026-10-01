//! ADAPT-15 source-bound admission peer. This program prepares the actual
//! Core-Bootstrap source, changes only the request's allowed effect bound, and
//! checks both submissions through the public kernel and compiler APIs.
//! Proof evidence is verified independently by the gate, not attached here.
//! No runtime, guest, host dispatcher, or WAT file is created.

use noble_contracts::{source::Session, Limits};
use noble_kernel::contracts::{Behavior, Definition, TEST_EMIT};
use noble_kernel::execution::Submission;
use noble_kernel::shapes::{EffectSlot, Pattern};
use noble_kernel::types::EffSet;
use noble_kernel::untrusted::{Constraint, Lit, Node, NodeId, Outcome};
use noble_kernel::words::{Variable, VariableKind};
use noble_wasm::source::Compiler;
use std::fmt::Debug;

const SOURCE: &[u8] = b"\"audit\" test.emit";
const EMIT: Definition = Definition(22);

fn source_submission(path: &str) -> Result<Submission, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
    if bytes.as_slice() != SOURCE {
        return Err("source bytes differ from the exact ADAPT-15 source".into());
    }
    let prepared = Session::new()
        .prepare(&bytes, &[], Limits::default())
        .map_err(|error| format!("source preparation failed: {error:?}"))?;
    if !prepared.output().is_empty() {
        return Err("source preparation produced a nonempty output stack".into());
    }
    let submission = prepared
        .submission()
        .cloned()
        .ok_or("source preparation produced no executable submission")?;
    let candidate = &submission.body.candidate;
    let expected = &submission.request.expected;
    if !submission.definitions.is_empty()
        || !expected.stack_in.is_empty()
        || !expected.stack_out.is_empty()
        || !matches!(expected.allowed_effects.as_slice(), [id] if *id == TEST_EMIT)
        || candidate.nodes.len() != 2
        || !matches!(candidate.body.as_slice(), [NodeId(0), NodeId(1)])
        || !matches!(
            candidate.nodes.as_slice(),
            [Node::Literal { lit: Lit::Text, .. }, Node::Invocation { def, .. }]
                if *def == EMIT
        )
        || !matches!(
            submission.body.texts.as_slice(),
            [text] if text.node == NodeId(0) && text.bytes == b"audit"
        )
    {
        return Err("source did not produce the exact typed audit/test.emit premise".into());
    }
    let Some(scheme) = submission.environment.scheme(EMIT) else {
        return Err("source environment has no test.emit scheme".into());
    };
    if submission.environment.kind(EMIT) != Some(Behavior::TestEmit)
        || submission.environment.effects.first() != Some(&TEST_EMIT)
        || !matches!(scheme.var_kinds.as_slice(), [VariableKind::Stack])
        || !matches!(
            scheme.stack_in.as_slice(),
            [Pattern::StackVar(Variable(0)), Pattern::Text]
        )
        || !matches!(
            scheme.stack_out.as_slice(),
            [Pattern::StackVar(Variable(0))]
        )
        || !matches!(scheme.effects.as_slice(), [EffectSlot::Effect(TEST_EMIT)])
    {
        return Err(format!(
            "source environment lost the fixed Text/test.emit host contract: kind={:?}, effects={:?}, scheme={scheme:?}",
            submission.environment.kind(EMIT),
            submission.environment.effects,
        ));
    }
    Ok(submission)
}

// The public Env, Candidate, and executable Body deliberately do not implement
// PartialEq. Compare their entire same-process Debug representations as an
// additional clone-integrity guard; the single assignment below is the only
// mutation, and the two actual kernel outcomes establish the semantic result.
fn unchanged<T: Debug>(left: &T, right: &T) -> bool {
    format!("{left:?}") == format!("{right:?}")
}

fn compiler_diagnostic(diagnostic: noble_wasm::Diagnostic) -> &'static str {
    match diagnostic {
        noble_wasm::Diagnostic::Invalid => "Invalid",
        noble_wasm::Diagnostic::Exhausted => "Exhausted",
        noble_wasm::Diagnostic::Unsupported => "Unsupported",
        noble_wasm::Diagnostic::Defective => "Defective",
    }
}

fn check(source: &str) -> Result<(), String> {
    let authentic = source_submission(source)?;
    let genuine = noble_kernel::acceptance::check(
        &authentic.environment,
        &authentic.request,
        &authentic.body.candidate,
    );
    match genuine {
        Outcome::Accepted(checked)
            if checked.interface.stack_in.is_empty()
                && checked.interface.stack_out.is_empty()
                && matches!(checked.interface.effects.as_slice(), [id] if *id == TEST_EMIT) => {}
        other => return Err(format!("authentic source was not accepted as test.emit: {other:?}")),
    }
    let wat_bytes = Compiler::new()
        .prepare(&authentic)
        .map_err(|error| format!(
            "authentic compiler preparation failed: {}", compiler_diagnostic(error)
        ))?
        .wat()
        .len();
    if wat_bytes == 0 {
        return Err("authentic compiler produced empty WAT".into());
    }

    let mut forged = authentic.clone();
    forged.request.expected.allowed_effects = EffSet::empty();
    let environment_unchanged = unchanged(&authentic.environment, &forged.environment);
    let candidate_unchanged = unchanged(&authentic.body.candidate, &forged.body.candidate);
    let non_effect_premises_unchanged = environment_unchanged
        && candidate_unchanged
        && unchanged(&authentic.body.texts, &forged.body.texts)
        && unchanged(&authentic.definitions, &forged.definitions)
        && authentic.request.input_bytes == forged.request.input_bytes
        && authentic.request.limits == forged.request.limits
        && authentic.request.expected.stack_in == forged.request.expected.stack_in
        && authentic.request.expected.stack_out == forged.request.expected.stack_out;
    if !non_effect_premises_unchanged || !forged.request.expected.allowed_effects.is_empty() {
        return Err("forgery changed a non-effect premise or did not empty the request bound".into());
    }

    let hostile = noble_kernel::acceptance::check(
        &forged.environment,
        &forged.request,
        &forged.body.candidate,
    );
    match hostile {
        Outcome::Invalid(diagnostic)
            if diagnostic.constraint == Constraint::EffectInclusion(TEST_EMIT)
                && diagnostic.node.is_none()
                && diagnostic.def.is_none()
                && diagnostic.expected.is_empty()
                && diagnostic.actual.is_empty()
                && !diagnostic.provenance_available
                && !diagnostic.truncated => {}
        other => {
            return Err(format!(
                "request-only forgery was not rejected at entry for test.emit: {other:?}"
            ))
        }
    }
    match Compiler::new().prepare(&forged) {
        Err(noble_wasm::Diagnostic::Invalid) => {}
        Err(other) => return Err(format!(
            "forged compiler refusal was not Invalid: {}", compiler_diagnostic(other)
        )),
        Ok(_) => return Err("forged submission unexpectedly compiled".into()),
    }

    // Every nonliteral report value is obtained from, or checked against, the
    // source-derived submission and the actual outcomes above before printing.
    println!(
        concat!(
            r#"{{"id":"ADAPT-15","source":"\"audit\" test.emit","#,
            r#""premise":{{"source_derived":true,"definitions":{},"candidate_nodes":{},"candidate_body":[0,1],"input_stack":[],"output_stack":[],"allowed_effects":["test.emit"],"#,
            r#""host":{{"definition":{},"behavior":"TestEmit","stack_in":["StackVar(0)","Text"],"stack_out":["StackVar(0)"],"effects":["test.emit"]}},"#,
            r#""invocations":[{{"definition":{},"behavior":"TestEmit"}}],"texts":["audit"]}},"#,
            r#""authentic":{{"kernel":{{"outcome":"Accepted","stack_in":[],"stack_out":[],"effects":["test.emit"]}},"compiler":{{"outcome":"prepared","wat_bytes":{}}}}},"#,
            r#""forged":{{"mutations":["request.expected.allowed_effects=[]"],"allowed_effects":[],"environment_unchanged":{},"candidate_unchanged":{},"non_effect_premises_unchanged":{},"#,
            r#""kernel":{{"outcome":"Invalid","constraint":"EffectInclusion(test.emit)","node":null,"definition":null,"expected":[],"actual":[],"provenance_available":false,"truncated":false}},"#,
            r#""compiler":{{"outcome":"Invalid","wat_bytes":null}},"accepted_program_created":false}},"guest_created":false,"guest_requests":0,"protected_operations":0}}"#
        ),
        authentic.definitions.len(),
        authentic.body.candidate.nodes.len(),
        EMIT.0,
        EMIT.0,
        wat_bytes,
        environment_unchanged,
        candidate_unchanged,
        non_effect_premises_unchanged,
    );
    Ok(())
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let Some(source) = args.next() else {
        return Err("usage: adapt15-check SOURCE_PATH".into());
    };
    if args.next().is_some() {
        return Err("usage: adapt15-check SOURCE_PATH".into());
    }
    check(&source)
}
