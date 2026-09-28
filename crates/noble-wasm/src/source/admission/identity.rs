#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; identity admission appends only to the prospective compiler's owned identity table and fresh recipe buffers while charging its private meter; borrowed submissions and published identities remain immutable."
)]

struct Encoding {
    out: crate::output::Buffer,
    todo: alloc::vec::Vec<Option<noble_kernel::untrusted::NodeId>>,
    fuel: usize,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the owned recipe worklist preserves source order while charging every step; explicit fuel, queue and output limits plus checked node lookup return diagnostics, never assertions on submitted identities."
)]
fn encode(
    submission: &noble_kernel::execution::Submission,
    body: &noble_kernel::execution::Body,
    work: &mut super::super::Work,
) -> Result<alloc::vec::Vec<u8>, crate::Diagnostic> {
    let mut encoding = Encoding {
        out: crate::output::Buffer::new(128),
        todo: alloc::vec::Vec::with_capacity(body.candidate.body.len()),
        fuel: super::super::NODE_LIMIT.saturating_mul(4),
    };
    let mut index = body.candidate.body.len();
    while index != 0 {
        index -= 1;
        encoding.todo.push(Some(body.candidate.body[index]));
    }
    let mut failure = None;
    while let Some(step) = encoding.todo.pop() {
        match encoding.step(submission, body, step, work) {
            Ok(()) => {}
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(encoding.out.finish()),
    }
}

impl Encoding {
    fn step(
        &mut self,
        submission: &noble_kernel::execution::Submission,
        body: &noble_kernel::execution::Body,
        step: Option<noble_kernel::untrusted::NodeId>,
        work: &mut super::super::Work,
    ) -> Result<(), crate::Diagnostic> {
        attempt!(work.entries(submission.definitions.len().saturating_add(1)));
        if self.fuel == 0 || self.todo.len() > super::super::NODE_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        self.fuel = self.fuel.saturating_sub(1);
        let node = match step {
            None => return self.out.append(b"]"),
            Some(node) => node,
        };
        match attempt!(super::node(&body.candidate, node)) {
            noble_kernel::untrusted::Node::Literal { lit, .. } => {
                literal(&mut self.out, lit, body, node)
            }
            noble_kernel::untrusted::Node::Invocation { def, .. } => {
                if def.0 < super::super::builtin_count(&submission.environment) {
                    attempt!(self.out.append(b"b"));
                    attempt!(self.out.number(u64::from(def.0)));
                } else {
                    match submission.environment.kind(*def) {
                        Some(noble_kernel::contracts::Behavior::Named) => {
                            attempt!(self.out.append(b"d"));
                            let index = attempt!(super::definition_index(submission, *def));
                            attempt!(self.out.number(submission.definitions[index].identity));
                        }
                        Some(noble_kernel::contracts::Behavior::BoundEmit(slot)) => {
                            let mut found = None;
                            let mut at = 0usize;
                            while at < submission.environment.bound_adapters.len() {
                                let row = &submission.environment.bound_adapters[at];
                                if row.definition == *def && row.adapter_slot == slot {
                                    found = Some(row);
                                    break;
                                }
                                at += 1;
                            }
                            let binding = match found {
                                Some(binding) => binding,
                                None => return Err(crate::Diagnostic::Invalid),
                            };
                            attempt!(work.entries(binding.adapter_identity.len()));
                            attempt!(self.out.append(b"a"));
                            attempt!(self.out.number(u64::from(slot)));
                            attempt!(self.out.append(b":"));
                            attempt!(self.out.index(binding.adapter_identity.len()));
                            attempt!(self.out.append(b":"));
                            attempt!(self.out.append(binding.adapter_identity.as_bytes()));
                        }
                        Some(kind) => {
                            let (tag, id) = match kind {
                                noble_kernel::contracts::Behavior::NominalNew(id) => (b'N', id),
                                noble_kernel::contracts::Behavior::NominalInto(id) => (b'I', id),
                                noble_kernel::contracts::Behavior::NominalLeft(id) => (b'L', id),
                                noble_kernel::contracts::Behavior::NominalRight(id) => (b'R', id),
                                noble_kernel::contracts::Behavior::NominalMatch(id) => (b'M', id),
                                noble_kernel::contracts::Behavior::Dup
                                | noble_kernel::contracts::Behavior::Drop
                                | noble_kernel::contracts::Behavior::Swap
                                | noble_kernel::contracts::Behavior::Dip
                                | noble_kernel::contracts::Behavior::Arith
                                | noble_kernel::contracts::Behavior::Equals
                                | noble_kernel::contracts::Behavior::Quote
                                | noble_kernel::contracts::Behavior::Compose
                                | noble_kernel::contracts::Behavior::Run
                                | noble_kernel::contracts::Behavior::Reflect
                                | noble_kernel::contracts::Behavior::Unit
                                | noble_kernel::contracts::Behavior::Pair
                                | noble_kernel::contracts::Behavior::Unpair
                                | noble_kernel::contracts::Behavior::Inl
                                | noble_kernel::contracts::Behavior::Inr
                                | noble_kernel::contracts::Behavior::Case
                                | noble_kernel::contracts::Behavior::If
                                | noble_kernel::contracts::Behavior::Nil
                                | noble_kernel::contracts::Behavior::Cons
                                | noble_kernel::contracts::Behavior::ListCase
                                | noble_kernel::contracts::Behavior::TestEmit
                                | noble_kernel::contracts::Behavior::Named
                                | noble_kernel::contracts::Behavior::BoundEmit(_) => {
                                    return Err(crate::Diagnostic::Invalid);
                                }
                            };
                            attempt!(self.out.append(&[tag]));
                            attempt!(self.out.number(id.module));
                            attempt!(self.out.append(b":"));
                            attempt!(self.out.number(u64::from(id.ordinal)));
                        }
                        None => return Err(crate::Diagnostic::Invalid),
                    }
                }
                self.out.append(b";")
            }
            noble_kernel::untrusted::Node::Quotation { body, .. } => {
                attempt!(self.out.append(b"["));
                self.todo.reserve(body.len().saturating_add(1));
                self.todo.push(None);
                let mut index = body.len();
                while index != 0 {
                    index -= 1;
                    self.todo.push(Some(body[index]));
                }
                Ok(())
            }
        }
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; literal identity encoding writes to an allocating bounded output buffer and resolves text metadata through non-const lookup; the borrowed literal and body remain unchanged."
)]
fn literal(
    out: &mut crate::output::Buffer,
    lit: &noble_kernel::untrusted::Lit,
    body: &noble_kernel::execution::Body,
    node: noble_kernel::untrusted::NodeId,
) -> Result<(), crate::Diagnostic> {
    match lit {
        noble_kernel::untrusted::Lit::I64(value) => {
            attempt!(out.append(b"i"));
            out.i64(*value)
        }
        noble_kernel::untrusted::Lit::Bool(value) => out.append(if *value { b"t" } else { b"f" }),
        noble_kernel::untrusted::Lit::Unit => out.append(b"u"),
        noble_kernel::untrusted::Lit::Text => {
            let bytes = attempt!(super::text(body, node));
            attempt!(out.append(b"s"));
            attempt!(out.index(bytes.len()));
            attempt!(out.append(b":"));
            out.append(bytes)
        }
    }
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the prospective compiler owns this identity table and must append newly admitted immutable recipes, requiring Vec growth; metered identity conflicts and capacity exhaustion return diagnostics without mutating existing recipes."
)]
pub(in crate::source) fn check(
    submission: &noble_kernel::execution::Submission,
    identities: &mut alloc::vec::Vec<super::super::Identity>,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut failure = None;
    while index < submission.definitions.len() {
        match admit(submission, index, identities, work) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the prospective compiler owns this bounded growable identity table; existing recipes are compared under the shared meter and never overwritten, while conflicts and capacity exhaustion return diagnostics."
)]
fn admit(
    submission: &noble_kernel::execution::Submission,
    index: usize,
    identities: &mut alloc::vec::Vec<super::super::Identity>,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let definition = &submission.definitions[index];
    let recipe = attempt!(encode(submission, &definition.body, work));
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
        if identities.len() >= super::super::DEFINITION_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        identities.push(super::super::Identity {
            id: definition.identity,
            recipe,
        });
    }
    Ok(())
}
