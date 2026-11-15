use super::*;

const ALIAS_WIRE_CLOSURE_OFFSET_BYTES: usize = 33;
const ALIAS_WIRE_MARKER_BYTES: usize = 65;

fn pair_fixture() -> Result<(Session, ProcedureLibrary, ProcedureRecord, Alias), String> {
    let namespace = Session::without_test_hosts();
    let target = polynomial_record(&[5])?;
    let candidate = polynomial_record(&[2, 3])?;
    let alias = Alias::certify(&candidate, &target)
        .map_err(|error| format!("certificate: {error:?}"))?;
    let mut library = ProcedureLibrary::new(revision(7));
    library
        .admit(&namespace, None, target)
        .map_err(|error| format!("target: {error:?}"))?;
    debug_assert!(candidate.alias_of.is_none());
    Ok((namespace, library, candidate, alias))
}

#[test]
fn forged_pair_closure_form_and_target_are_refused() -> Result<(), String> {
    let (namespace, mut library, mut candidate, alias) = pair_fixture()?;
    let mut closure_forgery = alias.clone();
    if let Certificate::ExactPair(pair) = &mut closure_forgery.certificate {
        pair.left_closure[0] ^= 1;
    }
    closure_forgery
        .attach(&mut candidate)
        .map_err(|error| format!("attach closure: {error:?}"))?;
    let refusal = library
        .admit(&namespace, None, candidate.clone())
        .expect_err("forged closure digest must not be an authorization");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "alias_of");

    let mut stale_form = alias.clone();
    if let Certificate::ExactPair(pair) = &mut stale_form.certificate {
        pair.left[0].terms[0].coefficient ^= 1;
    }
    candidate.alias_of = Some(stale_form);
    let refusal = library
        .admit(&namespace, None, candidate.clone())
        .expect_err("forged polynomial form must not be an authorization");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "alias_of");

    let mut stale_target = alias;
    stale_target.procedure_id[0] ^= 1;
    candidate.alias_of = Some(stale_target);
    let refusal = library
        .admit(&namespace, None, candidate)
        .expect_err("stale pair target is not retained");
    assert_eq!(refusal.kind, RefusalKind::StaleContext);
    assert_eq!(refusal.location.field, "alias_of");
    Ok(())
}

#[test]
fn forged_validation_evidence_is_refused() -> Result<(), String> {
    let (namespace, mut library, mut candidate, alias) = pair_fixture()?;
    candidate.alias_of = Some(alias.clone());
    let refusal = library
        .admit(&namespace, None, candidate.clone())
        .expect_err("missing exact-pair evidence must be refused");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "alias_of");

    candidate.evidence.push(super::super::evidence(
        &candidate,
        EvidenceClass::Validation,
        EvidenceOutcome::Passed,
    ));
    let refusal = library
        .admit(&namespace, None, candidate.clone())
        .expect_err("unrelated validation evidence must be refused");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "alias_of");

    candidate.evidence.clear();
    candidate.alias_of = None;
    alias
        .attach(&mut candidate)
        .map_err(|error| format!("attach evidence: {error:?}"))?;
    for case in 0..4 {
        let mut forged = candidate.clone();
        let record = forged
            .evidence
            .first_mut()
            .ok_or_else(|| String::from("missing own evidence"))?;
        match case {
            0 => record.class = EvidenceClass::Review,
            1 => record.outcome = EvidenceOutcome::Failed,
            2 => record.toolchain = vec![String::from("unrelated-checker")],
            _ => record.claim.push_str(":other-pair"),
        }
        let refusal = library
            .admit(&namespace, None, forged)
            .expect_err("changed class/outcome/checker/scope cannot claim this alias");
        assert_eq!(refusal.kind, RefusalKind::Invalid);
        assert_eq!(refusal.location.field, "alias_of");
    }
    Ok(())
}

#[test]
fn tampered_wire_is_refused_on_reload() -> Result<(), String> {
    let (namespace, mut library, mut candidate, alias) = pair_fixture()?;
    alias
        .attach(&mut candidate)
        .map_err(|error| format!("attach good: {error:?}"))?;
    library
        .admit(&namespace, None, candidate)
        .map_err(|error| format!("good alias: {error:?}"))?;
    let mut bytes = library.encode().map_err(|error| format!("encode: {error:?}"))?;
    let alias = library
        .records()
        .iter()
        .find_map(|record| record.alias_of.as_ref())
        .ok_or_else(|| String::from("missing retained alias"))?;
    let Certificate::ExactPair(pair) = &alias.certificate else {
        return Err(String::from("missing pair certificate"));
    };
    let mut marker = Vec::with_capacity(ALIAS_WIRE_MARKER_BYTES);
    marker.push(2);
    marker.extend_from_slice(&alias.procedure_id);
    marker.extend_from_slice(&pair.left_closure);
    assert_eq!(marker.len(), ALIAS_WIRE_MARKER_BYTES);
    let offset_bytes = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .ok_or_else(|| String::from("pair wire marker absent"))?;
    let closure_byte_offset_bytes = offset_bytes
        .checked_add(ALIAS_WIRE_CLOSURE_OFFSET_BYTES)
        .ok_or_else(|| String::from("pair wire offset overflow"))?;
    let byte = bytes
        .get_mut(closure_byte_offset_bytes)
        .ok_or_else(|| String::from("pair closure byte absent"))?;
    *byte ^= 1;
    let refusal = ProcedureLibrary::load(&bytes, &namespace)
        .expect_err("tampered persisted closure cannot reload");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    Ok(())
}
