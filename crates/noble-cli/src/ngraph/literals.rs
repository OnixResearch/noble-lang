use noble_ngraph::{Code, Diagnostic, IdentityScheme, Site};

const SCHEMA: &str = "noble-graph-import-receipt/v1";
const MODE: &str = "noble-graph-literal-modes/v2";
const SEMANTIC: &str = noble_ngraph::ir::SEMANTIC_REVISION;

pub(super) struct Receipt {
    pub(super) source: Vec<u8>,
    pub(super) canonical: Vec<u8>,
    pub(super) source_digest: [u8; 32],
    pub(super) canonical_digest: [u8; 32],
    pub(super) literal_count: usize,
}

fn invalid(detail: impl Into<String>) -> Diagnostic {
    Diagnostic::new(Code::Format, Site::Graph, detail)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

fn mode_declared(source: &str) -> bool {
    source.lines().any(|line| {
        let record = line.split('#').next().unwrap_or("");
        let mut tokens = record.split_whitespace();
        tokens.next() == Some("literal-modes")
            && tokens.next() == Some(MODE)
            && tokens.next().is_none()
    })
}

fn append_literal(token: &str, line: usize, records: &mut Vec<String>) -> Result<(), Diagnostic> {
    if !token.starts_with("rne:") {
        return Ok(());
    }
    let value = noble_ngraph::format::value::parse_with_mode(
        noble_ngraph::ScalarType::F32,
        token,
        line,
        noble_ngraph::format::LiteralMode::RneV2,
    )?;
    let noble_ngraph::Value::F32(bits) = value else {
        return Err(invalid("RNE import receipt encountered a non-f32 literal"));
    };
    records.push(format!(
        "literal {line} {} 0x{bits:08x}",
        hex(token.as_bytes())
    ));
    Ok(())
}

fn imported_literals(source: &str) -> Result<Vec<String>, Diagnostic> {
    let mut records = Vec::new();
    for (line_index, source_line) in source.lines().enumerate() {
        let body = source_line.split('#').next().unwrap_or("");
        let mut tokens = body.split_whitespace();
        let (first, second, third) = (tokens.next(), tokens.next(), tokens.next());
        let line = line_index + 1;
        if first == Some("fact") && third == Some("range") {
            for token in tokens.take(2) {
                append_literal(token, line, &mut records)?;
            }
        } else if second == Some("=") && third == Some("guard") {
            let _node = tokens.next();
            if tokens.next() == Some("range") {
                for token in tokens.take(2) {
                    append_literal(token, line, &mut records)?;
                }
            }
        } else if second == Some("=") && third == Some("const") {
            if tokens.next() == Some("f32") {
                if let Some(token) = tokens.next() {
                    append_literal(token, line, &mut records)?;
                }
            }
        } else if second == Some("=") && third == Some("f32.affine") {
            if let Some(bias) = tokens.next() {
                append_literal(bias, line, &mut records)?;
            }
            for coefficient in tokens.step_by(2) {
                append_literal(coefficient, line, &mut records)?;
            }
        }
    }
    Ok(records)
}

pub(super) fn encode(source: &str, canonical: &str) -> Result<(String, usize), Diagnostic> {
    if !mode_declared(source) {
        return Err(Diagnostic::new(
            Code::Profile,
            Site::Graph,
            "import requires literal-modes noble-graph-literal-modes/v2",
        ));
    }
    let literals = imported_literals(source)?;
    let source_bytes = source.as_bytes();
    let canonical_bytes = canonical.as_bytes();
    let mut out = format!(
        "{SCHEMA}\nmode {MODE}\nsemantic {SEMANTIC}\nsource-length {}\nsource-blake3 {}\ncanonical-length {}\ncanonical-blake3 {}\nsource-hex {}\ncanonical-hex {}\n",
        source_bytes.len(),
        noble_ngraph::ir::hex(&digest(source_bytes)),
        canonical_bytes.len(),
        noble_ngraph::ir::hex(&digest(canonical_bytes)),
        hex(source_bytes),
        hex(canonical_bytes),
    );
    for literal in &literals {
        out.push_str(literal);
        out.push('\n');
    }
    Ok((out, literals.len()))
}

fn record<'a>(line: Option<&'a str>, name: &str) -> Result<&'a str, Diagnostic> {
    let line = line.ok_or_else(|| invalid(format!("missing {name} record")))?;
    line.strip_prefix(name)
        .and_then(|value| value.strip_prefix(' '))
        .ok_or_else(|| invalid(format!("malformed {name} record")))
}

fn length(value: &str, name: &str) -> Result<usize, Diagnostic> {
    let parsed = noble_ngraph::format::decimal_u64(value)
        .ok_or_else(|| invalid(format!("malformed {name} length")))?;
    usize::try_from(parsed).map_err(|_| invalid(format!("{name} length is out of range")))
}

fn digest_value(value: &str, name: &str) -> Result<[u8; 32], Diagnostic> {
    let digest = noble_ngraph::format::digest_from_hex(value)
        .filter(|_| value.len() == 64)
        .ok_or_else(|| invalid(format!("malformed {name} digest")))?;
    if noble_ngraph::ir::hex(&digest) != value {
        return Err(invalid(format!("noncanonical {name} digest")));
    }
    Ok(digest)
}

fn hex_value(value: &str, name: &str) -> Result<Vec<u8>, Diagnostic> {
    noble_ngraph::format::bytes_from_hex(value)
        .ok_or_else(|| invalid(format!("malformed {name} bytes")))
}

pub(super) fn decode(text: &str, ids: &dyn IdentityScheme) -> Result<Receipt, Diagnostic> {
    let mut lines = text.lines();
    if lines.next() != Some(SCHEMA) {
        return Err(invalid("missing noble-graph-import-receipt/v1 schema"));
    }
    if record(lines.next(), "mode")? != MODE {
        return Err(invalid("unsupported import receipt literal mode"));
    }
    if record(lines.next(), "semantic")? != SEMANTIC {
        return Err(invalid("unsupported import receipt semantic revision"));
    }
    let source_length = length(record(lines.next(), "source-length")?, "source")?;
    let source_digest = digest_value(record(lines.next(), "source-blake3")?, "source")?;
    let canonical_length = length(record(lines.next(), "canonical-length")?, "canonical")?;
    let canonical_digest = digest_value(record(lines.next(), "canonical-blake3")?, "canonical")?;
    let source = hex_value(record(lines.next(), "source-hex")?, "source")?;
    let canonical = hex_value(record(lines.next(), "canonical-hex")?, "canonical")?;
    let literal_records = lines.collect::<Vec<_>>();
    if source.len() != source_length || digest(&source) != source_digest {
        return Err(invalid(
            "source bytes do not match their recorded length and digest",
        ));
    }
    if canonical.len() != canonical_length || digest(&canonical) != canonical_digest {
        return Err(invalid(
            "canonical bytes do not match their recorded length and digest",
        ));
    }
    let source_text = core::str::from_utf8(&source).map_err(|_| invalid("source is not UTF-8"))?;
    if !mode_declared(source_text) {
        return Err(invalid("source omits the v2 literal-mode declaration"));
    }
    let graph = noble_ngraph::format::graph::parse(ids, source_text)
        .map_err(|error| invalid(format!("source graph refused: {}", error.code_string())))?;
    let reproduced = noble_ngraph::format::graph::print(&graph);
    if reproduced.as_bytes() != canonical {
        return Err(invalid("canonical graph does not replay from the source"));
    }
    let expected_literals = imported_literals(source_text)?;
    if expected_literals
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        != literal_records
    {
        return Err(invalid(
            "literal provenance does not match the source graph",
        ));
    }
    Ok(Receipt {
        source,
        canonical,
        source_digest,
        canonical_digest,
        literal_count: expected_literals.len(),
    })
}
