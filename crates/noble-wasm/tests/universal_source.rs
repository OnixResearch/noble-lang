#![feature(register_tool)]
#![register_tool(tigerstyle)]

use noble_contracts::source::{ModuleSession, RetainedSourceLimit};
use noble_kernel::types::Ty;
use noble_wasm::source::Compiler;

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

const SOURCE: &[u8] = b"module stack_reuse@1 [
    signature pass forall<S:stack> [ S I64 -- S I64 ! pure ]
    def pass [ 1 + ]
    signature both forall<S:stack> [ S I64 -- S I64 ! pure ]
    export both
    def both [ dup [ pass ] dip pass drop ]
]";

fn diagnostic(error: noble_wasm::Diagnostic) -> &'static str {
    match error {
        noble_wasm::Diagnostic::Invalid => "invalid",
        noble_wasm::Diagnostic::Exhausted => "exhausted",
        noble_wasm::Diagnostic::Unsupported => "unsupported",
        noble_wasm::Diagnostic::Defective => "defective",
    }
}

#[test]
fn one_owner_code_handles_two_distinct_checked_stack_prefixes_and_reuses_old_span()
    -> Result<(), String>
{
    let session = ModuleSession::new_declared_text(&[], true,
        RetainedSourceLimit::new(65_536).ok_or("invalid retained source cap")?)
        .map_err(|error| error.diagnostic().message.clone())?;
    let staged = session.prepare(SOURCE, &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let (session, outcome) = session.commit(staged);
    outcome.map_err(|error| error.diagnostic().message.clone())?;
    let selected = session.prepare(b"stack_reuse@1.both", &[Ty::I64], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let owner = selected.declared_text_source_owner().ok_or("source owner absent")?;
    let mut compiler = Compiler::new_declared_text();
    if !owner.selected_signed_universal_intent()
        || compiler.prepare_declared_text(&owner).err() != Some(noble_wasm::Diagnostic::Unsupported)
    {
        return Err("signed universal owner fell through the concrete compiler ingress".into());
    }
    let initial = compiler.prepare_declared_text_universal(&owner)
        .map_err(|error| format!("checked universal admission failed: {}", diagnostic(error)))?;
    let first = initial.universal_metadata().ok_or("universal receipt absent")?;
    let selected_program = first.programs.iter()
        .find(|program| program.use_index == first.selected_use
            && program.quotation_node.is_none())
        .ok_or("selected concrete owner Program absent")?;
    let named_sites: Vec<_> = selected_program.sites.iter()
        .filter(|site| site.action == 2).collect();
    if named_sites.len() != 1 {
        return Err("selected body did not retain its checked named edge".into());
    }
    let called_owner = first.owners.iter()
        .find(|candidate| candidate.definition == named_sites[0].target_definition)
        .ok_or("named source identity absent")?;
    let called_owner_index = called_owner.owner_index;
    let calls: Vec<_> = first.programs.iter()
        .filter(|program| program.owner_index == called_owner_index
            && program.quotation_node.is_none()).collect();
    if calls.len() != 2 || calls[0].entry != calls[1].entry
        || calls[0].slot == calls[1].slot
        || calls[0].input_signature == calls[1].input_signature
        || calls[0].output_signature == calls[1].output_signature
    {
        return Err("different concrete tails aliased an ABI or duplicated owner code".into());
    }
    let old_entry = calls[0].entry;
    let code_end = initial.code_span().start + initial.code_span().length;
    compiler.commit(initial).map_err(|_| "first checked commit refused")?;
    let reused = compiler.prepare_declared_text_universal(&owner)
        .map_err(|error| format!("old source code reuse refused: {}", diagnostic(error)))?;
    let receipt = reused.universal_metadata().ok_or("reused source receipt absent")?;
    if receipt.owners.iter().any(|owner| owner.newly_compiled)
        || reused.code_span().start != code_end
        || reused.code_span().length != 1
        || receipt.programs.iter().filter(|program| program.owner_index == called_owner_index
            && program.quotation_node.is_none())
            .any(|program| program.entry != old_entry)
    {
        return Err("new selected generation recompiled or rebound the old owner body".into());
    }
    let committed_generation = compiler.declared_text_epoch_state()
        .ok_or("declared source epoch disappeared")?.0;
    compiler.retire_declared_text_epoch(committed_generation, code_end)
        .map_err(|_| "clean source epoch retirement refused")?;
    if compiler.commit(reused).is_ok() {
        return Err("stale old-generation descriptor published after source retirement".into());
    }
    let fresh = compiler.prepare_declared_text_universal(&owner)
        .map_err(|error| format!("new source epoch refused: {}", diagnostic(error)))?;
    let fresh_receipt = fresh.universal_metadata().ok_or("new source epoch receipt absent")?;
    if fresh_receipt.source_epoch != 1
        || fresh_receipt.owners.iter().any(|owner| !owner.newly_compiled)
        || fresh.code_span().start != 4
    {
        return Err("old-generation source code leaked across full epoch retirement".into());
    }
    Ok(())
}

#[test]
fn frozen_signed_addition_reuses_exact_recursive_scan_code_with_distinct_checked_abi()
    -> Result<(), String>
{
    const FROZEN: [&[u8]; 10] = [
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitstep.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitpair.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitfull.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitcompare.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bittrim.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitsigned.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitsum.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitaddcarry.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitorder.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitaddsigned.noble")),
    ];
    let mut session = ModuleSession::new_declared_text(&[], true,
        RetainedSourceLimit::new(262_144).ok_or("invalid original retained source cap")?)
        .map_err(|error| error.diagnostic().message.clone())?;
    for source in FROZEN {
        let staged = session.prepare(source, &[], noble_contracts::Limits::default())
            .map_err(|error| error.diagnostic().message.clone())?;
        let (next, result) = session.commit(staged);
        result.map_err(|error| error.diagnostic().message.clone())?;
        session = next;
    }
    let magnitude = Ty::Pair(Box::new(Ty::Bool),
        Box::new(Ty::List(Box::new(Ty::Bool))));
    let input = [magnitude.clone(), magnitude.clone()];
    let selected = session.prepare(b"bitaddsigned@1.add", &input,
        noble_contracts::Limits::default())
        .map_err(|error| error.diagnostic().message.clone())?;
    let owner = selected.declared_text_source_owner().ok_or("source owner absent")?;
    let mut compiler = Compiler::new_declared_text();
    let first = compiler.prepare_declared_text_universal(&owner)
        .map_err(|error| format!("frozen signed compiler admission: {}", diagnostic(error)))?;
    let metadata = first.universal_metadata().ok_or("universal receipt absent")?;
    if metadata.owners.iter().any(|owner|
        owner.stamp.generation > metadata.source_generation)
    {
        return Err("owner stamp exceeds the source namespace in its guest receipt".into());
    }
    let scan_owner = metadata.owners.iter()
        .find(|candidate| candidate.source.as_ref() == FROZEN[6]
            && candidate.definition_ordinal == 5)
        .ok_or("frozen bitsum.scan owner absent")?;
    let scan_identity = scan_owner.definition_identity;
    let scan_entry = scan_owner.body_entry;
    let scan_programs: Vec<_> = metadata.programs.iter()
        .filter(|program| program.owner_index == scan_owner.owner_index).collect();
    if scan_programs.is_empty() || !scan_programs.iter().any(|program|
        program.sites.iter().any(|site| site.action == 2
            && site.target_identity == scan_identity
            && site.target_use == Some(program.use_index)))
    {
        return Err("original bitsum.scan recursive quote has no checked self-edge".into());
    }
    let original_scans: Vec<_> = scan_programs.iter()
        .filter(|program| program.quotation_node.is_none())
        .map(|program| (program.input_signature, program.output_signature, program.slot))
        .collect();
    if original_scans.is_empty() { return Err("original scan Program absent".into()); }
    let old_span_end = first.code_span().start + first.code_span().length;
    compiler.commit(first).map_err(|_| "first frozen compile refused")?;
    let other_input = [Ty::I64, magnitude.clone(), magnitude];
    let next_selected = session.prepare(b"bitaddsigned@1.add", &other_input,
        noble_contracts::Limits::default())
        .map_err(|error| error.diagnostic().message.clone())?;
    let next_owner = next_selected.declared_text_source_owner()
        .ok_or("second source owner absent")?;
    let reused = compiler.prepare_declared_text_universal(&next_owner)
        .map_err(|error| format!("second frozen checked ABI refused: {}", diagnostic(error)))?;
    let next = reused.universal_metadata().ok_or("second universal receipt absent")?;
    let next_scan = next.owners.iter()
        .find(|candidate| candidate.definition_identity == scan_identity)
        .ok_or("new checked scan use lost old owner identity")?;
    let new_scans: Vec<_> = next.programs.iter().filter(|program|
        program.owner_index == next_scan.owner_index && program.quotation_node.is_none())
        .collect();
    if new_scans.is_empty() { return Err("new scan Program absent".into()); }
    if next.owners.iter().any(|candidate| candidate.newly_compiled)
        || reused.code_span().start != old_span_end
        || reused.code_span().length != 1
        || next_scan.body_entry != scan_entry
        || new_scans.iter().any(|program| program.entry != scan_entry
            || original_scans.iter().any(|(_, _, slot)| *slot == program.slot))
        || !new_scans.iter().any(|program| !original_scans.iter().any(|(input, output, _)|
            *input == program.input_signature && *output == program.output_signature))
    {
        return Err("frozen signed source recopied code or aliased concrete ABIs".into());
    }
    Ok(())
}

#[test]
fn empty_owner_and_empty_quote_have_checked_entries_not_unguarded_core_empty()
    -> Result<(), String>
{
    let source = b"module empty@1 [
        signature direct forall<S:stack> [ S I64 -- S I64 ! pure ]
        def direct [ ]
        signature selected forall<S:stack> [ S I64 -- S I64 ! pure ]
        export selected
        def selected [ [ ] dip direct ]
    ]";
    let session = ModuleSession::new_declared_text(&[], true,
        RetainedSourceLimit::new(65_536).ok_or("invalid retained source cap")?)
        .map_err(|error| error.diagnostic().message.clone())?;
    let staged = session.prepare(source, &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let (session, outcome) = session.commit(staged);
    outcome.map_err(|error| error.diagnostic().message.clone())?;
    let selected = session.prepare(b"empty@1.selected", &[Ty::I64], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let owner = selected.declared_text_source_owner().ok_or("source owner absent")?;
    let mut compiler = Compiler::new_declared_text();
    let first = compiler.prepare_declared_text_universal(&owner)
        .map_err(|error| format!("empty source admission failed: {}", diagnostic(error)))?;
    let receipt = first.universal_metadata().ok_or("universal receipt absent")?;
    let empty: Vec<_> = receipt.programs.iter()
        .filter(|program| program.atoms.is_empty()).collect();
    if empty.len() < 2 || empty.iter().any(|program| program.entry < 4) {
        return Err("empty checked owner or quote lacks a guard-only table entry".into());
    }
    let end = first.code_span().start + first.code_span().length;
    compiler.commit(first).map_err(|_| "empty source commit refused")?;
    let old_generation = compiler.declared_text_epoch_state()
        .ok_or("source epoch state absent")?.0;
    let stale = compiler.prepare_declared_text_universal(&owner)
        .map_err(|error| format!("old empty source reuse failed: {}", diagnostic(error)))?;
    compiler.retire_declared_text_epoch(old_generation, end)
        .map_err(|_| "full empty source retirement refused")?;
    if compiler.commit(stale).is_ok() {
        return Err("stale empty source preparation crossed retired epoch".into());
    }
    let fresh = compiler.prepare_declared_text_universal(&owner)
        .map_err(|error| format!("fresh empty source admission failed: {}", diagnostic(error)))?;
    if fresh.universal_metadata().ok_or("fresh empty receipt absent")?.source_epoch != 1 {
        return Err("new empty code did not bind new source epoch".into());
    }
    Ok(())
}
