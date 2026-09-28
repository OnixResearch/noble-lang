pub(super) struct ExpectedRows {
    pub(super) prefix: usize,
    pub(super) named: usize,
}

struct Cursor {
    fixed: noble_kernel::contracts::Env,
    next_nominal: usize,
    named: usize,
}

pub(super) fn check(
    env: &noble_kernel::contracts::Env,
    fixed: noble_kernel::contracts::Env,
    expected: ExpectedRows,
) -> Result<(), crate::Diagnostic> {
    let replay = attempt!(Cursor {
        fixed,
        next_nominal: 0,
        named: 0,
    }
    .check_rows(env, &expected));
    let has_all_declarations = replay.next_nominal == env.nominals.len()
        && replay.fixed.bound_adapters.len() == env.bound_adapters.len();
    let has_all_definitions =
        replay.fixed.defs.len() == env.defs.len() && replay.named == expected.named;
    if !has_all_declarations || !has_all_definitions {
        return Err(crate::Diagnostic::Invalid);
    }
    Ok(())
}

impl Cursor {
    fn check_rows(
        self,
        env: &noble_kernel::contracts::Env,
        expected: &ExpectedRows,
    ) -> Result<Self, crate::Diagnostic> {
        let mut index = 23usize;
        let mut outcome = Ok(self);
        while index < env.defs.len() {
            outcome = match outcome {
                Ok(cursor) => cursor.check_row(env, expected, index),
                Err(problem) => Err(problem),
            };
            if outcome.is_err() {
                break;
            }
            index += 1;
        }
        outcome
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; pinned const trials fail on Vec indexing, NominalTypeId comparison and owned Cursor destruction (March E0277/E0493, August E0015/E0658/E0493); reassess when these operations become const-capable."
    )]
    fn check_row(
        mut self,
        env: &noble_kernel::contracts::Env,
        expected: &ExpectedRows,
        index: usize,
    ) -> Result<Self, crate::Diagnostic> {
        if index >= self.fixed.defs.len() {
            if let noble_kernel::contracts::Behavior::NominalNew(id)
            | noble_kernel::contracts::Behavior::NominalLeft(id) = env.kinds[index]
            {
                let decl = match env.nominals.get(self.next_nominal) {
                    Some(decl) => decl,
                    None => return Err(crate::Diagnostic::Invalid),
                };
                if decl.id != id {
                    return Err(crate::Diagnostic::Invalid);
                }
                self = attempt!(self.append_nominal(decl));
            } else {
                self = attempt!(self.append_non_nominal(env, index));
            }
        }
        let has_matching_contract = self.fixed.kinds.get(index) == env.kinds.get(index)
            && super::same_scheme(&self.fixed.defs[index], &env.defs[index]);
        let has_matching_owner =
            self.fixed.definition_owners[index] == env.definition_owners[index];
        let has_matching_dependencies = env.kinds[index]
            == noble_kernel::contracts::Behavior::Named
            || env.deps[index] == self.fixed.deps[index];
        if !has_matching_contract || !has_matching_owner || !has_matching_dependencies {
            return Err(crate::Diagnostic::Invalid);
        }
        if env.kinds[index] == noble_kernel::contracts::Behavior::Named && index >= expected.prefix
        {
            self.named += 1;
        }
        Ok(self)
    }

    fn append_non_nominal(
        mut self,
        env: &noble_kernel::contracts::Env,
        index: usize,
    ) -> Result<Self, crate::Diagnostic> {
        if let noble_kernel::contracts::Behavior::Named = env.kinds[index] {
            self.fixed.defs.push(env.defs[index].clone());
            self.fixed
                .kinds
                .push(noble_kernel::contracts::Behavior::Named);
            self.fixed.deps.push(env.deps[index].clone());
            self.fixed
                .definition_owners
                .push(env.definition_owners[index]);
            return Ok(self);
        }
        if let noble_kernel::contracts::Behavior::BoundEmit(slot) = env.kinds[index] {
            return self.append_bound_emit(env, index, slot);
        }
        // Builtins and derived nominal operations cannot begin a declaration.
        Err(crate::Diagnostic::Invalid)
    }

    fn append_bound_emit(
        mut self,
        env: &noble_kernel::contracts::Env,
        index: usize,
        slot: u32,
    ) -> Result<Self, crate::Diagnostic> {
        let def = match u32::try_from(index) {
            Ok(value) => noble_kernel::contracts::Definition(value),
            Err(_) => return Err(crate::Diagnostic::Invalid),
        };
        let mut row_index = 0usize;
        while row_index < env.bound_adapters.len() {
            let row = &env.bound_adapters[row_index];
            if row.definition == def && row.adapter_slot == slot {
                break;
            }
            row_index += 1;
        }
        let row = match env.bound_adapters.get(row_index) {
            Some(row) => row,
            None => return Err(crate::Diagnostic::Invalid),
        };
        let owner = match env.definition_owners[index] {
            Some(owner) => owner,
            None => return Err(crate::Diagnostic::Invalid),
        };
        let registration = noble_kernel::contracts::BoundEmitRegistration {
            adapter_identity: row.adapter_identity.clone(),
            adapter_slot: slot,
            owner,
            input: row.input.clone(),
            output: row.output.clone(),
            effects: row.effects.clone(),
        };
        let (fixed, actual) = match self.fixed.declare_bound_emit(registration) {
            Ok(value) => value,
            Err(_) => return Err(crate::Diagnostic::Invalid),
        };
        if actual != def {
            return Err(crate::Diagnostic::Invalid);
        }
        self.fixed = fixed;
        Ok(self)
    }

    fn append_nominal(
        mut self,
        decl: &noble_kernel::contracts::NominalDecl,
    ) -> Result<Self, crate::Diagnostic> {
        let (fixed, _) = attempt!(self
            .fixed
            .declare_nominal(decl.clone())
            .map_err(|_| crate::Diagnostic::Invalid));
        self.fixed = fixed;
        self.next_nominal += 1;
        Ok(self)
    }
}
