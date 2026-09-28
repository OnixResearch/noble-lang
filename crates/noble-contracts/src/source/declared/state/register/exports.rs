pub(super) struct Installed {
    pub session: crate::source::declared::ModuleSession,
    pub local_exports: alloc::vec::Vec<crate::source::declared::Export>,
    pub words: alloc::vec::Vec<(alloc::string::String, crate::source::Target)>,
}

pub(super) fn install(
    session: crate::source::declared::ModuleSession,
    collected: &super::collection::Collected,
    resolved: &super::schema::Resolved,
) -> Result<Installed, crate::source::Error> {
    let installed = attempt!(initial(session, collected));
    let mut index = 0;
    let mut outcome = Ok(installed);
    while index < collected.schemas.len() && outcome.is_ok() {
        let schema = &collected.schemas[index];
        outcome = match outcome {
            Ok(installed) => install_at(installed, schema, resolved.get(index), &collected.exports),
            Err(problem) => Err(problem),
        };
        index += 1;
    }
    outcome
}

fn install_at(
    installed: Installed,
    schema: &crate::source::declared::Schema,
    entry: Option<&Option<(noble_kernel::types::Ty, noble_kernel::contracts::NominalOps)>>,
    exports: &[alloc::string::String],
) -> Result<Installed, crate::source::Error> {
    let Some(Some(entry)) = entry else {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check,
            "unresolved schema",
        ));
    };
    install_schema(installed, schema, entry, exports)
}

fn initial(
    session: crate::source::declared::ModuleSession,
    collected: &super::collection::Collected,
) -> Result<Installed, crate::source::Error> {
    let local_exports = alloc::vec::Vec::with_capacity(
        collected
            .schemas
            .len()
            .saturating_add(collected.definitions.len()),
    );
    let (session, words) = attempt!(take_words(session, collected));
    Ok(Installed {
        session,
        local_exports,
        words,
    })
}

fn take_words(
    mut session: crate::source::declared::ModuleSession,
    collected: &super::collection::Collected,
) -> Result<
    (
        crate::source::declared::ModuleSession,
        alloc::vec::Vec<(alloc::string::String, crate::source::Target)>,
    ),
    crate::source::Error,
> {
    let Some(context) = session.source.declared.as_mut() else {
        return Err(crate::source::declared::error(
            crate::source::Stage::Check,
            "missing declared context",
        ));
    };
    let mut words = core::mem::take(&mut context.words);
    let word_slots = collected
        .schemas
        .len()
        .saturating_mul(3)
        .saturating_add(collected.requirements.len());
    words.reserve(word_slots);
    Ok((session, words))
}

fn install_schema(
    installed: Installed,
    schema: &crate::source::declared::Schema,
    entry: &(noble_kernel::types::Ty, noble_kernel::contracts::NominalOps),
    exports: &[alloc::string::String],
) -> Result<Installed, crate::source::Error> {
    let (ty, ops) = entry;
    debug_assert!(!schema.name.is_empty());
    debug_assert!(matches!(ty, noble_kernel::types::Ty::Nominal(_, _)));
    let is_visible = exports.contains(&schema.name);
    let public = attempt!(super::schema::nominals::nominal_public(
        &installed.session,
        ty
    ));
    let builder = attempt!(super::publication::operations(
        schema,
        ops,
        public,
        is_visible,
        installed.words
    ));
    let builder = attempt!(builder.validate(schema, exports, is_visible));
    let Installed {
        session,
        local_exports,
        ..
    } = installed;
    Ok(builder.record_export(&schema.name, session, local_exports, ty, is_visible))
}
