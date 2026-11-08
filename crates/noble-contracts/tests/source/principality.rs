//! Issue #100: bounded evidence that a definition's inferred scheme is as general as its body.
//!
//! Principality says that every interface the kernel accepts for a program is an instance of the
//! frontend's inferred scheme. This bounded differential checks the consequence a frontend could get
//! wrong while every current gate passes: the body and its definition must be interchangeable. For
//! every generated nonrecursive program `P`, every context `C` and every concrete input stack `I` in
//! a bounded universe, `P C` and `p C`, with `def p [ P ]`, must agree at `I`:
//! - when `P C` is accepted (frontend inference at concrete types, then the independent kernel), the
//!   call is accepted with the same output stack;
//! - and conversely.
//!
//! The context is empty, or one word that consumes `P`'s outputs. An empty context catches a scheme
//! that is too narrow in its inputs. A consuming word catches a variable the frontend fixed when it
//! defined `p`, such as `Unit`, which no input stack and no output comparison can reveal.
//!
//! Every acceptance on either side has passed the kernel. This is bounded evidence over the stated
//! universe, not a proof. An interface the frontend cannot construct even at concrete inputs is
//! outside it, and so are effects, which #28's oracle covers.

/// The words a generated program, and a context, are built from: stack operations, literals,
/// quotation literals, composition, running a quotation, `dip` and branches.
const WORDS: [&str; 13] = [
    "dup", "drop", "swap", "1", "true", "[ ]", "[ dup ]", "[ 1 ]", "[ drop ]", "compose", "run",
    "dip", "if",
];

/// The longest generated program, in words.
const MAX_WORDS: usize = 3;

/// The longest program also placed before every one-word context.
const MAX_CONTEXT_WORDS: usize = 2;

/// The deepest input stack.
const MAX_DEPTH: usize = 2;

/// The largest number of discrepancies the report retains, after shrinking.
const MAX_RETAINED: usize = 16;

/// The value types input stacks are built from: two base types and two Program types.
fn value_types() -> [noble_kernel::types::Ty; 4] {
    let program = |inputs: Vec<noble_kernel::types::Ty>, outputs: Vec<noble_kernel::types::Ty>| {
        noble_kernel::types::Ty::Program(
            Box::new(inputs),
            Box::new(outputs),
            noble_kernel::types::EffSet::from_ids(&[]),
        )
    };
    [
        noble_kernel::types::Ty::I64,
        noble_kernel::types::Ty::Bool,
        program(vec![], vec![noble_kernel::types::Ty::I64]),
        program(
            vec![noble_kernel::types::Ty::I64],
            vec![noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64],
        ),
    ]
}

/// Advance an odometer of digits below `base`; false once it wraps to all zeros.
fn advance(digits: &mut [usize], base: usize) -> bool {
    for digit in digits.iter_mut().rev() {
        let next = digit.saturating_add(1);
        if next < base {
            *digit = next;
            return true;
        }
        *digit = 0;
    }
    false
}

/// Every word-index sequence of length 1 to `MAX_WORDS`, shortest first.
fn programs() -> Vec<Vec<usize>> {
    let mut all = Vec::with_capacity(2_400);
    for length in 1..=MAX_WORDS {
        let mut digits = vec![0; length];
        let mut has_next = true;
        while has_next {
            all.push(digits.clone());
            has_next = advance(&mut digits, WORDS.len());
        }
    }
    assert!(all.len() <= 2_400, "the program bound grew");
    assert_eq!(all.first().map(Vec::len), Some(1));
    all
}

/// Every input stack of depth 0 to `MAX_DEPTH` over `value_types`, bottom first.
fn input_stacks() -> Vec<Vec<noble_kernel::types::Ty>> {
    let values = value_types();
    let mut all = Vec::with_capacity(32);
    all.push(Vec::new());
    for depth in 1..=MAX_DEPTH {
        let mut digits = vec![0; depth];
        let mut has_next = true;
        while has_next {
            all.push(
                digits
                    .iter()
                    .filter_map(|index| values.get(*index).cloned())
                    .collect(),
            );
            has_next = advance(&mut digits, values.len());
        }
    }
    assert_eq!(all.len(), 21, "1 + 4 + 16 stacks");
    assert!(all.iter().all(|stack| stack.len() <= MAX_DEPTH));
    all
}

fn source(program: &[usize]) -> String {
    let words: Vec<&str> = program
        .iter()
        .filter_map(|index| WORDS.get(*index).copied())
        .collect();
    words.join(" ")
}

/// A text followed by the context word, if any.
fn in_context(text: &str, context: Option<usize>) -> String {
    context
        .and_then(|index| WORDS.get(index))
        .map_or_else(|| text.to_owned(), |word| format!("{text} {word}"))
}

/// What one side does at one input stack: the kernel-accepted output stack, or a refusal.
#[derive(Clone, Debug, PartialEq)]
enum Verdict {
    Accepted(Vec<noble_kernel::types::Ty>),
    Refused,
}

fn verdict(
    session: &noble_contracts::source::Session,
    text: &str,
    inputs: &[noble_kernel::types::Ty],
) -> Verdict {
    session
        .prepare(text.as_bytes(), inputs, super::limits())
        .map_or(Verdict::Refused, |prepared| {
            Verdict::Accepted(prepared.output().to_vec())
        })
}

/// One program in one context at one input stack where the body and its definition disagree.
#[derive(Clone, Debug)]
struct Discrepancy {
    program: Vec<usize>,
    context: Option<usize>,
    inputs: Vec<noble_kernel::types::Ty>,
    inline: Verdict,
    call: Verdict,
}

/// Counts for one part of the universe: (program, context) pairs, how many of their programs
/// define, and the comparisons with each side's acceptances.
#[derive(Clone, Copy, Debug, Default)]
struct Tally {
    pairs: u32,
    defined: u32,
    comparisons: u32,
    inline_accepted: u32,
    call_accepted: u32,
}

/// The body and its definition in one context at one input stack, with the definition committed
/// in `session`.
fn compare_at(
    session: &noble_contracts::source::Session,
    program: &[usize],
    context: Option<usize>,
    inputs: &[noble_kernel::types::Ty],
    is_defined: bool,
) -> (Verdict, Verdict) {
    let inline = verdict(session, &in_context(&source(program), context), inputs);
    let call = if is_defined {
        verdict(session, &in_context("p", context), inputs)
    } else {
        Verdict::Refused
    };
    (inline, call)
}

/// Define `p` as the program in a fresh session, and report whether the definition was accepted.
fn defined(program: &[usize]) -> (noble_contracts::source::Session, bool) {
    let mut session = noble_contracts::source::Session::new();
    let definition = format!("def p [ {} ]", source(program));
    let is_defined = super::define(&mut session, definition.as_bytes()).is_ok();
    (session, is_defined)
}

/// The first input stack at which the program in this context and its definition disagree.
fn check_pair(
    program: &[usize],
    context: Option<usize>,
    stacks: &[Vec<noble_kernel::types::Ty>],
    tally: &mut Tally,
) -> Option<Discrepancy> {
    assert!(
        !program.is_empty(),
        "a generated program has at least one word"
    );
    let (session, is_defined) = defined(program);
    tally.pairs = tally.pairs.saturating_add(1);
    tally.defined = tally.defined.saturating_add(u32::from(is_defined));
    for inputs in stacks {
        let (inline, call) = compare_at(&session, program, context, inputs, is_defined);
        tally.comparisons = tally.comparisons.saturating_add(1);
        let is_inline_accepted = matches!(inline, Verdict::Accepted(_));
        let is_call_accepted = matches!(call, Verdict::Accepted(_));
        tally.inline_accepted = tally
            .inline_accepted
            .saturating_add(u32::from(is_inline_accepted));
        tally.call_accepted = tally
            .call_accepted
            .saturating_add(u32::from(is_call_accepted));
        if inline != call {
            return Some(Discrepancy {
                program: program.to_vec(),
                context,
                inputs: inputs.clone(),
                inline,
                call,
            });
        }
    }
    assert!(tally.comparisons >= tally.inline_accepted);
    None
}

/// Shrink a discrepancy by removing words from the program while the body and its definition still
/// disagree in the same context at the same input stack. Each step removes one word, so this takes
/// at most `MAX_WORDS` steps.
fn shrink(found: Discrepancy) -> Discrepancy {
    let mut current = found;
    let mut is_shrinking = true;
    while is_shrinking {
        is_shrinking = false;
        for position in 0..current.program.len() {
            let mut shorter = current.program.clone();
            shorter.remove(position);
            if shorter.is_empty() {
                continue;
            }
            let (session, is_defined) = defined(&shorter);
            let (inline, call) = compare_at(
                &session,
                &shorter,
                current.context,
                &current.inputs,
                is_defined,
            );
            if inline != call {
                current = Discrepancy {
                    program: shorter,
                    context: current.context,
                    inputs: current.inputs.clone(),
                    inline,
                    call,
                };
                is_shrinking = true;
                break;
            }
        }
    }
    assert!(!current.program.is_empty());
    assert!(current.program.len() <= MAX_WORDS);
    current
}

fn json_verdict(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Accepted(outputs) => format!("{:?}", format!("accepted {outputs:?}")),
        Verdict::Refused => String::from("\"refused\""),
    }
}

fn json_tally(tally: &Tally) -> String {
    format!(
        "{{\"pairs\":{},\"defined\":{},\"comparisons\":{},\"inline_accepted\":{},\"call_accepted\":{}}}",
        tally.pairs, tally.defined, tally.comparisons, tally.inline_accepted, tally.call_accepted
    )
}

fn json_discrepancy(discrepancy: &Discrepancy) -> String {
    let context = discrepancy
        .context
        .and_then(|index| WORDS.get(index))
        .map_or_else(|| String::from("null"), |word| format!("{word:?}"));
    format!(
        "{{\"program\":{:?},\"context\":{context},\"inputs\":{:?},\"inline\":{},\"call\":{}}}",
        source(&discrepancy.program),
        format!("{:?}", discrepancy.inputs),
        json_verdict(&discrepancy.inline),
        json_verdict(&discrepancy.call)
    )
}

/// The control a zero-discrepancy result needs: a definition whose scheme is narrower than its
/// body's must be caught. `dup dup 1 + drop` has `dup`'s outputs on `I64` but forces `I64`, which
/// is what a frontend that generalised `dup` too narrowly would infer.
#[test]
fn an_overly_specific_scheme_is_caught() -> Result<(), String> {
    let mut session = noble_contracts::source::Session::new();
    super::define(&mut session, b"def p [ dup dup 1 + drop ]")?;
    let stacks = input_stacks();
    let mut caught = Vec::with_capacity(stacks.len());
    for inputs in &stacks {
        let inline = verdict(&session, "dup", inputs);
        let call = verdict(&session, "p", inputs);
        if inline != call {
            caught.push((inputs.clone(), inline, call));
        }
    }
    let bool_only = [noble_kernel::types::Ty::Bool];
    let is_bool_caught = caught.iter().any(|(inputs, inline, call)| {
        inputs.as_slice() == bool_only
            && *inline
                == Verdict::Accepted(vec![
                    noble_kernel::types::Ty::Bool,
                    noble_kernel::types::Ty::Bool,
                ])
            && *call == Verdict::Refused
    });
    assert!(
        is_bool_caught,
        "dup on Bool is accepted inline and refused by p: {caught:?}"
    );
    assert!(
        caught
            .iter()
            .all(|(inputs, ..)| inputs.last() != Some(&noble_kernel::types::Ty::I64)),
        "the narrow scheme still agrees wherever the top is I64"
    );
    Ok(())
}

/// A variable fixed when `p` is defined is invisible without a consumer. `nil`'s element type
/// appears only in its output, and `p` below is `nil` with that type fixed to `Unit`, the default a
/// frontend that defaulted at definition time would choose. With no context, both routes return
/// `List<Unit>` at every input stack. `cons` at an `I64` input refuses `p` and accepts the body.
#[test]
fn a_definition_time_default_is_caught_only_in_context() -> Result<(), String> {
    let mut session = noble_contracts::source::Session::new();
    super::define(&mut session, b"def p [ nil dup unit swap cons drop ]")?;
    let stacks = input_stacks();
    let mut plain_disagreements = 0_u32;
    for inputs in &stacks {
        let is_equal = verdict(&session, "nil", inputs) == verdict(&session, "p", inputs);
        plain_disagreements = plain_disagreements.saturating_add(u32::from(!is_equal));
    }
    assert_eq!(
        plain_disagreements, 0,
        "without a consumer the default is invisible"
    );
    let int = [noble_kernel::types::Ty::I64];
    assert_eq!(
        verdict(&session, "nil cons", &int),
        Verdict::Accepted(vec![noble_kernel::types::Ty::List(Box::new(
            noble_kernel::types::Ty::I64
        ))])
    );
    assert_eq!(verdict(&session, "p cons", &int), Verdict::Refused);
    Ok(())
}

/// The plain universe, then every short program before every one-word context. Each discrepancy is
/// counted, and the first `MAX_RETAINED` are shrunk.
fn run_universe(
    stacks: &[Vec<noble_kernel::types::Ty>],
    plain: &mut Tally,
    contextual: &mut Tally,
) -> (u32, Vec<Discrepancy>) {
    let all = programs();
    let mut found = Vec::with_capacity(MAX_RETAINED);
    let mut discrepancies = 0_u32;
    let mut record = |discrepancy: Option<Discrepancy>| {
        if let Some(discrepancy) = discrepancy {
            discrepancies = discrepancies.saturating_add(1);
            if found.len() < MAX_RETAINED {
                found.push(shrink(discrepancy));
            }
        }
    };
    for program in &all {
        record(check_pair(program, None, stacks, plain));
    }
    for program in all
        .iter()
        .filter(|program| program.len() <= MAX_CONTEXT_WORDS)
    {
        for context in 0..WORDS.len() {
            record(check_pair(program, Some(context), stacks, contextual));
        }
    }
    assert_eq!(usize::try_from(plain.pairs).ok(), Some(all.len()));
    assert!(found.len() <= MAX_RETAINED);
    (discrepancies, found)
}

/// Run the whole bounded universe, shrink every discrepancy and report.
#[test]
fn report() -> Result<(), String> {
    let stacks = input_stacks();
    let mut plain = Tally::default();
    let mut contextual = Tally::default();
    let (discrepancies, found) = run_universe(&stacks, &mut plain, &mut contextual);
    let retained: Vec<String> = found.iter().map(json_discrepancy).collect();
    let context_programs = programs()
        .iter()
        .filter(|program| program.len() <= MAX_CONTEXT_WORDS)
        .count();
    println!(
        "NOBLE-PRINCIPALITY-REPORT {{\"schema\":\"noble-principality-bounded/v2\",\"words\":{:?},\
         \"max_words\":{MAX_WORDS},\"max_context_words\":{MAX_CONTEXT_WORDS},\"context_programs\":{context_programs},\
         \"value_types\":{:?},\"max_depth\":{MAX_DEPTH},\"input_stacks\":{},\"plain\":{},\"contextual\":{},\
         \"discrepancies\":{discrepancies},\"retained\":[{}],\
         \"claim\":\"bounded evidence over this universe, not a proof\"}}",
        WORDS,
        format!("{:?}", value_types()),
        stacks.len(),
        json_tally(&plain),
        json_tally(&contextual),
        retained.join(",")
    );
    assert!(plain.comparisons > 0);
    assert!(contextual.comparisons > 0);
    if discrepancies == 0 {
        Ok(())
    } else {
        Err(format!(
            "{discrepancies} program/context pairs disagree with their definitions: {retained:?}"
        ))
    }
}
