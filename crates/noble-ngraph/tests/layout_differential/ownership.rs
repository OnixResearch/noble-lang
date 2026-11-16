//! Reusable packed workspaces and the compiled value they belong to (issue #133).
//!
//! `Compiled::evaluate_in` and `Compiled::reset` take a caller-owned `Workspace`.
//! These tests build real checked graphs whose packed layouts share the profile
//! and the register count but differ in their constants, register map,
//! input-slot map or output map, plus other-profile, smaller and larger
//! workspaces. They pin the documented contract: a workspace belongs to the
//! compiled value that created it and to that value's clones; every other
//! workspace is refused with `E-INTERFACE` before any register, counter or
//! scratch lease is touched, so no foreign register can reach a successful
//! result. Same-graph reuse, including reuse after every failure class, is
//! compared with fresh evaluation and with the reference interpreter.
//!
//! These are finite executed comparisons. They do not prove that the compiled
//! evaluator refines the reference (NG-PO-C), and the binding is an identity
//! check on the compiled value, not a layout, schedule or cost claim.

use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::compiled::{Compiled, Workspace};
use noble_ngraph::diag::{BudgetKind, Code, Site};
use noble_ngraph::eval::{self, Control, Obs, Report, ScratchCounters};
use noble_ngraph::ir::{
    AffineOperand, Fact, Graph, GraphBuilder, Literal, Op, ScalarType, SlotKind, SubProfile,
    Subject,
};
use noble_ngraph::sem::Value;

use super::common::IDS;

struct Admitted {
    checked: CheckedGraph,
    compiled: Compiled,
}

fn admit(graph: Graph) -> Admitted {
    let checked = check::check(&IDS, graph).expect("workspace-binding fixture checks");
    let compiled = Compiled::new(&IDS, &checked);
    Admitted { checked, compiled }
}

fn builder(profile: SubProfile, name: &str) -> GraphBuilder {
    GraphBuilder::new(profile, Subject::authored(&IDS, name))
}

/// The issue's minimal reproduction: no inputs, one `Const(I64(value))`, output it.
fn i64_constant(value: i64) -> Graph {
    let mut b = builder(SubProfile::I64, "workspace-binding/i64-constant");
    let constant = b.i64_const(value);
    b.output(constant);
    b.finish()
}

/// The F32 counterpart of `i64_constant`, with the same register count.
fn f32_constant(value: f32) -> Graph {
    let mut b = builder(SubProfile::F32, "workspace-binding/f32-constant");
    let constant = b.f32_const(value);
    b.output(constant);
    b.finish()
}

/// `2x + 5`. Both variants keep the constant in register 1 and the doubling in
/// register 2, but with `constant_first` false the doubling is node 1 and the
/// constant node 2, so only the node-to-register map differs.
fn i64_doubled_offset(constant_first: bool) -> Graph {
    let mut b = builder(SubProfile::I64, "workspace-binding/i64-doubled-offset");
    let x = b.input(ScalarType::I64);
    let (doubled, five) = if constant_first {
        let five = b.i64_const(5);
        (b.op(Op::I64Add(x, x)), five)
    } else {
        let doubled = b.op(Op::I64Add(x, x));
        (doubled, b.i64_const(5))
    };
    let sum = b.op(Op::I64Add(doubled, five));
    b.output(sum);
    b.finish()
}

/// `x + 5`. With `constant_first` the constant is created before the input, so the
/// constant and the input exchange registers 0 and 1.
fn i64_offset(constant_first: bool) -> Graph {
    let mut b = builder(SubProfile::I64, "workspace-binding/i64-offset");
    let (x, five) = if constant_first {
        let five = b.i64_const(5);
        (b.input(ScalarType::I64), five)
    } else {
        let x = b.input(ScalarType::I64);
        (x, b.i64_const(5))
    };
    let sum = b.op(Op::I64Add(x, five));
    b.output(sum);
    b.finish()
}

/// `slot 0 − slot 1`. With `swapped` slot 1's node is created first, so each input
/// slot lands in the other register while the register map stays the identity.
fn i64_slot_difference(swapped: bool) -> Graph {
    let mut b = builder(SubProfile::I64, "workspace-binding/i64-slot-difference");
    let first = b.declare_slot(SlotKind::Input, ScalarType::I64);
    let second = b.declare_slot(SlotKind::Input, ScalarType::I64);
    let (minuend, subtrahend) = if swapped {
        let subtrahend = b.op(Op::Input(second));
        (b.op(Op::Input(first)), subtrahend)
    } else {
        let minuend = b.op(Op::Input(first));
        (minuend, b.op(Op::Input(second)))
    };
    let difference = b.op(Op::I64Sub(minuend, subtrahend));
    b.output(difference);
    b.finish()
}

/// `x + 3` and `x`, output in either order.
fn i64_two_outputs(sum_first: bool) -> Graph {
    let mut b = builder(SubProfile::I64, "workspace-binding/i64-two-outputs");
    let x = b.input(ScalarType::I64);
    let three = b.i64_const(3);
    let sum = b.op(Op::I64Add(x, three));
    if sum_first {
        b.output(sum);
        b.output(x);
    } else {
        b.output(x);
        b.output(sum);
    }
    b.finish()
}

/// `s = x + y`, guarded as a bit, then `t = factor · s`; outputs `t` and `s`. The
/// constant sits in register 2 and the stream has three layers, so a guard failure,
/// fuel exhaustion or cancellation stops after `s` has been written.
fn i64_guarded(factor: i64) -> Graph {
    let mut b = builder(SubProfile::I64, "workspace-binding/i64-guarded");
    let x = b.input(ScalarType::I64);
    let y = b.input(ScalarType::I64);
    let constant = b.i64_const(factor);
    let sum = b.op(Op::I64Add(x, y));
    b.guard(Fact::Bit, sum);
    let product = b.op(Op::I64Mul(sum, constant));
    b.output(product);
    b.output(sum);
    b.finish()
}

/// `(a + b) + c`: the same register count as `i64_guarded`, with input `c` in the
/// register that holds `i64_guarded`'s constant.
fn i64_three_inputs() -> Graph {
    let mut b = builder(SubProfile::I64, "workspace-binding/i64-three-inputs");
    let first = b.input(ScalarType::I64);
    let second = b.input(ScalarType::I64);
    let third = b.input(ScalarType::I64);
    let partial = b.op(Op::I64Add(first, second));
    let total = b.op(Op::I64Add(partial, third));
    b.output(total);
    b.finish()
}

/// `x` added to itself `steps` times after the first copy: `steps + 2` registers.
fn i64_chain(steps: usize) -> Graph {
    let mut b = builder(SubProfile::I64, "workspace-binding/i64-chain");
    let x = b.input(ScalarType::I64);
    let mut value = x;
    for _ in 0..steps {
        value = b.op(Op::I64Add(value, x));
    }
    b.output(value);
    b.finish()
}

/// `x + 1` through the affine constant-one operand: three F32 registers (`x`, the
/// sum and the constant one).
fn f32_plus_one() -> Graph {
    let mut b = builder(SubProfile::F32, "workspace-binding/f32-plus-one");
    let x = b.input(ScalarType::F32);
    let sum = b.affine(
        0.0,
        &[(1.0, AffineOperand::Node(x)), (1.0, AffineOperand::One)],
    );
    b.output(sum);
    b.finish()
}

/// `(x + 4) + 5`: six F32 registers, with the constant 5 in register 2, the
/// register `f32_plus_one` reads as its constant one.
fn f32_wide() -> Graph {
    let mut b = builder(SubProfile::F32, "workspace-binding/f32-wide");
    let x = b.input(ScalarType::F32);
    let four = b.f32_const(4.0);
    let five = b.f32_const(5.0);
    let partial = b.op(Op::F32Add(x, four));
    let total = b.op(Op::F32Add(partial, five));
    b.output(total);
    b.finish()
}

/// `a = 2x + 1.25` through the affine constant-one operand, `r = relu(a − 0.5)`,
/// guarded by `x ∈ [−4, 4]`; outputs `a` and `r`.
fn f32_guarded() -> Graph {
    let mut b = builder(SubProfile::F32, "workspace-binding/f32-guarded");
    let x = b.input(ScalarType::F32);
    let half = b.f32_const(0.5);
    let affine = b.affine(
        0.25,
        &[(2.0, AffineOperand::Node(x)), (1.0, AffineOperand::One)],
    );
    let shifted = b.op(Op::F32Sub(affine, half));
    let rectified = b.op(Op::F32Relu(shifted));
    b.guard(
        Fact::Range {
            lo: Literal::f32(-4.0),
            hi: Literal::f32(4.0),
        },
        x,
    );
    b.output(affine);
    b.output(rectified);
    b.finish()
}

fn reference(admitted: &Admitted, inputs: &[Value]) -> Obs {
    eval::reference(&admitted.checked, inputs, None, &mut Control::default()).obs
}

/// The owner's evaluation in `workspace` equals its fresh evaluation exactly and
/// the reference interpreter under `≡obs`.
fn assert_own(admitted: &Admitted, workspace: &mut Workspace, inputs: &[Value]) -> Report {
    let reused = admitted
        .compiled
        .evaluate_in(workspace, inputs, None, &mut Control::default());
    let fresh = admitted
        .compiled
        .evaluate(inputs, None, &mut Control::default());
    assert_eq!(reused, fresh, "reused and fresh compiled evaluation");
    let reference = reference(admitted, inputs);
    assert!(
        reference.obs_eq(&reused.obs),
        "reference {reference:?} vs reused {:?}",
        reused.obs
    );
    reused
}

/// Runs `compiled` in a workspace it does not own and checks the documented
/// refusal: `E-INTERFACE` at the graph site, no node evaluated, no caller counter
/// or scratch lease touched, and every register left as it was.
fn assert_foreign_refused(compiled: &Compiled, workspace: &mut Workspace, inputs: &[Value]) {
    let before = format!("{workspace:?}");
    let mut evaluated = 0_u64;
    let mut scratch = ScratchCounters::default();
    let report = {
        let mut control = Control {
            scratch: Some(&mut scratch),
            evaluated_nodes: Some(&mut evaluated),
            ..Control::default()
        };
        compiled.evaluate_in(workspace, inputs, None, &mut control)
    };
    match &report.obs {
        Obs::Refused(d) => {
            assert_eq!(d.code, Code::Interface, "{d}");
            assert_eq!(d.site, Site::Graph, "{d}");
        }
        other => panic!("a foreign workspace must be refused, but evaluation reported {other:?}"),
    }
    assert_eq!(report.evaluated_nodes, 0);
    assert_eq!(evaluated, 0);
    assert_eq!(scratch, ScratchCounters::default());
    assert_eq!(
        format!("{workspace:?}"),
        before,
        "a refused evaluation must not write the workspace"
    );
}

#[test]
fn a_constant_only_workspace_never_supplies_its_constant_to_another_graph() {
    let a = admit(i64_constant(1));
    let b = admit(i64_constant(2));
    let (layout_a, layout_b) = (a.compiled.packed_layout(), b.compiled.packed_layout());
    // The pair a register-count comparison accepts: same profile, register count
    // and register map; only the constant's bits differ.
    assert_eq!(a.checked.graph().profile, b.checked.graph().profile);
    assert_eq!(
        (layout_a.regs, &layout_a.reg),
        (layout_b.regs, &layout_b.reg)
    );
    assert_eq!(a.compiled.scratch_bytes(), b.compiled.scratch_bytes());
    assert_eq!(layout_a.consts, vec![(0, 1)]);
    assert_eq!(layout_b.consts, vec![(0, 2)]);
    let expected_b = Obs::Normal(vec![Value::I64(2)]);
    assert_eq!(reference(&b, &[]), expected_b);
    assert_eq!(
        b.compiled.evaluate(&[], None, &mut Control::default()).obs,
        expected_b
    );

    let mut workspace = a.compiled.workspace();
    let crossed = b
        .compiled
        .evaluate_in(&mut workspace, &[], None, &mut Control::default());
    assert_ne!(
        crossed.obs,
        Obs::Normal(vec![Value::I64(1)]),
        "B reported A's constant as its own successful result"
    );
    assert_foreign_refused(&b.compiled, &mut workspace, &[]);
    assert_foreign_refused(&a.compiled, &mut b.compiled.workspace(), &[]);
    // The refused workspace still serves its owner.
    assert_eq!(
        assert_own(&a, &mut workspace, &[]).obs,
        Obs::Normal(vec![Value::I64(1)])
    );
}

/// Which declared component of a same-size pair differs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Differs {
    consts: bool,
    registers: bool,
    slots: bool,
    outputs: bool,
}

/// An identity scheme under which every digest collides.
struct CollidingIds;

struct ConstantHasher;

impl noble_ngraph::IdentityHasher for ConstantHasher {
    fn update(&mut self, _bytes: &[u8]) {}

    fn finish(self: Box<Self>) -> noble_ngraph::ir::Digest {
        [0; 32]
    }
}

impl noble_ngraph::IdentityScheme for CollidingIds {
    fn name(&self) -> &'static str {
        "workspace-binding-colliding"
    }

    fn start(&self) -> Box<dyn noble_ngraph::IdentityHasher + '_> {
        Box::new(ConstantHasher)
    }
}

#[test]
fn a_colliding_identity_scheme_cannot_make_a_foreign_workspace_acceptable() {
    // The binding is not a digest: under a scheme where every graph id and layout
    // key collide, the issue's A/B pair still refuses each other's workspace.
    let admit_colliding = |value: i64| {
        let mut b = GraphBuilder::new(
            SubProfile::I64,
            Subject::authored(&CollidingIds, "workspace-binding/colliding"),
        );
        let constant = b.i64_const(value);
        b.output(constant);
        let checked =
            check::check(&CollidingIds, b.finish()).expect("checks under the colliding scheme");
        let compiled = Compiled::new(&CollidingIds, &checked);
        Admitted { checked, compiled }
    };
    let (a, b) = (admit_colliding(1), admit_colliding(2));
    assert_eq!(a.checked.id(), b.checked.id());
    assert_eq!(a.compiled.layout_key(), b.compiled.layout_key());
    let mut workspace = a.compiled.workspace();
    assert_foreign_refused(&b.compiled, &mut workspace, &[]);
    let before = format!("{workspace:?}");
    assert!(b.compiled.reset(&mut workspace).is_err());
    assert_eq!(format!("{workspace:?}"), before);
    assert_eq!(
        assert_own(&a, &mut workspace, &[]).obs,
        Obs::Normal(vec![Value::I64(1)])
    );
    assert_eq!(
        assert_own(&b, &mut b.compiled.workspace(), &[]).obs,
        Obs::Normal(vec![Value::I64(2)])
    );
}

#[test]
fn same_size_graphs_with_different_layouts_refuse_each_others_workspaces() {
    let only = |consts, registers, slots, outputs| Differs {
        consts,
        registers,
        slots,
        outputs,
    };
    let cases = [
        (
            "constants",
            i64_constant(1),
            i64_constant(2),
            vec![],
            only(true, false, false, false),
        ),
        (
            "register map",
            i64_doubled_offset(false),
            i64_doubled_offset(true),
            eval::i64_inputs(&[10]),
            only(false, true, false, false),
        ),
        (
            "input-slot map",
            i64_slot_difference(false),
            i64_slot_difference(true),
            eval::i64_inputs(&[9, 4]),
            only(false, false, true, false),
        ),
        (
            "output map",
            i64_two_outputs(true),
            i64_two_outputs(false),
            eval::i64_inputs(&[6]),
            only(false, false, false, true),
        ),
        (
            "constant and input registers exchanged",
            i64_offset(false),
            i64_offset(true),
            eval::i64_inputs(&[10]),
            only(true, false, true, false),
        ),
    ];
    for (name, left, right, inputs, expected) in cases {
        let (left, right) = (admit(left), admit(right));
        let (l, r) = (
            left.compiled.packed_layout(),
            right.compiled.packed_layout(),
        );
        assert_eq!(
            left.checked.graph().profile,
            right.checked.graph().profile,
            "{name}"
        );
        assert_eq!(l.regs, r.regs, "{name}: the same register count");
        let observed = Differs {
            consts: l.consts != r.consts,
            registers: l.reg != r.reg,
            slots: l.slot_regs != r.slot_regs,
            outputs: left.checked.graph().outputs != right.checked.graph().outputs,
        };
        assert_eq!(observed, expected, "{name}: the declared difference");
        for (owner, other) in [(&left, &right), (&right, &left)] {
            let mut workspace = owner.compiled.workspace();
            assert_foreign_refused(&other.compiled, &mut workspace, &inputs);
            assert_own(owner, &mut workspace, &inputs);
            assert_own(other, &mut other.compiled.workspace(), &inputs);
        }
    }
}

#[test]
fn a_workspace_of_the_other_profile_is_refused_without_panic() {
    let pairs = [
        (
            admit(i64_constant(2)),
            admit(f32_constant(2.0)),
            vec![],
            vec![],
        ),
        (
            admit(i64_chain(1)),
            admit(f32_plus_one()),
            eval::i64_inputs(&[4]),
            eval::f32_inputs(&[4.0]),
        ),
    ];
    for (integer, float, integer_inputs, float_inputs) in pairs {
        assert_eq!(
            integer.compiled.packed_layout().regs,
            float.compiled.packed_layout().regs,
            "same register count, other profile"
        );
        let mut float_workspace = float.compiled.workspace();
        assert_foreign_refused(&integer.compiled, &mut float_workspace, &integer_inputs);
        assert_own(&float, &mut float_workspace, &float_inputs);
        let mut integer_workspace = integer.compiled.workspace();
        assert_foreign_refused(&float.compiled, &mut integer_workspace, &float_inputs);
        assert_own(&integer, &mut integer_workspace, &integer_inputs);
        // The binding is checked before the inputs: inputs that would fail this
        // graph's preflight at a slot still get the graph-site workspace refusal.
        assert_foreign_refused(&integer.compiled, &mut float_workspace, &float_inputs);
        assert_foreign_refused(&float.compiled, &mut integer_workspace, &integer_inputs);
    }
}

#[test]
fn a_smaller_workspace_is_refused_without_panic() {
    let small = admit(i64_constant(1));
    let large = admit(i64_guarded(3));
    assert!(small.compiled.packed_layout().regs < large.compiled.packed_layout().regs);
    let mut workspace = small.compiled.workspace();
    assert_foreign_refused(&large.compiled, &mut workspace, &eval::i64_inputs(&[0, 1]));
    assert_own(&small, &mut workspace, &[]);

    let small = admit(f32_plus_one());
    let large = admit(f32_wide());
    let mut workspace = small.compiled.workspace();
    assert_foreign_refused(&large.compiled, &mut workspace, &eval::f32_inputs(&[1.5]));
    assert_own(&small, &mut workspace, &eval::f32_inputs(&[1.5]));
}

#[test]
fn a_larger_workspace_is_refused_without_panic() {
    // `f32_plus_one` reads its constant one from register 2, where the larger
    // workspace holds the constant 5.
    let small = admit(f32_plus_one());
    let large = admit(f32_wide());
    assert!(small.compiled.packed_layout().regs < large.compiled.packed_layout().regs);
    let inputs = eval::f32_inputs(&[1.5]);
    assert_eq!(
        reference(&small, &inputs),
        Obs::Normal(eval::f32_inputs(&[2.5]))
    );
    let mut workspace = large.compiled.workspace();
    assert_foreign_refused(&small.compiled, &mut workspace, &inputs);
    assert_own(&large, &mut workspace, &inputs);

    let small = admit(i64_constant(2));
    let large = admit(i64_guarded(3));
    let mut workspace = large.compiled.workspace();
    assert_foreign_refused(&small.compiled, &mut workspace, &[]);
    assert_own(&large, &mut workspace, &eval::i64_inputs(&[1, 0]));
}

#[test]
fn reset_follows_the_same_binding_contract() {
    let owner = admit(i64_guarded(3));
    let foreign = [
        ("same size, other constant", admit(i64_guarded(7))),
        ("smaller", admit(i64_constant(9))),
        ("larger", admit(i64_chain(8))),
        ("other profile", admit(f32_wide())),
    ];
    let inputs = eval::i64_inputs(&[0, 1]);
    let mut workspace = owner.compiled.workspace();
    let fresh = format!("{workspace:?}");
    for (name, other) in &foreign {
        let before = format!("{workspace:?}");
        match other.compiled.reset(&mut workspace) {
            Err(d) => {
                assert_eq!(d.code, Code::Interface, "{name}: {d}");
                assert_eq!(d.site, Site::Graph, "{name}: {d}");
            }
            Ok(()) => panic!("{name}: a foreign reset must be refused"),
        }
        assert_eq!(
            format!("{workspace:?}"),
            before,
            "{name}: a refused reset must not write the workspace"
        );
        assert_own(&owner, &mut workspace, &inputs);
        // The foreign graph's own workspace is unaffected by the refusal.
        let mut own = other.compiled.workspace();
        assert_eq!(other.compiled.reset(&mut own), Ok(()), "{name}");
    }
    // The owner's own reset restores the post-construction registers exactly,
    // also after a guard failure left scratch values behind.
    let failed = owner.compiled.evaluate_in(
        &mut workspace,
        &eval::i64_inputs(&[2, 3]),
        None,
        &mut Control::default(),
    );
    assert!(matches!(failed.obs, Obs::GuardFailed { .. }), "{failed:?}");
    assert_ne!(
        format!("{workspace:?}"),
        fresh,
        "the failed run wrote scratch"
    );
    assert_eq!(owner.compiled.reset(&mut workspace), Ok(()));
    assert_eq!(format!("{workspace:?}"), fresh);
    // A clone shares the owner's identity, so it may reset the workspace too.
    assert_own(&owner, &mut workspace, &eval::i64_inputs(&[5, -4]));
    assert_eq!(owner.compiled.clone().reset(&mut workspace), Ok(()));
    assert_eq!(format!("{workspace:?}"), fresh);
    assert_own(&owner, &mut workspace, &inputs);
    // F32: the reset restores the constant-one register and the constants too.
    let owner = admit(f32_guarded());
    let mut workspace = owner.compiled.workspace();
    let fresh = format!("{workspace:?}");
    assert_own(&owner, &mut workspace, &eval::f32_inputs(&[1.0]));
    assert_ne!(format!("{workspace:?}"), fresh, "the run wrote scratch");
    assert_eq!(owner.compiled.reset(&mut workspace), Ok(()));
    assert_eq!(format!("{workspace:?}"), fresh);
    let other = admit(f32_plus_one());
    assert!(other.compiled.reset(&mut workspace).is_err());
    assert_eq!(format!("{workspace:?}"), fresh);
}

#[test]
fn same_graph_reuse_and_clones_keep_their_semantics() {
    let owner = admit(i64_guarded(3));
    let clone = owner.compiled.clone();
    let mut workspace = owner.compiled.workspace();
    let mut clone_workspace = clone.workspace();
    for row in [[0, 1], [1, 0], [2, 3], [-1, 1], [1, 1], [0, 0], [5, -4]] {
        let inputs = eval::i64_inputs(&row);
        let reused = assert_own(&owner, &mut workspace, &inputs);
        // The clone accepts the original's workspace and the original the clone's.
        assert_eq!(
            clone.evaluate_in(&mut workspace, &inputs, None, &mut Control::default()),
            reused
        );
        assert_eq!(
            owner.compiled.evaluate_in(
                &mut clone_workspace,
                &inputs,
                None,
                &mut Control::default()
            ),
            reused
        );
        // A cloned workspace stays bound to the same compiled identity.
        let mut copy = workspace.clone();
        assert_eq!(
            clone.evaluate_in(&mut copy, &inputs, None, &mut Control::default()),
            reused
        );
    }
    // The identity is the compiled value, not its layout: compiling the same checked
    // graph again declares an equal layout but is a distinct owner.
    let again = Compiled::new(&IDS, &owner.checked);
    assert_eq!(again.packed_layout(), owner.compiled.packed_layout());
    let inputs = eval::i64_inputs(&[1, 0]);
    assert_foreign_refused(&again, &mut workspace, &inputs);
    assert_foreign_refused(&owner.compiled, &mut again.workspace(), &inputs);
    let recompiled = Admitted {
        checked: owner.checked.clone(),
        compiled: again,
    };
    assert_own(&recompiled, &mut recompiled.compiled.workspace(), &inputs);
    assert_own(&owner, &mut workspace, &inputs);
}

#[test]
fn reuse_after_refusal_guard_failure_exhaustion_and_cancellation_matches_the_reference() {
    let owner = admit(i64_guarded(3));
    // `i64_three_inputs` writes its third input into the register that holds the
    // owner's constant; `i64_guarded(7)` would load its own constant there.
    let writer = admit(i64_three_inputs());
    let loader = admit(i64_guarded(7));
    assert_eq!(
        writer.compiled.packed_layout().regs,
        owner.compiled.packed_layout().regs
    );
    let bytes = owner.compiled.scratch_bytes();
    let mut workspace = owner.compiled.workspace();
    assert_own(&owner, &mut workspace, &eval::i64_inputs(&[0, 1]));

    // Input refusal.
    let refused = owner.compiled.evaluate_in(
        &mut workspace,
        &eval::i64_inputs(&[1]),
        None,
        &mut Control::default(),
    );
    assert!(matches!(refused.obs, Obs::Refused(_)), "{refused:?}");
    assert_own(&owner, &mut workspace, &eval::i64_inputs(&[1, 0]));

    // Guard failure after `s` was written.
    let failed = owner.compiled.evaluate_in(
        &mut workspace,
        &eval::i64_inputs(&[2, 3]),
        None,
        &mut Control::default(),
    );
    assert!(matches!(failed.obs, Obs::GuardFailed { .. }), "{failed:?}");
    assert_own(&owner, &mut workspace, &eval::i64_inputs(&[0, 0]));

    // Fuel exhaustion between the second and third layers: inputs and the constant
    // charge three, the sum one, and the guard and product two more.
    let exhausted = owner.compiled.evaluate_in(
        &mut workspace,
        &eval::i64_inputs(&[5, 6]),
        None,
        &mut Control {
            fuel: Some(4),
            ..Control::default()
        },
    );
    assert_eq!(exhausted.obs, Obs::Exhausted(BudgetKind::Fuel));
    assert_own(&owner, &mut workspace, &eval::i64_inputs(&[0, 1]));

    // Cancellation before the third layer.
    let stop = |layer: u32| layer == 2;
    let cancelled = owner.compiled.evaluate_in(
        &mut workspace,
        &eval::i64_inputs(&[7, 8]),
        None,
        &mut Control {
            cancel: Some(&stop),
            ..Control::default()
        },
    );
    assert_eq!(cancelled.obs, Obs::Cancelled);
    assert_own(&owner, &mut workspace, &eval::i64_inputs(&[1, 0]));

    // A foreign evaluation and a foreign reset are refused, so neither the writer's
    // input nor the loader's constant reaches the owner's next result.
    assert_foreign_refused(
        &writer.compiled,
        &mut workspace,
        &eval::i64_inputs(&[100, 200, 300]),
    );
    assert_own(&owner, &mut workspace, &eval::i64_inputs(&[0, 1]));
    let before = format!("{workspace:?}");
    assert!(loader.compiled.reset(&mut workspace).is_err());
    assert_eq!(format!("{workspace:?}"), before);
    assert_own(&owner, &mut workspace, &eval::i64_inputs(&[1, 0]));

    // A scratch refusal under a lowered limit.
    let lowered = owner.compiled.evaluate_in(
        &mut workspace,
        &eval::i64_inputs(&[1, 1]),
        None,
        &mut Control {
            forward_scratch: Some(bytes - 1),
            ..Control::default()
        },
    );
    assert!(matches!(lowered.obs, Obs::Refused(_)), "{lowered:?}");
    assert_own(&owner, &mut workspace, &eval::i64_inputs(&[0, 1]));

    // The F32 constant-one register survives every failure class too.
    let owner = admit(f32_guarded());
    let mut workspace = owner.compiled.workspace();
    assert_own(&owner, &mut workspace, &eval::f32_inputs(&[1.0]));
    let failed = owner.compiled.evaluate_in(
        &mut workspace,
        &eval::f32_inputs(&[10.0]),
        None,
        &mut Control::default(),
    );
    assert!(matches!(failed.obs, Obs::GuardFailed { .. }), "{failed:?}");
    assert_own(&owner, &mut workspace, &eval::f32_inputs(&[-1.5]));
    // Cancellation after the affine value (layer 1) was written.
    let stop = |layer: u32| layer == 2;
    let cancelled = owner.compiled.evaluate_in(
        &mut workspace,
        &eval::f32_inputs(&[3.0]),
        None,
        &mut Control {
            cancel: Some(&stop),
            ..Control::default()
        },
    );
    assert_eq!(cancelled.obs, Obs::Cancelled);
    assert_own(&owner, &mut workspace, &eval::f32_inputs(&[0.25]));
    let small = admit(f32_plus_one());
    assert_foreign_refused(&small.compiled, &mut workspace, &eval::f32_inputs(&[2.0]));
    assert_own(&owner, &mut workspace, &eval::f32_inputs(&[-0.0]));
}

#[test]
fn lowered_scratch_limits_are_enforced_on_a_reused_workspace() {
    let owner = admit(i64_guarded(3));
    let bytes = owner.compiled.scratch_bytes();
    let inputs = eval::i64_inputs(&[1, 0]);
    let mut workspace = owner.compiled.workspace();
    assert_own(&owner, &mut workspace, &inputs);
    let before = format!("{workspace:?}");
    let mut evaluated = 0_u64;
    let mut scratch = ScratchCounters::default();
    let (expected, lowered) = {
        let mut control = Control {
            forward_scratch: Some(bytes - 1),
            scratch: Some(&mut scratch),
            evaluated_nodes: Some(&mut evaluated),
            ..Control::default()
        };
        let expected = owner.compiled.scratch_refusal(&control);
        let lowered = owner
            .compiled
            .evaluate_in(&mut workspace, &inputs, None, &mut control);
        (expected, lowered)
    };
    match &lowered.obs {
        Obs::Refused(d) => {
            assert_eq!(d.code, Code::Budget(BudgetKind::ForwardScratch), "{d}");
            assert_eq!(d.budget, Some((bytes - 1, bytes)));
        }
        other => panic!("a lowered scratch limit must refuse, got {other:?}"),
    }
    assert_eq!(Some(lowered), expected);
    assert_eq!(evaluated, 0);
    assert_eq!(scratch, ScratchCounters::default());
    assert_eq!(format!("{workspace:?}"), before);

    // At exactly the limit the reused workspace runs and leases exactly its bytes.
    let at_limit = {
        let mut control = Control {
            forward_scratch: Some(bytes),
            scratch: Some(&mut scratch),
            ..Control::default()
        };
        owner
            .compiled
            .evaluate_in(&mut workspace, &inputs, None, &mut control)
    };
    assert!(reference(&owner, &inputs).obs_eq(&at_limit.obs));
    assert_eq!(scratch.snapshot(), (1, 1, 0));
    assert_eq!(scratch.peak_bytes, bytes);
}

#[test]
fn fresh_and_reused_execution_agree_with_the_reference() {
    let corpus = [
        (
            i64_guarded(3),
            vec![
                eval::i64_inputs(&[0, 1]),
                eval::i64_inputs(&[2, 3]),
                eval::i64_inputs(&[1, 0]),
                eval::i64_inputs(&[i64::MAX, i64::MIN]),
                eval::i64_inputs(&[-7, 7]),
            ],
        ),
        (
            i64_offset(true),
            vec![
                eval::i64_inputs(&[10]),
                eval::i64_inputs(&[i64::MAX]),
                eval::i64_inputs(&[-5]),
            ],
        ),
        (
            i64_doubled_offset(false),
            vec![eval::i64_inputs(&[10]), eval::i64_inputs(&[-3])],
        ),
        (
            i64_slot_difference(true),
            vec![eval::i64_inputs(&[9, 4]), eval::i64_inputs(&[i64::MIN, 1])],
        ),
        (
            i64_two_outputs(false),
            vec![eval::i64_inputs(&[6]), eval::i64_inputs(&[-3])],
        ),
        (
            f32_guarded(),
            vec![
                eval::f32_inputs(&[1.0]),
                eval::f32_inputs(&[10.0]),
                eval::f32_inputs(&[-0.0]),
                eval::f32_inputs(&[-4.0]),
                eval::f32_inputs(&[f32::NAN]),
            ],
        ),
        (
            f32_wide(),
            vec![eval::f32_inputs(&[1.5]), eval::f32_inputs(&[-9.0])],
        ),
        (
            f32_plus_one(),
            vec![eval::f32_inputs(&[1.5]), eval::f32_inputs(&[f32::INFINITY])],
        ),
    ];
    for (graph, rows) in corpus {
        let admitted = admit(graph);
        let clone = admitted.compiled.clone();
        let mut workspace = admitted.compiled.workspace();
        // Forward, then backward, so every row also follows a different predecessor.
        for inputs in rows.iter().chain(rows.iter().rev()) {
            let reused = assert_own(&admitted, &mut workspace, inputs);
            assert_eq!(
                clone.evaluate_in(&mut workspace, inputs, None, &mut Control::default()),
                reused
            );
        }
    }
}
