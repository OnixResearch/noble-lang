#![feature(register_tool)]
#![register_tool(tigerstyle)]

use noble_kernel::contracts::{Behavior, Definition, Env};
use noble_kernel::shapes::Pattern;
use noble_kernel::types::{EffId, EffSet, Ty};
use noble_kernel::universal::{self, CheckedNodeKind, Dependency, SchemeError, SourceStamp,
    SymBinding, SymStack, SymTy, SymbolicCandidate, SymbolicInst, SymbolicNode};
use noble_kernel::untrusted::{Expected, Limits, Lit, NodeId, Request};
use noble_kernel::words::{Binding, Inst, Scheme, Variable, VariableKind};

fn stamp(index:u8) -> SourceStamp {
    SourceStamp {source_tag:[index;32],grant_epoch:3,generation:u64::from(index)}
}
fn stack(suffix:Vec<SymTy>) -> SymStack { SymStack {prefix:true,suffix} }
fn closed(value:Ty) -> SymTy { SymTy::Concrete(value) }
fn i64_stack() -> SymStack { stack(vec![closed(Ty::I64)]) }
fn scheme(input:Vec<Pattern>,output:Vec<Pattern>) -> Scheme {
    let mut stack_in = vec![Pattern::StackVar(Variable(0))];
    stack_in.extend(input);
    let mut stack_out = vec![Pattern::StackVar(Variable(0))];
    stack_out.extend(output);
    Scheme {var_kinds:vec![VariableKind::Stack],stack_in,stack_out,effects:vec![]}
}
fn insert(env:&mut Env,decl:Scheme,deps:Vec<Definition>) -> Definition {
    let id = Definition(env.defs.len() as u32);
    env.defs.push(decl);
    env.kinds.push(Behavior::Named);
    env.deps.push(deps);
    env.definition_owners.push(Some(7));
    id
}
fn environment() -> Result<(Env,Definition),String> {
    let mut env = noble_kernel::contracts::environment().map_err(|e|format!("{e:?}"))?;
    env.caller_module = Some(7);
    env.declared_modules = true;
    let owner = insert(&mut env,scheme(vec![Pattern::I64],vec![Pattern::I64]),vec![]);
    Ok((env,owner))
}
fn request() -> Request {
    Request {
        input_bytes:128,
        expected:Expected {stack_in:vec![],stack_out:vec![],allowed_effects:EffSet::empty()},
        limits:Limits {bytes:4096,nodes:4096,depth:32,type_size:256,
            stack_height:256,work:4096,diagnostics:64},
    }
}
fn recursive(owner:Definition) -> SymbolicCandidate {
    SymbolicCandidate {
        nodes:vec![
            SymbolicNode::Quotation {body:vec![NodeId(1)],input:i64_stack(),
                output:i64_stack(),effects:EffSet::empty()},
            SymbolicNode::SignedSelf {def:owner},
            SymbolicNode::Invocation {
                def:Definition(10),
                inst:SymbolicInst {bindings:vec![
                    SymBinding::Stack(i64_stack()),SymBinding::Stack(i64_stack()),
                    SymBinding::Effect(EffSet::empty()),
                ]},
            },
        ],
        body:vec![NodeId(0),NodeId(2)],
    }
}

#[test]
fn one_symbolic_quote_with_recursive_self_checks_distinct_concrete_rows() -> Result<(),String> {
    let (env,owner)=environment()?;
    let seal=universal::check_scheme(&env,&request(),&recursive(owner),owner,stamp(1),&[])
        .map_err(|e|format!("symbolic quotation must validate: {e:?}"))?;
    for row in [vec![],vec![Ty::Bool]] {
        let instance=universal::instantiate_checked(&seal,&env,stamp(1),
            &Inst {bindings:vec![Binding::Stack(row.clone())]},request().limits,&[])
            .map_err(|e|format!("exact concrete row must instantiate: {e:?}"))?;
        let mut owner_input=row.clone();
        owner_input.push(Ty::I64);
        assert_eq!(instance.interface.stack_in,owner_input);
        assert_eq!(instance.interface.stack_out,owner_input);
        assert_eq!(instance.nodes.len(),3);
        assert_eq!(instance.nodes[0].id,NodeId(0));
        let mut quoted_output=owner_input.clone();
        quoted_output.push(Ty::program(owner_input.clone(),owner_input.clone(),EffSet::empty()));
        assert_eq!(instance.nodes[0].interface.stack_out,quoted_output,
            "quotation's concrete Program ABI must retain exactly the same S");
        let CheckedNodeKind::SignedSelf {def,inst}=&instance.nodes[1].kind else {
            return Err("quoted self must remain a checked owner occurrence".to_string());
        };
        assert_eq!(*def,owner);
        assert_eq!(inst.bindings.len(),1);
        assert!(matches!(&inst.bindings[0],Binding::Stack(actual) if actual==&row),
            "self may not instantiate owner S to any other row");
        let CheckedNodeKind::Invocation {def,inst}=&instance.nodes[2].kind else {
            return Err("run must retain its checked exact invocation binders".to_string());
        };
        assert_eq!(*def,Definition(10));
        assert!(matches!(&inst.bindings[0],Binding::Stack(actual) if actual==&owner_input));
    }
    Ok(())
}

#[test]
fn nested_pair_sum_and_list_retain_the_row_of_quoted_programs() -> Result<(),String> {
    let (env,owner)=environment()?;
    let program=SymTy::Program {
        input:i64_stack(),output:i64_stack(),effects:EffSet::empty(),
    };
    for (name,constructor,aggregate) in [
        (
            "pair",
            vec![
                SymbolicNode::Invocation {def:Definition(0),inst:SymbolicInst {bindings:vec![
                    SymBinding::Stack(i64_stack()),SymBinding::Value(program.clone()),
                ]}},
                SymbolicNode::Invocation {def:Definition(13),inst:SymbolicInst {bindings:vec![
                    SymBinding::Stack(i64_stack()),SymBinding::Value(program.clone()),
                    SymBinding::Value(program.clone()),
                ]}},
            ],
            SymTy::Pair(Box::new(program.clone()),Box::new(program.clone())),
        ),
        (
            "sum",
            vec![SymbolicNode::Invocation {def:Definition(15),inst:SymbolicInst {bindings:vec![
                SymBinding::Stack(i64_stack()),SymBinding::Value(program.clone()),
                SymBinding::Value(closed(Ty::I64)),
            ]}}],
            SymTy::Sum(Box::new(program.clone()),Box::new(closed(Ty::I64))),
        ),
        (
            "list",
            vec![
                SymbolicNode::Invocation {def:Definition(19),inst:SymbolicInst {bindings:vec![
                    SymBinding::Stack(stack(vec![closed(Ty::I64),program.clone()])),
                    SymBinding::Value(program.clone()),
                ]}},
                SymbolicNode::Invocation {def:Definition(20),inst:SymbolicInst {bindings:vec![
                    SymBinding::Stack(i64_stack()),SymBinding::Value(program.clone()),
                ]}},
            ],
            SymTy::List(Box::new(program.clone())),
        ),
    ] {
        let mut candidate=recursive(owner);
        candidate.nodes.pop(); // The quoted self remains, but run is replaced.
        candidate.body=vec![NodeId(0)];
        for node in constructor {
            candidate.body.push(NodeId(candidate.nodes.len() as u32));
            candidate.nodes.push(node);
        }
        candidate.body.push(NodeId(candidate.nodes.len() as u32));
        candidate.nodes.push(SymbolicNode::Invocation {
            def:Definition(1),inst:SymbolicInst {bindings:vec![
                SymBinding::Stack(i64_stack()),SymBinding::Value(aggregate),
            ]},
        });
        let seal=universal::check_scheme(&env,&request(),&candidate,owner,stamp(1),&[])
            .map_err(|e|format!("{name} of Program<S> must check symbolically: {e:?}"))?;
        for row in [vec![],vec![Ty::Bool]] {
            let checked=universal::instantiate_checked(&seal,&env,stamp(1),
                &Inst{bindings:vec![Binding::Stack(row.clone())]},request().limits,&[])
                .map_err(|e|format!("{name} of Program<S> must retain row {row:?}: {e:?}"))?;
            let mut expected=row;
            expected.push(Ty::I64);
            assert_eq!(checked.interface.stack_out,expected);
        }
    }
    Ok(())
}

#[test]
fn returned_work_spend_is_the_exact_original_check_boundary() -> Result<(),String> {
    let (env,owner)=environment()?;
    let body=recursive(owner);
    let original=request();
    let seal=universal::check_scheme(&env,&original,&body,owner,stamp(1),&[])
        .map_err(|e|format!("symbolic check should succeed: {e:?}"))?;
    let mut at=original.clone();
    at.limits.work=seal.work_spent();
    universal::check_scheme(&env,&at,&body,owner,stamp(1),&[])
        .map_err(|e|format!("exact symbolic work allowance should succeed: {e:?}"))?;
    let Some(below)=at.limits.work.checked_sub(1) else {
        return Err("successful symbolic body cannot spend zero work".to_string());
    };
    at.limits.work=below;
    let exhausted=universal::check_scheme(&env,&at,&body,owner,stamp(1),&[]);
    assert!(matches!(&exhausted,Err(SchemeError::WorkExhausted)),
        "one below actual symbolic debit must fail: {exhausted:?}");
    let witness=Inst{bindings:vec![Binding::Stack(vec![Ty::Bool])]};
    let checked=universal::instantiate_checked(
        &seal,&env,stamp(1),&witness,original.limits,&[])
        .map_err(|e|format!("concrete row should succeed: {e:?}"))?;
    let mut limits=original.limits;
    limits.work=checked.work_spent();
    universal::instantiate_checked(&seal,&env,stamp(1),&witness,limits,&[])
        .map_err(|e|format!("exact concrete work allowance should succeed: {e:?}"))?;
    let Some(below)=limits.work.checked_sub(1) else {
        return Err("successful concrete ABI cannot spend zero work".to_string());
    };
    limits.work=below;
    let exhausted=universal::instantiate_checked(
        &seal,&env,stamp(1),&witness,limits,&[]);
    assert!(matches!(&exhausted,Err(SchemeError::WorkExhausted)),
        "one below actual concrete debit must fail: {exhausted:?}");
    Ok(())
}

#[test]
fn nominal_and_generic_constructor_revocation_invalidates_checked_uses()
    -> Result<(),String> {
    use noble_kernel::contracts::{GenericVariantDecl,NominalDecl};
    use noble_kernel::types::{NominalShape,NominalTypeId};
    let nominal_id=NominalTypeId {module:8,ordinal:1};
    let nominal_decl=NominalDecl {
        id:nominal_id,shape:NominalShape::Opaque(Box::new(Ty::I64)),
        exported:true,public:[true,true],
    };
    let base=noble_kernel::contracts::environment().map_err(|e|format!("{e:?}"))?;
    let (mut env,ops)=base.declare_nominal(nominal_decl)
        .map_err(|e|format!("nominal registration: {e:?}"))?;
    env.caller_module=Some(7);
    let owner=insert(&mut env,scheme(vec![Pattern::I64],vec![Pattern::I64]),vec![]);
    let new=ops.new.ok_or("missing nominal new")?;
    let into=ops.into.ok_or("missing nominal into")?;
    let nominal_body=SymbolicCandidate {
        nodes:vec![new,into].into_iter().map(|def|
            SymbolicNode::Invocation {def,inst:SymbolicInst {bindings:vec![
                SymBinding::Stack(stack(vec![])),
            ]}}
        ).collect(),
        body:vec![NodeId(0),NodeId(1)],
    };
    let nominal_seal=universal::check_scheme(&env,&request(),&nominal_body,owner,stamp(1),&[])
        .map_err(|e|format!("public nominal constructor should check: {e:?}"))?;
    let use_owner=Inst{bindings:vec![Binding::Stack(vec![Ty::Bool])]};
    universal::instantiate_checked(&nominal_seal,&env,stamp(1),&use_owner,request().limits,&[])
        .map_err(|e|format!("public nominal constructor should instantiate: {e:?}"))?;
    let importing=insert(&mut env,scheme(vec![Pattern::I64],vec![Pattern::I64]),vec![owner]);
    let import_body=SymbolicCandidate {
        nodes:vec![SymbolicNode::Invocation {
            def:owner,inst:SymbolicInst {bindings:vec![SymBinding::Stack(stack(vec![]))]},
        }],
        body:vec![NodeId(0)],
    };
    let imported=universal::check_scheme(&env,&request(),&import_body,importing,stamp(3),
        &[Dependency{seal:&nominal_seal,current_stamp:stamp(1)}])
        .map_err(|e|format!("sealed nominal-dependent owner should check: {e:?}"))?;
    universal::instantiate_checked(&imported,&env,stamp(3),&use_owner,request().limits,
        &[Dependency{seal:&nominal_seal,current_stamp:stamp(1)}])
        .map_err(|e|format!("sealed nominal-dependent owner should instantiate: {e:?}"))?;
    env.nominals[0].public[0]=false;
    let revoked=universal::instantiate_checked(
        &nominal_seal,&env,stamp(1),&use_owner,request().limits,&[]);
    assert!(matches!(&revoked,Err(SchemeError::StaleEnvironment)),
        "public→private nominal new must invalidate sealed invocation: {revoked:?}");
    let stale_import=universal::instantiate_checked(&imported,&env,stamp(3),
        &use_owner,request().limits,
        &[Dependency{seal:&nominal_seal,current_stamp:stamp(1)}]);
    assert!(matches!(&stale_import,Err(SchemeError::StaleEnvironment)),
        "nominal revocation must also invalidate transitive checked consumers: {stale_import:?}");

    let generic_id=NominalTypeId {module:8,ordinal:2};
    let family=GenericVariantDecl {
        id:generic_id,payload_params:[0,1],exported:true,public:[true,true],
    };
    let base=noble_kernel::contracts::environment().map_err(|e|format!("{e:?}"))?;
    let (mut env,ops)=base.declare_generic_variant(family)
        .map_err(|e|format!("generic registration: {e:?}"))?;
    env.caller_module=Some(7);
    let owner=insert(&mut env,scheme(vec![Pattern::I64],vec![]),vec![]);
    let left=ops.left.ok_or("missing generic left")?;
    let instance=env.generic_instance(generic_id,[Ty::I64,Ty::Bool])
        .ok_or("missing generic type")?;
    let generic_body=SymbolicCandidate {
        nodes:vec![
            SymbolicNode::Invocation {def:left,inst:SymbolicInst {bindings:vec![
                SymBinding::Stack(stack(vec![])),SymBinding::Value(closed(Ty::I64)),
                SymBinding::Value(closed(Ty::Bool)),
            ]}},
            SymbolicNode::Invocation {def:Definition(1),inst:SymbolicInst {bindings:vec![
                SymBinding::Stack(stack(vec![])),SymBinding::Value(closed(instance)),
            ]}},
        ],
        body:vec![NodeId(0),NodeId(1)],
    };
    let generic_seal=universal::check_scheme(&env,&request(),&generic_body,owner,stamp(2),&[])
        .map_err(|e|format!("public generic constructor should check: {e:?}"))?;
    universal::instantiate_checked(&generic_seal,&env,stamp(2),&use_owner,request().limits,&[])
        .map_err(|e|format!("public generic constructor should instantiate: {e:?}"))?;
    let symbolic_program=SymTy::Program {
        input:i64_stack(),output:stack(vec![]),effects:EffSet::empty(),
    };
    let symbolic_generic=SymbolicCandidate {
        nodes:vec![
            SymbolicNode::Quotation {body:vec![NodeId(1)],input:i64_stack(),
                output:stack(vec![]),effects:EffSet::empty()},
            SymbolicNode::SignedSelf {def:owner},
            SymbolicNode::Invocation {def:left,inst:SymbolicInst {bindings:vec![
                SymBinding::Stack(i64_stack()),SymBinding::Value(symbolic_program),
                SymBinding::Value(closed(Ty::Bool)),
            ]}},
        ],
        body:vec![NodeId(0),NodeId(2)],
    };
    let unsafe_row=universal::check_scheme(
        &env,&request(),&symbolic_generic,owner,stamp(2),&[]);
    assert!(matches!(&unsafe_row,Err(SchemeError::UnsupportedShape)),
        "generic argument Program<S> must not be materialized at empty S: {unsafe_row:?}");
    env.generic_variants[0].public[0]=false;
    let revoked=universal::instantiate_checked(
        &generic_seal,&env,stamp(2),&use_owner,request().limits,&[]);
    assert!(matches!(&revoked,Err(SchemeError::StaleEnvironment)),
        "public→private generic left must invalidate sealed invocation: {revoked:?}");
    Ok(())
}

#[test]
fn self_cannot_rebind_s_inside_a_quotation() -> Result<(),String> {
    let (env,owner)=environment()?;
    let mut candidate=recursive(owner);
    if let SymbolicNode::Quotation {input,output,..}=&mut candidate.nodes[0] {
        *input=stack(vec![closed(Ty::Bool),closed(Ty::I64)]);
        *output=input.clone();
    }
    let observed=universal::check_scheme(&env,&request(),&candidate,owner,stamp(1),&[]);
    assert!(matches!(&observed,Err(SchemeError::StackJoin)),
        "self cannot treat the extra Bool as part of a new S: {observed:?}");
    Ok(())
}

#[test]
fn symbolic_check_rejects_opaque_row_inspection_unknown_effects_and_unvisited_nodes()
    -> Result<(),String> {
    let (env,owner)=environment()?;
    let drop_s=SymbolicCandidate {
        nodes:vec![SymbolicNode::Invocation {def:Definition(1),inst:SymbolicInst {
            bindings:vec![SymBinding::Stack(SymStack {prefix:false,suffix:vec![]}),
                SymBinding::Value(closed(Ty::I64))],
        }}],body:vec![NodeId(0)],
    };
    let rejected=universal::check_scheme(&env,&request(),&drop_s,owner,stamp(1),&[]);
    assert!(matches!(&rejected,Err(SchemeError::OutputJoin)),
        "a pure drop cannot silently erase an owner signature suffix: {rejected:?}");
    let mut unknown=recursive(owner);
    if let SymbolicNode::Invocation {inst,..}=&mut unknown.nodes[2] {
        inst.bindings[2]=SymBinding::Effect(EffSet::from_ids(&[EffId(99)]));
    }
    let rejected=universal::check_scheme(&env,&request(),&unknown,owner,stamp(1),&[]);
    assert!(matches!(&rejected,Err(SchemeError::UnknownEffect)),
        "an unknown latent effect may not be erased by a pure owner: {rejected:?}");
    let mut dangling=recursive(owner);
    dangling.nodes.push(SymbolicNode::Literal(Lit::Bool(false)));
    let rejected=universal::check_scheme(&env,&request(),&dangling,owner,stamp(1),&[]);
    assert!(matches!(&rejected,Err(SchemeError::UnreachableNode(NodeId(3)))),
        "every candidate arena node needs one independent symbolic derivation: {rejected:?}");
    Ok(())
}

#[test]
fn owner_source_dependency_and_definition_rows_remain_fresh_after_append() -> Result<(),String> {
    let (mut env,first)=environment()?;
    let dep_candidate=SymbolicCandidate {
        nodes:vec![SymbolicNode::Literal(Lit::I64(4))],body:vec![NodeId(0)],
    };
    // The first owner has I64 input/output; a second pure source owner is
    // installed after its seal, proving unrelated append cannot stale it.
    let first_seal=universal::check_scheme(&env,&request(),&recursive(first),first,stamp(1),&[])
        .map_err(|e|format!("first owner must validate: {e:?}"))?;
    let dep=insert(&mut env,scheme(vec![],vec![Pattern::I64]),vec![]);
    let dep_seal=universal::check_scheme(&env,&request(),&dep_candidate,dep,stamp(2),&[])
        .map_err(|e|format!("dependency body must validate: {e:?}"))?;
    let owner=insert(&mut env,scheme(vec![],vec![Pattern::I64]),vec![dep]);
    let body=SymbolicCandidate {nodes:vec![SymbolicNode::Invocation {def:dep,
        inst:SymbolicInst {bindings:vec![SymBinding::Stack(stack(vec![]))]} }],
        body:vec![NodeId(0)]};
    let proof=Dependency {seal:&dep_seal,current_stamp:stamp(2)};
    let owner_seal=universal::check_scheme(&env,&request(),&body,owner,stamp(3),&[proof])
        .map_err(|e|format!("bound named dependency must validate: {e:?}"))?;
    let instance=universal::instantiate_checked(&owner_seal,&env,stamp(3),
        &Inst{bindings:vec![Binding::Stack(vec![Ty::Bool])]},request().limits,
        &[Dependency {seal:&dep_seal,current_stamp:stamp(2)}])
        .map_err(|e|format!("Named ABI must instantiate: {e:?}"))?;
    assert_eq!(instance.nodes[0].interface.stack_in,vec![Ty::Bool]);
    assert_eq!(instance.nodes[0].interface.stack_out,vec![Ty::Bool,Ty::I64]);
    let stale=universal::instantiate_checked(&owner_seal,&env,stamp(3),
        &Inst{bindings:vec![Binding::Stack(vec![])]},request().limits,
        &[Dependency{seal:&dep_seal,current_stamp:stamp(9)}]);
    assert!(matches!(&stale,Err(SchemeError::StaleSource)),
        "revoked dependency stamp must not authorize compiled reuse: {stale:?}");
    let missing=universal::check_scheme(&env,&request(),&body,owner,stamp(3),&[]);
    assert!(matches!(&missing,Err(SchemeError::InvalidDependency(def)) if *def==dep),
        "Named Env metadata without a sealed body is not authority");
    let _unrelated=insert(&mut env,scheme(vec![],vec![]),vec![]);
    universal::instantiate_checked(&first_seal,&env,stamp(1),
        &Inst{bindings:vec![Binding::Stack(vec![])]},request().limits,&[])
        .map_err(|e|format!("unrelated appended definition must preserve older seal: {e:?}"))?;
    let mut revoked_grant=stamp(3);
    revoked_grant.grant_epoch+=1;
    let blocked=universal::instantiate_checked(&owner_seal,&env,revoked_grant,
        &Inst{bindings:vec![Binding::Stack(vec![])]},request().limits,
        &[Dependency{seal:&dep_seal,current_stamp:stamp(2)}]);
    assert!(matches!(&blocked,Err(SchemeError::StaleSource)),
        "a changed caller-supplied grant tag must not match the retained tag: {blocked:?}");
    env.defs[dep.0 as usize]=scheme(vec![Pattern::Bool],vec![Pattern::I64]);
    let changed=universal::instantiate_checked(&owner_seal,&env,stamp(3),
        &Inst{bindings:vec![Binding::Stack(vec![])]},request().limits,
        &[Dependency{seal:&dep_seal,current_stamp:stamp(2)}]);
    assert!(matches!(&changed,Err(SchemeError::StaleEnvironment)),
        "retargeting used dependency ABI must invalidate the checked graph: {changed:?}");
    Ok(())
}

#[test]
fn wrong_owner_effect_and_structural_meter_reject_before_sealing() -> Result<(),String> {
    let (mut env,owner)=environment()?;
    env.caller_module=Some(8);
    let wrong=universal::check_scheme(&env,&request(),&recursive(owner),owner,stamp(1),&[]);
    assert!(matches!(wrong,Err(SchemeError::InvalidOwner)));
    env.caller_module=Some(7);
    let mut impure=recursive(owner);
    if let SymbolicNode::Quotation {effects,..}=&mut impure.nodes[0] {
        *effects=EffSet::from_ids(&[noble_kernel::contracts::TEST_EMIT]);
    }
    let bad=universal::check_scheme(&env,&request(),&impure,owner,stamp(1),&[]);
    assert!(matches!(bad,Err(SchemeError::EffectMismatch)));
    let mut shallow=request();
    shallow.limits.depth=0;
    let depth=universal::check_scheme(&env,&shallow,&recursive(owner),owner,stamp(1),&[]);
    assert!(matches!(depth,Err(SchemeError::DepthExhausted)));
    let mut tight=request();
    tight.limits.work=1;
    let work=universal::check_scheme(&env,&tight,&recursive(owner),owner,stamp(1),&[]);
    assert!(matches!(work,Err(SchemeError::WorkExhausted)));
    Ok(())
}
