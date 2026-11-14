#![allow(
    tigerstyle::function_length,
    tigerstyle::ambient_env,
    reason = "Owner: noble-contracts; the two-process identity fixture compares the complete serialized manifest, using only a child-dispatch environment marker outside library inputs"
)]

use super::super::procedures::support::{literal_record, provenance, revision};
use noble_contracts::source::procedures::{
    LibraryDependency, ProcedureLibrary, ProcedureRecord, Producer,
};
use noble_contracts::source::Session;
use noble_kernel::types::Ty;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

#[test]
fn binds_body_dependencies_and_provenance() -> Result<(), String> {
    let first = literal_record(1, Producer::Chunk).map_err(|error| format!("first: {error:?}"))?;
    let identical =
        literal_record(1, Producer::Chunk).map_err(|error| format!("identical: {error:?}"))?;
    let changed_body =
        literal_record(2, Producer::Chunk).map_err(|error| format!("changed body: {error:?}"))?;
    let changed_producer = literal_record(1, Producer::Synthesis)
        .map_err(|error| format!("changed producer: {error:?}"))?;
    assert_eq!(first.procedure_id, identical.procedure_id);
    assert_ne!(first.procedure_id, changed_body.procedure_id);
    assert_ne!(first.procedure_id, changed_producer.procedure_id);
    assert_eq!(first.retrieval.stack_in, changed_body.retrieval.stack_in);
    assert_eq!(first.retrieval.stack_out, changed_body.retrieval.stack_out);

    let dependency = LibraryDependency {
        procedure_id: [0x11; 32],
        chunk_id: [0x22; 16],
        source_revision: [0x33; 32],
    };
    let one_edge = ProcedureRecord::candidate_with_dependencies(
        first.chunk.clone(),
        provenance(Producer::Chunk),
        vec![dependency],
    )
    .map_err(|error| format!("dependency identity: {error:?}"))?;
    let second_dependency = LibraryDependency {
        procedure_id: [0x44; 32],
        chunk_id: [0x55; 16],
        source_revision: [0x66; 32],
    };
    let ordered_edges = ProcedureRecord::candidate_with_dependencies(
        first.chunk.clone(),
        provenance(Producer::Chunk),
        vec![dependency, second_dependency],
    )
    .map_err(|error| format!("ordered dependencies: {error:?}"))?;
    let reversed_edges = ProcedureRecord::candidate_with_dependencies(
        first.chunk.clone(),
        provenance(Producer::Chunk),
        vec![second_dependency, dependency],
    )
    .map_err(|error| format!("reversed dependencies: {error:?}"))?;
    assert_ne!(first.procedure_id, one_edge.procedure_id);
    assert_ne!(ordered_edges.procedure_id, reversed_edges.procedure_id);
    Ok(())
}

#[test]
fn duplicate_is_idempotent_and_reload_reproduces_all_views() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let first = literal_record(1, Producer::Chunk).map_err(|error| format!("record: {error:?}"))?;
    let id = library
        .admit(&namespace, None, first.clone())
        .map_err(|error| format!("admit: {error:?}"))?;
    assert_eq!(library.current_generation(), 1);
    assert_eq!(
        library
            .admit(&namespace, None, first)
            .map_err(|error| format!("duplicate: {error:?}"))?,
        id
    );
    assert_eq!(library.current_generation(), 1);
    let second =
        literal_record(2, Producer::Chunk).map_err(|error| format!("second: {error:?}"))?;
    library
        .admit(&namespace, None, second)
        .map_err(|error| format!("second admit: {error:?}"))?;
    assert_eq!(library.records().len(), 2);
    assert_eq!(library.chunk_library().entries().len(), 2);
    assert_eq!(library.chunk_library_revision(), &revision(7));
    let expected_order = library
        .canonical_chunk_order()
        .map_err(|error| format!("chunk order: {error:?}"))?;
    assert_eq!(expected_order.len(), 2);

    let encoded = library
        .encode()
        .map_err(|error| format!("encode: {error:?}"))?;
    let restarted_a = ProcedureLibrary::load(&encoded, &namespace)
        .map_err(|error| format!("load A: {error:?}"))?;
    let restarted_b = ProcedureLibrary::load(&encoded, &namespace)
        .map_err(|error| format!("load B: {error:?}"))?;
    let manifest = library
        .identity_manifest()
        .map_err(|error| format!("manifest: {error:?}"))?;
    assert_eq!(
        restarted_a
            .identity_manifest()
            .map_err(|error| format!("manifest A: {error:?}"))?,
        manifest
    );
    assert_eq!(
        restarted_b
            .identity_manifest()
            .map_err(|error| format!("manifest B: {error:?}"))?,
        manifest
    );
    assert_eq!(
        restarted_a.procedure_library_revision(),
        library.procedure_library_revision()
    );
    assert_eq!(
        restarted_a.chunk_library_revision(),
        library.chunk_library_revision()
    );
    assert_eq!(
        restarted_a
            .canonical_chunk_order()
            .map_err(|error| format!("reloaded order: {error:?}"))?,
        expected_order
    );
    assert_eq!(
        restarted_a
            .records()
            .iter()
            .map(|record| record.procedure_id)
            .collect::<Vec<_>>(),
        library
            .records()
            .iter()
            .map(|record| record.procedure_id)
            .collect::<Vec<_>>()
    );
    let text =
        core::str::from_utf8(&manifest).map_err(|error| format!("manifest utf8: {error}"))?;
    for field in [
        "noble-procedure-library-export/1",
        "procedure_library_revision",
        "chunk_library_revision",
        "retrieval_order",
        "dependencies",
    ] {
        assert!(text.contains(field), "manifest omitted {field}");
    }
    for forbidden in [
        "timestamp",
        "process_id",
        "host_name",
        "teacher_config",
        "secret",
    ] {
        assert!(!text.contains(forbidden), "manifest exposed {forbidden}");
    }
    for process_index in 0..2 {
        let mut child = Command::new(
            std::env::current_exe().map_err(|error| format!("current test executable: {error}"))?,
        )
        .args([
            "--exact",
            "procedures::identity::restart_load_process",
            "--nocapture",
        ])
        .env("NPLIB_RELOAD_PROCESS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("restart process {process_index}: {error}"))?;
        child
            .stdin
            .take()
            .ok_or("restart child stdin unavailable")?
            .write_all(&encoded)
            .map_err(|error| format!("send persisted bytes to process {process_index}: {error}"))?;
        let output = child
            .wait_with_output()
            .map_err(|error| format!("wait process {process_index}: {error}"))?;
        assert!(
            output.status.success(),
            "restart process {process_index} failed: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let output = String::from_utf8(output.stdout)
            .map_err(|error| format!("restart process output: {error}"))?;
        let child_manifest = output
            .lines()
            .find_map(|line| line.strip_prefix("MANIFEST="))
            .ok_or("restart process did not emit a manifest")?;
        assert_eq!(
            child_manifest,
            hex(&manifest),
            "restart process {process_index} changed the identity manifest"
        );
    }
    assert_eq!(
        library
            .records()
            .iter()
            .map(|record| record.chunk.interface.stack_out[0].clone())
            .collect::<Vec<_>>(),
        vec![Ty::I64, Ty::I64]
    );
    Ok(())
}

#[test]
fn restart_load_process() -> Result<(), String> {
    if std::env::var_os("NPLIB_RELOAD_PROCESS").is_none() {
        return Ok(());
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read parent-provided encoded library: {error}"))?;
    let namespace = Session::without_test_hosts();
    let library = ProcedureLibrary::load(&bytes, &namespace)
        .map_err(|error| format!("re-admit library in fresh process: {error:?}"))?;
    let manifest = library
        .identity_manifest()
        .map_err(|error| format!("export fresh-process identity manifest: {error:?}"))?;
    println!("MANIFEST={}", hex(&manifest));
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}
