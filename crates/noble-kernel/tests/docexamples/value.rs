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
            Json::Obj(entries) => {
                let mut found: Option<&Json> = None;
                let mut seen = 0;
                for (key, value) in entries {
                    if key == name {
                        seen += 1;
                        found = Some(value);
                    }
                }
                match (seen, found) {
                    (1, Some(value)) => Ok(value),
                    (0, _) => Err(format!("missing field `{name}`")),
                    _ => Err(format!("duplicate field `{name}`")),
                }
            }
            _ => Err(format!("expected an object for field `{name}`")),
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
            Json::Str(text) => Ok(text),
            _ => Err("expected a string".to_string()),
        }
    }

    /// The entries of an array value.
    pub fn as_arr(&self) -> Result<&[Json], String> {
        match self {
            Json::Arr(items) => Ok(items),
            _ => Err("expected an array".to_string()),
        }
    }

    /// The integer of an integer value.
    pub fn as_int(&self) -> Result<i64, String> {
        match self {
            Json::Int(value) => Ok(*value),
            _ => Err("expected an integer".to_string()),
        }
    }
}
