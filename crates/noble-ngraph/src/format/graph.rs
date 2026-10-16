//! Graph files: the identity-complete text form of `ngraph-ir/0` (design D4
//! of `numeric-graph-v1`; option B0(b) authoring). Parsing only builds
//! producer data; [`crate::check`] decides admission.
//!
//! ```text
//! ngraph-ir/0 ngraph-f32/0
//! subject authored 3be1…(64 hex) closure -   # or: subject authored-name NAME
//! shape input s (p s u)                      # optional; trees s | u | (p A B)
//! shape output s
//! slot input f32
//! slot param f32
//! %0 = input 0
//! %1 = param 1
//! %2 = const f32 1.5            # decimal must be exact; or 0x3fc00000 bits
//! %3 = f32.affine -0 1.5 %0 2 one   # bias, then (coefficient operand)*
//! %4 = f32.index %0 refuse clamp %2 %3
//! %5 = guard %0 range 0 1
//! fact %0 integral input-validated
//! fact %3 finite derived interval
//! out %3
//! ```
//!
//! - `subject <authored|extracted|captured> <64 hex> closure <ids|->` is
//!   required exactly once; `subject authored-name NAME` (one or more tokens,
//!   joined by single spaces) is authoring sugar for
//!   [`Subject::authored`], and [`print`] always writes the hex form.
//! - `shape input|output <trees…>` records `Some(trees)`; an absent line is
//!   `None`.
//! - Range-fact literals: `0x…` is binary32, `true|false` Bool, `i64:N` an
//!   explicit `I64`; an unprefixed decimal takes the sub-profile's type.
//! - `derived <rule>` requires its rule; rules and evidence witnesses are
//!   printed so that they parse back exactly (percent-encoded rule bytes,
//!   hex witnesses, `""`/`-` for empty ones).
//!
//! `GraphLogicalId` is recomputed from the parsed records alone; comments,
//! whitespace, file names and paths are never identity-bearing, and producer
//! origins are not printed. [`print`] is canonical: `parse(print(g))` equals
//! `g` with origins cleared.
//!
//! Codes: unknown operations, syntax errors (including a number with a
//! leading `+`) and a missing, duplicate or malformed subject or shape line
//! (including a shape tree nesting more than [`MAX_SHAPE_DEPTH`] pairs) give
//! `E-GRAPH-MALFORMED`; an unknown sub-profile `E-PROFILE`; inexact decimal
//! literals `E-LITERAL-INEXACT`; `nan`/`inf` literals `E-LITERAL-NONFINITE`
//! (non-finite bit patterns are refused by the checker).

use alloc::{borrow::ToOwned, format, string::String, string::ToString, vec::Vec};
use core::fmt::Write as _;

use super::value::F32Refusal;
use crate::diag::{Code, Diagnostic, Malformed, Site};
use crate::identity::IdentityScheme;
use crate::ir::{
    hex, shape_depth_refusal, AffineOperand, AffineTerm, Fact, Graph, GraphBuilder, Literal,
    NodeId, Op, OutOfRange, Provenance, ScalarType, ShapeNode, ShapeTree, SlotKind, SubProfile,
    Subject, SubjectKind, IR_VERSION, MAX_SHAPE_DEPTH,
};

fn syntax(line: usize, why: impl Into<String>) -> Diagnostic {
    Diagnostic::malformed(Malformed::Syntax, Site::Line(line), why)
}

struct Toks<'a> {
    t: Vec<&'a str>,
    i: usize,
    line: usize,
}

impl<'a> Toks<'a> {
    fn next(&mut self) -> Result<&'a str, Diagnostic> {
        let t = self
            .t
            .get(self.i)
            .copied()
            .ok_or_else(|| syntax(self.line, "unexpected end of line"))?;
        self.i += 1;
        Ok(t)
    }
    fn peek(&self) -> Option<&'a str> {
        self.t.get(self.i).copied()
    }
    fn rest(&mut self) -> Vec<&'a str> {
        let r = self.t[self.i..].to_vec();
        self.i = self.t.len();
        r
    }
    fn done(&self) -> Result<(), Diagnostic> {
        if self.i == self.t.len() {
            Ok(())
        } else {
            Err(syntax(
                self.line,
                format!("trailing token {:?}", self.t[self.i]),
            ))
        }
    }
    fn node(&mut self) -> Result<NodeId, Diagnostic> {
        let t = self.next()?;
        t.strip_prefix('%')
            .and_then(super::decimal_u32)
            .map(NodeId)
            .ok_or_else(|| syntax(self.line, format!("expected %node, got {t:?}")))
    }
    fn u32(&mut self) -> Result<u32, Diagnostic> {
        let t = self.next()?;
        super::decimal_u32(t)
            .ok_or_else(|| syntax(self.line, format!("expected integer, got {t:?}")))
    }
    fn f32_bits(&mut self) -> Result<u32, Diagnostic> {
        let t = self.next()?;
        parse_f32(t, self.line)
    }
    fn i64(&mut self) -> Result<i64, Diagnostic> {
        let t = self.next()?;
        super::decimal_i64(t).ok_or_else(|| syntax(self.line, format!("expected i64, got {t:?}")))
    }
}

/// Parses an `F32` literal token: `0x` plus one to eight hex digits, or an
/// exactly representable decimal (the vector-file rule, design D4/D5).
///
/// # Errors
/// `E-LITERAL-NONFINITE`, `E-LITERAL-INEXACT` or a syntax error.
pub fn parse_f32(t: &str, line: usize) -> Result<u32, Diagnostic> {
    super::value::f32_token(t).map_err(|r| match r {
        F32Refusal::Malformed => syntax(line, format!("bad f32 literal {t:?}")),
        F32Refusal::Nonfinite => Diagnostic::new(
            Code::LiteralNonfinite,
            Site::Line(line),
            format!("non-finite f32 literal {t}"),
        ),
        F32Refusal::Inexact => Diagnostic::new(
            Code::LiteralInexact,
            Site::Line(line),
            format!("{t} is not exactly representable in binary32"),
        ),
    })
}

fn rule(t: &str, line: usize) -> Result<OutOfRange, Diagnostic> {
    match t {
        "refuse" => Ok(OutOfRange::Refuse),
        "clamp" => Ok(OutOfRange::Clamp),
        "zero" => Ok(OutOfRange::Zero),
        _ => Err(syntax(line, format!("bad out-of-range rule {t:?}"))),
    }
}

/// Parses a scalar type name (`i64`, `bool`, `f32`).
#[must_use]
pub fn scalar_type(t: &str) -> Option<ScalarType> {
    match t {
        "i64" => Some(ScalarType::I64),
        "bool" => Some(ScalarType::Bool),
        "f32" => Some(ScalarType::F32),
        _ => None,
    }
}

fn ty(t: &str, line: usize) -> Result<ScalarType, Diagnostic> {
    scalar_type(t).ok_or_else(|| syntax(line, format!("bad type {t:?}")))
}

/// A range-fact literal: `0x…` binary32, `true|false`, `i64:N`, or an
/// unprefixed decimal of the sub-profile's type.
fn range_literal(t: &mut Toks<'_>, profile: SubProfile) -> Result<Literal, Diagnostic> {
    let tok = t.next()?;
    let line = t.line;
    if tok.starts_with("0x") {
        return Ok(Literal::F32(parse_f32(tok, line)?));
    }
    match tok {
        "true" => return Ok(Literal::Bool(true)),
        "false" => return Ok(Literal::Bool(false)),
        _ => {}
    }
    if let Some(v) = tok.strip_prefix("i64:") {
        return super::decimal_i64(v)
            .map(Literal::I64)
            .ok_or_else(|| syntax(line, format!("expected i64, got {tok:?}")));
    }
    match profile {
        SubProfile::F32 => Ok(Literal::F32(parse_f32(tok, line)?)),
        SubProfile::I64 => super::decimal_i64(tok)
            .map(Literal::I64)
            .ok_or_else(|| syntax(line, format!("expected i64, got {tok:?}"))),
    }
}

fn fact(t: &mut Toks<'_>, profile: SubProfile) -> Result<Fact, Diagnostic> {
    let kind = t.next()?;
    Ok(match kind {
        "range" => Fact::Range {
            lo: range_literal(t, profile)?,
            hi: range_literal(t, profile)?,
        },
        "grid" => Fact::Grid {
            log2: i32::try_from(t.i64()?).map_err(|_| syntax(t.line, "grid exponent"))?,
        },
        "bit" => Fact::Bit,
        "finite" => Fact::Finite,
        "integral" => Fact::Integral,
        "shape" => Fact::Shape(t.u32()?),
        "one-hot" => {
            let allow_zero = match t.next()? {
                "allow-zero" => true,
                "exact" => false,
                o => return Err(syntax(t.line, format!("bad one-hot mode {o:?}"))),
            };
            let mut group = Vec::new();
            while t.peek().is_some_and(|p| p.starts_with('%')) {
                group.push(t.node()?);
            }
            Fact::OneHot { group, allow_zero }
        }
        o => return Err(syntax(t.line, format!("unknown fact kind {o:?}"))),
    })
}

fn subject_kind(t: &str) -> Option<SubjectKind> {
    match t {
        "authored" => Some(SubjectKind::Authored),
        "extracted" => Some(SubjectKind::Extracted),
        "captured" => Some(SubjectKind::Captured),
        _ => None,
    }
}

/// Parses `<64 hex> closure <ids|->` after the subject kind.
fn subject_record(t: &mut Toks<'_>, kind: SubjectKind) -> Result<Subject, Diagnostic> {
    let line = t.line;
    let identity = super::digest_from_hex(t.next()?)
        .ok_or_else(|| syntax(line, "subject identity must be 64 hex digits"))?;
    if t.next()? != "closure" {
        return Err(syntax(line, "expected `closure` in the subject line"));
    }
    let ids = t.next()?;
    let closure = if ids == "-" {
        Vec::new()
    } else {
        ids.split(',')
            .map(|d| {
                super::decimal_u64(d).ok_or_else(|| syntax(line, format!("bad closure id {d:?}")))
            })
            .collect::<Result<Vec<u64>, Diagnostic>>()?
    };
    Ok(Subject {
        kind,
        identity,
        closure,
    })
}

/// Parses the shape trees of a `shape input|output` line. A tree that would
/// nest more than [`MAX_SHAPE_DEPTH`] pairs is refused before the parser
/// descends any deeper (design D4).
fn shape_trees(tokens: &[&str], line: usize) -> Result<Vec<ShapeTree>, Diagnostic> {
    let joined = tokens.join(" ").replace('(', " ( ").replace(')', " ) ");
    let toks: Vec<&str> = joined.split_whitespace().collect();
    /// One tree inside `open` enclosing pairs.
    fn tree(
        toks: &[&str],
        at: &mut usize,
        line: usize,
        open: u32,
    ) -> Result<ShapeTree, Diagnostic> {
        let t = toks
            .get(*at)
            .copied()
            .ok_or_else(|| syntax(line, "unterminated shape tree"))?;
        *at += 1;
        match t {
            "s" => Ok(ShapeTree::SCALAR),
            "u" => Ok(ShapeTree::UNIT),
            "(" => {
                if open >= MAX_SHAPE_DEPTH {
                    return Err(shape_depth_refusal(Site::Line(line)));
                }
                if toks.get(*at).copied() != Some("p") {
                    return Err(syntax(line, "expected `p` after `(` in a shape tree"));
                }
                *at += 1;
                let a = tree(toks, at, line, open + 1)?;
                let b = tree(toks, at, line, open + 1)?;
                if toks.get(*at).copied() != Some(")") {
                    return Err(syntax(line, "expected `)` in a shape tree"));
                }
                *at += 1;
                ShapeTree::pair(a, b).map_err(|_| shape_depth_refusal(Site::Line(line)))
            }
            o => Err(syntax(line, format!("bad shape tree token {o:?}"))),
        }
    }
    let mut at = 0;
    let mut out = Vec::new();
    while at < toks.len() {
        out.push(tree(&toks, &mut at, line, 0)?);
    }
    Ok(out)
}

/// Percent-decodes a `derived` rule token (`""` is the empty rule).
fn decode_rule(tok: &str, line: usize) -> Result<String, Diagnostic> {
    if tok == "\"\"" {
        return Ok(String::new());
    }
    let b = tok.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let v = b
                .get(i + 1..i + 3)
                .and_then(|h| core::str::from_utf8(h).ok())
                .filter(|h| h.bytes().all(|c| c.is_ascii_hexdigit()))
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .ok_or_else(|| syntax(line, format!("bad escape in rule {tok:?}")))?;
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| syntax(line, format!("rule {tok:?} is not UTF-8")))
}

/// Percent-encodes a `derived` rule so that it is one printable token.
fn encode_rule(rule: &str) -> String {
    if rule.is_empty() {
        return "\"\"".to_owned();
    }
    let mut s = String::with_capacity(rule.len());
    for &b in rule.as_bytes() {
        if b.is_ascii_graphic() && !matches!(b, b'%' | b'#' | b'"') {
            s.push(char::from(b));
        } else {
            let _ = write!(s, "%{b:02X}");
        }
    }
    s
}

fn provenance(t: &mut Toks<'_>) -> Result<Provenance, Diagnostic> {
    let line = t.line;
    Ok(match t.next()? {
        "input-validated" => Provenance::InputValidated,
        "asserted" => Provenance::Asserted,
        "derived" => Provenance::Derived(decode_rule(t.next()?, line)?),
        "guard" => Provenance::Guard(t.node()?),
        "evidence" => {
            let w = t.next()?;
            Provenance::Evidence(if w == "-" {
                Vec::new()
            } else {
                super::bytes_from_hex(w)
                    .ok_or_else(|| syntax(line, format!("evidence witness {w:?} is not hex")))?
            })
        }
        o => return Err(syntax(line, format!("bad provenance {o:?}"))),
    })
}

fn op(t: &mut Toks<'_>, profile: SubProfile) -> Result<Op, Diagnostic> {
    let line = t.line;
    let name = t.next()?;
    Ok(match name {
        "input" => Op::Input(t.u32()?),
        "param" => Op::Param(t.u32()?),
        "const" => {
            let ty = ty(t.next()?, line)?;
            Op::Const(match ty {
                ScalarType::F32 => Literal::F32(t.f32_bits()?),
                ScalarType::I64 => Literal::I64(t.i64()?),
                ScalarType::Bool => Literal::Bool(match t.next()? {
                    "true" => true,
                    "false" => false,
                    o => return Err(syntax(line, format!("bad bool {o:?}"))),
                }),
            })
        }
        "i64.add" => Op::I64Add(t.node()?, t.node()?),
        "i64.sub" => Op::I64Sub(t.node()?, t.node()?),
        "i64.mul" => Op::I64Mul(t.node()?, t.node()?),
        "i64.eq" => Op::I64Eq(t.node()?, t.node()?),
        "f32.add" => Op::F32Add(t.node()?, t.node()?),
        "f32.sub" => Op::F32Sub(t.node()?, t.node()?),
        "f32.mul" => Op::F32Mul(t.node()?, t.node()?),
        "f32.affine" => {
            let bias = t.f32_bits()?;
            let mut terms = Vec::new();
            while t.peek().is_some() {
                let coefficient = t.f32_bits()?;
                let operand = if t.peek() == Some("one") {
                    t.next()?;
                    AffineOperand::One
                } else {
                    AffineOperand::Node(t.node()?)
                };
                terms.push(AffineTerm {
                    coefficient,
                    operand,
                });
            }
            Op::F32Affine { bias, terms }
        }
        "f32.relu" => Op::F32Relu(t.node()?),
        "f32.reglu" => Op::F32Reglu {
            left: t.node()?,
            gate: t.node()?,
        },
        "f32.step" => Op::F32Step(t.node()?),
        "f32.select" => Op::F32Select {
            bit: t.node()?,
            off: t.node()?,
            on: t.node()?,
        },
        "f32.index" => {
            let index = t.node()?;
            let below = rule(t.next()?, line)?;
            let above = rule(t.next()?, line)?;
            let mut table = Vec::new();
            while t.peek().is_some() {
                table.push(t.node()?);
            }
            Op::F32Index {
                index,
                table,
                below,
                above,
            }
        }
        "guard" => {
            let value = t.node()?;
            let fact = fact(t, profile)?;
            Op::Guard { fact, value }
        }
        other => {
            return Err(Diagnostic::malformed(
                Malformed::UnknownOp,
                Site::Line(line),
                format!("operation {other:?} is not in the ngraph-ir/0 whitelist"),
            ))
        }
    })
}

/// Parses a graph file. `ids` is used only for the `subject authored-name`
/// sugar.
///
/// # Errors
/// The first D6 refusal encountered while reading.
pub fn parse(ids: &dyn IdentityScheme, text: &str) -> Result<Graph, Diagnostic> {
    let mut lines = super::lines(text);
    let header = lines.next().ok_or_else(|| syntax(1, "empty graph file"))?;
    let hl = header.number;
    if header.tokens[0] != IR_VERSION {
        return Err(syntax(hl, "missing ngraph-ir/0 schema line"));
    }
    let profile = header
        .tokens
        .get(1)
        .copied()
        .and_then(SubProfile::from_name)
        .ok_or_else(|| Diagnostic::new(Code::Profile, Site::Line(hl), "unknown sub-profile"))?;
    if header.tokens.len() != 2 {
        return Err(syntax(hl, "trailing tokens on the schema line"));
    }
    let placeholder = Subject {
        kind: SubjectKind::Authored,
        identity: [0; 32],
        closure: Vec::new(),
    };
    let mut b = GraphBuilder::new(profile, placeholder);
    let mut subject: Option<Subject> = None;
    let mut input_shape: Option<Vec<ShapeTree>> = None;
    let mut output_shape: Option<Vec<ShapeTree>> = None;
    for l in lines {
        let line = l.number;
        let mut t = Toks {
            t: l.tokens,
            i: 0,
            line,
        };
        let head = t.next()?;
        match head {
            "subject" => {
                if subject.is_some() {
                    return Err(syntax(line, "duplicate subject line"));
                }
                let kind = t.next()?;
                subject = Some(if kind == "authored-name" {
                    let name = t.rest().join(" ");
                    if name.is_empty() {
                        return Err(syntax(line, "authored-name needs a name"));
                    }
                    Subject::authored(ids, &name)
                } else {
                    let k = subject_kind(kind)
                        .ok_or_else(|| syntax(line, format!("bad subject kind {kind:?}")))?;
                    subject_record(&mut t, k)?
                });
            }
            "shape" => {
                let which = t.next()?;
                let trees = shape_trees(&t.rest(), line)?;
                let slot = match which {
                    "input" => &mut input_shape,
                    "output" => &mut output_shape,
                    o => return Err(syntax(line, format!("bad shape side {o:?}"))),
                };
                if slot.is_some() {
                    return Err(syntax(line, format!("duplicate shape {which} line")));
                }
                *slot = Some(trees);
            }
            "slot" => {
                let kind = match t.next()? {
                    "input" => SlotKind::Input,
                    "param" => SlotKind::Param,
                    o => return Err(syntax(line, format!("bad slot kind {o:?}"))),
                };
                let ty = ty(t.next()?, line)?;
                b.declare_slot(kind, ty);
            }
            "out" => {
                let n = t.node()?;
                b.output(n);
            }
            "fact" => {
                let n = t.node()?;
                let f = fact(&mut t, profile)?;
                let p = provenance(&mut t)?;
                b.fact(n, f, p);
            }
            h if h.starts_with('%') => {
                let id = super::decimal_u64(&h[1..])
                    .and_then(|v| usize::try_from(v).ok())
                    .ok_or_else(|| syntax(line, "bad node id"))?;
                if id != b.len() {
                    return Err(syntax(
                        line,
                        format!("node %{id} out of sequence (next is %{})", b.len()),
                    ));
                }
                if t.next()? != "=" {
                    return Err(syntax(line, "expected ="));
                }
                let o = op(&mut t, profile)?;
                b.op(o);
            }
            other => return Err(syntax(line, format!("unknown line kind {other:?}"))),
        }
        t.done()?;
    }
    let mut g = b.finish();
    g.subject = subject.ok_or_else(|| syntax(hl, "missing subject line"))?;
    g.input_shape = input_shape;
    g.output_shape = output_shape;
    Ok(g)
}

fn lit(l: Literal, profile: SubProfile) -> String {
    match (l, profile) {
        (Literal::I64(v), SubProfile::I64) => v.to_string(),
        (Literal::I64(v), SubProfile::F32) => format!("i64:{v}"),
        (Literal::Bool(v), _) => v.to_string(),
        (Literal::F32(b), _) => format!("0x{b:08x}"),
    }
}

fn const_lit(l: Literal) -> String {
    match l {
        Literal::I64(v) => v.to_string(),
        Literal::Bool(v) => v.to_string(),
        Literal::F32(b) => format!("0x{b:08x}"),
    }
}

fn fact_text(f: &Fact, profile: SubProfile) -> String {
    match f {
        Fact::Range { lo, hi } => format!("range {} {}", lit(*lo, profile), lit(*hi, profile)),
        Fact::Grid { log2 } => format!("grid {log2}"),
        Fact::Bit => "bit".into(),
        Fact::Finite => "finite".into(),
        Fact::Integral => "integral".into(),
        Fact::Shape(n) => format!("shape {n}"),
        Fact::OneHot { group, allow_zero } => {
            let mut s = format!(
                "one-hot {}",
                if *allow_zero { "allow-zero" } else { "exact" }
            );
            for g in group {
                let _ = write!(s, " %{}", g.0);
            }
            s
        }
    }
}

fn shape_text(trees: &[ShapeTree]) -> String {
    // Recursion depth is bounded by `MAX_SHAPE_DEPTH`.
    fn tree(t: &ShapeTree, s: &mut String) {
        match t.node() {
            ShapeNode::Scalar => s.push('s'),
            ShapeNode::Unit => s.push('u'),
            ShapeNode::Pair(a, b) => {
                s.push_str("(p ");
                tree(a, s);
                s.push(' ');
                tree(b, s);
                s.push(')');
            }
        }
    }
    let mut s = String::new();
    for t in trees {
        s.push(' ');
        tree(t, &mut s);
    }
    s
}

/// The subject line body after `subject `: `<kind> <64 hex> closure <ids|->`.
#[must_use]
pub fn subject_text(s: &Subject) -> String {
    let closure = if s.closure.is_empty() {
        "-".to_owned()
    } else {
        s.closure
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    format!("{} {} closure {closure}", s.kind.name(), hex(&s.identity))
}

/// Prints a graph in the canonical file form (`F32` literals as exact bit
/// patterns; origins omitted).
#[must_use]
pub fn print(g: &Graph) -> String {
    let mut s = format!("{IR_VERSION} {}\n", g.profile.name());
    let _ = writeln!(s, "subject {}", subject_text(&g.subject));
    if let Some(t) = &g.input_shape {
        let _ = writeln!(s, "shape input{}", shape_text(t));
    }
    if let Some(t) = &g.output_shape {
        let _ = writeln!(s, "shape output{}", shape_text(t));
    }
    for slot in &g.slots {
        let k = match slot.kind {
            SlotKind::Input => "input",
            SlotKind::Param => "param",
        };
        let _ = writeln!(s, "slot {k} {}", slot.ty.name());
    }
    let n = |x: &NodeId| format!("%{}", x.0);
    for (i, node) in g.nodes.iter().enumerate() {
        let body = match &node.op {
            Op::Input(k) | Op::Param(k) => format!("{} {k}", node.op.name()),
            Op::Const(l) => format!("const {} {}", l.ty().name(), const_lit(*l)),
            Op::I64Add(a, b)
            | Op::I64Sub(a, b)
            | Op::I64Mul(a, b)
            | Op::I64Eq(a, b)
            | Op::F32Add(a, b)
            | Op::F32Sub(a, b)
            | Op::F32Mul(a, b) => format!("{} {} {}", node.op.name(), n(a), n(b)),
            Op::F32Affine { bias, terms } => {
                let mut s = format!("f32.affine 0x{bias:08x}");
                for t in terms {
                    let o = match t.operand {
                        AffineOperand::One => "one".to_owned(),
                        AffineOperand::Node(x) => n(&x),
                    };
                    let _ = write!(s, " 0x{:08x} {o}", t.coefficient);
                }
                s
            }
            Op::F32Relu(x) | Op::F32Step(x) => format!("{} {}", node.op.name(), n(x)),
            Op::F32Reglu { left, gate } => format!("f32.reglu {} {}", n(left), n(gate)),
            Op::F32Select { bit, off, on } => {
                format!("f32.select {} {} {}", n(bit), n(off), n(on))
            }
            Op::F32Index {
                index,
                table,
                below,
                above,
            } => {
                let mut s = format!("f32.index {} {} {}", n(index), below.name(), above.name());
                for t in table {
                    let _ = write!(s, " {}", n(t));
                }
                s
            }
            Op::Guard { fact, value } => {
                format!("guard {} {}", n(value), fact_text(fact, g.profile))
            }
        };
        let _ = writeln!(s, "%{i} = {body}");
    }
    for f in &g.facts {
        let p = match &f.provenance {
            Provenance::InputValidated => "input-validated".to_owned(),
            Provenance::Asserted => "asserted".to_owned(),
            Provenance::Derived(r) => format!("derived {}", encode_rule(r)),
            Provenance::Guard(gd) => format!("guard {}", n(gd)),
            Provenance::Evidence(w) if w.is_empty() => "evidence -".to_owned(),
            Provenance::Evidence(w) => format!("evidence {}", hex(w)),
        };
        let _ = writeln!(
            s,
            "fact {} {} {p}",
            n(&f.node),
            fact_text(&f.fact, g.profile)
        );
    }
    for o in &g.outputs {
        let _ = writeln!(s, "out {}", n(o));
    }
    s
}
