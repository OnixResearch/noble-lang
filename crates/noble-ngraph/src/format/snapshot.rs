//! Parameter snapshot files `noble-ngraph-snapshot/0` (design D6).
//!
//! ```text
//! noble-ngraph-snapshot/0
//! snapshot <64 hex>            # ParamSnapshotId
//! graph <64 hex>               # GraphLogicalId
//! layout <64 hex>              # param layout key
//! param <slot> 0x<8 hex>       # one line per param slot
//! optimizer-state <64 hex>
//! parent <64 hex>
//! subject <kind> <64 hex> closure <ids|->
//! profile ngraph-f32/0 ngraph-sem/0
//! facts <64 hex>               # fact-binding digest of the model graph
//! loss <64 hex>
//! data <64 hex>
//! policy ngraph-deriv-real/0
//! opt-in %<node> boolean straight-through|clamped-linear 0x<8 hex>|refuse
//! opt-in %<node> index zero|backward-difference|refuse
//! seed 0x<16 hex>
//! prng splitmix64/0
//! optimizer adam/0 lr 0x<16 hex> beta1 0x<16 hex> beta2 0x<16 hex> eps 0x<16 hex>
//! steps <n>
//! tools <tool revisions>
//! provenance <64 hex>          # SnapshotFile::provenance_digest over every record above
//! ```
//!
//! Loading recomputes the layout key, the snapshot identity and the
//! provenance digest from the recorded fields; any mismatch is
//! `E-SNAPSHOT-INTEGRITY`. A matching identity identifies bytes; it proves
//! nothing about the parameters, the optimizer or the evaluators.

#[allow(unused_imports)]
use crate::prelude::*;
use core::fmt::Write as _;

use super::value::format_diag;
use crate::check::CheckedGraph;
use crate::diag::{BudgetKind, Code, Diagnostic, Site};
use crate::grad::{BooleanPolicy, DerivOptions, IndexPolicy, DERIV_POLICY};
use crate::identity::IdentityScheme;
use crate::ir::{hex, Canon, Digest, NodeId, SubProfile, Subject, SubjectKind, SEMANTIC_REVISION};
use crate::prng::PRNG;
use crate::snapshot::{layout_key_of, ParamSnapshot};
use crate::train::{Adam, Provenance, ADAM};

/// Schema line.
pub const SCHEMA: &str = "noble-ngraph-snapshot/0";

/// A loaded or publishable snapshot file: the immutable parameter snapshot
/// and its training provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotFile {
    /// The parameter snapshot (graph, layout, slots, bytes, identity).
    pub snapshot: ParamSnapshot,
    /// The training provenance (`provenance.result == snapshot.id()`).
    pub provenance: Provenance,
}

fn integrity(why: impl Into<String>) -> Diagnostic {
    Diagnostic::new(Code::SnapshotIntegrity, Site::Graph, why)
}

impl SnapshotFile {
    /// The provenance digest recorded in the file: the identity encoding of
    /// every record in file order except the schema and `provenance` lines
    /// (design D6). With `bytes(x) = u64le(len x) ‖ x` and
    /// `str(s) = bytes(utf8 s)`, it is `H("noble.ngraph.provenance.v0" ‖ 0x00 ‖
    /// bytes(snapshot) ‖ bytes(graph) ‖ bytes(layout) ‖ u64le(#params) ‖
    /// (u32le(slot) ‖ u32le(bits))* ‖ bytes(optimizer-state) ‖ bytes(parent) ‖
    /// str(subject kind) ‖ bytes(subject identity) ‖ u64le(#closure) ‖
    /// u64le(id)* ‖ str(sub-profile) ‖ str(semantic revision) ‖ bytes(facts) ‖
    /// bytes(loss) ‖ bytes(data) ‖ str(policy) ‖ u64le(#opt-ins) ‖
    /// (u32le(node) ‖ str(kind) ‖ str(rule) ‖ [u32le(width bits)])* ‖
    /// u64le(seed) ‖ str(prng) ‖ str(optimizer) ‖ u64le(lr) ‖ u64le(beta1) ‖
    /// u64le(beta2) ‖ u64le(eps) ‖ u64le(steps) ‖ str(tools))`, where the
    /// width is present only for `clamped-linear`.
    #[must_use]
    pub fn provenance_digest(&self, ids: &dyn IdentityScheme) -> Digest {
        let s = &self.snapshot;
        let p = &self.provenance;
        let mut c = Canon::new(ids, "noble.ngraph.provenance.v0");
        c.bytes(&s.id());
        c.bytes(&s.graph());
        c.bytes(&s.layout());
        c.u64(s.slots().len() as u64);
        for (pos, k) in s.slots().iter().enumerate() {
            c.u32(*k);
            c.u32(s.bits_at(pos).unwrap_or(0));
        }
        c.bytes(&p.optimizer_state);
        c.bytes(&p.parent);
        c.str(p.subject.kind.name());
        c.bytes(&p.subject.identity);
        c.u64(p.subject.closure.len() as u64);
        for d in &p.subject.closure {
            c.u64(*d);
        }
        c.str(p.profile.name());
        c.str(SEMANTIC_REVISION);
        c.bytes(&p.facts);
        c.bytes(&p.loss);
        c.bytes(&p.data);
        c.str(DERIV_POLICY);
        let opts = opt_ins(&p.deriv);
        c.u64(opts.len() as u64);
        for o in &opts {
            c.u32(o.node);
            c.str(o.kind);
            c.str(o.rule);
            if let Some(w) = o.width {
                c.u32(w);
            }
        }
        c.u64(p.seed);
        c.str(PRNG);
        c.str(ADAM);
        c.u64(p.optimizer.lr.to_bits());
        c.u64(p.optimizer.beta1.to_bits());
        c.u64(p.optimizer.beta2.to_bits());
        c.u64(p.optimizer.eps.to_bits());
        c.u64(p.steps);
        c.str(&p.tools);
        c.finish()
    }

    /// Prints the file in canonical form.
    #[must_use]
    pub fn print(&self, ids: &dyn IdentityScheme) -> String {
        let s = &self.snapshot;
        let p = &self.provenance;
        let mut out = format!("{SCHEMA}\n");
        let _ = writeln!(out, "snapshot {}", hex(&s.id()));
        let _ = writeln!(out, "graph {}", hex(&s.graph()));
        let _ = writeln!(out, "layout {}", hex(&s.layout()));
        for (pos, k) in s.slots().iter().enumerate() {
            let bits = s.bits_at(pos).unwrap_or(0);
            let _ = writeln!(out, "param {k} 0x{bits:08x}");
        }
        let _ = writeln!(out, "optimizer-state {}", hex(&p.optimizer_state));
        let _ = writeln!(out, "parent {}", hex(&p.parent));
        let _ = writeln!(out, "subject {}", super::graph::subject_text(&p.subject));
        let _ = writeln!(out, "profile {} {SEMANTIC_REVISION}", p.profile.name());
        let _ = writeln!(out, "facts {}", hex(&p.facts));
        let _ = writeln!(out, "loss {}", hex(&p.loss));
        let _ = writeln!(out, "data {}", hex(&p.data));
        let _ = writeln!(out, "policy {DERIV_POLICY}");
        for o in opt_ins(&p.deriv) {
            let _ = match o.width {
                Some(w) => writeln!(out, "opt-in %{} {} {} 0x{w:08x}", o.node, o.kind, o.rule),
                None => writeln!(out, "opt-in %{} {} {}", o.node, o.kind, o.rule),
            };
        }
        let _ = writeln!(out, "seed 0x{:016x}", p.seed);
        let _ = writeln!(out, "prng {PRNG}");
        let a = &p.optimizer;
        let _ = writeln!(
            out,
            "optimizer {ADAM} lr 0x{:016x} beta1 0x{:016x} beta2 0x{:016x} eps 0x{:016x}",
            a.lr.to_bits(),
            a.beta1.to_bits(),
            a.beta2.to_bits(),
            a.eps.to_bits()
        );
        let _ = writeln!(out, "steps {}", p.steps);
        let _ = writeln!(out, "tools {}", p.tools);
        let _ = writeln!(out, "provenance {}", hex(&self.provenance_digest(ids)));
        out
    }

    /// Parses a snapshot file and recomputes its identities.
    ///
    /// # Errors
    /// `E-FORMAT` (schema, field order, syntax), `E-BUDGET-SNAPSHOT-BYTES`
    /// over `snapshot_bytes`, or `E-SNAPSHOT-INTEGRITY` when the layout key,
    /// the snapshot identity or the provenance digest differs from its
    /// recomputation, or a parameter is not finite.
    #[allow(clippy::too_many_lines)]
    pub fn parse(
        ids: &dyn IdentityScheme,
        text: &str,
        snapshot_bytes: u64,
    ) -> Result<Self, Diagnostic> {
        let mut c = Cursor {
            lines: super::lines(text).peekable(),
            last: 1,
        };
        let header = c
            .lines
            .next()
            .ok_or_else(|| format_diag(1, "missing noble-ngraph-snapshot/0 schema line"))?;
        if header.tokens != [SCHEMA] {
            return Err(format_diag(
                header.number,
                "missing noble-ngraph-snapshot/0 schema line",
            ));
        }
        c.last = header.number;
        let snapshot_id = c.digest("snapshot")?;
        let graph = c.digest("graph")?;
        let layout = c.digest("layout")?;
        let mut slots = Vec::new();
        let mut bytes = Vec::new();
        while let Some(l) = c.take_if("param") {
            let (k, b) = match l.tokens.as_slice() {
                [_, k, b] => (k.parse::<u32>().ok(), bits32(b)),
                _ => (None, None),
            };
            let (Some(k), Some(b)) = (k, b) else {
                return Err(format_diag(l.number, "expected `param <slot> 0x<bits>`"));
            };
            if !f32::from_bits(b).is_finite() {
                return Err(integrity(format!(
                    "non-finite parameter 0x{b:08x} in slot {k}"
                )));
            }
            slots.push(k);
            bytes.extend_from_slice(&b.to_le_bytes());
        }
        let observed = bytes.len() as u64;
        if observed > snapshot_bytes {
            return Err(Diagnostic::budget(
                BudgetKind::SnapshotBytes,
                snapshot_bytes,
                observed,
            ));
        }
        let optimizer_state = c.digest("optimizer-state")?;
        let parent = c.digest("parent")?;
        let (t, n) = c.field("subject")?;
        let subject = subject(&t, n)?;
        let (t, n) = c.field("profile")?;
        if t != [SubProfile::F32.name(), SEMANTIC_REVISION] {
            return Err(format_diag(
                n,
                "expected `profile ngraph-f32/0 ngraph-sem/0`",
            ));
        }
        let facts = c.digest("facts")?;
        let loss = c.digest("loss")?;
        let data = c.digest("data")?;
        let (t, n) = c.field("policy")?;
        if t != [DERIV_POLICY] {
            return Err(format_diag(n, "expected `policy ngraph-deriv-real/0`"));
        }
        let mut deriv = DerivOptions::default();
        let mut previous: Option<(u32, u8)> = None;
        while let Some(l) = c.take_if("opt-in") {
            let key = opt_in(&l.tokens[1..], l.number, &mut deriv)?;
            if previous.is_some_and(|p| p >= key) {
                return Err(format_diag(
                    l.number,
                    "opt-in lines must be strictly ordered by node and then kind",
                ));
            }
            previous = Some(key);
        }
        let (t, n) = c.field("seed")?;
        let seed = match t.as_slice() {
            [s] => hex_u64(s),
            _ => None,
        }
        .ok_or_else(|| format_diag(n, "expected `seed 0x<16 hex>`"))?;
        let (t, n) = c.field("prng")?;
        if t != [PRNG] {
            return Err(format_diag(n, "expected `prng splitmix64/0`"));
        }
        let (t, n) = c.field("optimizer")?;
        let optimizer = match t.as_slice() {
            [o, "lr", lr, "beta1", b1, "beta2", b2, "eps", eps] if *o == ADAM => {
                match (hex_u64(lr), hex_u64(b1), hex_u64(b2), hex_u64(eps)) {
                    (Some(lr), Some(b1), Some(b2), Some(eps)) => Some(Adam {
                        lr: f64::from_bits(lr),
                        beta1: f64::from_bits(b1),
                        beta2: f64::from_bits(b2),
                        eps: f64::from_bits(eps),
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
        .ok_or_else(|| {
            format_diag(
                n,
                "expected `optimizer adam/0 lr 0x… beta1 0x… beta2 0x… eps 0x…`",
            )
        })?;
        let (t, n) = c.field("steps")?;
        let steps = match t.as_slice() {
            [s] => s.parse::<u64>().ok(),
            _ => None,
        }
        .ok_or_else(|| format_diag(n, "expected `steps <n>`"))?;
        let (t, n) = c.field("tools")?;
        if t.is_empty() {
            return Err(format_diag(n, "expected `tools <revisions>`"));
        }
        let tools = t.join(" ");
        let recorded = c.digest("provenance")?;
        if let Some(l) = c.lines.next() {
            return Err(format_diag(
                l.number,
                "unexpected line after the provenance digest",
            ));
        }
        if layout_key_of(ids, &slots) != layout {
            return Err(integrity("the layout key differs from its recomputation"));
        }
        let snapshot = ParamSnapshot::from_parts(ids, graph, layout, slots, bytes);
        if snapshot.id() != snapshot_id {
            return Err(integrity(
                "the snapshot identity differs from its recomputation",
            ));
        }
        let file = Self {
            snapshot,
            provenance: Provenance {
                parent,
                subject,
                graph,
                profile: SubProfile::F32,
                facts,
                loss,
                data,
                deriv,
                seed,
                optimizer,
                steps,
                tools,
                result: snapshot_id,
                optimizer_state,
            },
        };
        if file.provenance_digest(ids) != recorded {
            return Err(integrity(
                "the provenance digest differs from its recomputation",
            ));
        }
        Ok(file)
    }

    /// Binds the snapshot to a checked graph before any node is evaluated
    /// (design D6).
    ///
    /// # Errors
    /// `E-SNAPSHOT-STALE` (another `GraphLogicalId` or fact-binding digest),
    /// `E-SNAPSHOT-LAYOUT` (another param layout) or `E-SNAPSHOT-ID` (not
    /// the caller-pinned snapshot).
    pub fn bind(
        &self,
        ids: &dyn IdentityScheme,
        checked: &CheckedGraph,
        pinned: Option<&Digest>,
    ) -> Result<(), Diagnostic> {
        if self.snapshot.graph() != checked.id() {
            return Err(Diagnostic::new(
                Code::SnapshotStale,
                Site::Graph,
                "snapshot belongs to another GraphLogicalId",
            ));
        }
        if self.provenance.facts != checked.fact_bindings(ids) {
            return Err(Diagnostic::new(
                Code::SnapshotStale,
                Site::Graph,
                "snapshot was trained under other fact bindings",
            ));
        }
        self.snapshot.check_use(checked)?;
        if let Some(p) = pinned {
            if *p != self.snapshot.id() {
                return Err(Diagnostic::new(
                    Code::SnapshotId,
                    Site::Graph,
                    "snapshot differs from the pinned snapshot identity",
                ));
            }
        }
        Ok(())
    }
}

/// Sequential field reader over significant lines.
struct Cursor<'a, I: Iterator<Item = super::Line<'a>>> {
    lines: core::iter::Peekable<I>,
    last: usize,
}

impl<'a, I: Iterator<Item = super::Line<'a>>> Cursor<'a, I> {
    /// The next line, which must start with `key`: its remaining tokens and
    /// its line number.
    fn field(&mut self, key: &str) -> Result<(Vec<&'a str>, usize), Diagnostic> {
        let l = self
            .lines
            .next()
            .ok_or_else(|| format_diag(self.last, format!("missing {key} line")))?;
        if l.tokens[0] != key {
            return Err(format_diag(
                l.number,
                format!("expected {key}, got {:?}", l.tokens[0]),
            ));
        }
        self.last = l.number;
        Ok((l.tokens[1..].to_vec(), l.number))
    }

    /// `key <64 hex>`.
    fn digest(&mut self, key: &str) -> Result<Digest, Diagnostic> {
        let (t, n) = self.field(key)?;
        match t.as_slice() {
            [h] => super::digest_from_hex(h)
                .ok_or_else(|| format_diag(n, format!("{key} must be 64 hex digits"))),
            _ => Err(format_diag(n, format!("{key} takes one digest"))),
        }
    }

    /// The next line if it starts with `key`.
    fn take_if(&mut self, key: &str) -> Option<super::Line<'a>> {
        if self.lines.peek().is_some_and(|l| l.tokens[0] == key) {
            let l = self.lines.next()?;
            self.last = l.number;
            Some(l)
        } else {
            None
        }
    }
}

fn bits32(s: &str) -> Option<u32> {
    let h = s.strip_prefix("0x")?;
    if h.is_empty() || h.len() > 8 {
        return None;
    }
    u32::from_str_radix(h, 16).ok()
}

fn hex_u64(s: &str) -> Option<u64> {
    let h = s.strip_prefix("0x")?;
    if h.is_empty() || h.len() > 16 {
        return None;
    }
    u64::from_str_radix(h, 16).ok()
}

fn subject(t: &[&str], n: usize) -> Result<Subject, Diagnostic> {
    let bad = || format_diag(n, "expected `subject <kind> <64 hex> closure <ids|->`");
    let [kind, id, c, ids] = t else {
        return Err(bad());
    };
    let kind = match *kind {
        "authored" => SubjectKind::Authored,
        "extracted" => SubjectKind::Extracted,
        "captured" => SubjectKind::Captured,
        _ => return Err(bad()),
    };
    let identity = super::digest_from_hex(id).ok_or_else(bad)?;
    if *c != "closure" {
        return Err(bad());
    }
    let closure = if *ids == "-" {
        Vec::new()
    } else {
        ids.split(',')
            .map(|d| d.parse::<u64>().map_err(|_| bad()))
            .collect::<Result<Vec<u64>, Diagnostic>>()?
    };
    Ok(Subject {
        kind,
        identity,
        closure,
    })
}

/// Parses one `opt-in` line into `deriv`; returns its `(node, kind)` key.
fn opt_in(t: &[&str], n: usize, deriv: &mut DerivOptions) -> Result<(u32, u8), Diagnostic> {
    let bad = || format_diag(n, "bad opt-in line");
    let node = t
        .first()
        .and_then(|s| s.strip_prefix('%'))
        .and_then(|s| s.parse::<u32>().ok())
        .map(NodeId)
        .ok_or_else(bad)?;
    match &t[1..] {
        ["boolean", rest @ ..] => {
            let p = match rest {
                ["refuse"] => BooleanPolicy::Refuse,
                ["straight-through"] => BooleanPolicy::StraightThrough,
                ["clamped-linear", w] => {
                    BooleanPolicy::ClampedLinear(f32::from_bits(bits32(w).ok_or_else(bad)?))
                }
                _ => return Err(bad()),
            };
            deriv.boolean.insert(node, p);
            Ok((node.0, 0))
        }
        ["index", rest @ ..] => {
            let p = match rest {
                ["refuse"] => IndexPolicy::Refuse,
                ["zero"] => IndexPolicy::Zero,
                ["backward-difference"] => IndexPolicy::BackwardDifference,
                _ => return Err(bad()),
            };
            deriv.index.insert(node, p);
            Ok((node.0, 1))
        }
        _ => Err(bad()),
    }
}

/// One derivative opt-in record.
struct OptIn {
    node: u32,
    kind: &'static str,
    rule: &'static str,
    width: Option<u32>,
}

/// The opt-in records of `deriv`, ordered by node and then kind
/// (`boolean` before `index`).
fn opt_ins(deriv: &DerivOptions) -> Vec<OptIn> {
    let mut out: Vec<(u8, OptIn)> = Vec::new();
    for (n, p) in &deriv.boolean {
        let (rule, width) = match p {
            BooleanPolicy::Refuse => ("refuse", None),
            BooleanPolicy::StraightThrough => ("straight-through", None),
            BooleanPolicy::ClampedLinear(w) => ("clamped-linear", Some(w.to_bits())),
        };
        out.push((
            0,
            OptIn {
                node: n.0,
                kind: "boolean",
                rule,
                width,
            },
        ));
    }
    for (n, p) in &deriv.index {
        let rule = match p {
            IndexPolicy::Refuse => "refuse",
            IndexPolicy::Zero => "zero",
            IndexPolicy::BackwardDifference => "backward-difference",
        };
        out.push((
            1,
            OptIn {
                node: n.0,
                kind: "index",
                rule,
                width: None,
            },
        ));
    }
    out.sort_by_key(|(k, o)| (o.node, *k));
    out.into_iter().map(|(_, o)| o).collect()
}
