//! Selected declared-text lexical sugar. No local instruction reaches the kernel.
//!
//! Physical stacks place immutable bound values above the visible stack.
//! Checked `dip` executes ordinary words beneath them, while `dup` and
//! `swap` project a bound value into the visible stack. A quotation captures
//! its free values at construction with checked `quote` and `compose`; its
//! recipe therefore retains the original values, not source spellings.

use alloc::vec::Vec;

struct Bound {
    name: Vec<u8>,
}

struct Lower<'a> {
    nodes: &'a mut Vec<super::Node>,
    meter: &'a mut crate::Meter,
}

pub(super) fn lower(tree: &mut super::Tree, meter: &mut crate::Meter) -> Result<(), crate::Diagnostic> {
    if !tree.nodes.iter().any(|node| matches!(&node.kind, super::Kind::Word(word) if word == b"let")) {
        return Ok(());
    }
    let body = core::mem::take(&mut tree.body);
    let mut lower = Lower { nodes: &mut tree.nodes, meter };
    tree.body = attempt!(lower.body(&body, Vec::new(), 0));
    Ok(())
}

fn name(bytes: &[u8]) -> bool {
    let Some((&first, rest)) = bytes.split_first() else { return false; };
    matches!(first, b'a'..=b'z' | b'A'..=b'Z' | b'_')
        && rest.iter().all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        && bytes != b"let"
}

fn source_node(nodes: &[super::Node], id: u32, span: crate::Span) -> Result<&super::Node, crate::Diagnostic> {
    nodes.get(attempt!(crate::offset(id, span))).ok_or_else(|| crate::internal(span))
}

/// Check lexical free occurrences before moving any quotation body. `shadow`
/// includes binders introduced inside the quoted body, not the outer locals.
fn free(
    nodes: &[super::Node], body: &[u32], outer: &[Bound], shadow: &mut Vec<Vec<u8>>,
    used: &mut [bool], meter: &mut crate::Meter, depth: u32, span: crate::Span,
) -> Result<(), crate::Diagnostic> {
    attempt!(meter.depth(depth, span));
    let scope = shadow.len();
    let mut at = 0;
    while at < body.len() {
        attempt!(meter.charge(1, span));
        let node = attempt!(source_node(nodes, body[at], span));
        match &node.kind {
            super::Kind::Word(word) if word == b"let" => {
                let next = body.get(at + 1).copied().ok_or_else(|| crate::invalid(node.span, "let requires a local name"))?;
                let bind = attempt!(source_node(nodes, next, node.span));
                let super::Kind::Word(name_bytes) = &bind.kind else {
                    return Err(crate::invalid(bind.span, "let requires a local name"));
                };
                if !name(name_bytes) || shadow[scope..].iter().any(|known| known == name_bytes) {
                    return Err(crate::invalid(bind.span, "duplicate or invalid local name"));
                }
                attempt!(meter.node(bind.span));
                shadow.push(name_bytes.clone());
                at += 2;
                continue;
            }
            super::Kind::Word(word) => {
                let mut hidden = false;
                for local in shadow.iter().rev() {
                    attempt!(meter.charge(1, node.span));
                    if local == word { hidden = true; break; }
                }
                if !hidden {
                    for (index, local) in outer.iter().enumerate() {
                        attempt!(meter.charge(1, node.span));
                        if local.name == *word {
                            used[index] = true;
                            break;
                        }
                    }
                }
            }
            super::Kind::Quotation(nested) => {
                let deeper = depth.checked_add(1).ok_or_else(|| crate::invalid(node.span, "lexical quotation depth overflow"))?;
                attempt!(free(nodes, nested, outer, shadow, used, meter, deeper, node.span));
            }
            _ => {}
        }
        at += 1;
    }
    shadow.truncate(scope);
    Ok(())
}

impl Lower<'_> {
    fn node(&mut self, kind: super::Kind, span: crate::Span) -> Result<u32, crate::Diagnostic> {
        attempt!(self.meter.node(span));
        let id = attempt!(crate::index(self.nodes.len(), span));
        self.nodes.push(super::Node { kind, span });
        Ok(id)
    }

    fn builtin(&mut self, definition: u32, span: crate::Span) -> Result<u32, crate::Diagnostic> {
        self.node(super::Kind::Call(super::Target::Builtin(definition)), span)
    }

    fn wrapped(&mut self, id: u32, hidden: usize, depth: u32, span: crate::Span)
        -> Result<Vec<u32>, crate::Diagnostic>
    {
        let mut code = alloc::vec![id];
        for layer in 0..hidden {
            let count = usize::try_from(depth).unwrap_or(usize::MAX)
                .checked_add(layer).and_then(|value| value.checked_add(1))
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| crate::invalid(span, "lexical quotation depth overflow"))?;
            attempt!(self.meter.depth(count, span));
            let quotation = attempt!(self.node(super::Kind::Quotation(code), span));
            let dip = attempt!(self.builtin(3, span));
            code = alloc::vec![quotation, dip];
        }
        Ok(code)
    }

    fn operation(&mut self, code: &mut Vec<u32>, definition: u32, hidden: usize,
        depth: u32, span: crate::Span) -> Result<(), crate::Diagnostic>
    {
        let id = attempt!(self.builtin(definition, span));
        code.extend(attempt!(self.wrapped(id, hidden, depth, span)));
        Ok(())
    }

    fn lookup(meter: &mut crate::Meter, locals: &[Bound], word: &[u8], span: crate::Span)
        -> Result<Option<usize>, crate::Diagnostic>
    {
        for (index, local) in locals.iter().enumerate() {
            attempt!(meter.charge(1, span));
            if local.name == word { return Ok(Some(index)); }
        }
        Ok(None)
    }

    /// Project a local at physical index `which` below *all* hidden values.
    /// Each `dup` or `swap` is itself an ordinary kernel-checked builtin.
    fn read(&mut self, code: &mut Vec<u32>, which: usize, locals: usize,
        depth: u32, span: crate::Span) -> Result<(), crate::Diagnostic>
    {
        if which >= locals { return Err(crate::internal(span)); }
        attempt!(self.operation(code, 0, which, depth, span));
        for older in 0..(locals - which - 1) {
            let hidden = which.checked_add(1).and_then(|count| count.checked_add(older))
                .ok_or_else(|| crate::internal(span))?;
            attempt!(self.operation(code, 2, hidden, depth, span));
        }
        Ok(())
    }

    fn quote(&mut self, id: u32, outer: &[Bound], depth: u32, span: crate::Span)
        -> Result<Vec<u32>, crate::Diagnostic>
    {
        let nested = {
            let node = self.nodes.get_mut(attempt!(crate::offset(id, span)))
                .ok_or_else(|| crate::internal(span))?;
            let super::Kind::Quotation(body) = &mut node.kind else { return Err(crate::internal(span)); };
            core::mem::take(body)
        };
        let mut used = alloc::vec![false; outer.len()];
        let deeper = depth.checked_add(1).ok_or_else(|| crate::invalid(span, "lexical quotation depth overflow"))?;
        attempt!(free(self.nodes, &nested, outer, &mut Vec::new(), &mut used,
            self.meter, deeper, span));
        let mut captured = Vec::new();
        for (index, local) in outer.iter().enumerate() {
            if used[index] {
                attempt!(self.meter.node(span));
                captured.push(Bound { name: local.name.clone() });
            }
        }
        let mut lowered = Vec::new();
        let mut first = true;
        // `outer[0]` is the newest/top binder. Capture oldest first, so
        // replaying the composed programs reconstructs that same stack order.
        for index in (0..used.len()).rev() {
            if used[index] {
                attempt!(self.read(&mut lowered, index, outer.len(), depth, span));
                attempt!(self.operation(&mut lowered, 8, outer.len(), depth, span));
                if !first { attempt!(self.operation(&mut lowered, 9, outer.len(), depth, span)); }
                first = false;
            }
        }
        let body = attempt!(self.body(&nested, captured, deeper));
        let node = self.nodes.get_mut(attempt!(crate::offset(id, span)))
            .ok_or_else(|| crate::internal(span))?;
        node.kind = super::Kind::Quotation(body);
        lowered.extend(attempt!(self.wrapped(id, outer.len(), depth, span)));
        if !first { attempt!(self.operation(&mut lowered, 9, outer.len(), depth, span)); }
        Ok(lowered)
    }

    fn body(&mut self, source: &[u32], mut locals: Vec<Bound>, depth: u32)
        -> Result<Vec<u32>, crate::Diagnostic>
    {
        let mut code = Vec::new();
        let mut introduced = 0usize;
        let mut at = 0usize;
        while at < source.len() {
            let id = source[at];
            let node = attempt!(source_node(self.nodes, id, crate::Span { start: 0, end: 0 }));
            let span = node.span;
            let is_binding = matches!(&node.kind, super::Kind::Word(word) if word == b"let");
            if is_binding {
                let next = source.get(at + 1).copied().ok_or_else(|| crate::invalid(span, "let requires a local name"))?;
                let target = attempt!(source_node(self.nodes, next, span));
                let super::Kind::Word(name_bytes) = &target.kind else {
                    return Err(crate::invalid(target.span, "let requires a local name"));
                };
                if !name(name_bytes) || locals[..introduced].iter().any(|prior| prior.name == *name_bytes) {
                    return Err(crate::invalid(target.span, "duplicate or invalid local name"));
                }
                attempt!(self.meter.depth(u32::try_from(locals.len() + 1)
                    .map_err(|_| crate::invalid(span, "lexical binding depth overflow"))?, span));
                // The new value is just below the previously hidden locals.
                // Move it above them before installing it as the newest
                // binder; `swap` moves ownership without copying or dropping.
                for hidden in (0..locals.len()).rev() {
                    attempt!(self.operation(&mut code, 2, hidden, depth, span));
                }
                let bound = core::mem::replace(&mut self.nodes[attempt!(crate::offset(next, span))].kind,
                    super::Kind::Quotation(Vec::new()));
                let super::Kind::Word(name_bytes) = bound else { return Err(crate::internal(span)); };
                self.nodes[attempt!(crate::offset(id, span))].kind = super::Kind::Quotation(Vec::new());
                locals.insert(0, Bound { name: name_bytes });
                introduced += 1;
                at += 2;
                continue;
            }
            let local = match &node.kind {
                super::Kind::Word(word) => attempt!(Self::lookup(self.meter, &locals, word, span)),
                _ => None,
            };
            if let Some(which) = local {
                self.nodes[attempt!(crate::offset(id, span))].kind = super::Kind::Quotation(Vec::new());
                attempt!(self.read(&mut code, which, locals.len(), depth, span));
            } else if matches!(&node.kind, super::Kind::Quotation(_)) {
                code.extend(attempt!(self.quote(id, &locals, depth, span)));
            } else {
                code.extend(attempt!(self.wrapped(id, locals.len(), depth, span)));
            }
            at += 1;
        }
        for _ in 0..locals.len() {
            code.push(attempt!(self.builtin(1, crate::Span { start: 0, end: 0 })));
        }
        Ok(code)
    }
}
