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
            .saturating_add(super::BINDING_SCHEMA.len())
            .saturating_add(abi.len())
            .saturating_add(self.identity.len())
            .saturating_add(self.wit.len())
            .saturating_add(16);
        let mut key = alloc::vec::Vec::with_capacity(capacity_bytes);
        key.extend_from_slice(profile.as_bytes());
        key.push(0);
        key.extend_from_slice(super::BINDING_SCHEMA.as_bytes());
        key.push(0);
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
}
