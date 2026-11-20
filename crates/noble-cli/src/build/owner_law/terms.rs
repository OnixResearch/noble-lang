//! Law obligation term syntax, structural comparison and surface rendering.
//!
//! The owner law states its `requires`/`ensures` obligation in the MC1
//! predicate surface syntax. This module parses that closed grammar, compares
//! the resulting term with the consumer's own regenerated expression graph, and
//! renders the candidate's term back to the same surface syntax for the
//! admission record. Unsupported constructors are refusals, never silent
//! agreement, and the comparison is a bounded worklist rather than recursion.

/// One law obligation term.
///
/// `Input`/`Output` carry the binding's position in the law's ordered binding
/// list; the comparison resolves that position against the candidate's own
/// prepared bindings, whose names and order were already compared.
pub(super) enum Term {
    I64(i64),
    Bool(bool),
    Unit,
    Input(u32),
    Output(u32),
    Not(std::boxed::Box<Term>),
    And(std::boxed::Box<Term>, std::boxed::Box<Term>),
    Or(std::boxed::Box<Term>, std::boxed::Box<Term>),
    Implies(std::boxed::Box<Term>, std::boxed::Box<Term>),
    Eq(std::boxed::Box<Term>, std::boxed::Box<Term>),
    Lt(std::boxed::Box<Term>, std::boxed::Box<Term>),
    Le(std::boxed::Box<Term>, std::boxed::Box<Term>),
    Add(std::boxed::Box<Term>, std::boxed::Box<Term>),
    Sub(std::boxed::Box<Term>, std::boxed::Box<Term>),
    Mul(std::boxed::Box<Term>, std::boxed::Box<Term>),
    Pair(std::boxed::Box<Term>, std::boxed::Box<Term>),
    First(std::boxed::Box<Term>),
    Second(std::boxed::Box<Term>),
    Inl(std::boxed::Box<Term>),
    Inr(std::boxed::Box<Term>),
    IsLeft(std::boxed::Box<Term>),
    Left(std::boxed::Box<Term>),
    Right(std::boxed::Box<Term>),
    Nil,
    Cons(std::boxed::Box<Term>, std::boxed::Box<Term>),
    IsNil(std::boxed::Box<Term>),
    Head(std::boxed::Box<Term>),
    Tail(std::boxed::Box<Term>),
    Length(std::boxed::Box<Term>),
    Maps(
        std::boxed::Box<Term>,
        std::boxed::Box<Term>,
        std::boxed::Box<Term>,
    ),
}

/// The ordered binding names one parsed predicate may reference.
pub(super) struct Names<'a> {
    inputs: &'a [std::string::String],
    outputs: &'a [std::string::String],
}

impl<'a> Names<'a> {
    pub(super) const fn new(
        inputs: &'a [std::string::String],
        outputs: &'a [std::string::String],
    ) -> Self {
        Self { inputs, outputs }
    }
}

#[derive(Clone, Copy)]
enum Token<'a> {
    Open,
    Close,
    Atom(&'a str),
}

struct Tokens<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Tokens<'a> {
    const fn new(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
            position: 0,
        }
    }

    fn next(&mut self) -> Option<Token<'a>> {
        while self.position < self.bytes.len()
            && matches!(self.bytes[self.position], b' ' | b'\t' | b'\n' | b'\r')
        {
            self.position = self.position.saturating_add(1);
        }
        if self.position >= self.bytes.len() {
            return None;
        }
        let byte = self.bytes[self.position];
        if byte == b'(' {
            self.position = self.position.saturating_add(1);
            return Some(Token::Open);
        }
        if byte == b')' {
            self.position = self.position.saturating_add(1);
            return Some(Token::Close);
        }
        let start = self.position;
        while self.position < self.bytes.len()
            && !matches!(
                self.bytes[self.position],
                b' ' | b'\t' | b'\n' | b'\r' | b'(' | b')'
            )
        {
            self.position = self.position.saturating_add(1);
        }
        std::str::from_utf8(self.bytes.get(start..self.position)?)
            .ok()
            .map(Token::Atom)
    }
}

/// Parse one law predicate into the closed term grammar.
///
/// # Errors
/// A diagnostic naming the unsupported constructor or binding, so a law this
/// consumer cannot interpret refuses instead of passing.
pub(super) fn parse(text: &str, names: &Names<'_>) -> Result<Term, std::string::String> {
    let mut tokens = Tokens::new(text);
    let term = form(&mut tokens, names, 0)?;
    match tokens.next() {
        None => Ok(term),
        Some(token) => Err(std::format!(
            "trailing law term input after a complete predicate: {}",
            spell(token)
        )),
    }
}

const MAX_DEPTH: u32 = 64;

fn form(
    tokens: &mut Tokens<'_>,
    names: &Names<'_>,
    depth: u32,
) -> Result<Term, std::string::String> {
    if depth >= MAX_DEPTH {
        return Err(std::string::String::from(
            "law predicate nesting is too deep",
        ));
    }
    match tokens.next() {
        Some(Token::Atom(text)) => atom(text),
        Some(Token::Open) => {
            let head = match tokens.next() {
                Some(Token::Atom(head)) => head,
                _ => return Err(std::string::String::from("law form requires a head symbol")),
            };
            let term = call(tokens, names, head, depth.saturating_add(1))?;
            match tokens.next() {
                Some(Token::Close) => Ok(term),
                _ => Err(std::format!(
                    "law form `{head}` is missing its closing parenthesis"
                )),
            }
        }
        Some(Token::Close) => Err(std::string::String::from(
            "unexpected `)` in a law predicate",
        )),
        None => Err(std::string::String::from("incomplete law predicate")),
    }
}

fn atom(text: &str) -> Result<Term, std::string::String> {
    match text {
        "true" => return Ok(Term::Bool(true)),
        "false" => return Ok(Term::Bool(false)),
        "unit" => return Ok(Term::Unit),
        "nil" => return Ok(Term::Nil),
        _ => {}
    }
    match text.parse::<i64>() {
        Ok(value) => Ok(Term::I64(value)),
        Err(_) => Err(std::format!(
            "unsupported law term atom `{text}`; only literals and the closed constructor grammar are interpreted"
        )),
    }
}

fn binding(
    tokens: &mut Tokens<'_>,
    names: &Names<'_>,
    head: &str,
    is_output: bool,
) -> Result<Term, std::string::String> {
    let name = match tokens.next() {
        Some(Token::Atom(name)) => name,
        _ => return Err(std::format!("law form `{head}` names no binding")),
    };
    let bindings = if is_output {
        names.outputs
    } else {
        names.inputs
    };
    if let Some(index) = bindings.iter().position(|binding| binding == name) {
        let index = u32::try_from(index)
            .map_err(|_| std::string::String::from("binding position is unrepresentable"))?;
        return Ok(if is_output {
            Term::Output(index)
        } else {
            Term::Input(index)
        });
    }
    Err(std::format!(
        "law form `{head}` names `{name}`, which is not one of the law's declared bindings"
    ))
}

fn unary(
    tokens: &mut Tokens<'_>,
    names: &Names<'_>,
    depth: u32,
    build: fn(std::boxed::Box<Term>) -> Term,
) -> Result<Term, std::string::String> {
    let argument = form(tokens, names, depth)?;
    Ok(build(std::boxed::Box::new(argument)))
}

fn binary(
    tokens: &mut Tokens<'_>,
    names: &Names<'_>,
    depth: u32,
    build: fn(std::boxed::Box<Term>, std::boxed::Box<Term>) -> Term,
) -> Result<Term, std::string::String> {
    let left = form(tokens, names, depth)?;
    let right = form(tokens, names, depth)?;
    Ok(build(
        std::boxed::Box::new(left),
        std::boxed::Box::new(right),
    ))
}

fn call(
    tokens: &mut Tokens<'_>,
    names: &Names<'_>,
    head: &str,
    depth: u32,
) -> Result<Term, std::string::String> {
    match head {
        "in" => binding(tokens, names, head, false),
        "out" => binding(tokens, names, head, true),
        "not" => unary(tokens, names, depth, Term::Not),
        "first" => unary(tokens, names, depth, Term::First),
        "second" => unary(tokens, names, depth, Term::Second),
        "inl" => unary(tokens, names, depth, Term::Inl),
        "inr" => unary(tokens, names, depth, Term::Inr),
        "isLeft" => unary(tokens, names, depth, Term::IsLeft),
        "left" => unary(tokens, names, depth, Term::Left),
        "right" => unary(tokens, names, depth, Term::Right),
        "isNil" => unary(tokens, names, depth, Term::IsNil),
        "head" => unary(tokens, names, depth, Term::Head),
        "tail" => unary(tokens, names, depth, Term::Tail),
        "length" => unary(tokens, names, depth, Term::Length),
        "and" => binary(tokens, names, depth, Term::And),
        "or" => binary(tokens, names, depth, Term::Or),
        "implies" => binary(tokens, names, depth, Term::Implies),
        "eq" => binary(tokens, names, depth, Term::Eq),
        "lt" => binary(tokens, names, depth, Term::Lt),
        "le" => binary(tokens, names, depth, Term::Le),
        "add" => binary(tokens, names, depth, Term::Add),
        "sub" => binary(tokens, names, depth, Term::Sub),
        "mul" => binary(tokens, names, depth, Term::Mul),
        "pair" => binary(tokens, names, depth, Term::Pair),
        "cons" => binary(tokens, names, depth, Term::Cons),
        "maps" => {
            let first = form(tokens, names, depth)?;
            let second = form(tokens, names, depth)?;
            let third = form(tokens, names, depth)?;
            Ok(Term::Maps(
                std::boxed::Box::new(first),
                std::boxed::Box::new(second),
                std::boxed::Box::new(third),
            ))
        }
        _ => Err(std::format!(
            "unsupported law obligation constructor `{head}`"
        )),
    }
}

const fn spell(token: Token<'_>) -> &'static str {
    match token {
        Token::Open => "(",
        Token::Close => ")",
        Token::Atom(_) => "an atom",
    }
}

/// The largest expression graph the comparison walks for one predicate.
const COMPARISON_BUDGET: u32 = 4096;

/// Compare one law term with the candidate's independently regenerated
/// expression at `index`.
///
/// # Errors
/// A diagnostic naming the first structural difference, an unavailable
/// definition body, or an exhausted comparison budget.
pub(super) fn compare(
    law: &Term,
    prepared: &noble_contracts::Prepared,
    index: u32,
) -> Result<(), std::string::String> {
    let mut pending: std::vec::Vec<(&Term, u32)> = std::vec::Vec::new();
    pending.push((law, index));
    let mut steps = 0u32;
    while let Some((term, at)) = pending.pop() {
        steps = steps.saturating_add(1);
        if steps > COMPARISON_BUDGET {
            return Err(std::string::String::from(
                "law term comparison exhausted its bounded work budget",
            ));
        }
        let expression = usize::try_from(at)
            .ok()
            .and_then(|position| prepared.expressions().get(position))
            .ok_or_else(|| std::format!("generated expression {at} is missing"))?;
        match (&expression.kind, term) {
            (noble_contracts::ExprKind::I64(observed), Term::I64(expected))
                if observed == expected => {}
            (noble_contracts::ExprKind::Bool(true), Term::Bool(true)) => {}
            (noble_contracts::ExprKind::Bool(false), Term::Bool(false)) => {}
            (noble_contracts::ExprKind::Unit, Term::Unit) => {}
            (noble_contracts::ExprKind::Input(observed), Term::Input(expected))
                if observed == expected => {}
            (noble_contracts::ExprKind::Output(observed), Term::Output(expected))
                if observed == expected => {}
            (noble_contracts::ExprKind::Not(a), Term::Not(x)) => pending.push((x, *a)),
            (noble_contracts::ExprKind::And(a, b), Term::And(x, y)) => {
                pending.push((x, *a));
                pending.push((y, *b));
            }
            (noble_contracts::ExprKind::Or(a, b), Term::Or(x, y)) => {
                pending.push((x, *a));
                pending.push((y, *b));
            }
            (noble_contracts::ExprKind::Implies(a, b), Term::Implies(x, y)) => {
                pending.push((x, *a));
                pending.push((y, *b));
            }
            (noble_contracts::ExprKind::Eq(a, b), Term::Eq(x, y)) => {
                pending.push((x, *a));
                pending.push((y, *b));
            }
            (noble_contracts::ExprKind::Lt(a, b), Term::Lt(x, y)) => {
                pending.push((x, *a));
                pending.push((y, *b));
            }
            (noble_contracts::ExprKind::Le(a, b), Term::Le(x, y)) => {
                pending.push((x, *a));
                pending.push((y, *b));
            }
            (noble_contracts::ExprKind::Add(a, b), Term::Add(x, y)) => {
                pending.push((x, *a));
                pending.push((y, *b));
            }
            (noble_contracts::ExprKind::Sub(a, b), Term::Sub(x, y)) => {
                pending.push((x, *a));
                pending.push((y, *b));
            }
            (noble_contracts::ExprKind::Mul(a, b), Term::Mul(x, y)) => {
                pending.push((x, *a));
                pending.push((y, *b));
            }
            (noble_contracts::ExprKind::Pair(a, b), Term::Pair(x, y)) => {
                pending.push((x, *a));
                pending.push((y, *b));
            }
            (noble_contracts::ExprKind::First(a), Term::First(x)) => pending.push((x, *a)),
            (noble_contracts::ExprKind::Second(a), Term::Second(x)) => pending.push((x, *a)),
            (noble_contracts::ExprKind::Inl(a), Term::Inl(x)) => pending.push((x, *a)),
            (noble_contracts::ExprKind::Inr(a), Term::Inr(x)) => pending.push((x, *a)),
            (noble_contracts::ExprKind::IsLeft(a), Term::IsLeft(x)) => pending.push((x, *a)),
            (noble_contracts::ExprKind::Left(a), Term::Left(x)) => pending.push((x, *a)),
            (noble_contracts::ExprKind::Right(a), Term::Right(x)) => pending.push((x, *a)),
            (noble_contracts::ExprKind::Nil, Term::Nil) => {}
            (noble_contracts::ExprKind::Cons(a, b), Term::Cons(x, y)) => {
                pending.push((x, *a));
                pending.push((y, *b));
            }
            (noble_contracts::ExprKind::IsNil(a), Term::IsNil(x)) => pending.push((x, *a)),
            (noble_contracts::ExprKind::Head(a), Term::Head(x)) => pending.push((x, *a)),
            (noble_contracts::ExprKind::Tail(a), Term::Tail(x)) => pending.push((x, *a)),
            (noble_contracts::ExprKind::Length(a), Term::Length(x)) => pending.push((x, *a)),
            (noble_contracts::ExprKind::Maps(a, b, c), Term::Maps(x, y, z)) => {
                pending.push((x, *a));
                pending.push((y, *b));
                pending.push((z, *c));
            }
            // A candidate may name a logical definition where the law spells
            // the term out; the definition's body is what the law must mean.
            (noble_contracts::ExprKind::Definition(definition), _) => {
                let definition = usize::try_from(*definition)
                    .ok()
                    .and_then(|position| prepared.definitions().get(position))
                    .ok_or_else(|| std::format!("generated definition {definition} is missing"))?;
                pending.push((term, definition.body));
            }
            (observed, _) => {
                return Err(std::format!(
                    "generated expression {at} is `{:?}`, which does not match the law term",
                    observed
                ))
            }
        }
    }
    Ok(())
}

/// Render the candidate's expression back to the law surface syntax.
///
/// # Errors
/// A diagnostic when the expression graph is unavailable or the render budget
/// is exhausted.
pub(super) fn render(
    prepared: &noble_contracts::Prepared,
    index: u32,
) -> Result<std::string::String, std::string::String> {
    let mut output = std::string::String::with_capacity(128);
    let mut pending: std::vec::Vec<Part> = std::vec::Vec::new();
    pending.push(Part::Expression(index));
    let mut steps = 0u32;
    while let Some(part) = pending.pop() {
        steps = steps.saturating_add(1);
        if steps > COMPARISON_BUDGET {
            return Err(std::string::String::from(
                "law term rendering exhausted its bounded work budget",
            ));
        }
        match part {
            Part::Text(text) => output.push_str(text),
            Part::Expression(at) => {
                let expression = usize::try_from(at)
                    .ok()
                    .and_then(|position| prepared.expressions().get(position))
                    .ok_or_else(|| std::format!("generated expression {at} is missing"))?;
                match &expression.kind {
                    noble_contracts::ExprKind::I64(value) => {
                        output.push_str(&std::format!("{value}"));
                    }
                    noble_contracts::ExprKind::Bool(value) => {
                        output.push_str(if *value { "true" } else { "false" });
                    }
                    noble_contracts::ExprKind::Unit => output.push_str("unit"),
                    noble_contracts::ExprKind::Input(position) => {
                        let name = binding_name(prepared.inputs(), *position)?;
                        output.push_str(&std::format!("(in {name})"));
                    }
                    noble_contracts::ExprKind::Output(position) => {
                        let name = binding_name(prepared.outputs(), *position)?;
                        output.push_str(&std::format!("(out {name})"));
                    }
                    noble_contracts::ExprKind::Definition(definition) => {
                        let definition = usize::try_from(*definition)
                            .ok()
                            .and_then(|position| prepared.definitions().get(position))
                            .ok_or_else(|| {
                                std::format!("generated definition {definition} is missing")
                            })?;
                        output.push_str(&std::format!("(definition {}", definition.name));
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(definition.body));
                    }
                    noble_contracts::ExprKind::Not(a) => {
                        output.push_str("(not ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*a));
                    }
                    noble_contracts::ExprKind::First(a) => {
                        output.push_str("(first ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*a));
                    }
                    noble_contracts::ExprKind::Second(a) => {
                        output.push_str("(second ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*a));
                    }
                    noble_contracts::ExprKind::Inl(a) => {
                        output.push_str("(inl ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*a));
                    }
                    noble_contracts::ExprKind::Inr(a) => {
                        output.push_str("(inr ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*a));
                    }
                    noble_contracts::ExprKind::IsLeft(a) => {
                        output.push_str("(isLeft ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*a));
                    }
                    noble_contracts::ExprKind::Left(a) => {
                        output.push_str("(left ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*a));
                    }
                    noble_contracts::ExprKind::Right(a) => {
                        output.push_str("(right ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*a));
                    }
                    noble_contracts::ExprKind::Nil => output.push_str("nil"),
                    noble_contracts::ExprKind::IsNil(a) => {
                        output.push_str("(isNil ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*a));
                    }
                    noble_contracts::ExprKind::Head(a) => {
                        output.push_str("(head ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*a));
                    }
                    noble_contracts::ExprKind::Tail(a) => {
                        output.push_str("(tail ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*a));
                    }
                    noble_contracts::ExprKind::Length(a) => {
                        output.push_str("(length ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*a));
                    }
                    noble_contracts::ExprKind::Maps(a, b, c) => {
                        output.push_str("(maps ");
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(*c));
                        pending.push(Part::Text(" "));
                        pending.push(Part::Expression(*b));
                        pending.push(Part::Text(" "));
                        pending.push(Part::Expression(*a));
                    }
                    kind => {
                        let head = binary_head(kind).ok_or_else(|| {
                            std::format!("unsupported generated expression `{kind:?}`")
                        })?;
                        let (left, right) = arguments(kind).ok_or_else(|| {
                            std::format!("unsupported generated expression `{kind:?}`")
                        })?;
                        output.push_str(&std::format!("({head} "));
                        pending.push(Part::Text(")"));
                        pending.push(Part::Expression(right));
                        pending.push(Part::Text(" "));
                        pending.push(Part::Expression(left));
                    }
                }
            }
        }
    }
    Ok(output)
}

enum Part {
    Text(&'static str),
    Expression(u32),
}

fn binding_name(
    bindings: &[noble_contracts::NamedType],
    position: u32,
) -> Result<&str, std::string::String> {
    usize::try_from(position)
        .ok()
        .and_then(|position| bindings.get(position))
        .map(|binding| binding.name.as_str())
        .ok_or_else(|| std::format!("binding {position} is missing from the law interface"))
}

const fn binary_head(kind: &noble_contracts::ExprKind) -> Option<&'static str> {
    match kind {
        noble_contracts::ExprKind::And(_, _) => Some("and"),
        noble_contracts::ExprKind::Or(_, _) => Some("or"),
        noble_contracts::ExprKind::Implies(_, _) => Some("implies"),
        noble_contracts::ExprKind::Eq(_, _) => Some("eq"),
        noble_contracts::ExprKind::Lt(_, _) => Some("lt"),
        noble_contracts::ExprKind::Le(_, _) => Some("le"),
        noble_contracts::ExprKind::Add(_, _) => Some("add"),
        noble_contracts::ExprKind::Sub(_, _) => Some("sub"),
        noble_contracts::ExprKind::Mul(_, _) => Some("mul"),
        noble_contracts::ExprKind::Pair(_, _) => Some("pair"),
        noble_contracts::ExprKind::Cons(_, _) => Some("cons"),
        _ => None,
    }
}

const fn arguments(kind: &noble_contracts::ExprKind) -> Option<(u32, u32)> {
    match kind {
        noble_contracts::ExprKind::And(a, b)
        | noble_contracts::ExprKind::Or(a, b)
        | noble_contracts::ExprKind::Implies(a, b)
        | noble_contracts::ExprKind::Eq(a, b)
        | noble_contracts::ExprKind::Lt(a, b)
        | noble_contracts::ExprKind::Le(a, b)
        | noble_contracts::ExprKind::Add(a, b)
        | noble_contracts::ExprKind::Sub(a, b)
        | noble_contracts::ExprKind::Mul(a, b)
        | noble_contracts::ExprKind::Pair(a, b)
        | noble_contracts::ExprKind::Cons(a, b) => Some((*a, *b)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{compare, parse, render, Names};

    const INCREMENT: &[u8] =
        include_bytes!("../../../../../verification/mc1/increment.noble-contract");

    fn prepared() -> noble_contracts::Prepared {
        noble_contracts::prepare(INCREMENT, crate::core::SOURCE_LIMITS)
            .expect("the frozen increment contract prepares")
    }

    fn names() -> (
        std::vec::Vec<std::string::String>,
        std::vec::Vec<std::string::String>,
    ) {
        (
            std::vec::Vec::from([std::string::String::from("x")]),
            std::vec::Vec::from([std::string::String::from("y")]),
        )
    }

    #[test]
    fn law_terms_compare_with_the_generated_obligation() {
        let prepared = prepared();
        let (inputs, outputs) = names();
        let names = Names::new(&inputs, &outputs);
        let ensures = parse("(eq (out y) (add (in x) 1))", &names)
            .expect("the law's ensures is in the closed grammar");
        assert!(compare(&ensures, &prepared, prepared.ensures()).is_ok());
        assert_eq!(
            render(&prepared, prepared.ensures()).expect("renders"),
            "(eq (out y) (add (in x) 1))"
        );
        let requires = parse("true", &names).expect("the law's requires is closed");
        assert!(compare(&requires, &prepared, prepared.requires()).is_ok());
        assert_eq!(
            render(&prepared, prepared.requires()).expect("renders"),
            "true"
        );
    }

    #[test]
    fn an_easier_statement_does_not_match() {
        let prepared = prepared();
        let (inputs, outputs) = names();
        let names = Names::new(&inputs, &outputs);
        let identity = parse("(eq (out y) (in x))", &names).expect("closed grammar");
        assert!(compare(&identity, &prepared, prepared.ensures()).is_err());
    }

    #[test]
    fn unsupported_or_malformed_terms_are_refused() {
        let (inputs, outputs) = names();
        let names = Names::new(&inputs, &outputs);
        for text in [
            "(definition foo)",
            "(param p)",
            "(add 1)",
            "(add 1 2 3)",
            "true false",
            "(unknown 1 2)",
            "(in z)",
        ] {
            assert!(parse(text, &names).is_err(), "{text} must refuse");
        }
    }
}
