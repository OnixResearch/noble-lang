#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; interning mutates only the fresh type registry, prospective compiler and private work meter; descriptor emission writes only its fresh owned sink and all borrowed type stacks remain immutable."
)]

mod emission;

#[derive(Clone, Copy)]
enum Shape {
    Scalar(u32),
    Pair(u32, u32),
    Sum(u32, u32),
    List(u32),
    Program(u32, u32, u32),
    Syntax,
    Text,
    Contract,
    Evidence,
    Certified,
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
            noble_kernel::types::Ty::Contract => Shape::Contract,
            noble_kernel::types::Ty::Evidence => Shape::Evidence,
            noble_kernel::types::Ty::Certified => Shape::Certified,
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
            match emission::shape(out, index, self.shapes[index]) {
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
