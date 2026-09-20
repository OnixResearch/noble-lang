//! The JSON value tree for the executed documentation examples (DX-DOC-01):
//! objects with unique keys kept in document order, arrays, strings,
//! integers, floats, booleans, and null, with strict accessors.

/// One parsed JSON value.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// The value of one object field.
    pub fn field(&self, name: &str) -> Result<&Json, String> {
        match self {
            Self::Obj(entries) => {
                let mut found: Option<&Json> = None;
                for (key, value) in entries {
                    if key == name && found.replace(value).is_some() {
                        return Err(format!("duplicate field `{name}`"));
                    }
                }
                found.ok_or_else(|| format!("missing field `{name}`"))
            }
            Self::Null
            | Self::Bool(_)
            | Self::Int(_)
            | Self::Float(_)
            | Self::Str(_)
            | Self::Arr(_) => Err(format!("expected an object for field `{name}`")),
        }
    }

    /// One field's value, or `None` when the field is absent.
    pub fn optional(&self, name: &str) -> Result<Option<&Json>, String> {
        match self.field(name) {
            Ok(value) => Ok(Some(value)),
            Err(problem) if problem.starts_with("missing") => Ok(None),
            Err(problem) => Err(problem),
        }
    }

    /// The string content of a string value.
    pub fn as_str(&self) -> Result<&str, String> {
        match self {
            Self::Str(text) => Ok(text),
            Self::Null
            | Self::Bool(_)
            | Self::Int(_)
            | Self::Float(_)
            | Self::Arr(_)
            | Self::Obj(_) => Err("expected a string".to_string()),
        }
    }

    /// The entries of an array value.
    pub fn as_arr(&self) -> Result<&[Json], String> {
        match self {
            Self::Arr(items) => Ok(items),
            Self::Null
            | Self::Bool(_)
            | Self::Int(_)
            | Self::Float(_)
            | Self::Str(_)
            | Self::Obj(_) => Err("expected an array".to_string()),
        }
    }

    /// The integer of an integer value.
    pub fn as_int(&self) -> Result<i64, String> {
        match self {
            Self::Int(value) => Ok(*value),
            Self::Null
            | Self::Bool(_)
            | Self::Float(_)
            | Self::Str(_)
            | Self::Arr(_)
            | Self::Obj(_) => Err("expected an integer".to_string()),
        }
    }
}
