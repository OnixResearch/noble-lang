//! Compiler-side fields of a calculator trace record. The trace writer
//! stores this object under the record's `compile` key; the headroom check
//! (`tools/calculator-headroom.mjs`) compares it against the embedded caps.

pub(super) fn compile_json(metrics: &noble_wasm::source::CompileMetrics) -> serde_json::Value {
    let caps = noble_wasm::source::CALCULATOR_CAPS;
    serde_json::json!({
        "work_initial": metrics.work_initial,
        "work_preflight_grant": metrics.work_preflight_grant,
        "work_environment_grant": metrics.work_environment_grant,
        "work_plan_grant": metrics.work_plan_grant,
        "work_issued": metrics.work_issued,
        "work_spent": metrics.work_spent,
        "environment_rows": metrics.environment_rows,
        "environment_work": metrics.environment_work,
        "named_bodies": metrics.named_bodies,
        "max_type_size": metrics.max_type_size,
        "signature_keys": metrics.signature_keys,
        "signature_key_charge": metrics.signature_key_charge,
        "identity_charge": metrics.identity_charge,
        "table_end": metrics.table_end,
        "text_end": metrics.text_end,
        "wat_bytes": metrics.wat_bytes,
        "caps": {
            "work_issued": caps.work_issued,
            "type_size": metrics.type_size_limit,
            "environment_rows": caps.environment_rows,
            "named_bodies": caps.named_bodies,
            "signature_keys": caps.signature_keys,
            "table_entries": caps.table_entries,
            "wat_bytes": caps.wat_bytes,
            "text_start": caps.text_start,
            "text_end": caps.text_end,
        },
    })
}
