mod machine;

use super::types::{CheckedArtifact, Refusal};

pub(super) fn check<'a>(
    ids: &dyn crate::identity::IdentityScheme,
    checked: &crate::check::CheckedGraph,
    bytes: &'a [u8],
) -> Result<CheckedArtifact<'a>, Refusal> {
    super::support::admit_graph(checked)?;
    let view = super::codec::decode(ids, bytes)?;
    if view.graph_id != checked.id() {
        return Err(Refusal::GraphIdentity);
    }
    machine::validate_stream(checked, &view)?;
    Ok(CheckedArtifact {
        bytes: view.bytes,
        graph_id: view.graph_id,
        capacity: view.capacity,
        save_slots: view.save_slots,
        instruction_digest: view.instruction_digest,
        artifact_binding: view.artifact_binding,
        instruction_count: view.instruction_count,
    })
}
