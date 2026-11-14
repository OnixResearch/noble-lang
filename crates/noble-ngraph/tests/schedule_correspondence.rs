#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::ambiguous_params,
    tigerstyle::assertion_density,
    tigerstyle::clone_then_borrow,
    tigerstyle::explicit_defaults,
    tigerstyle::fallible_int_fallback,
    tigerstyle::missing_const_fn,
    tigerstyle::no_unwrap,
    tigerstyle::no_panic,
    tigerstyle::non_trait_imports,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness builds checked graph fixtures, validates emitted schedules and mutates one obligation at a time, so terse fixture names, unwraps and assertion panics are the readable form."
)]

#[path = "schedule_correspondence_dir/base.rs"]
mod base;
#[path = "belt_dir/corpus.rs"]
mod belt_corpus;
#[path = "belt_dir/correspondence.rs"]
mod belt_correspondence;
#[path = "belt_dir/fixtures.rs"]
mod belt_fixtures;
#[path = "belt_dir/measure.rs"]
mod belt_measure;
#[path = "belt_dir/mutations.rs"]
mod belt_mutations;
#[path = "belt_target_dir/mod.rs"]
mod belt_target;
#[path = "schedule_lean_dir/corpus.rs"]
mod corpus;
#[path = "schedule_lean_dir/corpus_guard.rs"]
mod corpus_guard;
#[path = "schedule_lean_dir/corpus_shared.rs"]
mod corpus_shared;
#[path = "schedule_lean_dir/emit.rs"]
mod emit;
#[path = "schedule_lean_dir/emit_guard.rs"]
mod emit_guard;
#[path = "schedule_guards_dir/guards.rs"]
mod guards;
#[path = "schedule_cost_dir/limits.rs"]
mod limits;
#[path = "schedule_cost_dir/measure.rs"]
mod measure;
#[path = "schedule_lean_dir/pairs.rs"]
mod pairs;
use base::{declared, interleaved, packed};
mod common;
