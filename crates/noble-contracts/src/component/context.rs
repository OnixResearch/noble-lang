impl super::World {
    /// Full build binding, not a digest offered as authority.
    pub fn build_context(&self) -> alloc::vec::Vec<u8> {
        match self.profile() {
            super::Profile::Sync => self.context(super::PROFILE, super::SYNC_ABI),
            super::Profile::Async => self.context(super::ASYNC_PROFILE, super::ASYNC_ABI),
        }
    }

    #[expect(
        tigerstyle::ambiguous_params,
        reason = "Owner: noble-maintainers; the two private call sites pair fixed profile and ABI constants in an exhaustive Profile match, in wire order. Separate borrowed parameters avoid Aeneas's unsupported nested-borrow join for a branch-selected tuple without copying either string."
    )]
    fn context(&self, profile: &str, abi: &str) -> alloc::vec::Vec<u8> {
        let capacity_bytes = profile
            .len()
            .saturating_add(binding_schema_len())
            .saturating_add(self.selection_len())
            .saturating_add(abi.len())
            .saturating_add(self.identity.len())
            .saturating_add(self.wit.len())
            .saturating_add(16);
        let mut key = alloc::vec::Vec::with_capacity(capacity_bytes);
        key.extend_from_slice(profile.as_bytes());
        key.push(0);
        append_binding_schema(&mut key);
        key.push(0);
        self.append_selection(&mut key);
        key.extend_from_slice(&super::STREAM_U8_KIND.0.to_le_bytes());
        key.extend_from_slice(&super::FUTURE_S64_KIND.0.to_le_bytes());
        key.extend_from_slice(&super::FUTURE_RESULT_S64_STRING_KIND.0.to_le_bytes());
        key.extend_from_slice(abi.as_bytes());
        key.push(0);
        key.extend_from_slice(self.identity.as_bytes());
        key.push(0);
        key.extend_from_slice(&self.wit);
        key
    }

    /// Bytes `append_selection` adds for the selected checked and worker boundaries.
    fn selection_len(&self) -> usize {
        let checked = if self.checked_u64 {
            checked_u64_schema_len().saturating_add(1)
        } else {
            0
        };
        let worker = if self.worker_program_kind.is_some() {
            worker_schema_len(self.worker_program_effect).saturating_add(5)
        } else {
            0
        };
        let transition = if self.worker_transition_kind.is_some() {
            b"worker-program-v2:transition-frames"
                .len()
                .saturating_add(5)
        } else {
            0
        };
        checked.saturating_add(worker).saturating_add(transition)
    }

    #[expect(
        tigerstyle::borrowed_argument_types,
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers; the selected boundaries are appended to the caller's growable key, which a slice cannot express; the key is build_context's freshly allocated buffer."
    )]
    /// Append the selected checked-u64, worker Program and transition bindings.
    fn append_selection(&self, key: &mut alloc::vec::Vec<u8>) {
        if self.checked_u64 {
            append_checked_u64_schema(key);
            key.push(0);
        }
        if let Some(kind) = self.worker_program_kind {
            append_worker_schema(key, self.worker_program_effect);
            key.extend_from_slice(&kind.0.to_le_bytes());
            key.push(0);
        }
        if let Some(kind) = self.worker_transition_kind {
            key.extend_from_slice(b"worker-program-v2:transition-frames");
            key.extend_from_slice(&kind.0.to_le_bytes());
            key.push(0);
        }
    }
}

/// Length of the bounded WIT binding schema.
fn binding_schema_len() -> usize {
    super::BINDING_SCHEMA.len()
}

#[expect(
    tigerstyle::borrowed_argument_types,
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; the schema is appended to the caller's growable key, which a slice cannot express; the key is build_context's freshly allocated buffer."
)]
/// Append the bounded WIT binding schema.
fn append_binding_schema(key: &mut alloc::vec::Vec<u8>) {
    key.extend_from_slice(super::BINDING_SCHEMA.as_bytes());
}

/// Length of the checked u64 boundary schema.
fn checked_u64_schema_len() -> usize {
    super::CHECKED_U64_SCHEMA.len()
}

#[expect(
    tigerstyle::borrowed_argument_types,
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; the schema is appended to the caller's growable key, which a slice cannot express; the key is build_context's freshly allocated buffer."
)]
/// Append the checked u64 boundary schema.
fn append_checked_u64_schema(key: &mut alloc::vec::Vec<u8>) {
    key.extend_from_slice(super::CHECKED_U64_SCHEMA.as_bytes());
}

/// Length of the worker Program schema selected by `effect`.
const fn worker_schema_len(effect: Option<noble_kernel::types::EffId>) -> usize {
    match effect {
        Some(noble_kernel::contracts::TEST_EMIT) => {
            b"worker-program-v2:Program([I64]->[I64]!{test.emit})".len()
        }
        Some(noble_kernel::contracts::TEST_CODE_EXPORT) => {
            b"worker-program-v2:Program([I64]->[I64]!{test.code-export})".len()
        }
        Some(noble_kernel::contracts::TEST_SEND) => {
            b"worker-program-v2:Program([I64]->[I64]!{test.send})".len()
        }
        Some(_) | None => b"worker-program-v2:Program([I64]->[I64]!{})".len(),
    }
}

#[expect(
    tigerstyle::borrowed_argument_types,
    tigerstyle::mutating_input_in_pure,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the schema is appended to the caller's growable key through non-const Vec growth, which a slice cannot express; the key is build_context's freshly allocated buffer."
)]
/// Append the worker Program schema selected by `effect`.
fn append_worker_schema(key: &mut alloc::vec::Vec<u8>, effect: Option<noble_kernel::types::EffId>) {
    match effect {
        Some(noble_kernel::contracts::TEST_EMIT) => {
            key.extend_from_slice(b"worker-program-v2:Program([I64]->[I64]!{test.emit})");
        }
        Some(noble_kernel::contracts::TEST_CODE_EXPORT) => {
            key.extend_from_slice(b"worker-program-v2:Program([I64]->[I64]!{test.code-export})");
        }
        Some(noble_kernel::contracts::TEST_SEND) => {
            key.extend_from_slice(b"worker-program-v2:Program([I64]->[I64]!{test.send})");
        }
        Some(_) | None => key.extend_from_slice(b"worker-program-v2:Program([I64]->[I64]!{})"),
    }
}
