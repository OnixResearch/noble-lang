#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    tigerstyle::assertion_density,
    tigerstyle::ambiguous_params,
    tigerstyle::fallible_int_fallback,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; this integration harness reads frozen fixtures and prices their measured costs, so terse fixture names, unwraps and long setup legs are the readable form."
)]
mod common;

#[path = "rewrite_quality/acceptance.rs"]
mod acceptance;

#[test]
fn quality_acceptance_is_separate_from_certificate_validity(
) -> Result<(), Box<dyn std::error::Error>> {
    acceptance::run()
}
