//! Ordinary Core-Bootstrap source preparation for extraction subjects.
//!
//! This is the same public frontend the CLI session uses
//! (`noble_contracts::source::Session`): definitions are prepared and
//! committed one submission at a time, then the root body is prepared with
//! its ordered (bottom-first) input stack types. The resulting
//! [`Submission`] is untrusted producer data; [`super::admit`] re-runs the
//! kernel on it.

use noble_kernel::execution::Submission;
use noble_kernel::types::Ty;

/// A prepared root submission and the producer's claimed closure (every
/// definition identity the frontend placed in the submission).
#[derive(Clone, Debug)]
pub struct Prepared {
    /// The root submission.
    pub submission: Submission,
    /// Producer-claimed closure (sorted, unique).
    pub claimed_closure: Vec<u64>,
}

/// Prepares `definitions` (each one `def NAME [ … ]` submission, committed in
/// order) and then `root` with `inputs`, in a fresh `Session::new()` (the
/// CLI's Core-Bootstrap namespace with the `test.emit`/`test.abort` hosts).
///
/// # Errors
/// The frontend's refusal rendered as text.
pub fn prepare(definitions: &[&str], root: &str, inputs: &[Ty]) -> Result<Prepared, String> {
    let mut session = noble_contracts::source::Session::new();
    let limits = noble_contracts::Limits::default();
    for d in definitions {
        let p = session
            .prepare(d.as_bytes(), &[], limits)
            .map_err(|e| format!("definition {d:?}: {e:?}"))?;
        session
            .commit(p)
            .map_err(|e| format!("definition {d:?}: {e:?}"))?;
    }
    let p = session
        .prepare(root.as_bytes(), inputs, limits)
        .map_err(|e| format!("root {root:?}: {e:?}"))?;
    let submission = p
        .submission()
        .cloned()
        .ok_or_else(|| format!("root {root:?} produced no submission"))?;
    let mut claimed_closure: Vec<u64> = submission.definitions.iter().map(|d| d.identity).collect();
    claimed_closure.sort_unstable();
    claimed_closure.dedup();
    Ok(Prepared {
        submission,
        claimed_closure,
    })
}

/// `n` `I64` input types.
#[must_use]
pub fn i64s(n: usize) -> Vec<Ty> {
    vec![Ty::I64; n]
}
