use super::super::support::{envelope, literal_record, provenance, revision};
use noble_contracts::source::chunks::{Node, Op};
use noble_contracts::source::procedures::{
    Alias, Certificate, EvidenceClass, EvidenceOutcome, LibraryDependency, ProcedureLibrary,
    ProcedureRecord, Producer, RefusalKind,
};
use noble_contracts::source::Session;
use noble_kernel::types::Ty;
use std::fmt::Write as _;
use std::string::String;
use std::vec;
use std::vec::Vec;

#[path = "forgery.rs"]
mod forgery;

const NODES_PER_ADDITION: usize = 2;
const MAX_LITERAL_SOURCE_BYTES: usize = 24;

pub(super) fn polynomial_record(added: &[i64]) -> Result<ProcedureRecord, String> {
    let expected_node_count = added
        .len()
        .checked_mul(NODES_PER_ADDITION)
        .ok_or_else(|| String::from("node capacity overflow"))?;
    let source_capacity_bytes = added
        .len()
        .checked_mul(MAX_LITERAL_SOURCE_BYTES)
        .ok_or_else(|| String::from("source capacity overflow"))?;
    let mut source = String::with_capacity(source_capacity_bytes);
    let mut nodes = Vec::with_capacity(expected_node_count);
    let mut entry = Vec::with_capacity(expected_node_count);
    for value in added {
        write!(&mut source, "{value} + ")
            .map_err(|error| format!("literal formatting: {error}"))?;
        entry.push(u32::try_from(nodes.len()).map_err(|error| error.to_string())?);
        nodes.push(Node::I64(*value));
        entry.push(u32::try_from(nodes.len()).map_err(|error| error.to_string())?);
        nodes.push(Node::Operation {
            op: Op::Add,
            instantiation: 0,
        });
    }
    debug_assert_eq!(nodes.len(), entry.len());
    ProcedureRecord::candidate(
        envelope(&[Ty::I64], &[Ty::I64], source.as_bytes(), nodes, entry),
        provenance(Producer::Chunk),
    )
    .map_err(|error| format!("polynomial record: {error:?}"))
}

#[test]
fn equal_polynomial_distinct_bodies_survive_reload() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let target = polynomial_record(&[5])?;
    let mut candidate = polynomial_record(&[2, 3])?;
    assert_ne!(target.procedure_id, candidate.procedure_id);
    Alias::certify(&candidate, &target)
        .map_err(|error| format!("certificate: {error:?}"))?
        .attach(&mut candidate)
        .map_err(|error| format!("attach: {error:?}"))?;
    let alias = candidate
        .alias_of
        .as_ref()
        .ok_or_else(|| String::from("claim missing"))?;
    let Certificate::ExactPair(pair) = &alias.certificate else {
        return Err(String::from("a real alias requires a pair certificate"));
    };
    assert_eq!(pair.left, pair.right);
    assert_eq!(pair.left_closure, pair.right_closure);
    library
        .admit(&namespace, None, target)
        .map_err(|error| format!("target: {error:?}"))?;
    library
        .admit(&namespace, None, candidate.clone())
        .map_err(|error| format!("alias: {error:?}"))?;
    let encoded = library
        .encode()
        .map_err(|error| format!("encode: {error:?}"))?;
    let loaded = ProcedureLibrary::load(&encoded, &namespace)
        .map_err(|error| format!("reload: {error:?}"))?;
    assert_eq!(
        loaded
            .encode()
            .map_err(|error| format!("re-encode: {error:?}"))?,
        encoded
    );
    assert!(loaded.records().iter().any(|record| record == &candidate));
    Ok(())
}

#[test]
fn polynomial_distribution_is_an_exact_pair() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let left = ProcedureRecord::candidate(
        envelope(
            &[Ty::I64],
            &[Ty::I64],
            b"2 + 3 * ",
            vec![
                Node::I64(2),
                Node::Operation {
                    op: Op::Add,
                    instantiation: 0,
                },
                Node::I64(3),
                Node::Operation {
                    op: Op::Mul,
                    instantiation: 0,
                },
            ],
            vec![0, 1, 2, 3],
        ),
        provenance(Producer::Chunk),
    )
    .map_err(|error| format!("left distributive: {error:?}"))?;
    let right = ProcedureRecord::candidate(
        envelope(
            &[Ty::I64],
            &[Ty::I64],
            b"3 * 6 + ",
            vec![
                Node::I64(3),
                Node::Operation {
                    op: Op::Mul,
                    instantiation: 0,
                },
                Node::I64(6),
                Node::Operation {
                    op: Op::Add,
                    instantiation: 0,
                },
            ],
            vec![0, 1, 2, 3],
        ),
        provenance(Producer::Chunk),
    )
    .map_err(|error| format!("right distributive: {error:?}"))?;
    assert_ne!(left.procedure_id, right.procedure_id);
    let mut aliased = left;
    Alias::certify(&aliased, &right)
        .map_err(|error| format!("distributive pair: {error:?}"))?
        .attach(&mut aliased)
        .map_err(|error| format!("attach distributive pair: {error:?}"))?;
    library
        .admit(&namespace, None, right)
        .map_err(|error| format!("right: {error:?}"))?;
    library
        .admit(&namespace, None, aliased)
        .map_err(|error| format!("distributed alias: {error:?}"))?;
    assert_eq!(library.records().len(), 2);
    Ok(())
}

#[test]
fn wrapping_i64_coefficients_are_normalized() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let wrapped = polynomial_record(&[i64::MAX, 1])?;
    let minimum = polynomial_record(&[i64::MIN])?;
    assert_ne!(wrapped.procedure_id, minimum.procedure_id);
    let mut wrapped_alias = wrapped;
    Alias::certify(&wrapped_alias, &minimum)
        .map_err(|error| format!("wrapping pair: {error:?}"))?
        .attach(&mut wrapped_alias)
        .map_err(|error| format!("attach wrapping pair: {error:?}"))?;
    library
        .admit(&namespace, None, minimum)
        .map_err(|error| format!("minimum: {error:?}"))?;
    library
        .admit(&namespace, None, wrapped_alias)
        .map_err(|error| format!("wrapping alias: {error:?}"))?;
    assert_eq!(library.records().len(), 2);
    Ok(())
}

#[test]
fn unequal_polynomials_and_unsupported_bodies_are_refused() -> Result<(), String> {
    let target = polynomial_record(&[5])?;
    let mut different = polynomial_record(&[6])?;
    let refused = Alias::certify(&different, &target)
        .expect_err("different polynomial must not produce a certificate");
    assert_eq!(refused.kind, RefusalKind::Invalid);
    Alias::certify(&polynomial_record(&[2, 3])?, &target)
        .map_err(|error| format!("good pair: {error:?}"))?
        .attach(&mut different)
        .map_err(|error| format!("attach: {error:?}"))?;
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    library
        .admit(&namespace, None, target.clone())
        .map_err(|error| format!("target: {error:?}"))?;
    let refusal = library
        .admit(&namespace, None, different)
        .expect_err("a borrowed valid pair certificate cannot claim a different body");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "alias_of");

    let mut boolean =
        literal_record(50, Producer::Chunk).map_err(|error| format!("literal: {error:?}"))?;
    boolean.chunk.body.nodes.push(Node::Bool(true));
    boolean.chunk.body.entry.push(1);
    boolean.chunk.interface.stack_out.push(Ty::Bool);
    let refused =
        Alias::certify(&boolean, &target).expect_err("Boolean body is outside the alias fragment");
    assert_eq!(refused.kind, RefusalKind::Unsupported);

    let mut dependent = polynomial_record(&[5])?;
    dependent.library_dependencies.push(LibraryDependency {
        procedure_id: target.procedure_id,
        chunk_id: target.chunk.chunk_id,
        source_revision: target.chunk.source_revision,
    });
    let refused = Alias::certify(&dependent, &target)
        .expect_err("dependency-bearing body is outside the verified fragment");
    assert_eq!(refused.kind, RefusalKind::Unsupported);
    Ok(())
}
