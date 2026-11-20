//! Versioned v2 guard templates: the bounded `NonEmptyList` extension.
//!
//! Version 1 (`guard.rs`, [`crate::companion::GuardTemplate`]) is frozen: its
//! three templates, five recognized spellings, wrapper texts and digest domain
//! stay byte-identical, and the MC2 gate keeps pinning exactly three
//! constructors. This module adds a *separate* set version with its own digest
//! domain and exactly one more whole-root template,
//! `(not (is-nil (in x0)))` over a one-element `[List A]` interface whose
//! element code `A` is pure `Type0` data.
//!
//! Recognition stays whole-root. A conjunction, a nested negation, a length or
//! head selector, another input index, a second input or a non-list interface
//! never selects a template, and no path falls back to an assumed guard. The
//! list template's guard is `[ dup [ true ] [ drop drop false ] list.case ]`:
//! the flag is `true` exactly for `nil`, and the accepted branch runs the
//! candidate on the untouched argument.

/// The template-set version a v2 binding records. The v1 set is version 1.
#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; the set version is part of the public binding name so a v1 and a v2 identity stay distinguishable in a report, which the module path alone does not make visible."
)]
pub const GUARD_SET_VERSION_V2: u32 = 2;

/// Digest domain of the v2 template set. It is distinct from the v1 guard
/// domain (`digest::DOMAIN_GUARD`), so a v1 digest and a v2 digest can never be
/// compared as equal evidence even for the same scalar template tag.
const DOMAIN_GUARD_SET: u64 = 0x47_55_41_52_44_00_00_02;

const CODE_LT_I64_MAX: &str = "lt-i64-max";
const CODE_NE_I64_MIN: &str = "ne-i64-min";
const CODE_EQ_I64_LITERAL: &str = "eq-i64-literal";
const CODE_NON_EMPTY_LIST: &str = "non-empty-list";

/// Bound on one `Type0` eligibility walk: at most this many pending nodes and
/// this many visited nodes. The declared profile refuses a type that nests
/// more than 63 constructors, so every accepted `[List A]` interface fits.
const TYPE_STACK: usize = 64;
const TYPE_VISITS: u32 = 256;

/// The finite v2 guard template set: the three frozen v1 templates plus one
/// list template. A v2 binding records [`GUARD_SET_VERSION_V2`], so a v1 witness is
/// never reinterpreted as a v2 one.
#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; the public set version is part of the type name so a v1 and a v2 template cannot be mistaken at a use site or in a retained report."
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[octet::sealed_enum]
pub enum GuardTemplateV2 {
    /// `x < i64::MAX`, carried unchanged from v1.
    LtI64Max,
    /// `x != i64::MIN`, carried unchanged from v1.
    NeI64Min,
    /// `x = literal` for one compile-time literal, carried unchanged from v1.
    EqI64Literal(i64),
    /// `(not (is-nil (in x0)))` over `[List A]` with a pure `Type0` `A`.
    NonEmptyList,
}

impl GuardTemplateV2 {
    /// The template-set version this template belongs to.
    pub const fn set_version(self) -> u32 {
        GUARD_SET_VERSION_V2
    }

    /// Integrity index of one v2 template under the v2 domain. This is a
    /// lookup aid, never admission authority.
    pub(crate) fn digest(self) -> u64 {
        let mut fold = crate::companion::digest::Fold::new(DOMAIN_GUARD_SET);
        match self {
            GuardTemplateV2::LtI64Max => fold.absorb(1),
            GuardTemplateV2::NeI64Min => fold.absorb(2),
            GuardTemplateV2::EqI64Literal(value) => {
                fold.absorb(3);
                fold.absorb(u64::from_ne_bytes(value.to_ne_bytes()));
            }
            GuardTemplateV2::NonEmptyList => fold.absorb(4),
        }
        fold.finish()
    }

    /// The stable template code reported for a binding.
    pub const fn code(self) -> &'static str {
        match self {
            GuardTemplateV2::LtI64Max => CODE_LT_I64_MAX,
            GuardTemplateV2::NeI64Min => CODE_NE_I64_MIN,
            GuardTemplateV2::EqI64Literal(_) => CODE_EQ_I64_LITERAL,
            GuardTemplateV2::NonEmptyList => CODE_NON_EMPTY_LIST,
        }
    }

    /// The frozen v1 template carried unchanged into the v2 set.
    pub(crate) const fn from_v1(template: crate::companion::GuardTemplate) -> Self {
        match template {
            crate::companion::GuardTemplate::LtI64Max => GuardTemplateV2::LtI64Max,
            crate::companion::GuardTemplate::NeI64Min => GuardTemplateV2::NeI64Min,
            crate::companion::GuardTemplate::EqI64Literal(value) => {
                GuardTemplateV2::EqI64Literal(value)
            }
        }
    }
}

/// Pure `Type0` element codes only: `Unit`, `Bool`, `I64`, `Text` and the
/// recursive `Pair`, `Sum` and `List` constructions. Programs, syntax,
/// contracts, evidence, certified values, live references, resources and
/// nominal descriptors are refused, so a list element type can never carry
/// executable code or authority into a guard. The declared profile applies the
/// same accept set to contract binding types; this copy keeps the companion
/// core free of a dependency on that register stage.
fn is_type0(ty: &noble_kernel::types::Ty) -> bool {
    type0_subtree(ty, 0, TYPE_VISITS).is_some()
}

/// Visits `ty`, then its children left first with `pending` right siblings held
/// below it, spending one of `remaining` visits per node. Returns the visits
/// left, or `None` where the depth or visit bound refused. The declared
/// profile's contract-type reader caps nesting and type size the same way.
#[expect(
    clippy::question_mark,
    reason = "Owner: noble-maintainers; explicit Option propagation preserves the pinned extractor's supported control flow."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the walk's bounds are carried by its two explicit counters and every refusal returns None, so there is no invalid-value assertion precondition."
)]
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-maintainers; each call visits a child of a bounded prepared type and spends one of at most 256 visits, and plain recursion keeps a borrowed work stack out of the pinned extraction."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the walk dereferences borrowed Box children of a prepared type, so it stays a runtime predicate matching the declared profile's recursive contract-type check and the pinned extraction."
)]
fn type0_subtree(ty: &noble_kernel::types::Ty, pending: usize, remaining: u32) -> Option<u32> {
    let remaining = match remaining.checked_sub(1) {
        Some(remaining) => remaining,
        None => return None,
    };
    match ty {
        noble_kernel::types::Ty::Unit
        | noble_kernel::types::Ty::Bool
        | noble_kernel::types::Ty::I64
        | noble_kernel::types::Ty::Text => Some(remaining),
        noble_kernel::types::Ty::Pair(left, right) | noble_kernel::types::Ty::Sum(left, right) => {
            let above = match pending.checked_add(1) {
                Some(above) => above,
                None => return None,
            };
            if above >= TYPE_STACK {
                return None;
            }
            let remaining = match type0_subtree(left, above, remaining) {
                Some(remaining) => remaining,
                None => return None,
            };
            type0_subtree(right, pending, remaining)
        }
        noble_kernel::types::Ty::List(item) => {
            if pending >= TYPE_STACK {
                return None;
            }
            type0_subtree(item, pending, remaining)
        }
        noble_kernel::types::Ty::Syntax
        | noble_kernel::types::Ty::Contract
        | noble_kernel::types::Ty::Evidence
        | noble_kernel::types::Ty::Certified
        | noble_kernel::types::Ty::Program(..)
        | noble_kernel::types::Ty::LiveRef(..)
        | noble_kernel::types::Ty::Resource(..)
        | noble_kernel::types::Ty::Nominal(..)
        | noble_kernel::types::Ty::GenericNominal(..) => None,
    }
}

/// `(is-nil (in x0))`: list emptiness of the first input only.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; is_nil_input_zero resolves runtime Prepared expression indices through non-const accessors and returns false for an unavailable expression."
)]
fn is_nil_input_zero(prepared: &crate::Prepared, index: u32) -> bool {
    let slot = match crate::companion::admit::recognize::expression(prepared, index) {
        Some(slot) => slot,
        None => return false,
    };
    let body = match &slot.kind {
        crate::ExprKind::IsNil(body) => *body,
        _ => return false,
    };
    match crate::companion::admit::recognize::expression(prepared, body) {
        Some(body) => crate::companion::admit::recognize::is_input_zero(&body.kind),
        None => false,
    }
}

/// Match the complete v2 precondition on the actual inputs, never a fragment.
///
/// An `[I64]` interface keeps exactly the v1 recognition, lifted into the v2
/// set. A one-element `[List A]` interface with a pure `Type0` `A` selects the
/// list template only for the whole root `(not (is-nil (in x0)))`.
///
/// The interface comparison stays inline and the element is read through a
/// length check and an index. A slice borrow bound to a local that outlives a
/// loop reaches an unimplemented ended-shared-loan shape in the pinned
/// extractor's loop-context join, and a slice pattern needs a metadata
/// projection it cannot resolve.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every recognizer step returns an empty or partial bounded set for an unmatched shape; no predicate is asserted to belong to the template algebra."
)]
pub(crate) fn templates_of(
    prepared: &crate::Prepared,
) -> alloc::vec::Vec<crate::companion::GuardTemplateV2> {
    let mut found = alloc::vec::Vec::with_capacity(1);
    if prepared.checked().interface.stack_in.as_slice() == [noble_kernel::types::Ty::I64] {
        let lifted = crate::companion::guard::templates_of(prepared);
        let mut index = 0usize;
        while index < lifted.len() {
            found.push(GuardTemplateV2::from_v1(lifted[index]));
            index = index.saturating_add(1);
        }
        return found;
    }
    let stack_in = prepared.checked().interface.stack_in.as_slice();
    if stack_in.len() != 1 {
        return found;
    }
    let element: &noble_kernel::types::Ty = match &stack_in[0] {
        noble_kernel::types::Ty::List(item) => item,
        _ => return found,
    };
    if !is_type0(element) {
        return found;
    }
    let root = match crate::companion::admit::recognize::expression(prepared, prepared.requires()) {
        Some(slot) => slot,
        None => return found,
    };
    if let crate::ExprKind::Not(inner) = &root.kind {
        if is_nil_input_zero(prepared, *inner) {
            found.push(GuardTemplateV2::NonEmptyList);
        }
    }
    found
}

/// The retained v2 template set of a contract, under the current context.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; retained contract membership, current context and nonempty recognized v2 guards are checked before a length-bounded copy; absent or stale v2 guard authority must return Refusal, not panic."
)]
pub(crate) fn templates(
    engine: &crate::companion::Core,
    contract: crate::companion::ContractId,
) -> Result<alloc::vec::Vec<GuardTemplateV2>, crate::companion::Refusal> {
    let entry = attempt!(engine.registry.contract(contract));
    if crate::companion::registry::Registry::stale(
        engine,
        crate::companion::registry::ContextSnapshot {
            policy: entry.policy,
            revision: entry.revision,
        },
    ) {
        return Err(crate::companion::Refusal::StaleContext);
    }
    if entry.guards_v2.is_empty() {
        return Err(crate::companion::Refusal::UnsupportedGuardTemplate);
    }
    let mut out = alloc::vec::Vec::with_capacity(entry.guards_v2.len());
    let mut index = 0usize;
    while index < entry.guards_v2.len() {
        out.push(entry.guards_v2[index]);
        index = index.saturating_add(1);
    }
    Ok(out)
}

/// The exact `[List A]` argument type a retained v2 list guard was formed
/// over. A contract without a list guard, or with any other input interface,
/// refuses rather than guessing a type.
pub(crate) fn list_interface(
    engine: &crate::companion::Core,
    contract: crate::companion::ContractId,
) -> Result<noble_kernel::types::Ty, crate::companion::Refusal> {
    let entry = attempt!(engine.registry.contract(contract));
    if crate::companion::registry::Registry::stale(
        engine,
        crate::companion::registry::ContextSnapshot {
            policy: entry.policy,
            revision: entry.revision,
        },
    ) {
        return Err(crate::companion::Refusal::StaleContext);
    }
    if !entry.guards_v2.contains(&GuardTemplateV2::NonEmptyList) {
        return Err(crate::companion::Refusal::UnsupportedGuardTemplate);
    }
    if entry.input.as_slice().len() != 1 {
        return Err(crate::companion::Refusal::UnsupportedGuardTemplate);
    }
    Ok(entry.input[0].clone())
}

/// Produce one source wrapper quotation. The three v1 templates keep the exact
/// v1 text; the list template is
/// `[ dup [ true ] [ drop drop false ] list.case [ drop 1 inl ] [ [ body ] run inr ] if ]`.
pub(crate) fn wrapper_source(
    engine: &crate::companion::Core,
    template: GuardTemplateV2,
    subject: &str,
) -> alloc::string::String {
    match template {
        GuardTemplateV2::LtI64Max => crate::companion::guard::wrapper_source(
            engine,
            crate::companion::GuardTemplate::LtI64Max,
            subject,
        ),
        GuardTemplateV2::NeI64Min => crate::companion::guard::wrapper_source(
            engine,
            crate::companion::GuardTemplate::NeI64Min,
            subject,
        ),
        GuardTemplateV2::EqI64Literal(value) => crate::companion::guard::wrapper_source(
            engine,
            crate::companion::GuardTemplate::EqI64Literal(value),
            subject,
        ),
        GuardTemplateV2::NonEmptyList => {
            let mut out = alloc::string::String::new();
            out.push_str("[ dup [ true ] [ drop drop false ] list.case [ drop 1 inl ] [ [ ");
            out.push_str(subject);
            out.push_str(" ] run inr ] if ]");
            out
        }
    }
}

/// Execute the already observed compiled subject on `[argument, program]`. The
/// three v1 templates keep the exact v1 text; the list template is
/// `[ swap dup [ true ] [ drop drop false ] list.case [ drop drop 1 inl ] [ swap run inr ] if ]`.
/// Rejection consumes both values without starting the protected body.
pub(crate) fn invocation_wrapper(template: GuardTemplateV2) -> alloc::string::String {
    match template {
        GuardTemplateV2::LtI64Max => crate::companion::guard::invocation_wrapper(
            crate::companion::GuardTemplate::LtI64Max,
        ),
        GuardTemplateV2::NeI64Min => crate::companion::guard::invocation_wrapper(
            crate::companion::GuardTemplate::NeI64Min,
        ),
        GuardTemplateV2::EqI64Literal(value) => crate::companion::guard::invocation_wrapper(
            crate::companion::GuardTemplate::EqI64Literal(value),
        ),
        GuardTemplateV2::NonEmptyList => alloc::string::String::from(
            "[ swap dup [ true ] [ drop drop false ] list.case [ drop drop 1 inl ] [ swap run inr ] if ]",
        ),
    }
}
