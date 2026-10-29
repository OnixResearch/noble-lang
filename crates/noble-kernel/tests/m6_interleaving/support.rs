//! Shared fixtures for the issue #46 negative controls; the limits and request shapes mirror the
//! M6 input planes so a control never depends on a fixture the table's own tests do not use.

pub(super) fn accepted<T, E: core::fmt::Debug>(result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| format!("{error:?}"))
}

const fn limits(tasks: usize) -> noble_kernel::async_tasks::Limits {
    noble_kernel::async_tasks::Limits {
        tasks,
        terminal_results: 2,
        bytes: 128,
        parked_payloads: 2,
        pins: 4,
        wakeups: 4,
        retirement_work: 32,
        generations: 16,
    }
}

pub(super) const fn request() -> noble_kernel::async_tasks::Request {
    noble_kernel::async_tasks::Request {
        context: noble_kernel::async_tasks::Context(7),
        native: noble_kernel::async_tasks::NativeId(11),
        inputs: 3,
        results: 2,
        input_bytes: 8,
        result_bytes: 16,
        parked_bytes: 4,
        pins: 2,
        wakeups: 2,
    }
}

pub(super) const fn completion() -> noble_kernel::async_tasks::Completion {
    noble_kernel::async_tasks::Completion {
        inputs: noble_kernel::async_tasks::Disposition {
            returned: 1,
            consumed: 2,
            retired: 4,
        },
        produced: 3,
        bytes: 12,
    }
}

pub(super) fn table(tasks: usize) -> Result<noble_kernel::async_tasks::Table, String> {
    accepted(noble_kernel::async_tasks::Table::new(
        noble_kernel::async_tasks::TableId(3),
        limits(tasks),
    ))
}
