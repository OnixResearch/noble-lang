#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; interning mutates only the fresh type registry, prospective compiler and private work meter; descriptor emission writes only its fresh owned sink and all borrowed type stacks remain immutable."
)]

#[derive(Clone, Copy)]
enum Shape {
    Scalar(u32),
    Pair(u32, u32),
    Sum(u32, u32),
    List(u32),
    Program(u32, u32, u32),
    Syntax,
    Text,
}

pub(super) struct Registry {
    keys: alloc::vec::Vec<noble_kernel::types::Ty>,
    shapes: alloc::vec::Vec<Shape>,
}

impl Registry {
    pub(super) const fn new() -> Self {
        Self {
            keys: alloc::vec::Vec::new(),
            shapes: alloc::vec::Vec::new(),
        }
    }

    fn intern(
        &mut self,
        ty: &noble_kernel::types::Ty,
        work: &mut super::Work,
    ) -> Result<u32, crate::Diagnostic> {
        // Every accepted type has at most 512 structural nodes. Reserve the
        // complete equality/clone traversal before touching each interned type.
        attempt!(work.charge(512));
        if let Some(index) = attempt!(self.find(ty, work)) {
            return super::plan::number(index);
        }
        if self.keys.len() >= super::NODE_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        let id = attempt!(super::plan::number(self.keys.len()));
        self.keys.push(ty.clone());
        Ok(id)
    }

    fn find(
        &self,
        ty: &noble_kernel::types::Ty,
        work: &mut super::Work,
    ) -> Result<Option<usize>, crate::Diagnostic> {
        let mut index = 0usize;
        let mut found = None;
        let mut failure = None;
        while index < self.keys.len() {
            match self.key_matches(index, ty, work) {
                Ok(true) => {
                    found = Some(index);
                    break;
                }
                Ok(false) => index += 1,
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(found),
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; the live-pool lookup reserves bounded structural comparison work before invoking the kernel type's non-const equality implementation."
    )]
    fn key_matches(
        &self,
        index: usize,
        ty: &noble_kernel::types::Ty,
        work: &mut super::Work,
    ) -> Result<bool, crate::Diagnostic> {
        attempt!(work.charge(512));
        Ok(self.keys[index] == *ty)
    }

    pub(super) fn stack(
        &mut self,
        stack: &[noble_kernel::types::Ty],
        compiler: &mut super::Compiler,
        work: &mut super::Work,
    ) -> Result<alloc::vec::Vec<u32>, crate::Diagnostic> {
        let mut result = alloc::vec::Vec::with_capacity(stack.len());
        let mut index = 0usize;
        let mut failure = None;
        while index < stack.len() {
            match self.intern(&stack[index], work) {
                Ok(id) => {
                    result.push(id);
                    index += 1;
                }
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        if let Some(problem) = failure {
            return Err(problem);
        }
        let mut failure = None;
        while self.shapes.len() < self.keys.len() {
            match self.next_shape(compiler, work) {
                Ok(shape) => self.shapes.push(shape),
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(result),
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; each owned type clone is charged before inspection and child interning allocates in the prospective registry and live signature pool, so shape construction cannot be const."
    )]
    fn next_shape(
        &mut self,
        compiler: &mut super::Compiler,
        work: &mut super::Work,
    ) -> Result<Shape, crate::Diagnostic> {
        attempt!(work.charge(512));
        let ty = self.keys[self.shapes.len()].clone();
        let shape = match ty {
            noble_kernel::types::Ty::I64 => Shape::Scalar(1),
            noble_kernel::types::Ty::Bool => Shape::Scalar(2),
            noble_kernel::types::Ty::Unit => Shape::Scalar(3),
            noble_kernel::types::Ty::Text => Shape::Text,
            noble_kernel::types::Ty::Syntax => Shape::Syntax,
            noble_kernel::types::Ty::Resource(_) => return Err(crate::Diagnostic::Unsupported),
            noble_kernel::types::Ty::Pair(left, right) => Shape::Pair(
                attempt!(self.intern(&left, work)),
                attempt!(self.intern(&right, work)),
            ),
            noble_kernel::types::Ty::Sum(left, right) => Shape::Sum(
                attempt!(self.intern(&left, work)),
                attempt!(self.intern(&right, work)),
            ),
            noble_kernel::types::Ty::List(item) => Shape::List(attempt!(self.intern(&item, work))),
            noble_kernel::types::Ty::Program(input, output, effects) => Shape::Program(
                attempt!(compiler.signature(&input, work)),
                attempt!(compiler.signature(&output, work)),
                attempt!(super::plan::effect_mask(&effects)),
            ),
        };
        Ok(shape)
    }

    pub(super) fn emit(&self, out: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
        let mut index = 0usize;
        let mut failure = None;
        while index < self.shapes.len() {
            match emit_shape(out, index, self.shapes[index]) {
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
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; Shape is the closed semantic storage vocabulary and every variant needs its explicit observation predicate; emitting to the allocating bounded sink cannot be const and output failures propagate diagnostics rather than assertions."
)]
fn emit_shape(
    out: &mut crate::output::Buffer,
    index: usize,
    shape: Shape,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func $t"));
    attempt!(out.index(index));
    attempt!(out.append(b" (param $tag i32) (param $value i64) (result i32) (local $h i32)\n(if (i32.eqz (call $observation_tick)) (then (return (i32.const 0))))\n"));
    match shape {
        Shape::Scalar(tag) => {
            attempt!(out.append(b"(if (i32.ne (local.get $tag) "));
            attempt!(out.i32(tag));
            attempt!(out.append(b") (then (return (i32.const 0))))\n"));
            match tag {
                2 => attempt!(out.append(b"(i64.le_u (local.get $value) (i64.const 1))\n")),
                3 => attempt!(out.append(b"(i64.eqz (local.get $value))\n")),
                _ => attempt!(out.append(b"(i32.const 1)\n")),
            }
        }
        Shape::Program(input, output_signature, effects) => {
            attempt!(reference(out, 4));
            attempt!(out.append(b"(i32.and (i32.eq (call $y (local.get $h)) "));
            attempt!(out.i32(input));
            attempt!(out.append(b") (i32.and (i32.eq (call $z (local.get $h)) "));
            attempt!(out.i32(output_signature));
            attempt!(out.append(b") (i64.eq (call $payload (local.get $h)) "));
            attempt!(out.i64(i64::from(effects)));
            attempt!(out.append(b")))\n"));
        }
        Shape::Text => {
            attempt!(reference(out, 11));
            attempt!(out.append(b"(i32.const 1)\n"));
        }
        Shape::Syntax => {
            attempt!(reference(out, 10));
            attempt!(out.append(b"(i32.const 1)\n"));
        }
        Shape::Pair(left, right) => {
            attempt!(reference(out, 5));
            attempt!(out.append(b"(if (i32.eqz "));
            attempt!(child(out, left, b"a"));
            attempt!(out.append(b") (then (return (i32.const 0))))\n"));
            attempt!(child(out, right, b"b"));
            attempt!(out.append(b"\n"));
        }
        Shape::Sum(left, right) => {
            attempt!(out.append(b"(if (i32.and (i32.ne (local.get $tag) (i32.const 12)) (i32.ne (local.get $tag) (i32.const 13))) (then (return (i32.const 0))))\n"));
            attempt!(live(out));
            attempt!(
                out.append(b"(if (result i32) (i32.eq (local.get $tag) (i32.const 12)) (then ")
            );
            attempt!(child(out, left, b"a"));
            attempt!(out.append(b") (else "));
            attempt!(child(out, right, b"a"));
            attempt!(out.append(b"))\n"));
        }
        Shape::List(item) => {
            attempt!(out.append(b"(if (i32.and (i32.ne (local.get $tag) (i32.const 6)) (i32.ne (local.get $tag) (i32.const 7))) (then (return (i32.const 0))))\n"));
            attempt!(live(out));
            attempt!(out.append(b"(loop $list\n(if (i32.eqz (call $observation_tick)) (then (return (i32.const 0))))\n(if (i32.eq (call $kind (local.get $h)) (i32.const 7)) (then (return (i32.const 1))))\n(if (i32.ne (call $kind (local.get $h)) (i32.const 6)) (then (return (i32.const 0))))\n(if (i32.eqz "));
            attempt!(child(out, item, b"a"));
            attempt!(out.append(b") (then (return (i32.const 0))))\n(local.set $h (call $b (local.get $h)))\n(br $list))\n(i32.const 0)\n"));
        }
    }
    out.append(b")\n")
}

fn live(out: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
    out.append(b"(if (i64.gt_u (local.get $value) (i64.const 4294967295)) (then (return (i32.const 0))))\n(local.set $h (i32.wrap_i64 (local.get $value)))\n(if (i32.eqz (call $is_live (local.get $h))) (then (return (i32.const 0))))\n(if (i32.ne (call $kind (local.get $h)) (local.get $tag)) (then (return (i32.const 0))))\n")
}

fn reference(out: &mut crate::output::Buffer, tag: u32) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(if (i32.ne (local.get $tag) "));
    attempt!(out.i32(tag));
    attempt!(out.append(b") (then (return (i32.const 0))))\n"));
    live(out)
}

fn child(out: &mut crate::output::Buffer, id: u32, edge: &[u8]) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call $t"));
    attempt!(out.number(u64::from(id)));
    attempt!(out.append(b" (call $kind (call $"));
    attempt!(out.append(edge));
    attempt!(out.append(b" (local.get $h))) (call $boxed_value (call $"));
    attempt!(out.append(edge));
    out.append(b" (local.get $h))))")
}
