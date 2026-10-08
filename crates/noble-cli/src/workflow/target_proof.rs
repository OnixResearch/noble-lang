//! Independent, exact returned-Program Lean obligation. This never consumes a
//! builder theorem or treats a Wasm SHA as proof of Wasm-to-model semantics.
use serde_json::Value;

use super::{output, verification};

fn invalid(message: &str) -> output::Failure {
    output::Failure::unsupported("live-slot-target-subject", message.into())
}

fn digest<'a>(value: &'a Value, key: &str) -> Result<&'a str, output::Failure> {
    let digest = value[key].as_str().ok_or_else(|| invalid("target digest is missing"))?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(invalid("target digest is not lowercase SHA-256"));
    }
    Ok(digest)
}

// All inserted text originates in the independently reconstructed Host
// subject; decimal captures and lowercase digests cannot inject Lean syntax.
fn lower(checked: &Value) -> Result<(String, String), output::Failure> {
    let subject = &checked["subject"];
    if subject["schema"] != "noble-live-slot-returned-target-lean/v1"
        || subject["claim"] != "pure-i64-i64-to-i64-two-literal-three-wrapping-add/v1"
        || subject["assumptions"] != serde_json::json!([])
        || subject["manifest"]["schema"] != "noble-live-slot-composite-code-donors/v1"
    {
        return Err(invalid("unsupported returned Program proof model or assumption"));
    }
    let key = digest(checked, "theorem_key")?;
    let mut keyed = b"noble live-slot exact returned target lean theorem v1\0".to_vec();
    keyed.extend_from_slice(&serde_json::to_vec(subject)
        .map_err(|_| invalid("exact target subject is not serializable"))?);
    if key != super::intrinsic::sha256(&keyed) {
        return Err(invalid("target theorem key differs from complete reconstructed subject"));
    }
    let manifest = digest(subject, "manifest_sha256")?;
    let mut described = b"noble live-slot composite code donors v1\0".to_vec();
    described.extend_from_slice(&serde_json::to_vec(&subject["manifest"])
        .map_err(|_| invalid("exact target composite descriptor is not serializable"))?);
    if manifest != super::intrinsic::sha256(&described) {
        return Err(invalid("target theorem subject has an inconsistent composite descriptor"));
    }
    let native = digest(&subject["manifest"]["donors"]["native"], "artifact_sha256")?;
    let static_sha = digest(&subject["manifest"]["donors"]["static"], "artifact_sha256")?;
    let native_bodies = &subject["native_body_witness"];
    if native_bodies["schema"] != "noble-live-slot-native-byte-closure/v1"
        || native_bodies["semantic_refinement"] != "not-established"
        || digest(&native_bodies["native"], "module_sha256")? != native
        || digest(&native_bodies["static"], "module_sha256")? != static_sha
        || native_bodies["checked_direct_bodies"] != serde_json::json!([
            "quote_entry","compose_entry","op_add","dispatch","selected_add_table_wrappers"])
    {
        return Err(invalid("exact installed native body witness is missing or inconsistent"));
    }
    let state = &subject["one_state_replay"];
    if state["schema"] != "noble-live-slot-one-idle-state-replay/v1"
        || state["scope"] != "one-state-layout-only; native helper/dispatch semantics not established"
        || state["memory_length"] != 20 * 65536
        || state["owner_handle"] != subject["manifest"]["owner"]["handle"]
        || state["retained_graph_sha256"] != subject["manifest"]["graph_sha256"]
        || state["physical_literals"] != subject["literal_values"]
    {
        return Err(invalid("exact target idle memory/roots witness is missing or inconsistent"));
    }
    let initial_memory = digest(state, "memory_sha256")?;
    let installed_storage = &state["installed_storage_probe"];
    if installed_storage["schema"] != "noble-live-slot-installed-storage-one-state/v1"
        || installed_storage["scope"] != "exact selected storage/child/metadata reads and stale-epoch negatives only; writes/refcounts/future calls unproved"
        || installed_storage["native_module_sha256"] != native
        || installed_storage["static_module_sha256"] != static_sha
        || installed_storage["selected_handles"].as_array()
            .is_none_or(|handles| handles.is_empty() || handles.len() > 32)
        || installed_storage["selected_handles"][0] != state["owner_handle"]
        || installed_storage["heap_cursor"].as_u64().is_none_or(|cursor| cursor == 0 || cursor > 4096) {
        return Err(invalid("installed helper one-state execution witness is missing or inconsistent"));
    }
    let owner_root = state["owner_handle"].as_u64()
        .ok_or_else(|| invalid("exact target idle memory owner root is missing"))?;
    let byte_donors = &subject["installed_donor_bytes"];
    if digest(&byte_donors["native"], "sha256")? != native
        || digest(&byte_donors["static"], "sha256")? != static_sha {
        return Err(invalid("retained installed bytes do not match selected donor digests"));
    }
    let length = |donor: &Value| -> Result<u64, output::Failure> {
        donor["length"].as_u64()
            .filter(|bytes| *bytes > 0 && *bytes <= 16 * 1024 * 1024)
            .ok_or_else(|| invalid("selected installed donor byte length is invalid"))
    };
    let native_len = length(&byte_donors["native"])?;
    let static_len = length(&byte_donors["static"])?;
    let captures = subject["literal_values"].as_array()
        .filter(|rows| rows.len() == 2)
        .ok_or_else(|| invalid("exact returned target requires two physical literals"))?;
    let decimal = |value: &Value| -> Result<(String, i64), output::Failure> {
        let text = value.as_str().ok_or_else(|| invalid("target literal is not a decimal string"))?;
        let parsed = text.parse::<i64>()
            .map_err(|_| invalid("target literal is outside signed I64"))?;
        if text != parsed.to_string() {
            return Err(invalid("target literal has a noncanonical representation"));
        }
        Ok((text.to_owned(), parsed))
    };
    let (first, first_value) = decimal(&captures[0])?;
    let (second, _) = decimal(&captures[1])?;
    let quote = &installed_storage["scalar_quote_step"];
    let handles = installed_storage["selected_handles"].as_array()
        .ok_or_else(|| invalid("selected captured graph handles are missing"))?;
    let quote_handle = quote["environment"].as_u64()
        .ok_or_else(|| invalid("direct quote environment is missing"))?;
    let capture_handle = quote["capture"].as_u64()
        .ok_or_else(|| invalid("direct quote captured cell is missing"))?;
    let before = quote["before_slot"].as_array().filter(|slot| slot.len() == 16)
        .ok_or_else(|| invalid("direct quote initial stack slot is incomplete"))?;
    let mut expected_slot = [0u8; 16];
    for (destination, byte) in expected_slot.iter_mut().zip(before) {
        *destination = byte.as_u64().and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| invalid("direct quote initial stack byte is invalid"))?;
    }
    expected_slot[..4].copy_from_slice(&1u32.to_le_bytes());
    expected_slot[8..].copy_from_slice(&first_value.to_le_bytes());
    let peak = quote["initial_operand_peak"].as_u64()
        .filter(|peak| *peak <= 2048)
        .ok_or_else(|| invalid("direct quote initial operand peak is invalid"))?;
    let calls_text = quote["initial_quote_invocations"].as_str()
        .ok_or_else(|| invalid("direct quote counter is missing"))?;
    let calls = calls_text.parse::<i32>().ok()
        .ok_or_else(|| invalid("direct quote counter is invalid"))?;
    if calls.to_string() != calls_text {
        return Err(invalid("direct quote counter is noncanonical"));
    }
    if quote["schema"] != "noble-live-slot-installed-direct-scalar-quote/v1"
        || quote["scope"] != "one captured I64 quote-entry step only; no dispatch or full composite proof"
        || quote["execution_backend"] != "bounded decoded installed operators; not an actual V8 guest transition"
        || quote["sidecar_scope"] != "unchanged under speculative slot overlay only; actual guest sidecar not observed"
        || quote["module_sha256"] != native
        || quote["memory_sha256"] != initial_memory
        || quote["literal"] != first
        || quote["initial_sp"] != 0 || quote["final_sp"] != 1
        || quote["final_operand_peak"] != peak.max(16)
        || quote["final_quote_invocations"] != ((calls as u32).wrapping_add(1) as i32).to_string()
        || quote["after_slot"] != serde_json::json!(expected_slot)
        || quote["heap_and_refcount_writes"] != 0
        || quote["wasm_lean_c_refinement"] != "not-established"
        || !handles.contains(&serde_json::json!(quote_handle))
        || !handles.contains(&serde_json::json!(capture_handle))
        || capture_handle == 0 {
        return Err(invalid("direct scalar quote witness differs from selected captured stack step"));
    }
    let _sidecar = digest(quote, "refcount_sidecar_sha256")?;
    if subject["target_code"] != serde_json::json!([
        {"op":"lit-i64","value":first.as_str()},
        {"op":"lit-i64","value":second.as_str()},
        {"op":"word","index":4},
        {"op":"word","index":4},
        {"op":"word","index":4}
    ]) {
        return Err(invalid("checked target code differs from the finite proved recipe"));
    }
    let obligation = format!(r#"import NobleContracts.Model
namespace LiveSlotTargetObligation
open NobleContracts

-- This is the reconstructed returned value's pure code, not its builder.
-- Artifact correspondence is a separate Host-checked premise, not a Lean axiom.
-- Native operator semantics are not established by this Host provenance check.
def generatedCode (first second : Int) : List Op :=
  [.lit (NobleContracts.i64 first), .lit (NobleContracts.i64 second),
    .word 4, .word 4, .word 4]

structure Subject where
  theoremKey : String
  claimName : String
  assumptions : List String
  manifestSHA256 : String
  nativeWasmSHA256 : String
  staticWasmSHA256 : String
  nativeWasmBytes : Nat
  staticWasmBytes : Nat
  initialMemorySHA256 : String
  ownerRoot : Nat
  initialStateScope : String
  first : Int
  second : Int
  code : List Op

def subject : Subject := {{
  theoremKey := "{key}"
  claimName := "pure-i64-i64-to-i64-two-literal-three-wrapping-add/v1"
  assumptions := []
  manifestSHA256 := "{manifest}"
  nativeWasmSHA256 := "{native}"
  staticWasmSHA256 := "{static_sha}"
  nativeWasmBytes := {native_len}
  staticWasmBytes := {static_len}
  initialMemorySHA256 := "{initial_memory}"
  ownerRoot := {owner_root}
  initialStateScope := "one-state-layout-only; native helper/dispatch semantics not established"
  first := ({first})
  second := ({second})
  code := generatedCode ({first}) ({second})
}}

-- The metadata equalities bind the checked theorem's *type* to this owner's
-- independently computed theorem key and two installed donor byte hashes.
-- They do not claim Lean can derive a Wasm correspondence from a digest.
def claim : Prop :=
  subject.theoremKey = "{key}" ∧
  subject.claimName = "pure-i64-i64-to-i64-two-literal-three-wrapping-add/v1" ∧
  subject.assumptions = [] ∧
  subject.manifestSHA256 = "{manifest}" ∧
  subject.nativeWasmSHA256 = "{native}" ∧
  subject.staticWasmSHA256 = "{static_sha}" ∧
  subject.nativeWasmBytes = {native_len} ∧
  subject.staticWasmBytes = {static_len} ∧
  subject.initialMemorySHA256 = "{initial_memory}" ∧
  subject.ownerRoot = {owner_root} ∧
  subject.initialStateScope = "one-state-layout-only; native helper/dispatch semantics not established" ∧
  CodeTyped subject.code [.i64, .i64] [.i64] ∧
  ∀ (x y : BitVec 64) (tail result : Stack),
    Exec subject.code (tail ++ [.i64 x, .i64 y]) result →
      result = tail ++ [.i64 (x + (y +
        (BitVec.ofInt 64 subject.first + BitVec.ofInt 64 subject.second)))]
end LiveSlotTargetObligation
"#);
    let term = format!(r#"by
  unfold LiveSlotTargetObligation.claim LiveSlotTargetObligation.subject
  refine ⟨rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl, ?_, ?_⟩
  · change NobleContracts.CodeTyped
      (LiveSlotTargetObligation.generatedCode ({first}) ({second}))
      [.i64, .i64] [.i64]
    unfold LiveSlotTargetObligation.generatedCode
    exact .cons (NobleContracts.OpTyped.lit (s := [.i64, .i64]) NobleContracts.HasType.i64)
      (.cons (NobleContracts.OpTyped.lit (s := [.i64, .i64, .i64]) NobleContracts.HasType.i64)
        (.cons (NobleContracts.OpTyped.add (s := [.i64, .i64]))
          (.cons (NobleContracts.OpTyped.add (s := [.i64]))
            (.cons (NobleContracts.OpTyped.add (s := [])) .nil))))
  · intro x y tail result run
    have exact_run : NobleContracts.Exec
        (LiveSlotTargetObligation.generatedCode ({first}) ({second}))
        (tail ++ [.i64 x, .i64 y]) result ↔
        result = tail ++ [.i64 (x + (y +
          (BitVec.ofInt 64 ({first}) + BitVec.ofInt 64 ({second}))))] := by
      simp [LiveSlotTargetObligation.generatedCode, NobleContracts.Exec,
        NobleContracts.i64]
    exact exact_run.mp run"#);
    if obligation.len() > super::SOURCE_LIMIT || term.len() > super::PROOF_LIMIT {
        return Err(invalid("exact returned target Lean source exceeds bound"));
    }
    Ok((obligation, term))
}

pub(crate) fn verify(checked: &Value) -> Result<verification::Acceptance, output::Failure> {
    let (obligation, term) = lower(checked)?;
    let library = super::rules::read_library()?;
    super::intrinsic::pinned_library(&library)?;
    verification::lslot_target::verify(&obligation, &term, &library, super::DEFAULT_TIMEOUT)
}
