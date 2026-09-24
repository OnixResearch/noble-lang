use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use wasmtime::component::Val;

pub fn decode(value: &Value) -> Result<Val> {
    let kind = value["type"].as_str().context("missing typed input")?;
    let payload = &value["value"];
    Ok(match kind {
        "s64" => Val::S64(
            payload
                .as_str()
                .context("s64 must be decimal text")?
                .parse()?,
        ),
        "u64" => Val::U64(
            payload
                .as_str()
                .context("u64 must be decimal text")?
                .parse()?,
        ),
        "bool" => Val::Bool(payload.as_bool().context("invalid bool")?),
        "string" => Val::String(payload.as_str().context("invalid string")?.to_owned()),
        "list<u8>" => Val::List(
            payload
                .as_array()
                .context("invalid byte list")?
                .iter()
                .map(|value| {
                    Ok(Val::U8(u8::try_from(
                        value.as_u64().context("invalid byte")?,
                    )?))
                })
                .collect::<Result<Vec<_>>>()?,
        ),
        _ => bail!(
            "unsupported peer input type: {kind}; resource authority is not decoded from bytes"
        ),
    })
}

pub fn encode(value: &Val) -> Result<Value> {
    Ok(match value {
        Val::S64(number) => json!({"type":"s64", "value":number.to_string()}),
        Val::U64(number) => json!({"type":"u64", "value":number.to_string()}),
        Val::U8(number) => json!({"type":"u8", "value":number}),
        Val::U32(number) => json!({"type":"u32", "value":number}),
        Val::Bool(flag) => json!({"type":"bool", "value":flag}),
        Val::String(text) => json!({"type":"string", "value":text}),
        Val::List(values) => {
            json!({"type":"list", "value":values.iter().map(encode).collect::<Result<Vec<_>>>()?})
        }
        Val::Result(Ok(value)) => {
            json!({"type":"result", "ok":value.as_deref().map(encode).transpose()?})
        }
        Val::Result(Err(value)) => {
            json!({"type":"result", "err":value.as_deref().map(encode).transpose()?})
        }
        Val::Tuple(values) => {
            json!({"type":"tuple", "value":values.iter().map(encode).collect::<Result<Vec<_>>>()?})
        }
        Val::Record(fields) => {
            json!({"type":"record", "value":fields.iter().map(|(name,value)| Ok((name.clone(),encode(value)?))).collect::<Result<serde_json::Map<_,_>>>()?})
        }
        Val::Resource(_) => bail!("a live resource is not serializable peer output"),
        _ => bail!("unsupported peer output: {value:?}"),
    })
}
