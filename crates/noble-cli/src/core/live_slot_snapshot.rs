//! Independent, one-idle-state interpreter of the selected linear cell layout.
//! It checks actual imported memory bytes and all imported globals against the
//! captured owner graph. It does NOT prove that future guest executions leave
//! memory unchanged, nor does it prove helper/dispatcher Wasm refinement.
use std::collections::BTreeMap;
use serde_json::{Value, json};

const MEMORY_BYTES: usize = 20 * 65536;
const CELL_BASE: usize = 65536;
const CELL_BYTES: usize = 48;
const MAX_CELLS: usize = 32;
const MAX_HEAP_CURSOR: usize = 4096;
const SAVED_ROOTS_BASE: usize = 1060864;

fn denied(reason: &str) -> String { format!("selected exact idle memory refused: {reason}") }

fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn memory(snapshot: &Value) -> Result<Vec<u8>, String> {
    let encoded = snapshot["memory_hex"].as_str()
        .ok_or_else(|| denied("exact imported memory bytes are missing"))?;
    if encoded.len() != MEMORY_BYTES * 2 {
        return Err(denied("exact imported linear memory has a different byte length"));
    }
    let mut bytes = Vec::with_capacity(MEMORY_BYTES);
    for pair in encoded.as_bytes().chunks_exact(2) {
        let high = nibble(pair[0]).ok_or_else(|| denied("invalid memory hex digit"))?;
        let low = nibble(pair[1]).ok_or_else(|| denied("invalid memory hex digit"))?;
        bytes.push((high << 4) | low);
    }
    if snapshot["memory_sha256"] != crate::workflow::intrinsic::sha256(&bytes) {
        return Err(denied("imported memory SHA differs from supplied exact bytes"));
    }
    Ok(bytes)
}

fn globals(snapshot: &Value) -> Result<(Value, u32), String> {
    let base: Value = serde_json::from_str(include_str!("runtime/abi.json"))
        .map_err(|_| denied("host imported-global ABI is unavailable"))?;
    let mut expected = base["globals"].as_array()
        .ok_or_else(|| denied("host imported-global ABI is incomplete"))?
        .iter().map(|row| (row["name"].as_str(), row["type"].as_str()))
        .collect::<Vec<_>>();
    expected.extend([(Some("live_target"), Some("i32")),
        (Some("live_frame"), Some("i32"))]);
    let rows = snapshot["imported_globals"].as_array()
        .filter(|rows| rows.len() == expected.len())
        .ok_or_else(|| denied("imported global snapshot is incomplete"))?;
    let mut heap_cursor = None;
    for (row, (name, ty)) in rows.iter().zip(&expected) {
        if row["name"].as_str() != *name || row["type"].as_str() != *ty {
            return Err(denied("imported global name, type or ABI order differs"));
        }
        let value = row["value"].as_str()
            .ok_or_else(|| denied("imported global lacks exact decimal value"))?;
        match ty {
            Some("i32") => {
                let number = value.parse::<i32>()
                    .map_err(|_| denied("imported I32 global is out of range"))?;
                if number.to_string() != value { return Err(denied("imported I32 global is noncanonical")); }
                if *name == Some("heap_cursor") {
                    heap_cursor = u32::try_from(number).ok();
                }
                if [Some("failure"), Some("sp"), Some("cp")].contains(name) && number != 0 {
                    return Err(denied("owner inspection is not an idle normal state"));
                }
            }
            Some("i64") => {
                let number = value.parse::<i64>()
                    .map_err(|_| denied("imported I64 global is out of range"))?;
                if number.to_string() != value { return Err(denied("imported I64 global is noncanonical")); }
            }
            _ => return Err(denied("unsupported imported-global type")),
        }
    }
    let cursor = heap_cursor.filter(|cursor| *cursor > 0 && *cursor <= MAX_HEAP_CURSOR as u32)
        .ok_or_else(|| denied("live heap cursor is invalid"))?;
    Ok((snapshot["imported_globals"].clone(), cursor))
}

fn u32_at(memory: &[u8], address: usize) -> Result<u32, String> {
    let bytes: [u8; 4] = memory.get(address..address + 4)
        .ok_or_else(|| denied("cell field extends beyond imported memory"))?
        .try_into().map_err(|_| denied("invalid I32 cell field"))?;
    Ok(u32::from_le_bytes(bytes))
}
fn u64_at(memory: &[u8], address: usize) -> Result<u64, String> {
    let bytes: [u8; 8] = memory.get(address..address + 8)
        .ok_or_else(|| denied("cell field extends beyond imported memory"))?
        .try_into().map_err(|_| denied("invalid I64 cell field"))?;
    Ok(u64::from_le_bytes(bytes))
}

fn cell(memory: &[u8], handle: u32, cursor: u32) -> Result<([u32; 3], Value), String> {
    let slot = handle & 4095;
    if handle < 4096 || slot >= cursor {
        return Err(denied("cell handle is outside the live epoch/heap cursor"));
    }
    let address = CELL_BASE + slot as usize * CELL_BYTES;
    if u32_at(memory, address + 4)? != handle {
        return Err(denied("cell epoch header differs from observed handle"));
    }
    let kind = u32_at(memory, address)? & 255;
    if kind == 0 || kind > 16 || kind == 11 {
        return Err(denied("captured Program graph has an unknown or text cell"));
    }
    let edges = [u32_at(memory, address + 16)?,
        u32_at(memory, address + 20)?, u32_at(memory, address + 24)?];
    Ok((edges, json!({"kind":kind,"payload":(u64_at(memory, address + 8)? as i64).to_string(),
        "a":null,"b":null,"c":null,"x":u32_at(memory, address + 28)?,
        "y":u32_at(memory, address + 32)?,"z":u32_at(memory, address + 36)?,
        "w":u32_at(memory, address + 40)?,"n":u32_at(memory, address + 44)?,
        "textBytes":null})))
}

fn complete_graph(memory: &[u8], root: u32, cursor: u32) -> Result<Value, String> {
    let mut handles = vec![root];
    let mut indices = BTreeMap::from([(root, 0usize)]);
    let mut rows = Vec::new();
    let mut next = 0;
    while next < handles.len() {
        let (edges, mut row) = cell(memory, handles[next], cursor)?;
        for (key, child) in ["a", "b", "c"].into_iter().zip(edges) {
            if child == 0 { continue; }
            let at = if let Some(at) = indices.get(&child) { *at } else {
                if handles.len() == MAX_CELLS {
                    return Err(denied("retained graph exceeds complete observation bound"));
                }
                let at = handles.len();
                handles.push(child);
                indices.insert(child, at);
                at
            };
            row[key] = json!(at);
        }
        rows.push(row);
        next += 1;
    }
    Ok(json!(rows))
}

fn roots(snapshot: &Value, key: &str, memory: &[u8], cursor: u32) -> Result<Value, String> {
    let rows = snapshot[key].as_array().filter(|rows| rows.len() <= 128)
        .ok_or_else(|| denied("installed module roots are missing or unbounded"))?;
    for row in rows {
        let handle = row.as_u64().and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| denied("installed module root is not a handle"))?;
        let (_, node) = cell(memory, handle, cursor)?;
        if node["kind"] != 4 { return Err(denied("installed module root is not a live Program")); }
    }
    Ok(snapshot[key].clone())
}

/// This *executes* the selected 48-byte cell layout over actual imported
/// memory, not over the worker's exported accessor claims. Its output binds
/// one immutable snapshot only; a full Wasm helper/dispatch semantics theorem
/// and a later fresh-state relation remain separate, mandatory prerequisites.
pub(super) fn check(snapshot: &Value, manifest: &Value, subject: &Value) -> Result<Value, String> {
    if snapshot["schema"] != "noble-live-slot-idle-memory-snapshot/v1"
        || snapshot["scope"] != "one-idle-state-only; not future-invocation semantics" {
        return Err(denied("snapshot does not declare its one-state scope"));
    }
    let owner = manifest["descriptor"]["owner"]["handle"].as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| denied("checked owner has no physical root handle"))?;
    if snapshot["owner_handle"] != owner {
        return Err(denied("idle snapshot root differs from retained owner"));
    }
    let memory = memory(snapshot)?;
    let (globals, cursor) = globals(snapshot)?;
    let slot = owner & 4095;
    let saved = SAVED_ROOTS_BASE + slot as usize * 2;
    let saved_count = memory.get(saved..saved + 2)
        .and_then(|bytes| bytes.try_into().ok()).map(u16::from_le_bytes)
        .ok_or_else(|| denied("retained owner root count is outside imported memory"))?;
    if slot >= cursor || saved_count == 0 {
        return Err(denied("retained owner has no saved root in imported memory"));
    }
    let graph = complete_graph(&memory, owner, cursor)?;
    let graph_bytes = serde_json::to_vec(&graph)
        .map_err(|_| denied("imported memory graph is not serializable"))?;
    let graph_sha = crate::workflow::intrinsic::sha256(&graph_bytes);
    if manifest["descriptor"]["graph_sha256"] != graph_sha {
        return Err(denied("imported linear memory graph differs from saved source target"));
    }
    let literal = |quote: &Value| -> Result<Value, String> {
        let quote_at = quote.as_u64().and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| denied("snapshot has no quote edge"))?;
        let quotes = graph.as_array().ok_or_else(|| denied("snapshot graph is not an array"))?;
        let node = quotes.get(quote_at).ok_or_else(|| denied("snapshot quote node is missing"))?;
        if node["kind"] != 4 || node["x"] != 1 { return Err(denied("snapshot quote differs from native entry")); }
        let atom = node["a"].as_u64().and_then(|n| usize::try_from(n).ok())
            .and_then(|n| quotes.get(n)).ok_or_else(|| denied("snapshot quote capture is missing"))?;
        if atom["kind"] != 1 { return Err(denied("snapshot quotation did not capture I64")); }
        Ok(atom["payload"].clone())
    };
    let rows = graph.as_array().ok_or_else(|| denied("snapshot graph is missing"))?;
    let inner = rows.first().and_then(|row| row["a"].as_u64())
        .and_then(|index| rows.get(index as usize))
        .ok_or_else(|| denied("snapshot has no nested composition"))?;
    let literals = json!([literal(&inner["a"])?, literal(&inner["b"])?]);
    if subject["literal_values"] != literals {
        return Err(denied("snapshot memory literals differ from checked source captures"));
    }
    let native_roots = roots(snapshot, "native_roots", &memory, cursor)?;
    let static_roots = roots(snapshot, "static_roots", &memory, cursor)?;
    Ok(json!({"schema":"noble-live-slot-one-idle-state-replay/v1",
        "scope":"one-state-layout-only; native helper/dispatch semantics not established",
        "memory_sha256":snapshot["memory_sha256"],"memory_length":memory.len(),
        "owner_handle":owner,"imported_globals":globals,
        "native_roots":native_roots,"static_roots":static_roots,
        "retained_graph_sha256":graph_sha,"physical_literals":literals}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_cells_reject_recycled_child_handle() -> Result<(), String> {
        let mut imported = vec![0; MEMORY_BYTES];
        let root = CELL_BASE;
        let child = root + CELL_BYTES;
        imported[root..root + 4].copy_from_slice(&4u32.to_le_bytes());
        imported[root + 4..root + 8].copy_from_slice(&4096u32.to_le_bytes());
        imported[root + 16..root + 20].copy_from_slice(&4097u32.to_le_bytes());
        imported[child..child + 4].copy_from_slice(&1u32.to_le_bytes());
        imported[child + 4..child + 8].copy_from_slice(&4097u32.to_le_bytes());
        imported[child + 8..child + 16].copy_from_slice(&(-7i64).to_le_bytes());
        let graph = complete_graph(&imported, 4096, 2)?;
        if graph[0]["a"] != 1 || graph[1]["payload"] != "-7" {
            return Err("raw imported memory did not reconstruct the selected child".into());
        }
        imported[child + 4..child + 8].copy_from_slice(&8193u32.to_le_bytes());
        if complete_graph(&imported, 4096, 2).is_ok() {
            return Err("recycled physical child slot passed an obsolete epoch handle".into());
        }
        Ok(())
    }
}
