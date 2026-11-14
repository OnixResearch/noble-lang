#![no_std]

extern crate alloc;

pub struct Environment {
    pub deps: alloc::vec::Vec<alloc::vec::Vec<u8>>,
}

pub fn append_empty_dependencies(mut env: Environment, rows: usize) -> Environment {
    env.deps
        .extend(core::iter::repeat_with(alloc::vec::Vec::new).take(rows));
    env
}
