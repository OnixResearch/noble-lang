/// A fresh kernel consumes only the Host-lowered exact returned target and
/// finite proof term. No builder theorem, submitted .olean, or worker assertion
/// is imported as proof authority.
pub(in crate::workflow) fn verify(
    obligation: &str,
    term: &str,
    library: &super::super::rules::Library,
    timeout_ms: u64,
) -> Result<super::Acceptance, super::super::output::Failure> {
    let deadline = attempt!(crate::sandbox::now()
        .checked_add(std::time::Duration::from_millis(timeout_ms))
        .ok_or_else(|| super::super::output::Failure::error(
            "timeout-overflow", "invalid returned target proof deadline".into()
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
        &generated.join("LiveSlotTargetObligation.lean"), obligation.as_bytes()
    ));
    let object = std::path::Path::new("LiveSlotTargetObligation.olean");
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
            source: "/obligation/LiveSlotTargetObligation.lean",
            root: "/obligation",
            output: "/obligation/LiveSlotTargetObligation.olean",
        }
    ));
    attempt!(super::require_success(
        transcript, "live-slot-target-obligation-rejected",
        "exact returned target obligation failed strict Lean compilation"
    ));
    attempt!(super::super::artifacts::finish_outputs(generated, object, &writable));
    let consumer = std::format!(r#"import Lean
import LiveSlotTargetObligation
open NobleContracts
open Lean Elab Command
theorem live_slot_target_proof : LiveSlotTargetObligation.claim := {term}
elab "check_live_slot_target_axioms" : command => do
  let env ← getEnv
  let some (.thmInfo proof) := env.find? ``live_slot_target_proof
    | throwError "returned target proof was not a theorem"
  let imported ← liftIO <| importModules
    #[{{ module := `LiveSlotTargetObligation }}] {{}} (trustLevel := 0) (loadExts := false)
  let empty ← liftIO mkEmptyEnvironment
  let trusted ← liftIO <| empty.replay imported.toKernelEnv.constants.map₁
  let checked ← match trusted.toKernelEnv.addDeclCore 0 (.thmDecl proof) (cancelTk? := none) with
    | .ok next => pure next
    | .error _ => throwError "fresh kernel rejected exact returned target proof"
  match Kernel.isDefEq (.ofKernelEnv checked) {{}} proof.type
      (mkConst `LiveSlotTargetObligation.claim) with
  | .ok true => pure ()
  | _ => throwError "returned target theorem has wrong exact claim type"
  let axioms ← liftCoreM (collectAxioms ``live_slot_target_proof)
  for ax in axioms do
    unless [``propext, ``Classical.choice, ``Quot.sound].contains ax do
      throwError "disallowed returned target axiom {{ax}}"
  logInfo "NOBLE-LSLOT-TARGET-ACCEPT"
  for ax in axioms do
    logInfo m!"NOBLE-LSLOT-TARGET-AXIOM:{{ax}}"
check_live_slot_target_axioms
"#);
    if consumer.len() > super::super::PROOF_LIMIT {
        return Err(super::super::output::Failure::error(
            "live-slot-target-proof-limit", "lowered target proof exceeded bounded source size".into()
        ));
    }
    attempt!(super::super::artifacts::write_source(
        &session.workspace.consumer.join("LiveSlotTargetConsumer.lean"),
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
            "/consumer/LiveSlotTargetConsumer.lean".into()
        ],
        deadline
    ));
    let transcript = attempt!(super::require_intrinsic_consumer_success(transcript));
    let axioms = attempt!(super::intrinsic_axioms(&transcript.stdout, "NOBLE-LSLOT-TARGET"));
    Ok(super::Acceptance {
        axioms, wire: consumer,
        tools: super::tools(&session.sandbox, &probe.stdout),
    })
}
