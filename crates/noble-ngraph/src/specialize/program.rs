//! Checked Program residualization over an untouched, opaque stack prefix.
//!
//! The original submission is checked in full before graph extraction. An
//! extractor marker refuses every attempt to consume or move a prefix value;
//! the graph sees only the supported active suffix. Pair/Unit input values are
//! flattened left-to-right into scalar leaves (Unit contributes none). The
//! residual accepts only the ordered dynamic scalar leaves above the unchanged
//! prefix; checked output shapes are rebuilt with `unit` and `pair`. The
//! emitted source is independently prepared and admitted at that full
//! interface, leaving the prefix below its `let` bindings. The returned
//! ModulePrepared is an ordinary Declared-Text-v1 Program, not a graph proxy.
//!
//! This implementation and its finite differential tests do not prove
//! Rust/model, extraction/Program or backend/runtime correspondence.

use crate::identity::IdentityScheme;

/// Exact checked subject, binding map and bounded re-admission inputs.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    /// Untrusted producer bytes: the operation independently accepts the
    /// root and every reached definition before it uses them.
    pub original: &'a noble_kernel::execution::Submission,
    /// Independently checked exact, sorted definition closure.
    pub claimed_closure: &'a [u64],
    /// Optional externally retained original subject identity. A stale pin
    /// refuses; a caller must not treat a self-computed pin as authority.
    pub expected_original: Option<crate::ir::Digest>,
    /// Count of untouched stack values below the active suffix.
    pub preserved_tail: usize,
    /// One optional I64 value per scalar leaf of the active suffix, in
    /// bottom-first stack-item and left-to-right pair order. Unit contributes
    /// no leaf; the preserved tail has no binding positions in this library
    /// map. A Bool leaf may only remain dynamic.
    pub static_inputs: &'a [Option<i64>],
    /// Extraction, specialization and checker caps.
    pub budgets: &'a crate::check::Budgets,
    /// Ordinary declared-text preparation limits.
    pub source_limits: noble_contracts::Limits,
}

/// A source-prepared residual Program and its checked derivation.
#[derive(Debug)]
pub struct Residual {
    /// Emitted source for the ordinary Declared-Text-v1 path.
    pub source: alloc::string::String,
    /// The source-prepared ordinary Program, checked on the full interface.
    pub prepared: noble_contracts::source::ModulePrepared,
    /// Original full-stack subject and its exact reached closure.
    pub original_subject: crate::ir::Subject,
    /// Fresh Program derivation identity; its closure is the independently
    /// recomputed residual closure (empty for the let-only emitter).
    pub residual_subject: crate::ir::Subject,
    /// Specialized and independently checked graph before source emission.
    pub specialized: super::Residual,
    /// Independently re-extracted graph from the full-interface Program.
    pub readmitted: crate::check::CheckedGraph,
    /// Number of unchanged bottom-most stack values on both interfaces.
    pub preserved_tail: usize,
    /// Checked original active stack-item types, including pair/Unit shape.
    pub original_active_inputs: alloc::vec::Vec<noble_kernel::types::Ty>,
}

/// One reconstructed active input item for the original ordinary Program.
/// The opaque tail is supplied unchanged before these items by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveValue {
    /// Checked I64 or Bool leaf.
    Scalar(crate::sem::Value),
    /// Unit is a stack item but has no scalar binding position.
    Unit,
    /// Original nested pair shape, in left-to-right leaf order.
    Pair(alloc::boxed::Box<(ActiveValue, ActiveValue)>),
}

#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-ngraph; the source frontend and admission bound pair nesting to MAX_SHAPE_DEPTH"
)]
fn leaves(ty: &noble_kernel::types::Ty) -> usize {
    match ty {
        noble_kernel::types::Ty::Pair(left, right) => {
            leaves(left).saturating_add(leaves(right))
        }
        noble_kernel::types::Ty::Unit => 0,
        _ => 1,
    }
}

/// Number of flattened scalar input leaves. This only counts shapes: the
/// full original submission must still pass independent source admission
/// before any active type can be trusted as supported.
#[must_use]
pub fn input_leaf_count(inputs: &[noble_kernel::types::Ty]) -> usize {
    inputs.iter().fold(0usize, |n, ty| n.saturating_add(leaves(ty)))
}

impl Residual {
    /// Reconstruct the original active stack items from the residual's flat,
    /// checked dynamic leaf interface. Prefix tail items remain unchanged.
    /// Wrong arity/type or an inconsistent binding map returns `None`.
    #[must_use]
    pub fn merge_active(&self, dynamic: &[crate::sem::Value]) -> Option<alloc::vec::Vec<ActiveValue>> {
        if dynamic.len() != self.specialized.report.dynamic_inputs.len()
            || dynamic
                .iter()
                .zip(&self.specialized.report.dynamic_inputs)
                .any(|(value, ty)| value.ty() != *ty)
        {
            return None;
        }
        let mut leaf = 0;
        let mut rank = 0;
        let mut original = alloc::vec::Vec::with_capacity(self.original_active_inputs.len());
        for ty in &self.original_active_inputs {
            original.push(merge_value(
                ty,
                &self.specialized.report.static_inputs,
                dynamic,
                &mut leaf,
                &mut rank,
            )?);
        }
        if leaf != self.specialized.report.static_inputs.len() || rank != dynamic.len() {
            return None;
        }
        Some(original)
    }
}

#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-ngraph; independently accepted original types bound pair nesting to MAX_SHAPE_DEPTH"
)]
fn merge_value(
    ty: &noble_kernel::types::Ty,
    bindings: &[Option<i64>],
    dynamic: &[crate::sem::Value],
    leaf: &mut usize,
    rank: &mut usize,
) -> Option<ActiveValue> {
    match ty {
        noble_kernel::types::Ty::Pair(left, right) => Some(ActiveValue::Pair(
            alloc::boxed::Box::new((
                merge_value(left, bindings, dynamic, leaf, rank)?,
                merge_value(right, bindings, dynamic, leaf, rank)?,
            )),
        )),
        noble_kernel::types::Ty::Unit => Some(ActiveValue::Unit),
        noble_kernel::types::Ty::I64 | noble_kernel::types::Ty::Bool => {
            let known = *bindings.get(*leaf)?;
            *leaf += 1;
            let scalar = match (ty, known) {
                (noble_kernel::types::Ty::I64, Some(value)) => crate::sem::Value::I64(value),
                (_, Some(_)) => return None,
                (_, None) => {
                    let value = *dynamic.get(*rank)?;
                    *rank += 1;
                    value
                }
            };
            Some(ActiveValue::Scalar(scalar))
        }
        _ => None,
    }
}

fn refuse(
    code: crate::diag::Code,
    detail: impl Into<alloc::string::String>,
) -> crate::diag::Diagnostic {
    crate::diag::Diagnostic::new(code, crate::diag::Site::Graph, detail)
}

fn graph_budget(budgets: &crate::check::Budgets) -> crate::extract::Budget {
    crate::extract::Budget {
        work_units: budgets.work_units,
        expanded_nodes: budgets.expanded_nodes,
        inline_depth: budgets.inline_depth,
    }
}

fn output_type(ty: crate::ir::ScalarType) -> Option<noble_kernel::types::Ty> {
    match ty {
        crate::ir::ScalarType::I64 => Some(noble_kernel::types::Ty::I64),
        crate::ir::ScalarType::Bool => Some(noble_kernel::types::Ty::Bool),
        crate::ir::ScalarType::F32 => None,
    }
}

fn static_encoding(static_inputs: &[Option<i64>], preserved_tail: usize) -> alloc::vec::Vec<u8> {
    let mut encoded = alloc::vec::Vec::with_capacity(8 + static_inputs.len().saturating_mul(9));
    encoded.extend_from_slice(&(preserved_tail as u64).to_le_bytes());
    for entry in static_inputs {
        match entry {
            Some(value) => {
                encoded.push(1);
                encoded.extend_from_slice(&value.to_le_bytes());
            }
            None => encoded.push(0),
        }
    }
    encoded
}

/// Specialize an exact checked Program into a newly source-prepared,
/// independently checked residual Program. The observation domain is the
/// resource-free stack: `tail ++ merge_active(static_inputs, dynamic)` on the
/// original side and `tail ++ dynamic_scalar_leaves` on the residual side.
/// Every original active pair/Unit item is reconstructed in its original
/// shape; `tail` is an unchanged prefix of opaque stack items. No operation
/// of the original may inspect or move `tail`; unsupported subjects refuse.
/// Neither side is executed here, and the ordinary unspecialized path is
/// unchanged.
///
/// # Errors
/// Returns the original admission, extraction or graph-specialization
/// diagnostic, `E-SUBJECT` for a stale identity or refused residual source,
/// `E-CLOSURE` for a nonempty residual dependency closure, and `E-INTERFACE`
/// for a changed full stack interface. No partial Program is returned.
pub fn residualize(
    ids: &dyn IdentityScheme,
    request: &Request<'_>,
) -> Result<Residual, crate::diag::Diagnostic> {
    let original = crate::extract::admit_with_tail(
        ids,
        request.original,
        &crate::extract::Request {
            profile: "ngraph-i64/0",
            claimed_closure: request.claimed_closure,
            budget: graph_budget(request.budgets),
        },
        request.preserved_tail,
    )?;
    if request
        .expected_original
        .is_some_and(|expected| expected != original.subject().identity)
    {
        return Err(refuse(
            crate::diag::Code::Subject,
            "the checked original Program identity differs from the retained identity",
        ));
    }
    let extracted = crate::extract::extract(ids, &original, request.budgets)?;
    let specialized = super::specialize_inner(
        ids,
        &extracted.checked,
        &super::Request {
            static_inputs: request.static_inputs,
            budgets: request.budgets,
        },
        true,
    )?;
    let source = super::emit_program_source(
        &specialized.checked,
        extracted.checked.graph().output_shape.as_deref(),
    )?;
    let mut dynamic_inputs =
        request.original.request.expected.stack_in[..request.preserved_tail].to_vec();
    dynamic_inputs.extend(super::interface_types(&specialized.checked)?);
    let retained = noble_contracts::source::RetainedSourceLimit::new(request.source_limits.bytes)
        .ok_or_else(|| {
        refuse(
            crate::diag::Code::Profile,
            "the source byte limit exceeds the retained-source cap",
        )
    })?;
    let session = noble_contracts::source::ModuleSession::new_declared_text(&[], false, retained)
        .map_err(super::source_refusal)?;
    let prepared = session
        .prepare(source.as_bytes(), &dynamic_inputs, request.source_limits)
        .map_err(super::source_refusal)?;
    if prepared.kind() != noble_contracts::source::ModuleKind::Expression {
        return Err(refuse(
            crate::diag::Code::Subject,
            "residual source is not an ordinary expression Program",
        ));
    }
    let submission = prepared.submission().ok_or_else(|| {
        refuse(
            crate::diag::Code::Subject,
            "residual source produced no checked Program submission",
        )
    })?;
    let expected_output = &request.original.request.expected.stack_out;
    if prepared.output() != expected_output {
        return Err(refuse(
            crate::diag::Code::Interface,
            "the full residual Program output differs from the original stack interface",
        ));
    }
    let readmitted = crate::extract::admit_with_tail(
        ids,
        submission,
        &crate::extract::Request {
            profile: "ngraph-i64/0",
            claimed_closure: &[],
            budget: graph_budget(request.budgets),
        },
        request.preserved_tail,
    )?;
    let residual_subject = crate::ir::Subject::derive(
        ids,
        crate::ir::SubjectKind::Extracted,
        &[
            b"noble.ngraph.residual-program.v0",
            super::ACCOUNTING_REVISION.as_bytes(),
            &original.subject().identity,
            &readmitted.subject().identity,
            &static_encoding(request.static_inputs, request.preserved_tail),
        ],
        readmitted.closure().to_vec(),
    );
    let readmitted_graph = crate::extract::extract(ids, &readmitted, request.budgets)?.checked;
    if readmitted_graph.graph().outputs.len() != specialized.checked.graph().outputs.len()
        || readmitted_graph
            .graph()
            .outputs
            .iter()
            .zip(&specialized.checked.graph().outputs)
            .any(|(actual, expected)| {
                readmitted_graph.ty(*actual).and_then(output_type)
                    != specialized.checked.ty(*expected).and_then(output_type)
            })
    {
        return Err(refuse(
            crate::diag::Code::Interface,
            "residual Program scalar outputs differ from the checked residual graph",
        ));
    }
    Ok(Residual {
        source,
        prepared,
        original_subject: original.subject().clone(),
        residual_subject,
        specialized,
        readmitted: readmitted_graph,
        preserved_tail: request.preserved_tail,
        original_active_inputs: request.original.request.expected.stack_in[request.preserved_tail..]
            .to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Hasher(u64);

    impl crate::identity::IdentityHasher for Hasher {
        fn update(&mut self, bytes: &[u8]) {
            for &byte in bytes {
                self.0 = self.0.wrapping_mul(31).wrapping_add(u64::from(byte));
            }
        }

        fn finish(self: alloc::boxed::Box<Self>) -> crate::ir::Digest {
            let mut out = [0; 32];
            out[..8].copy_from_slice(&self.0.to_le_bytes());
            out
        }
    }

    struct Ids;

    impl IdentityScheme for Ids {
        fn name(&self) -> &'static str {
            "test"
        }

        fn start(&self) -> alloc::boxed::Box<dyn crate::identity::IdentityHasher + '_> {
            alloc::boxed::Box::new(Hasher(0))
        }
    }

    fn checked(
        definitions: &[&str],
        root: &str,
        input: &[noble_kernel::types::Ty],
    ) -> crate::extract::source::Prepared {
        crate::extract::source::prepare(
            noble_contracts::source::Session::new(),
            noble_contracts::Limits::default(),
            definitions,
            root,
            input,
        )
        .expect("original source prepares through the ordinary frontend")
    }

    fn prepare(
        original: &crate::extract::source::Prepared,
        tail: usize,
        static_inputs: &[Option<i64>],
        expected_original: Option<crate::ir::Digest>,
        claimed_closure: &[u64],
    ) -> Result<Residual, crate::diag::Diagnostic> {
        residualize(
            &Ids,
            &Request {
                original: &original.submission,
                claimed_closure,
                expected_original,
                preserved_tail: tail,
                static_inputs,
                budgets: &crate::check::Budgets::default(),
                source_limits: noble_contracts::Limits::default(),
            },
        )
    }

    fn observe(
        graph: &crate::check::CheckedGraph,
        inputs: &[crate::sem::Value],
    ) -> alloc::vec::Vec<crate::sem::Value> {
        match crate::eval::reference(
            graph,
            inputs,
            None,
            &mut crate::eval::Control::default(),
        )
        .obs
        {
            crate::eval::Obs::Normal(values) => values,
            other => panic!("unexpected graph observation {other:?}"),
        }
    }

    fn nested_pair() -> noble_kernel::types::Ty {
        noble_kernel::types::Ty::Pair(
            alloc::boxed::Box::new(noble_kernel::types::Ty::I64),
            alloc::boxed::Box::new(noble_kernel::types::Ty::Pair(
                alloc::boxed::Box::new(noble_kernel::types::Ty::Unit),
                alloc::boxed::Box::new(noble_kernel::types::Ty::I64),
            )),
        )
    }

    fn original_graph(
        original: &crate::extract::source::Prepared,
        tail: usize,
    ) -> crate::check::CheckedGraph {
        let budgets = crate::check::Budgets::default();
        let admitted = crate::extract::admit_with_tail(
            &Ids,
            &original.submission,
            &crate::extract::Request {
                profile: "ngraph-i64/0",
                claimed_closure: &original.claimed_closure,
                budget: graph_budget(&budgets),
            },
            tail,
        )
        .expect("original independently admitted");
        crate::extract::extract(&Ids, &admitted, &budgets)
            .expect("original independently checked graph")
            .checked
    }

    #[test]
    fn nested_pair_partial_bindings_reconstruct_checked_shape_and_dynamic_order() {
        let inputs = [
            noble_kernel::types::Ty::Text,
            nested_pair(),
            noble_kernel::types::Ty::Unit,
            noble_kernel::types::Ty::Bool,
            noble_kernel::types::Ty::I64,
        ];
        let original = checked(
            &[],
            "let y let flag drop unpair unpair let z drop y + unit z y - pair pair flag",
            &inputs,
        );
        let original_graph = original_graph(&original, 1);
        assert_eq!(input_leaf_count(&inputs[1..]), 4);
        let residual = prepare(
            &original,
            1,
            &[Some(2), None, None, Some(4)],
            None,
            &[],
        )
        .expect("the independently checked active pair/Unit and mixed leaves specialize");
        assert_eq!(
            residual
                .prepared
                .submission()
                .expect("ordinary checked Program")
                .request
                .expected
                .stack_in,
            [
                noble_kernel::types::Ty::Text,
                noble_kernel::types::Ty::I64,
                noble_kernel::types::Ty::Bool
            ]
        );
        assert_eq!(
            residual.prepared.output(),
            original.submission.request.expected.stack_out
        );
        assert_eq!(
            residual.readmitted.graph().output_shape,
            original_graph.graph().output_shape
        );
        assert!(residual.source.contains("unit\n"));
        assert!(residual.source.matches("pair\n").count() >= 2);
        for (z, flag) in [(9i64, true), (i64::MIN, false), (0, true)] {
            let dynamic = [crate::sem::Value::I64(z), crate::sem::Value::Bool(flag)];
            let full = [
                crate::sem::Value::I64(2),
                crate::sem::Value::I64(z),
                crate::sem::Value::Bool(flag),
                crate::sem::Value::I64(4),
            ];
            assert_eq!(
                residual.merge_active(&dynamic),
                Some(alloc::vec![
                    ActiveValue::Pair(alloc::boxed::Box::new((
                        ActiveValue::Scalar(crate::sem::Value::I64(2)),
                        ActiveValue::Pair(alloc::boxed::Box::new((
                            ActiveValue::Unit,
                            ActiveValue::Scalar(crate::sem::Value::I64(z)),
                        ))),
                    ))),
                    ActiveValue::Unit,
                    ActiveValue::Scalar(crate::sem::Value::Bool(flag)),
                    ActiveValue::Scalar(crate::sem::Value::I64(4)),
                ])
            );
            let expected = alloc::vec![
                crate::sem::Value::I64(6),
                crate::sem::Value::I64(z.wrapping_sub(4)),
                crate::sem::Value::Bool(flag),
            ];
            assert_eq!(observe(&residual.specialized.checked, &dynamic), expected);
            assert_eq!(observe(&residual.readmitted, &dynamic), expected);
            assert_eq!(observe(&original_graph, &full), expected);
        }
        assert_eq!(residual.merge_active(&[crate::sem::Value::I64(9)]), None);
        assert_eq!(
            residual.merge_active(&[
                crate::sem::Value::Bool(true),
                crate::sem::Value::I64(9)
            ]),
            None
        );
    }

    #[test]
    fn nested_pair_all_and_no_static_leaves_keep_unit_and_output_shape() {
        let inputs = [nested_pair(), noble_kernel::types::Ty::Unit];
        let original = checked(&[], "drop unpair unpair swap drop + unit pair", &inputs);
        let original_graph = original_graph(&original, 0);
        for (map, dynamic, full) in [
            (
                &[None, None][..],
                alloc::vec![
                    crate::sem::Value::I64(i64::MAX),
                    crate::sem::Value::I64(1)
                ],
                alloc::vec![
                    crate::sem::Value::I64(i64::MAX),
                    crate::sem::Value::I64(1)
                ],
            ),
            (
                &[Some(i64::MAX), Some(1)][..],
                alloc::vec![],
                alloc::vec![
                    crate::sem::Value::I64(i64::MAX),
                    crate::sem::Value::I64(1)
                ],
            ),
        ] {
            let residual = prepare(&original, 0, map, None, &[])
                .expect("all/no static nested pair leaves specialize");
            let expected_inputs = if map.iter().all(Option::is_some) {
                alloc::vec![]
            } else {
                alloc::vec![noble_kernel::types::Ty::I64; 2]
            };
            assert_eq!(
                residual
                    .prepared
                    .submission()
                    .expect("ordinary Program")
                    .request
                    .expected
                    .stack_in,
                expected_inputs
            );
            assert_eq!(
                residual.prepared.output(),
                &[noble_kernel::types::Ty::Pair(
                    alloc::boxed::Box::new(noble_kernel::types::Ty::I64),
                    alloc::boxed::Box::new(noble_kernel::types::Ty::Unit),
                )]
            );
            assert_eq!(
                residual.merge_active(&dynamic),
                Some(alloc::vec![
                    ActiveValue::Pair(alloc::boxed::Box::new((
                        ActiveValue::Scalar(crate::sem::Value::I64(i64::MAX)),
                        ActiveValue::Pair(alloc::boxed::Box::new((
                            ActiveValue::Unit,
                            ActiveValue::Scalar(crate::sem::Value::I64(1)),
                        ))),
                    ))),
                    ActiveValue::Unit,
                ])
            );
            // The accepted source output keeps Pair<I64,Unit>; the scalar
            // observation and wrapping result are the same on both paths.
            assert_eq!(
                observe(&original_graph, &full),
                alloc::vec![crate::sem::Value::I64(i64::MIN)]
            );
            assert_eq!(
                observe(&residual.readmitted, &dynamic),
                alloc::vec![crate::sem::Value::I64(i64::MIN)]
            );
        }
    }

    #[test]
    fn unit_only_active_input_and_pair_output_need_no_scalar_binding() {
        let original = checked(&[], "unit pair", &[noble_kernel::types::Ty::Unit]);
        let residual = prepare(&original, 0, &[], None, &[])
            .expect("a checked Unit-only shape is reconstructible without scalar leaves");
        assert_eq!(input_leaf_count(&[noble_kernel::types::Ty::Unit]), 0);
        assert_eq!(
            residual
                .prepared
                .submission()
                .expect("ordinary Program")
                .request
                .expected
                .stack_in,
            alloc::vec![]
        );
        assert_eq!(
            residual.prepared.output(),
            &[noble_kernel::types::Ty::Pair(
                alloc::boxed::Box::new(noble_kernel::types::Ty::Unit),
                alloc::boxed::Box::new(noble_kernel::types::Ty::Unit),
            )]
        );
        assert_eq!(residual.source, "unit\nunit\npair\n");
        assert_eq!(residual.merge_active(&[]), Some(alloc::vec![ActiveValue::Unit]));
        assert_eq!(observe(&residual.readmitted, &[]), alloc::vec![]);
        assert_eq!(residual.readmitted.graph().outputs.len(), 0);
    }

    #[test]
    fn repeated_output_reuses_one_bound_node_while_single_use_constant_is_inlined() {
        let original = checked(&[], "1 + dup pair", &[noble_kernel::types::Ty::I64]);
        let residual = prepare(&original, 0, &[None], None, &[])
            .expect("a pair of repeated results reuses its common subexpression");
        assert_eq!(residual.source.matches("let v").count(), 2);
        assert!(!residual.source.contains("1 let v"));
        assert_eq!(residual.source.matches("v2 ").count(), 2);
        let original_graph = original_graph(&original, 0);
        for x in [0, 7, i64::MAX] {
            let input = [crate::sem::Value::I64(x)];
            assert_eq!(
                observe(&residual.readmitted, &input),
                alloc::vec![crate::sem::Value::I64(x.wrapping_add(1)); 2]
            );
            assert_eq!(
                observe(&original_graph, &input),
                observe(&residual.readmitted, &input)
            );
        }
    }

    #[test]
    fn boolean_leaf_cannot_be_static_inside_a_pair() {
        let pair = noble_kernel::types::Ty::Pair(
            alloc::boxed::Box::new(noble_kernel::types::Ty::Bool),
            alloc::boxed::Box::new(noble_kernel::types::Ty::I64),
        );
        let original = checked(&[], "unpair drop", &[pair]);
        assert_eq!(
            prepare(&original, 0, &[Some(1), None], None, &[])
                .expect_err("I64 cannot bind a Bool leaf")
                .code,
            crate::diag::Code::Interface
        );
        assert_eq!(
            prepare(&original, 0, &[None], None, &[])
                .expect_err("a whole pair item is not one scalar leaf")
                .code,
            crate::diag::Code::Interface
        );
    }

    #[test]
    fn checked_root_keeps_opaque_tail_and_exact_closures() {
        let pair = noble_kernel::types::Ty::Pair(
            alloc::boxed::Box::new(noble_kernel::types::Ty::I64),
            alloc::boxed::Box::new(noble_kernel::types::Ty::Bool),
        );
        let inputs = [
            noble_kernel::types::Ty::Text,
            noble_kernel::types::Ty::Unit,
            pair,
            noble_kernel::types::Ty::I64,
            noble_kernel::types::Ty::I64,
        ];
        let original = checked(&["def twice [ dup + ]"], "twice swap 3 +", &inputs);
        assert_eq!(&original.submission.request.expected.stack_in, &inputs);
        let residual = prepare(
            &original,
            3,
            &[Some(5), None],
            None,
            &original.claimed_closure,
        )
        .expect("checked Program specializes while preserving an opaque tail");
        assert_eq!(residual.preserved_tail, 3);
        assert_eq!(residual.original_subject.closure, original.claimed_closure);
        assert!(residual.residual_subject.closure.is_empty());
        assert_ne!(
            residual.original_subject.identity,
            residual.residual_subject.identity
        );
        assert_eq!(
            residual
                .prepared
                .submission()
                .expect("ordinary Program")
                .request
                .expected
                .stack_in,
            [
                inputs[..3].to_vec(),
                alloc::vec![noble_kernel::types::Ty::I64],
            ]
            .concat()
        );
        assert_eq!(
            residual.prepared.output(),
            original.submission.request.expected.stack_out
        );
        assert_eq!(
            residual.readmitted.graph().slots.len(),
            1,
            "the residual graph contains only the dynamic active suffix"
        );
    }

    #[test]
    fn stale_subject_and_missing_dependency_refuse_before_publication() {
        let original = checked(
            &["def twice [ dup + ]"],
            "twice",
            &[noble_kernel::types::Ty::Text, noble_kernel::types::Ty::I64],
        );
        assert!(!original.claimed_closure.is_empty());
        assert_eq!(
            prepare(&original, 1, &[None], None, &[])
                .expect_err("a missing named definition must refuse")
                .code,
            crate::diag::Code::Closure
        );
        let retained = prepare(&original, 1, &[None], None, &original.claimed_closure)
            .expect("the source-bound original identity")
            .original_subject
            .identity;
        let mut stale = original.clone();
        stale.submission.definitions[0].identity += 1;
        stale.claimed_closure[0] += 1;
        assert_eq!(
            prepare(&stale, 1, &[None], Some(retained), &stale.claimed_closure)
                .expect_err("a changed definition identity must not match the retained subject")
                .code,
            crate::diag::Code::Subject
        );
    }

    #[test]
    fn a_checked_root_may_not_inspect_or_move_its_tail() {
        let original = checked(
            &[],
            "swap",
            &[noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64],
        );
        assert_eq!(
            prepare(&original, 1, &[None], None, &[])
                .expect_err("the opaque tail is consumed")
                .code,
            crate::diag::Code::Interface
        );
        let effectful = checked(
            &[],
            "\"audit\" test.emit 1 +",
            &[noble_kernel::types::Ty::Text, noble_kernel::types::Ty::I64],
        );
        assert_eq!(
            prepare(&effectful, 1, &[None], None, &[])
                .expect_err("host effects are outside the pure Program slice")
                .code,
            crate::diag::Code::Effect
        );
    }
}
