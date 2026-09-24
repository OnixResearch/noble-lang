//! Adapter-local borrows have no conversion into a live guest resource, guest
//! literal, or retained source preparation. These are external Rust boundary
//! controls; owning resources inside guest products are deliberately not tested
//! as if they were borrow tokens.
//!
//! Pair / returned-pair:
//! ```compile_fail
//! use noble_kernel::resources::Borrow;
//! use wasmtime::component::Val;
//! fn escape(token: Borrow) -> Val {
//!     Val::Tuple(vec![Val::Resource(token.into())])
//! }
//! ```
//!
//! List:
//! ```compile_fail
//! use noble_kernel::resources::Borrow;
//! use wasmtime::component::Val;
//! fn escape(token: Borrow) -> Val {
//!     Val::List(vec![Val::Resource(token.into())])
//! }
//! ```
//!
//! Quotation capture / quote:
//! ```compile_fail
//! use noble_kernel::{resources::Borrow, untrusted::Node, words::Inst};
//! fn capture(token: Borrow, inst: Inst) -> Node {
//!     Node::Literal { lit: token.into(), inst }
//! }
//! ```
//!
//! Session storage / session:
//! ```compile_fail
//! use noble_kernel::resources::Borrow;
//! use noble_contracts::source::{Session, Error};
//! fn retain(session: &mut Session, token: Borrow) -> Result<(), Error> {
//!     session.commit(token.into())
//! }
//! ```
//!
//! Positive controls keep the exact resource, literal and session ingress APIs
//! live. An absent dependency or an accidentally renamed API must fail this
//! control, not turn every negative snippet into a passing test.
//! ```
//! use noble_kernel::untrusted::{Lit, Node};
//! use noble_kernel::words::Inst;
//! use noble_contracts::source::{Session, Prepared, Error};
//! use wasmtime::component::{ResourceAny, Val};
//! fn pair(owner: ResourceAny) -> Val { Val::Tuple(vec![Val::Resource(owner)]) }
//! fn list(owner: ResourceAny) -> Val { Val::List(vec![Val::Resource(owner)]) }
//! fn literal(lit: Lit, inst: Inst) -> Node { Node::Literal { lit, inst } }
//! fn retain(session: &mut Session, prepared: Prepared) -> Result<(), Error> {
//!     session.commit(prepared)
//! }
//! ```
