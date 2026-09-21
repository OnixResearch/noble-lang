pub(super) mod paths;

pub(super) fn check(
    inputs: &[noble_kernel::types::Ty],
    has_test_hosts: bool,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    if inputs.len() > attempt!(crate::offset(crate::inference::STACK_CAP, span)) {
        return Err(super::exhausted(span, "source input stack limit exceeded"));
    }
    let mut at = 0usize;
    let mut failure = None;
    while at < inputs.len() {
        if let Err(problem) = value(&inputs[at], has_test_hosts, span, meter) {
            failure = Some(problem);
            break;
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

#[derive(Clone, Copy)]
pub(super) enum PathStep {
    Root,
    Left,
    Right,
    Item,
    Input(usize),
    Output(usize),
}

struct Visit {
    step: PathStep,
    depth: u32,
}

struct Traversal {
    pending: alloc::vec::Vec<Visit>,
    path: alloc::vec::Vec<PathStep>,
    type_nodes: usize,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the owned continuation traversal checks constructor, work and depth bounds before following each borrowed input node, retaining refusals rather than asserting on caller types."
)]
fn value(
    input: &noble_kernel::types::Ty,
    has_test_hosts: bool,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let mut walk = Traversal {
        pending: alloc::vec::Vec::with_capacity(1),
        path: alloc::vec::Vec::new(),
        type_nodes: 1,
    };
    attempt!(meter.node(span));
    walk.pending.push(Visit {
        step: PathStep::Root,
        depth: 0,
    });
    let mut failure = None;
    while let Some(entry) = walk.pending.pop() {
        if let Err(problem) = walk.visit(entry, input, has_test_hosts, span, meter) {
            failure = Some(problem);
            break;
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

impl Traversal {
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; charged input traversal checks runtime limits and reserves owned continuation capacity, with owned diagnostics on rejection."
    )]
    fn visit(
        &mut self,
        entry: Visit,
        root: &noble_kernel::types::Ty,
        has_test_hosts: bool,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        let Visit { step, depth } = entry;
        attempt!(meter.charge(1, span));
        attempt!(meter.depth(depth, span));
        // LIFO siblings share the prefix above their depth. Both the path and
        // pending selectors are bounded by the admitted constructor count.
        self.path.truncate(attempt!(crate::offset(depth, span)));
        self.path.push(step);
        let ty = attempt!(paths::locate(root, &self.path, span));
        let children = match ty {
            noble_kernel::types::Ty::Pair(_, _) | noble_kernel::types::Ty::Sum(_, _) => 2,
            noble_kernel::types::Ty::List(_) => 1,
            noble_kernel::types::Ty::Program(input, output, _) => {
                input.len().saturating_add(output.len())
            }
            noble_kernel::types::Ty::Unit
            | noble_kernel::types::Ty::Bool
            | noble_kernel::types::Ty::I64
            | noble_kernel::types::Ty::Text
            | noble_kernel::types::Ty::Syntax
            | noble_kernel::types::Ty::Resource(_) => 0,
        };
        self.type_nodes = self.type_nodes.saturating_add(children);
        if self.type_nodes > attempt!(crate::offset(crate::syntax::TYPE_CAP, span)) {
            return Err(super::exhausted(
                span,
                "source input exceeds the 256-constructor type limit",
            ));
        }
        self.pending.reserve(children);
        self.expand(ty, depth, has_test_hosts, span, meter)
    }

    fn expand(
        &mut self,
        ty: &noble_kernel::types::Ty,
        depth: u32,
        has_test_hosts: bool,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        let next_depth = depth.saturating_add(1);
        match ty {
            noble_kernel::types::Ty::Resource(_) => Err(crate::invalid(
                span,
                "resource-bearing input is not eligible for Core-Bootstrap data or capture",
            )),
            noble_kernel::types::Ty::Pair(_, _) | noble_kernel::types::Ty::Sum(_, _) => {
                attempt!(meter.node(span));
                attempt!(meter.node(span));
                self.pending.push(Visit {
                    step: PathStep::Left,
                    depth: next_depth,
                });
                self.pending.push(Visit {
                    step: PathStep::Right,
                    depth: next_depth,
                });
                Ok(())
            }
            noble_kernel::types::Ty::List(_) => {
                attempt!(meter.node(span));
                self.pending.push(Visit {
                    step: PathStep::Item,
                    depth: next_depth,
                });
                Ok(())
            }
            noble_kernel::types::Ty::Program(input, output, effects) => {
                let cap = attempt!(crate::offset(crate::inference::STACK_CAP, span));
                if input.len() > cap || output.len() > cap {
                    return Err(super::exhausted(
                        span,
                        "source program interface stack limit exceeded",
                    ));
                }
                attempt!(host_effects(effects, has_test_hosts, span, meter));
                attempt!(self.schedule(input.len(), false, next_depth, span, meter));
                self.schedule(output.len(), true, next_depth, span, meter)
            }
            noble_kernel::types::Ty::Unit
            | noble_kernel::types::Ty::Bool
            | noble_kernel::types::Ty::I64
            | noble_kernel::types::Ty::Text
            | noble_kernel::types::Ty::Syntax => Ok(()),
        }
    }

    fn schedule(
        &mut self,
        count: usize,
        output: bool,
        depth: u32,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        self.pending.reserve(count);
        let mut at = 0usize;
        let mut failure = None;
        while at < count {
            if let Err(problem) = meter.node(span) {
                failure = Some(problem);
                break;
            }
            let step = if output {
                PathStep::Output(at)
            } else {
                PathStep::Input(at)
            };
            self.pending.push(Visit { step, depth });
            at = at.saturating_add(1);
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(()),
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every latent effect consumes work and must belong to the explicitly enabled two-effect test environment; invalid effects and exhaustion return diagnostics."
)]
fn host_effects(
    effects: &noble_kernel::types::EffSet,
    has_test_hosts: bool,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let effects = effects.as_slice();
    let mut at = 0usize;
    let mut failure = None;
    while at < effects.len() {
        let effect = effects[at];
        if let Err(problem) = meter.charge(1, span) {
            failure = Some(problem);
            break;
        }
        if effect.0 > 1 || !has_test_hosts {
            failure = Some(crate::Diagnostic::new(
                crate::DiagnosticKind::Unsupported,
                span,
                "unknown input program host effect",
            ));
            break;
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}
