//! Resolve inert contracts and proofs against the same staged source namespace.
//! No proof is accepted or published here: the independent host gate owns that step.

mod origin;

pub(super) struct Staged {
    pub contracts: alloc::vec::Vec<crate::source::declared::LogicalContract>,
    pub proof_exports: alloc::vec::Vec<alloc::string::String>,
    pub pending: Option<crate::intrinsic::ProofBatch>,
}

pub(super) struct Request<'a> {
    pub session: &'a crate::source::declared::ModuleSession,
    pub collected: &'a super::collection::Collected,
    pub name: &'a str,
    pub version: u32,
    pub identity: u64,
    pub source: &'a [u8],
    pub generation: u64,
    pub limits: crate::Limits,
}

fn at(stage: crate::source::Stage, span: crate::Span, reason: &str) -> crate::source::Error {
    crate::source::declared::diagnostic(stage, crate::invalid(span, reason))
}

pub(super) fn stage(request: Request<'_>) -> Result<Staged, crate::source::Error> {
    let mut contracts = alloc::vec::Vec::with_capacity(request.collected.contracts.len());
    for contract in &request.collected.contracts {
        let subject = attempt!(bind_subject(&request, contract));
        let goal = crate::intrinsic::ContractGoal {
            revision: contract.revision,
            contract_name: contract.name.clone(),
            contract_span: contract.span,
            subject,
            inputs: contract.inputs.iter().map(|binding| (binding.name.clone(), binding.ty.clone())).collect(),
            outputs: contract.outputs.iter().map(|binding| (binding.name.clone(), binding.ty.clone())).collect(),
            requires: contract.requires.clone(),
            ensures: contract.ensures.clone(),
        };
        let prepared = match contract.revision {
            1 => crate::intrinsic::prepare_contract(&goal, request.limits).map(|_| ()),
            2 => crate::intrinsic::prepare_named_contract(&goal, request.limits).map(|_| ()),
            _ => return Err(at(crate::source::Stage::Check, contract.span,
                "unsupported contract revision")),
        };
        attempt!(prepared
            .map_err(|problem| crate::source::declared::diagnostic(crate::source::Stage::Check, problem)));
        contracts.push(crate::source::declared::LogicalContract {
            name: contract.name.clone(),
            goal,
            exported: request.collected.exports.contains(&contract.name),
        });
    }
    let proof_exports = request.collected.proofs.iter()
        .filter(|proof| request.collected.exports.contains(&proof.name))
        .map(|proof| proof.name.clone()).collect();
    if request.collected.proofs.is_empty() {
        return Ok(Staged { contracts, proof_exports, pending: None });
    }
    let mut dependencies = alloc::vec::Vec::new();
    let mut obligations = alloc::vec::Vec::with_capacity(request.collected.proofs.len());
    for proof in &request.collected.proofs {
        let goal = match &proof.claim {
            crate::source::declared::parsing::ProofClaim::Pure(proposition) =>
                crate::intrinsic::PendingGoal::Pure { proposition: proposition.clone() },
            crate::source::declared::parsing::ProofClaim::For(reference, span) => {
                let contract = attempt!(resolve_contract(&request, &contracts, reference, *span));
                if proof.revision != contract.revision {
                    return Err(at(crate::source::Stage::Check, *span,
                        "proof and contract revisions differ"));
                }
                crate::intrinsic::PendingGoal::Contract { contract: alloc::boxed::Box::new(contract) }
            }
        };
        let mut term = proof.term.clone();
        attempt!(resolve_uses(&request, &mut term, &obligations, &mut dependencies));
        obligations.push(crate::intrinsic::ProofObligation {
            revision: proof.revision, name: proof.name.clone(), goal, term, span: proof.span,
        });
    }
    let limit = usize::try_from(request.limits.bytes).map_err(|_| crate::source::declared::error(
        crate::source::Stage::Check, "proof dependency byte limit exceeds address space",
    ))?;
    let mut copied = 0usize;
    for dependency in &dependencies {
        let bytes = attempt!(dependency_bytes(dependency, limit));
        copied = copied.checked_add(bytes).ok_or_else(|| crate::source::declared::error(
            crate::source::Stage::Check, "proof dependency byte count overflow",
        ))?;
    }
    if copied > limit / obligations.len() {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check, "proof dependency snapshot byte budget exhausted",
        ));
    }
    Ok(Staged {
        contracts,
        proof_exports,
        pending: Some(crate::intrinsic::ProofBatch {
            module: alloc::string::String::from(request.name),
            version: request.version,
            source: request.source.to_vec(),
            generation: request.generation,
            dependencies,
            obligations,
        }),
    })
}

fn dependency_bytes(
    dependency: &crate::intrinsic::ProofDependency,
    limit: usize,
) -> Result<usize, crate::source::Error> {
    let mut pending = alloc::vec![dependency];
    let mut total = 0usize;
    while let Some(next) = pending.pop() {
        total = total.checked_add(next.source.len()).ok_or_else(|| crate::source::declared::error(
            crate::source::Stage::Check, "proof dependency byte count overflow",
        ))?;
        if total > limit {
            return Err(crate::source::declared::error(
                crate::source::Stage::Check, "proof dependency snapshot byte budget exhausted",
            ));
        }
        pending.extend(next.dependencies.iter());
    }
    Ok(total)
}

fn pure_type(ty: &noble_kernel::types::Ty) -> bool {
    use noble_kernel::types::Ty;
    match ty {
        Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text => true,
        Ty::Pair(left, right) | Ty::Sum(left, right) => pure_type(left) && pure_type(right),
        Ty::List(item) => pure_type(item),
        Ty::Syntax | Ty::Contract | Ty::Evidence | Ty::Certified | Ty::Program(..)
        | Ty::Resource(..) | Ty::Nominal(..) | Ty::GenericNominal(..) => false,
    }
}

fn types(
    bindings: &[crate::source::declared::parsing::Binding],
    environment: &noble_kernel::contracts::Env,
) -> Result<alloc::vec::Vec<noble_kernel::types::Ty>, crate::source::Error> {
    let mut result = alloc::vec::Vec::with_capacity(bindings.len());
    for binding in bindings {
        let parsed = attempt!(crate::source::declared::types::parse_with_families(
            &binding.ty, &[], &[], Some(environment), binding.span,
        ).map_err(|problem| crate::source::declared::diagnostic(crate::source::Stage::Check, problem)));
        if !pure_type(&parsed.ty) {
            return Err(at(crate::source::Stage::Check, binding.span,
                "contract stack contains a non-Type0 value"));
        }
        result.push(parsed.ty);
    }
    Ok(result)
}

struct SourceOrigin<'a> {
    module_name: &'a str,
    version: u32,
    module_source: &'a [u8],
    definition: &'a crate::source::declared::DefinitionSource,
    ordinal: u32,
    owner: u64,
}

fn source_origin<'a>(
    request: &'a Request<'_>,
    definition: &crate::source::Named,
    reference_span: crate::Span,
) -> Result<SourceOrigin<'a>, crate::source::Error> {
    let owner = definition.owner.ok_or_else(|| at(
        crate::source::Stage::Resolve, reference_span,
        "contract subject has no immutable module owner",
    ))?;
    let (module_name, version, module_source, spans) = if owner == request.identity {
        (request.name, request.version, request.source, &request.collected.definition_spans)
    } else {
        let Some(module) = request.session.modules.iter().find(|module| module.identity == owner) else {
            return Err(crate::source::declared::error(
                crate::source::Stage::Resolve, "unknown contract subject module",
            ));
        };
        (module.name.as_str(), module.version, module.source.as_slice(), &module.definition_spans)
    };
    let mut found = None;
    for (position, source) in spans.iter().enumerate() {
        if source.name == definition.name {
            if found.is_some() {
                return Err(crate::source::declared::error(
                    crate::source::Stage::Check, "ambiguous original subject definition",
                ));
            }
            let ordinal = u32::try_from(position).map_err(|_| crate::source::declared::error(
                crate::source::Stage::Check, "subject definition ordinal exceeds address space",
            ))?;
            found = Some((source, ordinal));
        }
    }
    let Some((source, ordinal)) = found else {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check, "missing original subject source span",
        ));
    };
    Ok(SourceOrigin { module_name, version, module_source,
        definition: source, ordinal, owner })
}

impl SourceOrigin<'_> {
    fn definition_source(&self) -> Result<&[u8], crate::source::Error> {
        let start = usize::try_from(self.definition.span.start).map_err(|_| {
            crate::source::declared::error(crate::source::Stage::Check, "subject span exceeds address space")
        })?;
        let end = usize::try_from(self.definition.span.end).map_err(|_| {
            crate::source::declared::error(crate::source::Stage::Check, "subject span exceeds address space")
        })?;
        self.module_source.get(start..end).ok_or_else(|| crate::source::declared::error(
            crate::source::Stage::Check, "subject span is outside retained module source",
        ))
    }
}

fn bind_subject(
    request: &Request<'_>,
    contract: &crate::source::declared::parsing::Contract,
) -> Result<crate::intrinsic::SubjectBinding, crate::source::Error> {
    let Some(context) = request.session.source.declared.as_ref() else {
        return Err(crate::source::declared::error(crate::source::Stage::Check, "missing declared source environment"));
    };
    let lookup = if let Some((scope, local)) = contract.subject.split_once('.') {
        if is_current(request, scope) { local } else { contract.subject.as_str() }
    } else {
        contract.subject.as_str()
    };
    let found = context.words.iter().find(|(name, _)| name == lookup);
    let Some((_, crate::source::Target::Named(index))) = found else {
        return Err(at(crate::source::Stage::Resolve, contract.subject_span,
            "contract subject is not an accepted source definition"));
    };
    let Some(definition) = request.session.source.definitions.get(*index as usize) else {
        return Err(crate::source::declared::error(crate::source::Stage::Check, "missing resolved contract subject"));
    };
    let original = attempt!(source_origin(request, definition, contract.subject_span));
    let definition_source = attempt!(original.definition_source());
    let inputs = attempt!(types(&contract.inputs, &context.environment));
    let outputs = attempt!(types(&contract.outputs, &context.environment));
    let mut prepared = attempt!(request.session.source.prepare(lookup.as_bytes(), &inputs, request.limits));
    if prepared.output() != outputs.as_slice() || prepared.submission().is_none() {
        return Err(at(crate::source::Stage::Check, contract.subject_span,
            "contract subject does not have the full ordered stack interface"));
    }
    let Some(submission) = prepared.submission.take() else {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check, "missing accepted contract subject submission",
        ));
    };
    let effects = submission.request.expected.allowed_effects.clone();
    if effects != noble_kernel::types::EffSet::empty() {
        return Err(at(crate::source::Stage::Check, contract.subject_span,
            "contract subject is not a pure definition"));
    }
    let mut source_dependencies = alloc::vec::Vec::new();
    for body in &submission.definitions {
        let index = usize::try_from(body.definition.0).map_err(|_| crate::source::declared::error(
            crate::source::Stage::Check, "subject dependency identity exceeds address space",
        ))?;
        let dependency_owner = submission.environment.definition_owners.get(index)
            .copied().flatten().ok_or_else(|| crate::source::declared::error(
                crate::source::Stage::Check, "subject dependency has no immutable module owner",
            ))?;
        if source_dependencies.iter().any(|known: &crate::intrinsic::ResolvedSourceModule| {
            known.owner == dependency_owner
        }) {
            continue;
        }
        let dependency = if dependency_owner == request.identity {
            crate::intrinsic::ResolvedSourceModule {
                owner: dependency_owner,
                module: alloc::string::String::from(request.name),
                version: request.version,
                full_source: request.source.to_vec(),
            }
        } else {
            let Some(module) = request.session.modules.iter().find(|module| module.identity == dependency_owner) else {
                return Err(crate::source::declared::error(
                    crate::source::Stage::Resolve, "subject dependency module is absent",
                ));
            };
            crate::intrinsic::ResolvedSourceModule {
                owner: dependency_owner,
                module: module.name.clone(),
                version: module.version,
                full_source: module.source.clone(),
            }
        };
        source_dependencies.push(dependency);
    }
    let named_uses = if contract.revision == 2 {
        attempt!(origin::trace(request, *index, &submission, &source_dependencies))
    } else {
        alloc::vec::Vec::new()
    };
    let named_imports = if contract.revision == 2 {
        attempt!(origin::imports(request, original.owner, &source_dependencies))
    } else {
        alloc::vec::Vec::new()
    };
    Ok(crate::intrinsic::SubjectBinding {
        module: alloc::string::String::from(original.module_name),
        version: original.version,
        module_source: original.module_source.to_vec(),
        definition: definition.name.clone(),
        definition_source: definition_source.to_vec(),
        definition_identity: definition.identity,
        definition_ordinal: original.ordinal,
        definition_owner: original.owner,
        named_uses,
        named_imports,
        accepted_submission: submission,
        input_types: inputs,
        output_types: outputs,
        effects,
        source_span: original.definition.span,
        source_dependencies,
    })
}

fn resolve_contract(
    request: &Request<'_>,
    staged: &[crate::source::declared::LogicalContract],
    reference: &str,
    span: crate::Span,
) -> Result<crate::intrinsic::ContractGoal, crate::source::Error> {
    let Some((scope, name)) = reference.split_once('.') else {
        return staged.iter().find(|contract| contract.name == reference)
            .map(|contract| contract.goal.clone())
            .ok_or_else(|| at(crate::source::Stage::Resolve, span, "unknown local proof contract"));
    };
    if is_current(request, scope) {
        return staged.iter().find(|contract| contract.name == name)
            .map(|contract| contract.goal.clone())
            .ok_or_else(|| at(crate::source::Stage::Resolve, span, "unknown local proof contract"));
    }
    let module = attempt!(imported_module(request.session, scope, span));
    module.contracts.iter().find(|contract| contract.name == name && contract.exported)
        .map(|contract| contract.goal.clone())
        .ok_or_else(|| at(crate::source::Stage::Resolve, span, "unknown or private imported contract"))
}

fn is_current(request: &Request<'_>, scope: &str) -> bool {
    scope.split_once('@').is_some_and(|(name, version)|
        name == request.name && version.parse::<u32>().ok() == Some(request.version))
}

fn imported_module<'a>(
    session: &'a crate::source::declared::ModuleSession,
    scope: &str,
    span: crate::Span,
) -> Result<&'a crate::source::declared::Module, crate::source::Error> {
    let module = if let Some((name, version)) = scope.split_once('@') {
        let version = version.parse::<u32>().ok();
        session.modules.iter().find(|module| module.name == name && Some(module.version) == version)
    } else {
        session.aliases.iter().rev().find(|alias| alias.spelling == scope)
            .and_then(|alias| session.modules.get(alias.module))
    };
    module.ok_or_else(|| at(crate::source::Stage::Resolve, span,
        "missing imported proof module version"))
}

fn resolve_uses(
    request: &Request<'_>,
    term: &mut crate::intrinsic::Form,
    prior: &[crate::intrinsic::ProofObligation],
    dependencies: &mut alloc::vec::Vec<crate::intrinsic::ProofDependency>,
) -> Result<(), crate::source::Error> {
    use crate::intrinsic::FormKind;
    let FormKind::List(items) = &mut term.kind else { return Ok(()); };
    if matches!(items.first().map(|form| &form.kind), Some(FormKind::Atom(name)) if name == "use") {
        if items.len() != 2 {
            return Err(at(crate::source::Stage::Resolve, term.span,
                "use requires one resolved proof name"));
        }
        let reference_span = items[1].span;
        let FormKind::Atom(reference) = &mut items[1].kind else {
            return Err(at(crate::source::Stage::Resolve, reference_span,
                "use requires a proof identifier"));
        };
        if let Some((scope, name)) = reference.split_once('.') {
            let (scope, name) = (alloc::string::String::from(scope), alloc::string::String::from(name));
            if is_current(request, &scope) {
                if !prior.iter().any(|proof| proof.name == name) {
                    return Err(at(crate::source::Stage::Resolve, reference_span,
                        "forward or cyclic local proof use"));
                }
                *reference = name;
            } else {
                let module = attempt!(imported_module(request.session, &scope, reference_span));
                let Some(published) = module.proofs.iter().find(|proof| proof.reference.name == name && proof.reference.exported) else {
                    return Err(at(crate::source::Stage::Resolve, reference_span,
                        "unknown or private imported proof"));
                };
                if !dependencies.iter().any(|proof| proof.name == *reference) {
                    let mut dependency = published.reference.clone();
                    dependency.name = reference.clone();
                    dependencies.push(dependency);
                }
            }
        } else if !prior.iter().any(|proof| proof.name == *reference) {
            return Err(at(crate::source::Stage::Resolve, reference_span,
                "unknown, forward or cyclic proof use"));
        }
        return Ok(());
    }
    for item in items {
        attempt!(resolve_uses(request, item, prior, dependencies));
    }
    Ok(())
}
