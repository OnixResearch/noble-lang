use super::common::IDS;
use noble_ngraph::ir::{Op, Subject};
use noble_ngraph::{check, Code, GraphBuilder, ScalarType, SubProfile};

#[test]
fn authored_graph_is_limited_to_closed_wrapping_i64_operations() {
    let nickel_subject = || Subject::authored_nickel([0x5a; 32]);
    let mut valid = GraphBuilder::new(SubProfile::I64, nickel_subject());
    let input = valid.input(ScalarType::I64);
    let one = valid.i64_const(1);
    let sum = valid.op(Op::I64Add(input, one));
    valid.output(sum);
    assert!(check(&IDS, valid.finish()).is_ok());

    let mut equality = GraphBuilder::new(SubProfile::I64, nickel_subject());
    let input = equality.input(ScalarType::I64);
    let zero = equality.i64_const(0);
    let equal = equality.op(Op::I64Eq(input, zero));
    equality.output(equal);
    assert_eq!(
        check(&IDS, equality.finish()).unwrap_err().code,
        Code::Profile
    );

    let mut f32_graph = GraphBuilder::new(SubProfile::F32, nickel_subject());
    let input = f32_graph.input(ScalarType::F32);
    f32_graph.output(input);
    assert_eq!(
        check(&IDS, f32_graph.finish()).unwrap_err().code,
        Code::Profile
    );
}
