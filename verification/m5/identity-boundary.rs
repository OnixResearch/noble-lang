//! The WIT build binding includes the selected versioned world. Named body
//! identities, in contrast, are interned from resolved composition within their
//! namespace. Equal numeric IDs from different sessions are not portable IDs.
//!
//! ```
//! use noble_contracts::{component::World, source::Session, Limits};
//! use noble_kernel::types::Ty;
//! fn install(session: &mut Session, body: &[u8], limits: Limits) {
//!     let prepared = session.prepare(body, &[], limits).expect("definition must type-check");
//!     session.commit(prepared).expect("same-namespace commit");
//! }
//! let limits = Limits { bytes: 65_536, nodes: 16_384, depth: 64, work: 1_000_000 };
//! let before = b"package noble-test:math@1.0.0; interface arithmetic { inc: func(x: s64) -> s64; dec: func(x: s64) -> s64; } world demo { import arithmetic; }";
//! let after = b"package noble-test:math@1.1.0; interface arithmetic { inc: func(x: s64) -> s64; dec: func(x: s64) -> s64; } world demo { import arithmetic; }";
//! let before = World::parse(before, "demo", limits).expect("version 1.0 world");
//! let after = World::parse(after, "demo", limits).expect("version 1.1 world");
//! assert_ne!(before.build_context(), after.build_context());
//! for world in [&before, &after] {
//!     let mut session = world.session().expect("generated typed bindings");
//!     install(&mut session, b"def a [ arithmetic.inc ]", limits);
//!     install(&mut session, b"def same [arithmetic.inc # name and spacing do not change meaning\n]", limits);
//!     install(&mut session, b"def other [ arithmetic.dec ]", limits);
//!     let prepared = session.prepare(b"a same other", &[Ty::I64], limits).expect("typed named calls");
//!     let submission = prepared.submission().expect("executable candidate");
//!     assert_eq!(submission.definitions.len(), 3);
//!     assert_eq!(submission.definitions[0].identity, submission.definitions[1].identity);
//!     assert_ne!(submission.definitions[0].identity, submission.definitions[2].identity);
//!     assert_eq!(submission.request.expected.allowed_effects.as_slice().len(), 2);
//! }
//! ```
