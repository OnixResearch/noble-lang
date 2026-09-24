pub(super) fn accepted<T, E: core::fmt::Debug>(result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| format!("{error:?}"))
}

pub(super) fn limits() -> noble_kernel::async_tasks::Limits {
    noble_kernel::async_tasks::Limits {
        tasks: 2,
        terminal_results: 2,
        bytes: 128,
        parked_payloads: 2,
        pins: 4,
        wakeups: 4,
        retirement_work: 32,
        generations: 16,
    }
}

pub(super) fn request() -> noble_kernel::async_tasks::Request {
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

pub(super) fn completion() -> noble_kernel::async_tasks::Completion {
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

pub(super) fn table() -> Result<noble_kernel::async_tasks::Table, String> {
    accepted(noble_kernel::async_tasks::Table::new(
        noble_kernel::async_tasks::TableId(3),
        limits(),
    ))
}
