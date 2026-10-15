//! D6 stage-scoped diagnostics. Every refusal carries a stable code; none
//! executes any graph node.

use crate::ir::{Fact, NodeId, SourceRef};

/// D5 budget kinds (`E-BUDGET-<kind>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BudgetKind {
    /// Extraction work units.
    Work,
    /// Expanded nodes before pruning/CSE/fusion.
    Expansion,
    /// Inline depth.
    InlineDepth,
    /// Live nodes after normalization.
    LiveNodes,
    /// Layers (critical path).
    Layers,
    /// Interface inputs and params.
    Inputs,
    /// Interface outputs.
    Outputs,
    /// Terms per `affine`.
    AffineTerms,
    /// Index table length.
    TableLength,
    /// Total index table entries.
    TableEntries,
    /// Facts.
    Facts,
    /// Retained graph bytes.
    GraphBytes,
    /// Snapshot bytes.
    SnapshotBytes,
    /// Forward scratch per evaluation.
    ForwardScratch,
    /// Backward scratch per evaluation.
    BackwardScratch,
    /// Optimizer steps per host call.
    Steps,
    /// Host evaluation fuel (node evaluations) — runtime exhaustion, `Obs::Exhausted`.
    Fuel,
}

impl BudgetKind {
    /// Stable suffix of `E-BUDGET-<kind>`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Work => "WORK",
            Self::Expansion => "EXPANSION",
            Self::InlineDepth => "INLINE-DEPTH",
            Self::LiveNodes => "LIVE-NODES",
            Self::Layers => "LAYERS",
            Self::Inputs => "INPUTS",
            Self::Outputs => "OUTPUTS",
            Self::AffineTerms => "AFFINE-TERMS",
            Self::TableLength => "TABLE-LENGTH",
            Self::TableEntries => "TABLE-ENTRIES",
            Self::Facts => "FACTS",
            Self::GraphBytes => "GRAPH-BYTES",
            Self::SnapshotBytes => "SNAPSHOT-BYTES",
            Self::ForwardScratch => "FORWARD-SCRATCH",
            Self::BackwardScratch => "BACKWARD-SCRATCH",
            Self::Steps => "STEPS",
            Self::Fuel => "FUEL",
        }
    }
}

/// Why `E-GRAPH-MALFORMED` was reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Malformed {
    /// A dependency cycle (including a self reference).
    Cycle,
    /// An operand defined later in canonical order (acyclic).
    ForwardReference,
    /// An operand, output or fact names a node id that does not exist.
    UnknownNode,
    /// A node that reaches no output and no guard.
    DanglingNode,
    /// An operation name outside the D2 whitelist (text form).
    UnknownOp,
    /// An operand of the wrong type within one sub-profile.
    TypeError,
    /// Two nodes for one interface slot.
    DuplicateSlot,
    /// A slot without a node, a node for an undeclared slot, or a kind mismatch.
    SlotMismatch,
    /// A guard used as a value operand or output.
    GuardAsValue,
    /// Empty `affine`-free structural payload error (empty table, bad fact payload).
    Payload,
    /// Text form syntax error.
    Syntax,
}

impl Malformed {
    /// Stable reason name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cycle => "cycle",
            Self::ForwardReference => "forward-reference",
            Self::UnknownNode => "unknown-node",
            Self::DanglingNode => "dangling-node",
            Self::UnknownOp => "unknown-op",
            Self::TypeError => "type-error",
            Self::DuplicateSlot => "duplicate-slot",
            Self::SlotMismatch => "slot-mismatch",
            Self::GuardAsValue => "guard-as-value",
            Self::Payload => "payload",
            Self::Syntax => "syntax",
        }
    }
}

/// D6 codes. `Budget` and `GraphMalformed` carry their sub-kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Code {
    /// `E-PROFILE`.
    Profile,
    /// `E-SUBJECT`.
    Subject,
    /// `E-CLOSURE`.
    Closure,
    /// `E-INTERFACE`.
    Interface,
    /// `E-EFFECT`.
    Effect,
    /// `E-RESOURCE`.
    Resource,
    /// `E-DYNAMIC-RUN`.
    DynamicRun,
    /// `E-LIVE-SLOT`.
    LiveSlot,
    /// `E-RECURSION`.
    Recursion,
    /// `E-CONTROL`.
    Control,
    /// `E-AGGREGATE`.
    Aggregate,
    /// `E-REFLECT`.
    Reflect,
    /// `E-PROGRAM-OUTPUT`.
    ProgramOutput,
    /// `E-TYPE-MIX`.
    TypeMix,
    /// `E-LITERAL-INEXACT`.
    LiteralInexact,
    /// `E-LITERAL-NONFINITE`.
    LiteralNonfinite,
    /// `E-BUDGET-<kind>`.
    Budget(BudgetKind),
    /// `E-FACT-UNCHECKED`.
    FactUnchecked,
    /// `E-FACT-INPUT`.
    FactInput,
    /// `G-FACT-FALSE` (guard failure observation; no outputs).
    FactFalse,
    /// `E-GRAPH-MALFORMED`.
    GraphMalformed(Malformed),
    /// `E-DERIV-BOOLEAN`.
    DerivBoolean,
    /// `E-DERIV-INDEX`.
    DerivIndex,
    /// `E-DERIV-I64`.
    DerivI64,
    /// `E-SNAPSHOT-STALE`.
    SnapshotStale,
    /// `E-SNAPSHOT-LAYOUT`.
    SnapshotLayout,
    /// `E-PARAM-UNDECLARED`.
    ParamUndeclared,
    /// `E-NONFINITE-UPDATE`.
    NonfiniteUpdate,
    /// `C-CANCELLED`.
    Cancelled,
}

impl Code {
    /// The stable D6 code string.
    #[must_use]
    pub fn as_string(self) -> String {
        match self {
            Self::Budget(k) => format!("E-BUDGET-{}", k.name()),
            other => other.base().to_owned(),
        }
    }

    /// The code string without the budget kind (`E-BUDGET` for budgets).
    #[must_use]
    pub const fn base(self) -> &'static str {
        match self {
            Self::Profile => "E-PROFILE",
            Self::Subject => "E-SUBJECT",
            Self::Closure => "E-CLOSURE",
            Self::Interface => "E-INTERFACE",
            Self::Effect => "E-EFFECT",
            Self::Resource => "E-RESOURCE",
            Self::DynamicRun => "E-DYNAMIC-RUN",
            Self::LiveSlot => "E-LIVE-SLOT",
            Self::Recursion => "E-RECURSION",
            Self::Control => "E-CONTROL",
            Self::Aggregate => "E-AGGREGATE",
            Self::Reflect => "E-REFLECT",
            Self::ProgramOutput => "E-PROGRAM-OUTPUT",
            Self::TypeMix => "E-TYPE-MIX",
            Self::LiteralInexact => "E-LITERAL-INEXACT",
            Self::LiteralNonfinite => "E-LITERAL-NONFINITE",
            Self::Budget(_) => "E-BUDGET",
            Self::FactUnchecked => "E-FACT-UNCHECKED",
            Self::FactInput => "E-FACT-INPUT",
            Self::FactFalse => "G-FACT-FALSE",
            Self::GraphMalformed(_) => "E-GRAPH-MALFORMED",
            Self::DerivBoolean => "E-DERIV-BOOLEAN",
            Self::DerivIndex => "E-DERIV-INDEX",
            Self::DerivI64 => "E-DERIV-I64",
            Self::SnapshotStale => "E-SNAPSHOT-STALE",
            Self::SnapshotLayout => "E-SNAPSHOT-LAYOUT",
            Self::ParamUndeclared => "E-PARAM-UNDECLARED",
            Self::NonfiniteUpdate => "E-NONFINITE-UPDATE",
            Self::Cancelled => "C-CANCELLED",
        }
    }
}

/// Where a diagnostic applies.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Site {
    /// Whole request/graph.
    Graph,
    /// A graph node.
    Node(NodeId),
    /// An interface slot.
    Slot(u32),
    /// A fact record index.
    Fact(usize),
    /// A source node (extraction).
    Source(SourceRef),
    /// A text-form line (1-based).
    Line(usize),
}

/// A refusal: stable code, site and human detail. Budget refusals carry the
/// limit and the observed count up to the point of refusal (D5).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    /// D6 code.
    pub code: Code,
    /// Site.
    pub site: Site,
    /// Budget limit and observed count, for `E-BUDGET-*`.
    pub budget: Option<(u64, u64)>,
    /// Failing fact, for `G-FACT-FALSE`, `E-FACT-*`.
    pub fact: Option<Fact>,
    /// Human-readable detail (not part of the stable contract).
    pub detail: String,
}

impl Diagnostic {
    /// A diagnostic with code, site and detail.
    #[must_use]
    pub fn new(code: Code, site: Site, detail: impl Into<String>) -> Self {
        Self {
            code,
            site,
            budget: None,
            fact: None,
            detail: detail.into(),
        }
    }

    /// `E-GRAPH-MALFORMED(kind)` at `site`.
    #[must_use]
    pub fn malformed(kind: Malformed, site: Site, detail: impl Into<String>) -> Self {
        Self::new(Code::GraphMalformed(kind), site, detail)
    }

    /// `E-BUDGET-<kind>` with limit and observed count.
    #[must_use]
    pub fn budget(kind: BudgetKind, limit: u64, observed: u64) -> Self {
        Self {
            code: Code::Budget(kind),
            site: Site::Graph,
            budget: Some((limit, observed)),
            fact: None,
            detail: format!("observed {observed} > limit {limit}"),
        }
    }

    /// Attaches a fact.
    #[must_use]
    pub fn with_fact(mut self, fact: Fact) -> Self {
        self.fact = Some(fact);
        self
    }

    /// Stable code string.
    #[must_use]
    pub fn code_string(&self) -> String {
        self.code.as_string()
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at {:?}: {}",
            self.code.as_string(),
            self.site,
            self.detail
        )
    }
}

impl std::error::Error for Diagnostic {}
