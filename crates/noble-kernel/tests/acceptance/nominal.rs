fn opaque(
    id: noble_kernel::types::NominalTypeId,
    inner: noble_kernel::types::Ty,
    public: bool,
) -> noble_kernel::contracts::NominalDecl {
    noble_kernel::contracts::NominalDecl {
        id,
        shape: noble_kernel::types::NominalShape::Opaque(Box::new(inner)),
        exported: true,
        public: [public, public],
    }
}

fn variant(
    id: noble_kernel::types::NominalTypeId,
    left: noble_kernel::types::Ty,
    right: noble_kernel::types::Ty,
    public: [bool; 2],
) -> noble_kernel::contracts::NominalDecl {
    noble_kernel::contracts::NominalDecl {
        id,
        shape: noble_kernel::types::NominalShape::Variant(Box::new(left), Box::new(right)),
        exported: true,
        public,
    }
}

fn declared_type(decl: &noble_kernel::contracts::NominalDecl) -> noble_kernel::types::Ty {
    noble_kernel::types::Ty::Nominal(decl.id, Box::new(decl.shape.clone()))
}

fn env() -> Result<noble_kernel::contracts::Env, String> {
    noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))
}

fn check(
    env: &noble_kernel::contracts::Env,
    expected: noble_kernel::untrusted::Request,
    program: noble_kernel::untrusted::Candidate,
) -> noble_kernel::untrusted::Outcome {
    noble_kernel::acceptance::check(env, &expected, &program)
}

fn single(
    def: noble_kernel::contracts::Definition,
    bindings: Vec<noble_kernel::words::Binding>,
) -> noble_kernel::untrusted::Candidate {
    super::support::candidate(vec![super::support::invocation(def, bindings)], vec![0])
}

fn rejected(
    outcome: noble_kernel::untrusted::Outcome,
    expected: noble_kernel::untrusted::Constraint,
) {
    assert!(
        matches!(&outcome, noble_kernel::untrusted::Outcome::Invalid(problem) if problem.constraint == expected),
        "expected {expected:?} rejection, got {outcome:?}"
    );
}

trait Required<T> {
    fn require(self, context: &'static str) -> Result<T, String>;
}

impl<T> Required<T> for Option<T> {
    fn require(self, context: &'static str) -> Result<T, String> {
        self.ok_or_else(|| format!("missing {context}"))
    }
}

impl<T, E: core::fmt::Debug> Required<T> for Result<T, E> {
    fn require(self, context: &'static str) -> Result<T, String> {
        self.map_err(|error| format!("{context}: {error:?}"))
    }
}

macro_rules! required {
    ($value:expr, $context:literal) => {{
        match crate::nominal::Required::require($value, $context) {
            Ok(value) => value,
            Err(error) => return Err(error),
        }
    }};
}

macro_rules! declare {
    ($env:ident, $decl:expr, $context:literal) => {{
        let (next, ops) = required!($env.declare_nominal($decl), $context);
        $env = next;
        ops
    }};
}

#[path = "nominal/eligibility.rs"]
mod eligibility;
#[path = "nominal/forgery.rs"]
mod forgery;
#[path = "nominal/generic.rs"]
mod generic;
#[path = "nominal/identity.rs"]
mod identity;
#[path = "nominal/matching.rs"]
mod matching;
#[path = "nominal/visibility.rs"]
mod visibility;
