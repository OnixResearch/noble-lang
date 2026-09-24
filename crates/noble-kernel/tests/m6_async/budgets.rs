#[test]
fn each_admission_budget_rejects_without_consuming_supplied_obligations() -> Result<(), String> {
    struct Obligation(u64);
    let mut cases = [(
        crate::support::limits(),
        noble_kernel::async_tasks::Error::TerminalCapacity,
    ); 6];
    cases[0].0.terminal_results = 0;
    cases[1] = (
        noble_kernel::async_tasks::Limits {
            bytes: 27,
            ..crate::support::limits()
        },
        noble_kernel::async_tasks::Error::ByteCapacity,
    );
    cases[2] = (
        noble_kernel::async_tasks::Limits {
            parked_payloads: 0,
            ..crate::support::limits()
        },
        noble_kernel::async_tasks::Error::ParkedCapacity,
    );
    cases[3] = (
        noble_kernel::async_tasks::Limits {
            pins: 1,
            ..crate::support::limits()
        },
        noble_kernel::async_tasks::Error::PinCapacity,
    );
    cases[4] = (
        noble_kernel::async_tasks::Limits {
            wakeups: 1,
            ..crate::support::limits()
        },
        noble_kernel::async_tasks::Error::WakeCapacity,
    );
    cases[5] = (
        noble_kernel::async_tasks::Limits {
            retirement_work: 8,
            ..crate::support::limits()
        },
        noble_kernel::async_tasks::Error::RetirementCapacity,
    );
    for (bound, error) in cases {
        let mut table = crate::support::accepted(noble_kernel::async_tasks::Table::new(
            noble_kernel::async_tasks::TableId(1),
            bound,
        ))?;
        let rejected = table
            .admit(crate::support::request(), Obligation(91))
            .err()
            .ok_or("admitted over budget")?;
        assert_eq!(rejected.error, error);
        assert_eq!(rejected.input.0, 91);
        assert_eq!(
            table.observation().reserved,
            noble_kernel::async_tasks::Footprint::empty()
        );
        assert_eq!(table.observation().pending, 0);
    }
    Ok(())
}

#[test]
fn abandoned_preparation_does_not_consume_capacity_or_generation() -> Result<(), String> {
    let mut bound = crate::support::limits();
    bound.generations = 1;
    let mut table = crate::support::accepted(noble_kernel::async_tasks::Table::new(
        noble_kernel::async_tasks::TableId(1),
        bound,
    ))?;
    {
        let _preparation = crate::support::accepted(table.prepare(crate::support::request()))?;
    }
    assert_eq!(
        table.observation().reserved,
        noble_kernel::async_tasks::Footprint::empty()
    );
    let admitted = crate::support::accepted(table.prepare(crate::support::request()))?.commit(37);
    assert_eq!(admitted.inputs, 37);
    assert_eq!(admitted.callback.task.generation, 1);
    let rejected = table
        .admit(crate::support::request(), 41)
        .err()
        .ok_or("generation wrapped")?;
    assert_eq!(
        rejected.error,
        noble_kernel::async_tasks::Error::GenerationExhausted
    );
    assert_eq!(rejected.input, 41);
    Ok(())
}
