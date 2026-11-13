#[test]
fn memberships_are_rejected_without_consuming_admission() -> Result<(), String> {
    let mut table = super::super::support::table(1)?;
    let registry = super::super::registry::ScopeRegistry::shared();
    let mut source = super::super::manager::Scope::new(
        super::super::manager::ScopeId(1),
        noble_kernel::async_tasks::TableId(3),
        std::rc::Rc::clone(&registry),
    );
    let mut foreign_scope = super::super::manager::Scope::new(
        super::super::manager::ScopeId(2),
        noble_kernel::async_tasks::TableId(3),
        std::rc::Rc::clone(&registry),
    );
    let admission = source
        .admit(&mut table, super::super::support::request(), 1_u8)
        .map_err(|failure| format!("{failure:?}"))?;
    let mut admission = Some(admission);
    let failure = match foreign_scope.attach(&table, &mut admission) {
        Ok(_) => return Err("foreign scope attachment succeeded".to_owned()),
        Err(failure) => failure,
    };
    assert_eq!(failure, super::super::manager::ScopeError::ForeignScope);
    let admission = admission.ok_or("foreign scope refusal consumed the admission")?;
    let member = super::super::scope_support::attach(&mut source, &table, admission)?;
    assert_eq!(source.members(), vec![member]);
    assert!(foreign_scope.members().is_empty());
    table_mismatch_does_not_consume_admission(&registry)
}

fn table_mismatch_does_not_consume_admission(
    registry: &std::rc::Rc<std::cell::RefCell<super::super::registry::ScopeRegistry<u8>>>,
) -> Result<(), String> {
    let foreign_table_id = noble_kernel::async_tasks::TableId(4);
    let mut foreign_table = noble_kernel::async_tasks::Table::new(
        foreign_table_id,
        noble_kernel::async_tasks::Limits {
            tasks: 1,
            terminal_results: 2,
            bytes: 128,
            parked_payloads: 2,
            pins: 4,
            wakeups: 4,
            retirement_work: 32,
            generations: 16,
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut same_identity_wrong_table = super::super::manager::Scope::new(
        super::super::manager::ScopeId(3),
        noble_kernel::async_tasks::TableId(4),
        std::rc::Rc::clone(registry),
    );
    let mut target = super::super::manager::Scope::new(
        super::super::manager::ScopeId(3),
        noble_kernel::async_tasks::TableId(3),
        std::rc::Rc::clone(registry),
    );
    let admission = same_identity_wrong_table
        .admit(&mut foreign_table, super::super::support::request(), 2_u8)
        .map_err(|failure| format!("{failure:?}"))?;
    let mut admission = Some(admission);
    let failure = match target.attach(&foreign_table, &mut admission) {
        Ok(_) => return Err("foreign table attachment succeeded".to_owned()),
        Err(failure) => failure,
    };
    assert_eq!(failure, super::super::manager::ScopeError::ForeignTable);
    let admission = admission.ok_or("foreign table refusal consumed the admission")?;
    let retained = super::super::scope_support::attach(
        &mut same_identity_wrong_table,
        &foreign_table,
        admission,
    )?;
    assert_eq!(same_identity_wrong_table.members(), vec![retained]);
    assert!(target.members().is_empty());
    Ok(())
}
