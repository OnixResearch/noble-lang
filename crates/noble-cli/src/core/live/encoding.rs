//! In-process encoding for compiler-produced WAT at the opt-in live CLI boundary.
//! A valid ABI is not source authenticity: callers must first independently
//! accept the source and must not feed arbitrary WAT to the resident engine.

use std::collections::HashSet;
use std::sync::LazyLock;

use serde_json::Value;
use wasmparser::{
    DataKind, ElementKind, Encoding, ExternalKind, FuncType, Import, MemoryType, Parser, Payload,
    RefType, TableType, TypeRef, ValType, Validator, WasmFeatures,
};

const MAX_BYTES: usize = 4 * 1024 * 1024;
const ABI_JSON: &str = include_str!("../runtime/abi.json");
static ABI: LazyLock<Result<Value, String>> = LazyLock::new(|| {
    serde_json::from_str(ABI_JSON).map_err(|error| format!("invalid core ABI: {error}"))
});

#[derive(Debug, PartialEq, Eq)]
pub(super) enum EncodeError {
    HostEffect(String),
    Other(String),
}

impl EncodeError {
    pub(super) fn as_str(&self) -> &str {
        match self {
            Self::HostEffect(message) | Self::Other(message) => message,
        }
    }
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str(self.as_str())
    }
}

impl From<String> for EncodeError {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}

impl From<&'static str> for EncodeError {
    fn from(message: &'static str) -> Self {
        Self::Other(message.to_owned())
    }
}

fn abi() -> Result<&'static Value, String> {
    ABI.as_ref().map_err(Clone::clone)
}

fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing core ABI {field}"))
}

fn number(value: &Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing core ABI {field}"))
}

fn val_type(name: &str) -> Result<ValType, String> {
    match name {
        "i32" => Ok(ValType::I32),
        "i64" => Ok(ValType::I64),
        _ => Err(format!("unsupported core ABI value type: {name}")),
    }
}

fn func_type<'a>(
    functions: &[u32],
    types: &'a [FuncType],
    index: u32,
) -> Result<&'a FuncType, String> {
    let type_index = *functions
        .get(index as usize)
        .ok_or_else(|| format!("missing function index {index}"))?;
    types
        .get(type_index as usize)
        .ok_or_else(|| format!("missing function type {type_index}"))
}

fn matches_params<'a>(
    actual: &[ValType],
    expected: impl Iterator<Item = &'a str>,
) -> Result<bool, String> {
    let mut actual = actual.iter();
    for expected_type in expected {
        if actual.next() != Some(&val_type(expected_type)?) {
            return Ok(false);
        }
    }
    Ok(actual.next().is_none())
}

fn check_export_signature(actual: &FuncType, descriptor: &str, name: &str) -> Result<(), String> {
    let (params, result) = descriptor
        .split_once(")->")
        .and_then(|(params, result)| params.strip_prefix('(').map(|params| (params, result)))
        .ok_or_else(|| format!("invalid ABI export descriptor: {name}"))?;
    let matches_inputs = matches_params(
        actual.params(),
        params
            .split(',')
            .filter(|parameter| !parameter.is_empty())
            .map(|parameter| parameter.rsplit_once(':').map_or("", |(_, kind)| kind)),
    )?;
    let result = result.split(';').next().unwrap_or("");
    let result = result.rsplit_once(':').map_or(result, |(_, kind)| kind);
    if !matches_inputs || !matches_params(actual.results(), std::iter::once(result))? {
        return Err(format!("invalid ABI export signature: {name}"));
    }
    Ok(())
}

fn check_memory(actual: MemoryType, manifest: &Value) -> Result<(), String> {
    if actual.memory64
        || actual.shared
        || actual.page_size_log2.is_some()
        || actual.initial != number(manifest, "initial")?
        || actual.maximum != Some(number(manifest, "maximum")?)
        || manifest.get("shared").and_then(Value::as_bool) != Some(false)
    {
        return Err("invalid core memory import type".into());
    }
    Ok(())
}

fn check_table(actual: TableType, manifest: &Value) -> Result<(), String> {
    if actual.table64
        || actual.shared
        || actual.element_type != RefType::FUNCREF
        || actual.initial != number(manifest, "initial")?
        || actual.maximum != Some(number(manifest, "maximum")?)
        || string(manifest, "element")? != "anyfunc"
    {
        return Err("invalid core table import type".into());
    }
    Ok(())
}

fn check_import(
    import: Import<'_>,
    manifest: &Value,
    types: &[FuncType],
    live: bool,
) -> Result<Option<u32>, EncodeError> {
    if let TypeRef::Func(index) | TypeRef::FuncExact(index) = import.ty {
        let required = match (live, import.module, import.name) {
            (true, "noble", "live_propose") => Some((&[ValType::I64, ValType::I64, ValType::I32][..], ValType::I32)),
            (true, "noble", "live_generation") => Some((&[][..], ValType::I64)),
            _ => None,
        };
        let actual = types.get(index as usize);
        if !required.is_some_and(|(params, result)| {
            actual.is_some_and(|ty| ty.params() == params && ty.results() == [result])
        }) {
            return Err(EncodeError::HostEffect(format!(
                "unsupported effect import: {}.{}",
                import.module, import.name
            )));
        }
        return Ok(Some(index));
    }
    if import.module != string(manifest, "module")? {
        return Err(format!("foreign Wasm import: {}.{}", import.module, import.name).into());
    }
    let memory = &manifest["memory"];
    let table = &manifest["table"];
    match import.ty {
        TypeRef::Memory(ty) if import.name == string(memory, "name")? => {
            check_memory(ty, memory)?;
            Ok(None)
        }
        TypeRef::Table(ty) if import.name == string(table, "name")? => {
            check_table(ty, table)?;
            Ok(None)
        }
        TypeRef::Global(ty) => {
            let globals = manifest["globals"]
                .as_array()
                .ok_or("missing core ABI globals")?;
            let expected = globals
                .iter()
                .find(|global| global["name"].as_str() == Some(import.name))
                .ok_or_else(|| format!("undeclared global import: {}", import.name))?;
            if !ty.mutable || ty.shared || ty.content_type != val_type(string(expected, "type")?)? {
                return Err(format!("invalid global import type: {}", import.name).into());
            }
            Ok(None)
        }
        _ => Err(format!("undeclared or mistyped Wasm import: {}", import.name).into()),
    }
}

fn check_export(
    name: &str,
    kind: ExternalKind,
    index: u32,
    manifest: &Value,
    functions: &[u32],
    types: &[FuncType],
) -> Result<(), String> {
    if name == string(&manifest["memory"], "name")? {
        if kind != ExternalKind::Memory || index != 0 {
            return Err("core memory export must alias the imported memory".into());
        }
        return Ok(());
    }
    let descriptor = manifest["exports"]
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("undeclared Wasm export: {name}"))?;
    if kind != ExternalKind::Func {
        return Err(format!("invalid ABI export kind: {name}"));
    }
    check_export_signature(func_type(functions, types, index)?, descriptor, name)
}

fn check_binary(bytes: &[u8], manifest: &Value, live: bool) -> Result<(), EncodeError> {
    let features = WasmFeatures::WASM1
        .difference(WasmFeatures::FLOATS)
        .difference(WasmFeatures::GC_TYPES)
        .union(WasmFeatures::BULK_MEMORY)
        .union(WasmFeatures::REFERENCE_TYPES)
        .union(WasmFeatures::MULTI_VALUE)
        .union(WasmFeatures::SIGN_EXTENSION);
    Validator::new_with_features(features)
        .validate_all(bytes)
        .map_err(|error| format!("unsupported or invalid core Wasm: {error}"))?;

    let mut types = Vec::new();
    let mut functions = Vec::new();
    let mut imports = HashSet::new();
    let mut exports = HashSet::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(|error| format!("invalid core Wasm section: {error}"))? {
            Payload::Version {
                num: 1,
                encoding: Encoding::Module,
                ..
            } => {}
            Payload::Version { .. } => return Err("expected a standard core Wasm v1 module".into()),
            Payload::TypeSection(section) => {
                for ty in section.into_iter_err_on_gc_types() {
                    types.push(ty.map_err(|error| format!("invalid core function type: {error}"))?);
                }
            }
            Payload::ImportSection(section) => {
                for import in section {
                    let import = import.map_err(|error| format!("invalid Wasm import: {error}"))?;
                    if !imports.insert(import.name) {
                        return Err(format!("duplicate Wasm import: {}", import.name).into());
                    }
                    if let Some(index) = check_import(import, manifest, &types, live)? {
                        functions.push(index);
                    }
                }
            }
            Payload::FunctionSection(section) => {
                for index in section {
                    functions
                        .push(index.map_err(|error| format!("invalid Wasm function: {error}"))?);
                }
            }
            Payload::MemorySection(section) if section.count() != 0 => {
                return Err("module must use only the imported core memory".into());
            }
            Payload::TableSection(section) if section.count() != 0 => {
                return Err("module must use only the imported core table".into());
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export.map_err(|error| format!("invalid Wasm export: {error}"))?;
                    if !exports.insert(export.name) {
                        return Err(format!("duplicate Wasm export: {}", export.name).into());
                    }
                    check_export(
                        export.name,
                        export.kind,
                        export.index,
                        manifest,
                        &functions,
                        &types,
                    )?;
                }
            }
            Payload::StartSection { .. } => return Err("Wasm start function is forbidden".into()),
            Payload::DataSection(section) => {
                for data in section {
                    if matches!(
                        data.map_err(|error| format!("invalid Wasm data segment: {error}"))?
                            .kind,
                        DataKind::Active { .. }
                    ) {
                        return Err("active Wasm data segment is forbidden".into());
                    }
                }
            }
            Payload::ElementSection(section) => {
                for element in section {
                    if matches!(
                        element
                            .map_err(|error| format!("invalid Wasm element segment: {error}"))?
                            .kind,
                        ElementKind::Active { .. }
                    ) {
                        return Err("active Wasm element segment is forbidden".into());
                    }
                }
            }
            _ => {}
        }
    }

    let globals = manifest["globals"]
        .as_array()
        .ok_or("missing core ABI globals")?;
    for global in globals {
        let name = string(global, "name")?;
        if !imports.contains(name) {
            return Err(format!("missing core global import: {name}").into());
        }
    }
    for required in [
        string(&manifest["memory"], "name")?,
        string(&manifest["table"], "name")?,
    ] {
        if !imports.contains(required) {
            return Err(format!("missing core import: {required}").into());
        }
    }
    if !exports.contains(string(&manifest["memory"], "name")?) {
        return Err("missing core memory export".into());
    }
    let functions = manifest["exports"]
        .as_object()
        .ok_or("missing core ABI exports")?;
    for name in functions.keys() {
        if !exports.contains(name.as_str()) {
            return Err(format!("missing core function export: {name}").into());
        }
    }
    Ok(())
}

/// Only UTF-8 WAT, never a binary passed through `wat::parse_bytes`, may be
/// encoded. Every byte and ABI check completes before any engine installation.
fn encode(wat: &[u8], live: bool) -> Result<Vec<u8>, EncodeError> {
    if wat.len() > MAX_BYTES {
        return Err("WAT exceeds 4 MiB".into());
    }
    let text = std::str::from_utf8(wat).map_err(|error| format!("WAT is not UTF-8: {error}"))?;
    let bytes = wat::parse_str(text).map_err(|error| format!("WAT syntax: {error}"))?;
    if bytes.len() > MAX_BYTES {
        return Err("encoded Wasm exceeds 4 MiB".into());
    }
    if !bytes.starts_with(b"\0asm\x01\0\0\0") {
        return Err("expected standard core Wasm v1 header".into());
    }
    check_binary(&bytes, abi()?, live)?;
    Ok(bytes)
}

/// Ordinary sessions have no live host imports, even when the WAT names them.
pub(super) fn encode_checked(wat: &[u8]) -> Result<Vec<u8>, EncodeError> {
    encode(wat, false)
}

/// Live source is independently checked by the frontend before this encoding;
/// only its two explicitly typed observation/proposal imports are admitted.
pub(super) fn encode_live_checked(wat: &[u8]) -> Result<Vec<u8>, EncodeError> {
    encode(wat, true)
}

#[cfg(test)]
mod tests {
    use super::encode_checked;

    #[test]
    fn rejects_non_text_and_unsupported_features() {
        assert!(encode_checked(b"\0asm\x01\0\0\0").is_err());
        assert!(encode_checked(b"\xff\xfe").is_err());
        assert!(
            encode_checked(b"(module (func (drop (v128.const i32x4 0 0 0 0))))")
                .unwrap_err()
                .as_str()
                .contains("unsupported or invalid core Wasm")
        );
    }

    #[test]
    fn rejects_all_host_effect_imports() {
        for wat in [
            &b"(module (import \"noble\" \"test_emit\" (func (param i32 i32) (result i32))))"[..],
            &b"(module (import \"foreign\" \"effect\" (func)))"[..],
        ] {
            let problem = encode_checked(wat).unwrap_err();
            assert!(
                matches!(&problem, super::EncodeError::HostEffect(_)),
                "{problem}"
            );
        }
    }

    #[test]
    fn rejects_instantiation_mutations_before_abi_completion() {
        let start = encode_checked(b"(module (func) (start 0))").unwrap_err();
        assert!(start.as_str().contains("start function"), "{start}");
        let data = encode_checked(
            b"(module (import \"noble\" \"memory\" (memory 16 16)) (data (i32.const 0) \"x\"))",
        )
        .unwrap_err();
        assert!(data.as_str().contains("active Wasm data"), "{data}");
        let element = encode_checked(
            b"(module (import \"noble\" \"table\" (table 16384 16384 funcref)) (func) (elem (i32.const 0) func 0))",
        )
        .unwrap_err();
        assert!(
            element.as_str().contains("active Wasm element"),
            "{element}"
        );
    }
}
