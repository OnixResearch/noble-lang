/// A distinct consumer for the host-authenticated, generated NamedV2 obligation.
/// Neither the producer module nor its compiled declarations enter this process.
pub(in crate::workflow) fn verify(
    obligation: &str,
    term: &str,
    library: &super::super::rules::Library,
    timeout_ms: u64,
) -> Result<super::Acceptance, super::super::output::Failure> {
    let deadline = attempt!(crate::sandbox::now()
        .checked_add(std::time::Duration::from_millis(timeout_ms))
        .ok_or_else(|| super::super::output::Failure::error(
            "timeout-overflow", "invalid named proof deadline".into()
        )));
    let sandbox = attempt!(crate::sandbox::Environment::discover(deadline));
    let probe = attempt!(super::probe_toolchain(&sandbox, deadline));
    let session = super::Session {
        sandbox,
        workspace: attempt!(super::Workspace::create(library, false)),
        deadline,
    };
    attempt!(session.compile_library(library));
    let generated = &session.workspace.obligation;
    attempt!(super::super::artifacts::write_source(
        &generated.join("NamedV2Obligation.lean"), obligation.as_bytes()
    ));
    let object = std::path::Path::new("NamedV2Obligation.olean");
    let writable = attempt!(super::super::artifacts::output_slots(generated, object));
    let mounts = attempt!(super::super::artifacts::output_mounts(
        std::vec![
            super::super::artifacts::read_mount(&session.workspace.library, "/library"),
            super::super::artifacts::read_mount(generated, "/obligation"),
        ],
        generated, &writable, "/obligation"
    ));
    let transcript = attempt!(session.compile_module(
        mounts, "/library:/obligation",
        super::compilation::Job {
            source: "/obligation/NamedV2Obligation.lean",
            root: "/obligation",
            output: "/obligation/NamedV2Obligation.olean",
        }
    ));
    attempt!(super::require_success(
        transcript, "named-obligation-rejected",
        "generated named source obligation failed strict Lean compilation"
    ));
    attempt!(super::super::artifacts::finish_outputs(generated, object, &writable));
    // The only inserted text is the finite reviewed lowering of the checked
    // source rule, not a submitted Lean proof, declaration, tactic or import.
    let consumer = std::format!(r#"import Lean
import NamedV2Obligation
open NobleContracts
open Lean Elab Command
theorem named_v2_proof : NamedV2Obligation.claim := {term}
elab "check_named_v2_axioms" : command => do
  let env ← getEnv
  let some (.thmInfo proof) := env.find? ``named_v2_proof
    | throwError "named v2 proof was not a theorem"
  -- Do not treat imported .olean declarations or successful elaboration as
  -- authority. Replay the complete selected import closure into an empty
  -- strict kernel environment, then add the newly elaborated theorem anew.
  let imported ← liftIO <| importModules
    #[{{ module := `NamedV2Obligation }}] {{}} (trustLevel := 0) (loadExts := false)
  let empty ← liftIO mkEmptyEnvironment
  let trusted ← liftIO <| empty.replay imported.toKernelEnv.constants.map₁
  let checked ← match trusted.toKernelEnv.addDeclCore 0 (.thmDecl proof) (cancelTk? := none) with
    | .ok next => pure next
    | .error _ => throwError "named v2 fresh kernel rejected proof"
  match Kernel.isDefEq (.ofKernelEnv checked) {{}} proof.type
      (mkConst `NamedV2Obligation.claim) with
  | .ok true => pure ()
  | _ => throwError "named v2 theorem has wrong exact claim type"
  let axioms ← liftCoreM (collectAxioms ``named_v2_proof)
  for ax in axioms do
    unless [``propext, ``Classical.choice, ``Quot.sound].contains ax do
      throwError "disallowed named v2 axiom {{ax}}"
  logInfo "NOBLE-NAMED-V2-ACCEPT"
  for ax in axioms do
    logInfo m!"NOBLE-NAMED-V2-AXIOM:{{ax}}"
check_named_v2_axioms
"#);
    if consumer.len() > super::super::PROOF_LIMIT {
        return Err(super::super::output::Failure::error(
            "named-proof-limit", "lowered named proof exceeded bounded source size".into()
        ));
    }
    attempt!(super::super::artifacts::write_source(
        &session.workspace.consumer.join("NamedV2Consumer.lean"),
        consumer.as_bytes()
    ));
    let mounts = std::vec![
        super::super::artifacts::read_mount(&session.workspace.library, "/library"),
        super::super::artifacts::read_mount(generated, "/obligation"),
        super::super::artifacts::read_mount(&session.workspace.consumer, "/consumer"),
    ];
    let transcript = attempt!(session.sandbox.run(
        &mounts, "/library:/obligation",
        &std::vec![
            "-R".into(), "/consumer".into(), "-j".into(), "2".into(),
            "/consumer/NamedV2Consumer.lean".into()
        ],
        deadline
    ));
    let transcript = attempt!(super::require_intrinsic_consumer_success(transcript));
    let axioms = attempt!(super::intrinsic_axioms(&transcript.stdout, "NOBLE-NAMED-V2"));
    Ok(super::Acceptance {
        axioms, wire: consumer,
        tools: super::tools(&session.sandbox, &probe.stdout),
    })
}
