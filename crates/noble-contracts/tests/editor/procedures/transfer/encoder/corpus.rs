use super::*;

const CORPUS: &str = "verification/procedure-library/receipts/encoder-corpus-v1.json";

#[test]
fn finite_subject_bytes_match_lean_generated() -> Result<(), String> {
    let receipt = super::producer_targets::read_json(CORPUS)?;
    if receipt["schema"] != "noble-procedure-library-lean-byte-receipt/1"
        || receipt["corpus"]["schema"] != "noble-procedure-library-lean-byte-corpus/1"
    {
        return Err(String::from("finite Lean byte corpus schema drifted"));
    }
    let cases = receipt["corpus"]["cases"]
        .as_array()
        .filter(|cases| cases.len() == 2)
        .ok_or_else(|| String::from("finite Lean byte corpus case count drifted"))?;
    for (index, (task, producer, name)) in [
        (
            SourceTask::Commuted,
            Producer::Synthesis,
            "arithCommuted/Synthesis",
        ),
        (SourceTask::Subtract, Producer::Chunk, "arithSubtract/Chunk"),
    ]
    .iter()
    .copied()
    .enumerate()
    {
        let row = &cases[index];
        if row["name"] != name {
            return Err(format!("{name}: finite Lean corpus order drifted"));
        }
        let chunk_id: Vec<u8> = serde_json::from_value(row["chunk_id"].clone())
            .map_err(|error| format!("{name}: decode Lean chunk id: {error}"))?;
        let source_revision: Vec<u8> = serde_json::from_value(row["source_revision"].clone())
            .map_err(|error| format!("{name}: decode Lean source revision: {error}"))?;
        let subject: Vec<u8> = serde_json::from_value(row["subject_bytes"].clone())
            .map_err(|error| format!("{name}: decode Lean subject bytes: {error}"))?;
        let actual = super::producer::task_record(task, producer)?;
        if actual.chunk.chunk_id.as_slice() != chunk_id
            || actual.source_revision().as_slice() != source_revision
            || actual
                .identity_subject_bytes()
                .map_err(|error| format!("{name}: encode Rust immutable subject: {error:?}"))?
                != subject
        {
            return Err(format!(
                "{name}: Rust subject bytes differ from Lean generated fixture"
            ));
        }
    }
    Ok(())
}
