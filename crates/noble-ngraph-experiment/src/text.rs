//! Text form of `ngraph-ir/0` for directly authored graphs (approval P2,
//! option B0(b)). Parsing only builds producer data; [`crate::check`] decides.
//!
//! ```text
//! ngraph-ir/0 ngraph-f32/0
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
//! Codes: unknown operations and syntax errors give `E-GRAPH-MALFORMED`;
//! inexact decimal literals `E-LITERAL-INEXACT`; `nan`/`inf` literals
//! `E-LITERAL-NONFINITE` (non-finite bit patterns are refused by the checker).

use std::fmt::Write as _;

use crate::diag::{Code, Diagnostic, Malformed, Site};
use crate::ir::{
    AffineOperand, AffineTerm, Fact, Graph, GraphBuilder, Literal, NodeId, Op, OutOfRange,
    Provenance, ScalarType, SlotKind, SubProfile, Subject,
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
            .and_then(|n| n.parse::<u32>().ok())
            .map(NodeId)
            .ok_or_else(|| syntax(self.line, format!("expected %node, got {t:?}")))
    }
    fn u32(&mut self) -> Result<u32, Diagnostic> {
        let t = self.next()?;
        t.parse()
            .map_err(|_| syntax(self.line, format!("expected integer, got {t:?}")))
    }
    fn f32_bits(&mut self) -> Result<u32, Diagnostic> {
        let t = self.next()?;
        parse_f32(t, self.line)
    }
    fn i64(&mut self) -> Result<i64, Diagnostic> {
        let t = self.next()?;
        t.parse()
            .map_err(|_| syntax(self.line, format!("expected i64, got {t:?}")))
    }
}

/// Parses an `F32` literal token: exact decimal or `0x` bits.
///
/// # Errors
/// `E-LITERAL-NONFINITE`, `E-LITERAL-INEXACT` or a syntax error.
pub fn parse_f32(t: &str, line: usize) -> Result<u32, Diagnostic> {
    if let Some(h) = t.strip_prefix("0x") {
        return u32::from_str_radix(h, 16).map_err(|_| syntax(line, format!("bad f32 bits {t:?}")));
    }
    let lower = t.to_ascii_lowercase();
    if lower.contains("nan") || lower.contains("inf") {
        return Err(Diagnostic::new(
            Code::LiteralNonfinite,
            Site::Line(line),
            format!("non-finite f32 literal {t}"),
        ));
    }
    if t.parse::<f64>().is_err() {
        return Err(syntax(line, format!("bad f32 literal {t:?}")));
    }
    crate::sem::exact_f32_decimal(t).ok_or_else(|| {
        Diagnostic::new(
            Code::LiteralInexact,
            Site::Line(line),
            format!("{t} is not exactly representable in binary32"),
        )
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

fn ty(t: &str, line: usize) -> Result<ScalarType, Diagnostic> {
    match t {
        "i64" => Ok(ScalarType::I64),
        "bool" => Ok(ScalarType::Bool),
        "f32" => Ok(ScalarType::F32),
        _ => Err(syntax(line, format!("bad type {t:?}"))),
    }
}

fn fact(t: &mut Toks<'_>, profile: SubProfile) -> Result<Fact, Diagnostic> {
    let kind = t.next()?;
    Ok(match kind {
        "range" => match profile {
            SubProfile::F32 => Fact::Range {
                lo: Literal::F32(t.f32_bits()?),
                hi: Literal::F32(t.f32_bits()?),
            },
            SubProfile::I64 => Fact::Range {
                lo: Literal::I64(t.i64()?),
                hi: Literal::I64(t.i64()?),
            },
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

/// Parses the text form. `subject_name` names the authored subject.
///
/// # Errors
/// The first D6 refusal encountered while reading.
#[allow(clippy::too_many_lines)]
pub fn parse(text: &str, subject_name: &str) -> Result<Graph, Diagnostic> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.split('#').next().unwrap_or("").trim()))
        .filter(|(_, l)| !l.is_empty());
    let (hl, header) = lines.next().ok_or_else(|| syntax(1, "empty text"))?;
    let mut h = header.split_whitespace();
    if h.next() != Some(crate::ir::IR_VERSION) {
        return Err(syntax(hl, "missing ngraph-ir/0 header"));
    }
    let profile = h
        .next()
        .and_then(SubProfile::from_name)
        .ok_or_else(|| Diagnostic::new(Code::Profile, Site::Line(hl), "unknown sub-profile"))?;
    let mut b = GraphBuilder::new(profile, Subject::authored(subject_name));
    for (line, l) in lines {
        let mut t = Toks {
            t: l.split_whitespace().collect(),
            i: 0,
            line,
        };
        let head = t.next()?;
        match head {
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
                let p = match t.next()? {
                    "input-validated" => Provenance::InputValidated,
                    "asserted" => Provenance::Asserted,
                    "derived" => Provenance::Derived(t.next().unwrap_or("unnamed").to_owned()),
                    "guard" => Provenance::Guard(t.node()?),
                    "evidence" => Provenance::Evidence(t.next()?.as_bytes().to_vec()),
                    o => return Err(syntax(line, format!("bad provenance {o:?}"))),
                };
                b.fact(n, f, p);
            }
            h if h.starts_with('%') => {
                let id = h[1..]
                    .parse::<usize>()
                    .map_err(|_| syntax(line, "bad node id"))?;
                if id != b.len() {
                    return Err(syntax(
                        line,
                        format!("node %{id} out of sequence (next is %{})", b.len()),
                    ));
                }
                if t.next()? != "=" {
                    return Err(syntax(line, "expected ="));
                }
                let name = t.next()?;
                let op = match name {
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
                        let fact = fact(&mut t, profile)?;
                        Op::Guard { fact, value }
                    }
                    other => {
                        return Err(Diagnostic::malformed(
                            Malformed::UnknownOp,
                            Site::Line(line),
                            format!("operation {other:?} is not in the ngraph-ir/0 whitelist"),
                        ))
                    }
                };
                b.op(op);
            }
            other => return Err(syntax(line, format!("unknown line kind {other:?}"))),
        }
        t.done()?;
    }
    Ok(b.finish())
}

fn lit(l: Literal) -> String {
    match l {
        Literal::I64(v) => v.to_string(),
        Literal::Bool(v) => v.to_string(),
        Literal::F32(b) => format!("0x{b:08x}"),
    }
}

fn fact_text(f: &Fact) -> String {
    match f {
        Fact::Range { lo, hi } => format!("range {} {}", lit(*lo), lit(*hi)),
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

/// Prints a graph in the text form (`F32` literals as exact bit patterns).
#[must_use]
pub fn print(g: &Graph) -> String {
    let mut s = format!("{} {}\n", crate::ir::IR_VERSION, g.profile.name());
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
            Op::Const(l) => format!("const {} {}", l.ty().name(), lit(*l)),
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
            Op::Guard { fact, value } => format!("guard {} {}", n(value), fact_text(fact)),
        };
        let _ = writeln!(s, "%{i} = {body}");
    }
    for f in &g.facts {
        let p = match &f.provenance {
            Provenance::InputValidated => "input-validated".to_owned(),
            Provenance::Asserted => "asserted".to_owned(),
            Provenance::Derived(r) => format!("derived {r}"),
            Provenance::Guard(gd) => format!("guard {}", n(gd)),
            Provenance::Evidence(w) => format!("evidence {}", crate::ir::hex(w)),
        };
        let _ = writeln!(s, "fact {} {} {p}", n(&f.node), fact_text(&f.fact));
    }
    for o in &g.outputs {
        let _ = writeln!(s, "out {}", n(o));
    }
    s
}
