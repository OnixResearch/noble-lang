//! Substitution must refuse an oversized result before it exists.
//!
//! A `GenericNominal` pattern copies both substituted arguments into the
//! resulting descriptor, so one nesting level doubles the node count. The
//! pattern itself stays small, which is why the environment check accepts it:
//! forty levels are 81 pattern nodes but about 2^42 substituted nodes. This
//! control states that shape and requires the sized refusal, not an allocation
//! the process cannot survive.

use noble_kernel::contracts::{Definition, Env, GenericVariantDecl};
use noble_kernel::shapes::Pattern;
use noble_kernel::types::{NominalTypeId, Ty};
use noble_kernel::untrusted::LimitKind;

use super::builders::{self, Seen};

/// Nesting depth of the oversized control: 81 pattern nodes, 2^42 result nodes.
const OVERSIZED_DEPTH: u32 = 40;
/// Nesting depth of the positive control: a small result well inside the cap.
const SMALL_DEPTH: u32 = 3;

fn family() -> GenericVariantDecl {
    GenericVariantDecl {
        id: NominalTypeId {
            module: 91,
            ordinal: 0,
        },
        payload_params: [0, 1],
        exported: true,
        public: [true, true],
    }
}

/// One nested generic pattern: `P0 = Unit`, `Pk = G[P(k-1), Unit]`.
fn nested_pattern(depth: u32) -> Pattern {
    let mut pattern = Pattern::Unit;
    let mut index = 0;
    while index < depth {
        pattern = Pattern::GenericNominal(family().id, Box::new([pattern, Pattern::Unit]), [0, 1]);
        index += 1;
    }
    pattern
}

/// The bootstrap environment plus one named row whose result is `S Pk`.
fn environment_with(depth: u32) -> Result<(Env, Definition), String> {
    let base = noble_kernel::contracts::environment()
        .map_err(|defect| format!("table must validate: {defect:?}"))?;
    let (mut env, _ops) = base
        .declare_generic_variant(family())
        .map_err(|error| format!("family must register: {error:?}"))?;
    let row = env.defs.len();
    let definition = match u32::try_from(row) {
        Ok(index) => Definition(index),
        Err(_) => return Err("definition index must fit u32".to_string()),
    };
    let scheme = noble_kernel::words::Scheme {
        var_kinds: vec![noble_kernel::words::VariableKind::Stack],
        stack_in: vec![Pattern::StackVar(noble_kernel::words::Variable(0))],
        stack_out: vec![
            Pattern::StackVar(noble_kernel::words::Variable(0)),
            nested_pattern(depth),
        ],
        effects: vec![],
    };
    env.defs.push(scheme);
    env.kinds.push(noble_kernel::contracts::Behavior::Named);
    env.deps.push(Vec::new());
    env.definition_owners.push(None);
    Ok((env, definition))
}

fn call(definition: Definition) -> noble_kernel::untrusted::Candidate {
    builders::candidate(
        vec![builders::invocation(
            definition,
            vec![builders::segment(Vec::new())],
        )],
        vec![0],
    )
}

#[test]
fn nested_generic_pattern_refuses_before_materializing() -> Result<(), String> {
    let (env, definition) = environment_with(OVERSIZED_DEPTH)?;
    let request = builders::request(Vec::new(), Vec::new(), builders::limits(10_000, 16, 512));
    let observed = builders::seen(&env, &request, &call(definition))?;
    assert!(
        matches!(observed, Seen::Exhausted(LimitKind::TypeSize)),
        "an exponentially nested generic pattern must refuse as TypeSize, got {observed:?}"
    );
    Ok(())
}

#[test]
fn small_generic_pattern_substitutes_and_accepts() -> Result<(), String> {
    let (env, definition) = environment_with(SMALL_DEPTH)?;
    // The expected result is the environment's own canonical instance chain,
    // so the control states the shape without restating the descriptor rule.
    let mut expected = Ty::Unit;
    let mut index = 0;
    while index < SMALL_DEPTH {
        expected = env
            .generic_instance(family().id, [expected, Ty::Unit])
            .ok_or_else(|| format!("canonical instance {index} must exist"))?;
        index += 1;
    }
    let request = builders::request(
        Vec::new(),
        vec![expected],
        builders::limits(10_000, 16, 512),
    );
    let observed = builders::seen(&env, &request, &call(definition))?;
    assert!(
        matches!(observed, Seen::Accepted),
        "a shallow nested generic pattern must substitute and accept, got {observed:?}"
    );
    Ok(())
}
