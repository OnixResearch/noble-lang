//! Identity record admission for a checked submission.

pub(super) fn admit(
    submission: &noble_kernel::execution::Submission,
    index: usize,
    identities: &mut alloc::vec::Vec<super::super::super::Identity>,
    calculator: bool,
    work: &mut super::super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let definition = &submission.definitions[index];
    let recipe = attempt!(super::encode(submission, &definition.body, work));
    attempt!(work.entries(identities.len()));
    let mut found = 0usize;
    while found < identities.len() && identities[found].id != definition.identity {
        found += 1;
    }
    if found < identities.len() {
        attempt!(work.entries(recipe.len()));
        if identities[found].recipe != recipe {
            return Err(crate::Diagnostic::Invalid);
        }
    } else {
        let cap = if calculator {
            super::super::super::CALCULATOR_DEFINITION_LIMIT
        } else {
            super::super::super::DEFINITION_LIMIT
        };
        if identities.len() >= cap {
            return Err(crate::Diagnostic::Exhausted);
        }
        identities.push(super::super::super::Identity {
            id: definition.identity,
            recipe,
        });
    }
    Ok(())
}
