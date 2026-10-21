//! Pure direct-style boundary compiler. The shell assembles the returned core
//! WAT with the original WIT and a pinned synchronous or native async Canonical
//! ABI component toolchain. Engine suspension never schedules later Noble words.
//! Guest borrowed-owner threading is not the WIT borrow: only the call operand
//! is lowered as borrow, while the owner is unavailable to guest instructions.
mod abi;
mod admission;
mod emit;
mod lower;

pub const BOOTSTRAP_WIT: &[u8] = include_str!("../../wit/bootstrap.wit").as_bytes();
pub const ASYNC_WIT: &[u8] = include_str!("../../wit/async.wit").as_bytes();
pub const SYNDICATE_WIT: &[u8] = include_str!("../../wit/syndicate.wit").as_bytes();
pub const MEMORY_BYTES: u32 = 1_048_576;
pub const HEAP_START: u32 = 65_536;
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; rustc checks this subtraction during constant evaluation, so a heap start beyond the fixed memory size is a compile error, not a wrapping runtime configuration."
)]
pub const DEFAULT_ALLOCATION_LIMIT: u32 = MEMORY_BYTES - HEAP_START;
const NODE_LIMIT: usize = 4096;
const STACK_LIMIT: usize = 256;
const LOCAL_LIMIT: usize = 16_384;
const FLAT_PARAMETER_LIMIT: usize = 16;
const TEXT_BYTE_LIMIT: usize = 60_000;
const TEXT_START: u32 = 1024;

pub struct Artifact {
    wat: alloc::vec::Vec<u8>,
    build_context: alloc::vec::Vec<u8>,
}
impl Artifact {
    pub fn wat(&self) -> &[u8] {
        &self.wat
    }
    pub fn build_context(&self) -> &[u8] {
        &self.build_context
    }
}

/// Compile every selected-world export, exactly once. No partial module is
/// exposed on any refusal. Kernel acceptance replays against independently
/// regenerated bindings, not the producer-supplied environment.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; independent admission, lowering and bounded emission each return typed refusals; no artifact is published on failure, and producer data must not trigger assertion panics."
)]
pub fn compile(
    world: &noble_contracts::component::World,
    exports: &[noble_contracts::component::CheckedExport],
) -> Result<Artifact, crate::Diagnostic> {
    let text_count = attempt!(admission::check(world, exports));
    let mut plans = alloc::vec::Vec::with_capacity(exports.len());
    let mut data = lower::Data {
        segments: alloc::vec::Vec::with_capacity(text_count),
        end: TEXT_START,
    };
    let mut index = 0usize;
    let mut failure = None;
    let count = exports.len();
    while index < count && failure.is_none() {
        match lower::function(world, &exports[index], &mut data) {
            Ok(plan) => plans.push(plan),
            Err(error) => failure = Some(error),
        }
        index = index.saturating_add(1);
    }
    if let Some(error) = failure {
        return Err(error);
    }
    let wat = attempt!(emit::module(world, &plans, &data));
    let build_context = attempt!(context(world, exports));
    Ok(Artifact { wat, build_context })
}

fn context(
    world: &noble_contracts::component::World,
    exports: &[noble_contracts::component::CheckedExport],
) -> Result<alloc::vec::Vec<u8>, crate::Diagnostic> {
    let domain: &[u8] = if world.is_async() {
        b"\0noble-component-async-canonical32-v1\0"
    } else {
        b"\0noble-component-canonical32-v1\0"
    };
    let mut build_context = world.build_context();
    let capacity_bytes = attempt!(context_capacity(exports, domain.len()));
    build_context.reserve_exact(capacity_bytes);
    build_context.extend_from_slice(domain);
    attempt!(context_exports(exports, &mut build_context));
    Ok(build_context)
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; export framing grows only the fresh compiler-owned Vec, which cannot be replaced by a fixed-length slice; conversion failure prevents publication of the whole artifact."
)]
fn context_exports(
    exports: &[noble_contracts::component::CheckedExport],
    build_context: &mut alloc::vec::Vec<u8>,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut failure = None;
    let count = exports.len();
    while index < count && failure.is_none() {
        if let Err(error) = context_export(&exports[index], build_context) {
            failure = Some(error);
        }
        index = index.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    tigerstyle::borrowed_argument_types,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; framing needs the growing compiler-owned Vec and non-const TryFrom conversions, not a fixed slice; no partial identity escapes compilation."
)]
fn context_export(
    export: &noble_contracts::component::CheckedExport,
    build_context: &mut alloc::vec::Vec<u8>,
) -> Result<(), crate::Diagnostic> {
    let length_bytes = match u32::try_from(export.source().len()) {
        Ok(length) => length,
        Err(_) => return Err(crate::Diagnostic::Exhausted),
    };
    let ordinal = match u32::try_from(export.index()) {
        Ok(ordinal) => ordinal,
        Err(_) => return Err(crate::Diagnostic::Exhausted),
    };
    let ordinal_bytes = ordinal.to_le_bytes();
    let length_bytes = length_bytes.to_le_bytes();
    build_context.extend_from_slice(&ordinal_bytes);
    build_context.extend_from_slice(&length_bytes);
    build_context.extend_from_slice(export.source());
    Ok(())
}

fn context_capacity(
    exports: &[noble_contracts::component::CheckedExport],
    mut bytes: usize,
) -> Result<usize, crate::Diagnostic> {
    let mut index = 0usize;
    let mut failure = None;
    let count = exports.len();
    while index < count && failure.is_none() {
        match export_capacity(&exports[index], bytes) {
            Ok(capacity) => bytes = capacity,
            Err(error) => failure = Some(error),
        }
        index = index.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(bytes),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the exact retained source length comes from a non-const borrowed export accessor; checked additions preserve Exhausted on overflow."
)]
fn export_capacity(
    export: &noble_contracts::component::CheckedExport,
    bytes: usize,
) -> Result<usize, crate::Diagnostic> {
    const HEADER_BYTES: usize = 8;
    let bytes = match bytes.checked_add(HEADER_BYTES) {
        Some(bytes) => bytes,
        None => return Err(crate::Diagnostic::Exhausted),
    };
    bytes
        .checked_add(export.source().len())
        .ok_or(crate::Diagnostic::Exhausted)
}

// Test harness only: scenario-named tests over checked fixtures.
#[allow(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; test harness: descriptive scenario names, fixture imports and long setup legs are the readable form."
)]
#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;
    use noble_contracts::component::World;
    use noble_contracts::source::{BoundOperation, ModuleSession, RetainedSourceLimit};
    use noble_kernel::contracts::{TEST_CODE_EXPORT, TEST_EMIT};
    use noble_kernel::types::{EffSet, Ty};

    const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
        bytes: 65_536,
        nodes: 4_096,
        depth: 32,
        work: 3_000_000,
    };

    #[test]
    fn selected_emit_compiles_only_exact_effectful_program_owner() {
        let binding = BoundOperation {
            module_name: "Ledger".into(),
            module_version: 1,
            operation: "test.emit".into(),
            adapter_identity: "worker-test-emit".into(),
            adapter_slot: 4,
            input: vec![Ty::Text],
            output: vec![],
            effects: vec![TEST_EMIT],
        };
        let retained = RetainedSourceLimit::new(LIMITS.bytes).unwrap();
        let session = ModuleSession::new(core::slice::from_ref(&binding), retained).unwrap();
        let module = session.prepare(
            b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"private-ledger-marker\" emit 1 + ] ]",
            &[], LIMITS,
        ).unwrap();
        let owner = module
            .checked_program_owner_for_binding("plus", &binding)
            .unwrap();
        let (session, committed) = session.commit(module);
        committed.unwrap();
        let invocation = session
            .prepare(b"Ledger@1.plus", &[Ty::I64], LIMITS)
            .unwrap();
        let interface = invocation
            .checked_program_interface_for_binding(&owner, &binding)
            .unwrap();
        let world = World::parse(
            include_bytes!("../../wit/worker-program.wit"),
            "guest",
            LIMITS,
        )
        .unwrap()
        .select_worker_program_for_test_emit(&interface)
        .unwrap();
        let checked = world
            .prepare_export("compute", b"programs.admit swap programs.invoke", LIMITS)
            .unwrap();
        let kind = world.worker_program_kind().unwrap();
        let selected = world
            .noble_type(noble_contracts::component::Type::Own(kind))
            .unwrap();
        let wrong_effect = Ty::program(
            vec![Ty::I64],
            vec![Ty::I64],
            EffSet::from_ids(&[TEST_CODE_EXPORT]),
        );
        assert!(
            matches!(
                super::abi::value_type(&wrong_effect, Some((kind, &selected))),
                Err(crate::Diagnostic::Unsupported)
            ),
            "a different latent effect cannot alias selected own<program>"
        );
        assert!(
            world
                .prepare_export("compute", b"programs.admit run", LIMITS)
                .is_err(),
            "selected latent effect is not directly callable"
        );
        assert!(
            super::compile(&world, core::slice::from_ref(&checked)).is_ok(),
            "selected import-only guest must publish a complete component artifact"
        );
    }

    fn checked_interface(
        source: &[u8],
        invocation: &[u8],
        name: &str,
        binding: Option<&BoundOperation>,
    ) -> noble_contracts::source::CheckedProgramInterface {
        let bindings: Vec<_> = binding.into_iter().cloned().collect();
        let session =
            ModuleSession::new(&bindings, RetainedSourceLimit::new(LIMITS.bytes).unwrap()).unwrap();
        let module = session.prepare(source, &[], LIMITS).unwrap();
        let owner = match binding {
            Some(binding) => module
                .checked_program_owner_for_binding(name, binding)
                .unwrap(),
            None => module.checked_program_owner(name).unwrap(),
        };
        let (session, committed) = session.commit(module);
        committed.unwrap();
        let invocation = session.prepare(invocation, &[Ty::I64], LIMITS).unwrap();
        match binding {
            Some(binding) => invocation
                .checked_program_interface_for_binding(&owner, binding)
                .unwrap(),
            None => invocation.checked_program_interface(&owner).unwrap(),
        }
    }

    #[test]
    fn selected_pure_and_code_export_compile_only_exact_program_owners() {
        let wit = include_bytes!("../../wit/worker-program.wit");
        let pure = checked_interface(
            b"module plain@1 [ export plus def plus [ 1 + ] ]",
            b"plain@1.plus",
            "plus",
            None,
        );
        let pure_world = World::parse(wit, "guest", LIMITS)
            .unwrap()
            .select_worker_program(&pure)
            .unwrap();
        let pure_guest = pure_world
            .prepare_export("compute", b"programs.admit swap programs.invoke", LIMITS)
            .unwrap();
        assert!(
            super::compile(&pure_world, core::slice::from_ref(&pure_guest)).is_ok(),
            "pure selected Program must compile through the same exact own<program> ABI"
        );

        let binding = BoundOperation {
            module_name: "ledger".into(),
            module_version: 1,
            operation: "test.code-export".into(),
            adapter_identity: "worker-test-code-export@1.0.0".into(),
            adapter_slot: 4,
            input: vec![Ty::Text, Ty::Text],
            output: vec![],
            effects: vec![TEST_CODE_EXPORT],
        };
        let effectful = checked_interface(
            b"module ledger@1 [ require code_export Text Text -- ! test.code-export export capture def capture [ \"private-export-marker\" \"hidden-capture-destination\" code_export ] ]",
            b"ledger@1.capture", "capture", Some(&binding),
        );
        assert!(
            World::parse(wit, "guest", LIMITS)
                .unwrap()
                .select_worker_program_for_test_emit(&effectful)
                .is_err(),
            "the wrong effect must not select a different Program owner"
        );
        let selected = World::parse(wit, "guest", LIMITS)
            .unwrap()
            .select_worker_program_for_test_code_export(&effectful)
            .unwrap();
        assert!(
            selected
                .prepare_export("compute", b"programs.admit run", LIMITS)
                .is_err(),
            "a selected latent code-export effect remains noncallable"
        );
        let checked = selected
            .prepare_export(
                "compute",
                b"programs.admit \"denied-destination\" programs.request-code-export drop",
                LIMITS,
            )
            .unwrap();
        assert!(
            super::compile(&selected, core::slice::from_ref(&checked)).is_ok(),
            "selected code-export imports must compile without granting a direct effect"
        );
    }
}
