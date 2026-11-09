//! Issue #100, extended after #99: bounded evidence that a signature-checked recursive definition's
//! interface is exactly its declared signature.
//!
//! The frontend does not infer a recursive definition's scheme. Under signed self recursion it checks
//! the body against the declared signature, with every self call at the owner's own instance. The
//! frontend could still get two consequences of principality wrong while every current gate passes:
//! - **Uses.** A call of an accepted definition must be accepted at exactly the concrete input stacks
//!   that instantiate the declared signature, with the instantiated output stack. A frontend that
//!   fixed a variable of the declared signature while checking the body would refuse some instance.
//! - **Instances.** A body accepted at a declared signature must also be accepted at every declared
//!   instance of it, as the substitution lemma requires for monomorphic recursion.
//!
//! The expected verdicts come from the declared signatures alone, by matching a concrete stack against
//! `S inputs -- S outputs`, never from the frontend. Every acceptance has passed the kernel. This is
//! bounded evidence over the stated universe, not a proof.

/// The two words a signed body adds to the nonrecursive ones: a self call and a quoted self call.
const SELF_WORDS: [&str; 2] = ["p", "[ p ]"];

/// The longest generated body, in words.
const MAX_BODY_WORDS: usize = 3;

/// The deepest input stack a call is tried at: one deeper than the widest signature, so every
/// signature is also tried with a nonempty row below its inputs.
const MAX_SIGNED_DEPTH: usize = 3;

/// A base value type a declared signature is written over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Base {
    I64,
    Bool,
}

impl Base {
    #[expect(
        tigerstyle::fragile_exhaustive_enum_match,
        reason = "Owner: noble-maintainers; each base type has exactly one source spelling, so a new base type must say how it is spelled."
    )]
    const fn spelling(self) -> &'static str {
        match self {
            Self::I64 => "I64",
            Self::Bool => "Bool",
        }
    }

    #[expect(
        tigerstyle::fragile_exhaustive_enum_match,
        reason = "Owner: noble-maintainers; each base type is exactly one kernel type, so a new base type must say which."
    )]
    const fn ty(self) -> noble_kernel::types::Ty {
        match self {
            Self::I64 => noble_kernel::types::Ty::I64,
            Self::Bool => noble_kernel::types::Ty::Bool,
        }
    }
}

/// A declared signature `forall<S:stack> [ S inputs -- S outputs ! pure ]`.
struct Signature {
    inputs: &'static [Base],
    outputs: &'static [Base],
}

impl Signature {
    /// Whether this signature is `general` with its row instantiated to a fixed prefix.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; the helper compares slices with the non-const PartialEq."
    )]
    fn instantiates(&self, general: &Self) -> bool {
        let (Some(input_row), Some(output_row)) = (
            self.inputs.len().checked_sub(general.inputs.len()),
            self.outputs.len().checked_sub(general.outputs.len()),
        ) else {
            return false;
        };
        let (input_prefix, input_top) = self.inputs.split_at(input_row);
        let (output_prefix, output_top) = self.outputs.split_at(output_row);
        input_top == general.inputs
            && output_top == general.outputs
            && input_prefix == output_prefix
    }

    fn spelling(&self) -> String {
        let side = |types: &[Base]| {
            types.iter().fold(String::from("S"), |mut text, base| {
                text.push(' ');
                text.push_str(base.spelling());
                text
            })
        };
        format!(
            "forall<S:stack> [ {} -- {} ! pure ]",
            side(self.inputs),
            side(self.outputs)
        )
    }
}

/// The declared signatures. Six pairs are instances: the empty one of `I64 -- I64`, `Bool -- Bool`
/// and `Bool I64 -- Bool I64`; `-- I64` of `I64 -- I64 I64`; `I64 --` of `I64 I64 -- I64`; and
/// `I64 -- I64` of `Bool I64 -- Bool I64`.
const SIGNATURES: [Signature; 8] = [
    Signature {
        inputs: &[],
        outputs: &[],
    },
    Signature {
        inputs: &[],
        outputs: &[Base::I64],
    },
    Signature {
        inputs: &[Base::I64],
        outputs: &[],
    },
    Signature {
        inputs: &[Base::I64],
        outputs: &[Base::I64],
    },
    Signature {
        inputs: &[Base::Bool],
        outputs: &[Base::Bool],
    },
    Signature {
        inputs: &[Base::I64],
        outputs: &[Base::I64, Base::I64],
    },
    Signature {
        inputs: &[Base::I64, Base::I64],
        outputs: &[Base::I64],
    },
    Signature {
        inputs: &[Base::Bool, Base::I64],
        outputs: &[Base::Bool, Base::I64],
    },
];

/// The word a body index names: the nonrecursive words, then the two self words.
fn word(index: usize) -> Option<&'static str> {
    super::WORDS.get(index).copied().or_else(|| {
        index
            .checked_sub(super::WORDS.len())
            .and_then(|offset| SELF_WORDS.get(offset).copied())
    })
}

fn body_text(body: &[usize]) -> String {
    let words: Vec<&str> = body.iter().filter_map(|index| word(*index)).collect();
    words.join(" ")
}

fn has_self(body: &[usize]) -> bool {
    body.iter().any(|index| *index >= super::WORDS.len())
}

/// Every body of 1 to `MAX_BODY_WORDS` words with at least one self word, shortest first.
fn bodies() -> Vec<Vec<usize>> {
    let base = super::WORDS.len().saturating_add(SELF_WORDS.len());
    let mut all = Vec::with_capacity(1_236);
    for length in 1..=MAX_BODY_WORDS {
        let mut digits = vec![0; length];
        let mut has_next = true;
        while has_next {
            if has_self(&digits) {
                all.push(digits.clone());
            }
            has_next = super::advance(&mut digits, base);
        }
    }
    assert_eq!(all.len(), 1_236, "2 + 56 + 1178 bodies");
    assert!(all.iter().all(|body| has_self(body)));
    all
}

/// Every input stack of depth 0 to `MAX_SIGNED_DEPTH` over the nonrecursive value types.
fn call_stacks() -> Vec<Vec<noble_kernel::types::Ty>> {
    let values = super::value_types();
    let mut all = Vec::with_capacity(85);
    all.push(Vec::new());
    for depth in 1..=MAX_SIGNED_DEPTH {
        let mut digits = vec![0; depth];
        let mut has_next = true;
        while has_next {
            all.push(
                digits
                    .iter()
                    .filter_map(|index| values.get(*index).cloned())
                    .collect(),
            );
            has_next = super::advance(&mut digits, values.len());
        }
    }
    assert_eq!(all.len(), 85, "1 + 4 + 16 + 64 stacks");
    assert!(all.iter().all(|stack| stack.len() <= MAX_SIGNED_DEPTH));
    all
}

/// What the declared signature alone says a call does at a concrete stack: accepted exactly when
/// the top of the stack is the signature's inputs, keeping the row below and pushing its outputs.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the oracle compares kernel types through non-const iterators and builds an owned stack."
)]
fn expected(signature: &Signature, inputs: &[noble_kernel::types::Ty]) -> super::Verdict {
    let Some(row) = inputs.len().checked_sub(signature.inputs.len()) else {
        return super::Verdict::Refused;
    };
    let (below, top) = inputs.split_at(row);
    let is_instance = top
        .iter()
        .zip(signature.inputs)
        .all(|(ty, base)| *ty == base.ty());
    if is_instance {
        let mut outputs = below.to_vec();
        outputs.extend(signature.outputs.iter().map(|base| base.ty()));
        super::Verdict::Accepted(outputs)
    } else {
        super::Verdict::Refused
    }
}

fn module_text(body: &[usize], signature: &Signature) -> String {
    format!(
        "module m@1 [ signature p {} export p def p [ {} ] ]",
        signature.spelling(),
        body_text(body)
    )
}

/// A fresh signed session with `def p [ body ]` declared at the signature, if the module registers.
fn registered(
    body: &[usize],
    signature: &Signature,
) -> Option<noble_contracts::source::ModuleSession> {
    let retained = noble_contracts::source::RetainedSourceLimit::new(65_536)
        .expect("valid retained source cap");
    let session = noble_contracts::source::ModuleSession::new_signed_self(&[], retained)
        .expect("signed source session");
    let staged = session
        .prepare(
            module_text(body, signature).as_bytes(),
            &[],
            crate::limits(),
        )
        .ok()?;
    let (session, outcome) = session.commit(staged);
    outcome.ok().map(|()| session)
}

/// A call of `m@1.p` at a concrete stack, through the frontend and the kernel.
fn call(
    session: &noble_contracts::source::ModuleSession,
    inputs: &[noble_kernel::types::Ty],
) -> super::Verdict {
    session
        .prepare(b"m@1.p", inputs, crate::limits())
        .map_or(super::Verdict::Refused, |prepared| {
            super::Verdict::Accepted(prepared.output().to_vec())
        })
}

/// Counts over the signed universe.
#[derive(Clone, Copy, Debug, Default)]
struct Counts {
    bodies: u32,
    pairs: u32,
    accepted: u32,
    comparisons: u32,
    expected_accepted: u32,
    call_accepted: u32,
    instance_checks: u32,
    instance_accepted: u32,
    non_instance_refusals: u32,
}

/// One disagreement with a declared signature.
#[derive(Clone, Debug)]
enum Finding {
    /// A call disagrees with the declared signature at one stack.
    Use {
        body: Vec<usize>,
        signature: usize,
        inputs: Vec<noble_kernel::types::Ty>,
        expected: super::Verdict,
        call: super::Verdict,
    },
    /// A body accepted at `general` is refused at its declared instance `special`.
    Instance {
        body: Vec<usize>,
        general: usize,
        special: usize,
    },
}

/// The first stack at which a call of the body declared at one signature disagrees with that
/// signature.
fn check_uses(
    body: &[usize],
    signature: usize,
    session: &noble_contracts::source::ModuleSession,
    stacks: &[Vec<noble_kernel::types::Ty>],
    counts: &mut Counts,
) -> Option<Finding> {
    assert!(!stacks.is_empty(), "calls are tried at some stack");
    let declared = SIGNATURES.get(signature)?;
    for inputs in stacks {
        let expected = expected(declared, inputs);
        let call = call(session, inputs);
        counts.comparisons = counts.comparisons.saturating_add(1);
        counts.expected_accepted = counts
            .expected_accepted
            .saturating_add(u32::from(matches!(expected, super::Verdict::Accepted(_))));
        counts.call_accepted = counts
            .call_accepted
            .saturating_add(u32::from(matches!(call, super::Verdict::Accepted(_))));
        if expected != call {
            return Some(Finding::Use {
                body: body.to_vec(),
                signature,
                inputs: inputs.clone(),
                expected,
                call,
            });
        }
    }
    assert!(counts.call_accepted <= counts.comparisons);
    None
}

/// Every declared instance of a signature the body is accepted at must accept it too. Refusals at
/// signatures that are not instances are counted, so the closure is shown not to be vacuous.
fn check_instances(body: &[usize], accepted: &[bool], counts: &mut Counts) -> Vec<Finding> {
    assert_eq!(
        accepted.len(),
        SIGNATURES.len(),
        "one verdict per signature"
    );
    let mut found = Vec::with_capacity(SIGNATURES.len());
    for (general, declared) in SIGNATURES.iter().enumerate() {
        if accepted.get(general) != Some(&true) {
            continue;
        }
        for (special, candidate) in SIGNATURES.iter().enumerate() {
            let is_accepted = accepted.get(special) == Some(&true);
            if special == general {
                continue;
            }
            if candidate.instantiates(declared) {
                counts.instance_checks = counts.instance_checks.saturating_add(1);
                counts.instance_accepted = counts
                    .instance_accepted
                    .saturating_add(u32::from(is_accepted));
                if !is_accepted {
                    found.push(Finding::Instance {
                        body: body.to_vec(),
                        general,
                        special,
                    });
                }
            } else {
                counts.non_instance_refusals = counts
                    .non_instance_refusals
                    .saturating_add(u32::from(!is_accepted));
            }
        }
    }
    assert!(found.len() <= SIGNATURES.len().saturating_mul(SIGNATURES.len()));
    found
}

/// Whether the body still shows the finding's disagreement.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the helper registers the body through the non-const source session and calls it."
)]
fn still_disagrees(finding: &Finding, body: &[usize]) -> bool {
    match finding {
        Finding::Use {
            signature, inputs, ..
        } => SIGNATURES.get(*signature).is_some_and(|declared| {
            registered(body, declared)
                .is_some_and(|session| expected(declared, inputs) != call(&session, inputs))
        }),
        Finding::Instance {
            general, special, ..
        } => {
            let is_general_accepted = SIGNATURES
                .get(*general)
                .is_some_and(|declared| registered(body, declared).is_some());
            let is_special_refused = SIGNATURES
                .get(*special)
                .is_some_and(|declared| registered(body, declared).is_none());
            is_general_accepted && is_special_refused
        }
    }
}

/// The finding with its body replaced.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the helper registers the body through the non-const source session and calls it."
)]
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; each finding kind keeps its own fields, so a new kind must say which it keeps."
)]
fn with_body(finding: &Finding, body: Vec<usize>) -> Finding {
    match finding.clone() {
        Finding::Use {
            signature, inputs, ..
        } => {
            let declared = SIGNATURES.get(signature).expect("a recorded signature");
            let session = registered(&body, declared).expect("a shrunk body still registers");
            Finding::Use {
                expected: expected(declared, &inputs),
                call: call(&session, &inputs),
                body,
                signature,
                inputs,
            }
        }
        Finding::Instance {
            general, special, ..
        } => Finding::Instance {
            body,
            general,
            special,
        },
    }
}

const fn finding_body(finding: &Finding) -> &Vec<usize> {
    match finding {
        Finding::Use { body, .. } | Finding::Instance { body, .. } => body,
    }
}

/// Shrink a finding by removing words from its body while the body keeps a self word and still
/// shows the same disagreement. Each step removes one word, so this takes at most
/// `MAX_BODY_WORDS` steps.
fn shrink(found: Finding) -> Finding {
    let mut current = found;
    let mut is_shrinking = true;
    while is_shrinking {
        is_shrinking = false;
        let body = finding_body(&current).clone();
        for position in 0..body.len() {
            let mut shorter = body.clone();
            shorter.remove(position);
            if has_self(&shorter) && still_disagrees(&current, &shorter) {
                current = with_body(&current, shorter);
                is_shrinking = true;
                break;
            }
        }
    }
    assert!(has_self(finding_body(&current)));
    assert!(finding_body(&current).len() <= MAX_BODY_WORDS);
    current
}

/// Every body at every declared signature: its calls, then its declared instances. Each finding
/// is counted, and the first `MAX_RETAINED` are shrunk.
fn run_universe(
    stacks: &[Vec<noble_kernel::types::Ty>],
    counts: &mut Counts,
) -> (u32, Vec<Finding>) {
    let mut found = Vec::with_capacity(super::MAX_RETAINED);
    let mut discrepancies = 0_u32;
    for body in bodies() {
        counts.bodies = counts.bodies.saturating_add(1);
        let sessions: Vec<Option<noble_contracts::source::ModuleSession>> = SIGNATURES
            .iter()
            .map(|signature| registered(&body, signature))
            .collect();
        let accepted: Vec<bool> = sessions.iter().map(Option::is_some).collect();
        let mut findings = Vec::with_capacity(SIGNATURES.len().saturating_mul(2));
        for (signature, session) in sessions.iter().enumerate() {
            counts.pairs = counts.pairs.saturating_add(1);
            if let Some(session) = session {
                counts.accepted = counts.accepted.saturating_add(1);
                findings.extend(check_uses(&body, signature, session, stacks, counts));
            }
        }
        findings.extend(check_instances(&body, &accepted, counts));
        for finding in findings {
            discrepancies = discrepancies.saturating_add(1);
            if found.len() < super::MAX_RETAINED {
                found.push(shrink(finding));
            }
        }
    }
    assert_eq!(counts.pairs, counts.bodies.saturating_mul(8));
    assert!(found.len() <= super::MAX_RETAINED);
    (discrepancies, found)
}

fn json_finding(finding: &Finding) -> String {
    match finding {
        Finding::Use {
            body,
            signature,
            inputs,
            expected,
            call,
        } => format!(
            "{{\"kind\":\"use\",\"body\":{:?},\"signature\":{:?},\"inputs\":{:?},\"expected\":{},\"call\":{}}}",
            body_text(body),
            SIGNATURES.get(*signature).map(Signature::spelling),
            format!("{inputs:?}"),
            super::json_verdict(expected),
            super::json_verdict(call)
        ),
        Finding::Instance {
            body,
            general,
            special,
        } => format!(
            "{{\"kind\":\"instance\",\"body\":{:?},\"general\":{:?},\"special\":{:?}}}",
            body_text(body),
            SIGNATURES.get(*general).map(Signature::spelling),
            SIGNATURES.get(*special).map(Signature::spelling)
        ),
    }
}

fn json_counts(counts: &Counts) -> String {
    format!(
        "{{\"bodies\":{},\"pairs\":{},\"accepted\":{},\"comparisons\":{},\"expected_accepted\":{},\
         \"call_accepted\":{},\"instance_checks\":{},\"instance_accepted\":{},\"non_instance_refusals\":{}}}",
        counts.bodies,
        counts.pairs,
        counts.accepted,
        counts.comparisons,
        counts.expected_accepted,
        counts.call_accepted,
        counts.instance_checks,
        counts.instance_accepted,
        counts.non_instance_refusals
    )
}

/// The declared instance pairs `(general, special)` among the signatures.
fn instance_edges() -> Vec<(usize, usize)> {
    let mut edges = Vec::with_capacity(SIGNATURES.len().saturating_mul(SIGNATURES.len()));
    for (general, declared) in SIGNATURES.iter().enumerate() {
        for (special, candidate) in SIGNATURES.iter().enumerate() {
            if special != general && candidate.instantiates(declared) {
                edges.push((general, special));
            }
        }
    }
    edges
}

/// Run the whole signed universe, shrink every finding and report.
#[test]
fn report() -> Result<(), String> {
    let stacks = call_stacks();
    let edges = instance_edges();
    assert_eq!(edges, [(0, 3), (0, 4), (0, 7), (1, 5), (2, 6), (3, 7)]);
    let mut counts = Counts::default();
    let (discrepancies, found) = run_universe(&stacks, &mut counts);
    let retained: Vec<String> = found.iter().map(json_finding).collect();
    let signatures: Vec<String> = SIGNATURES.iter().map(Signature::spelling).collect();
    let edge_pairs: Vec<String> = edges
        .iter()
        .map(|(general, special)| format!("[{general},{special}]"))
        .collect();
    println!(
        "NOBLE-PRINCIPALITY-SIGNED-REPORT {{\"schema\":\"noble-principality-signed/v1\",\"words\":{:?},\
         \"self_words\":{SELF_WORDS:?},\"max_body_words\":{MAX_BODY_WORDS},\"signatures\":{signatures:?},\
         \"instance_edges\":[{}],\"value_types\":{:?},\"max_depth\":{MAX_SIGNED_DEPTH},\"input_stacks\":{},\
         \"counts\":{},\"discrepancies\":{discrepancies},\"retained\":[{}],\
         \"claim\":\"bounded evidence over this universe, not a proof\"}}",
        super::WORDS,
        edge_pairs.join(","),
        format!("{:?}", super::value_types()),
        stacks.len(),
        json_counts(&counts),
        retained.join(",")
    );
    assert!(counts.comparisons > 0);
    assert!(counts.instance_checks > 0);
    assert!(
        counts.non_instance_refusals > 0,
        "the instance closure is not vacuous"
    );
    if discrepancies == 0 {
        Ok(())
    } else {
        Err(format!(
            "{discrepancies} signed definitions disagree with their declared signatures: {retained:?}"
        ))
    }
}

/// The control a zero-discrepancy result needs: a definition declared narrower than the signature
/// it is judged against must be caught. `dup [ p ] drop` is accepted at `S I64 -- S I64 I64` and
/// at its instance `S I64 I64 -- S I64 I64 I64`. Declared at the instance and judged against the
/// general signature, its calls disagree wherever the second value is not `I64`, and agree wherever
/// it is, which is what a frontend that fixed the row's top value would produce.
#[test]
fn a_narrowed_signature_is_caught() {
    let general = SIGNATURES.get(5).expect("S I64 -- S I64 I64");
    let narrow = Signature {
        inputs: &[Base::I64, Base::I64],
        outputs: &[Base::I64, Base::I64, Base::I64],
    };
    assert!(narrow.instantiates(general));
    let body = [0, 14, 1];
    assert_eq!(body_text(&body), "dup [ p ] drop");
    assert!(
        registered(&body, general).is_some(),
        "accepted at the general signature"
    );
    let session = registered(&body, &narrow).expect("accepted at the narrow instance");
    let stacks = call_stacks();
    let mut caught = Vec::with_capacity(stacks.len());
    for inputs in &stacks {
        let (want, got) = (expected(general, inputs), call(&session, inputs));
        if want != got {
            caught.push((inputs.clone(), want, got));
        }
    }
    let bool_below = [noble_kernel::types::Ty::Bool, noble_kernel::types::Ty::I64];
    let is_bool_caught = caught.iter().any(|(inputs, want, got)| {
        inputs.as_slice() == bool_below
            && *want
                == super::Verdict::Accepted(vec![
                    noble_kernel::types::Ty::Bool,
                    noble_kernel::types::Ty::I64,
                    noble_kernel::types::Ty::I64,
                ])
            && *got == super::Verdict::Refused
    });
    assert!(
        is_bool_caught,
        "Bool below I64 is an instance only of the general signature: {caught:?}"
    );
    let two_ints = [noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64];
    assert!(
        caught
            .iter()
            .all(|(inputs, ..)| !inputs.ends_with(&two_ints)),
        "the narrow definition still agrees wherever the top two values are I64"
    );
}
