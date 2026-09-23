#[expect(
    tigerstyle::assertion_density,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; Shape is the closed semantic storage vocabulary and every variant needs its explicit observation predicate; bounded output failures propagate diagnostics rather than assertions."
)]
pub(super) fn shape(
    out: &mut crate::output::Buffer,
    index: usize,
    shape: super::Shape,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func $t"));
    attempt!(out.index(index));
    attempt!(out.append(b" (param $tag i32) (param $value i64) (result i32) (local $h i32)\n(if (i32.eqz (call $observation_tick)) (then (return (i32.const 0))))\n"));
    match shape {
        super::Shape::Scalar(tag) => attempt!(scalar(out, tag)),
        super::Shape::Program(input, output_signature, effects) => {
            attempt!(reference(out, 4));
            attempt!(out.append(b"(i32.and (i32.eq (call $y (local.get $h)) "));
            attempt!(out.i32(input));
            attempt!(out.append(b") (i32.and (i32.eq (call $z (local.get $h)) "));
            attempt!(out.i32(output_signature));
            attempt!(out.append(b") (i64.eq (call $payload (local.get $h)) "));
            attempt!(out.i64(i64::from(effects)));
            attempt!(out.append(b")))\n"));
        }
        super::Shape::Text => {
            attempt!(reference(out, 11));
            attempt!(out.append(b"(i32.const 1)\n"));
        }
        super::Shape::Syntax => {
            attempt!(reference(out, 10));
            attempt!(out.append(b"(i32.const 1)\n"));
        }
        super::Shape::Contract => {
            attempt!(reference(out, 14));
            attempt!(out.append(b"(i32.const 1)\n"));
        }
        super::Shape::Evidence => {
            attempt!(reference(out, 15));
            attempt!(out.append(b"(i32.and (i32.eqz (i32.eq (call $a (local.get $h)) (i32.const 0))) (i32.eq (call $kind (call $a (local.get $h))) (i32.const 14)))\n"));
        }
        super::Shape::Certified => {
            attempt!(reference(out, 16));
            attempt!(out.append(b"(i32.and (i32.eq (call $kind (call $a (local.get $h))) (i32.const 4)) (i32.and (i32.eq (call $kind (call $b (local.get $h))) (i32.const 14)) (i32.eq (call $kind (call $c (local.get $h))) (i32.const 15))))\n"));
        }
        super::Shape::Pair(left, right) => {
            attempt!(reference(out, 5));
            attempt!(out.append(b"(if (i32.eqz "));
            attempt!(child(out, left, b"a"));
            attempt!(out.append(b") (then (return (i32.const 0))))\n"));
            attempt!(child(out, right, b"b"));
            attempt!(out.append(b"\n"));
        }
        super::Shape::Sum(left, right) => {
            attempt!(out.append(b"(if (i32.and (i32.ne (local.get $tag) (i32.const 12)) (i32.ne (local.get $tag) (i32.const 13))) (then (return (i32.const 0))))\n"));
            attempt!(live(out));
            attempt!(
                out.append(b"(if (result i32) (i32.eq (local.get $tag) (i32.const 12)) (then ")
            );
            attempt!(child(out, left, b"a"));
            attempt!(out.append(b") (else "));
            attempt!(child(out, right, b"a"));
            attempt!(out.append(b"))\n"));
        }
        super::Shape::List(item) => {
            attempt!(out.append(b"(if (i32.and (i32.ne (local.get $tag) (i32.const 6)) (i32.ne (local.get $tag) (i32.const 7))) (then (return (i32.const 0))))\n"));
            attempt!(live(out));
            attempt!(out.append(b"(loop $list\n(if (i32.eqz (call $observation_tick)) (then (return (i32.const 0))))\n(if (i32.eq (call $kind (local.get $h)) (i32.const 7)) (then (return (i32.const 1))))\n(if (i32.ne (call $kind (local.get $h)) (i32.const 6)) (then (return (i32.const 0))))\n(if (i32.eqz "));
            attempt!(child(out, item, b"a"));
            attempt!(out.append(b") (then (return (i32.const 0))))\n(local.set $h (call $b (local.get $h)))\n(br $list))\n(i32.const 0)\n"));
        }
    }
    out.append(b")\n")
}

fn live(out: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
    out.append(b"(if (i64.gt_u (local.get $value) (i64.const 4294967295)) (then (return (i32.const 0))))\n(local.set $h (i32.wrap_i64 (local.get $value)))\n(if (i32.eqz (call $is_live (local.get $h))) (then (return (i32.const 0))))\n(if (i32.ne (call $kind (local.get $h)) (local.get $tag)) (then (return (i32.const 0))))\n")
}

fn reference(out: &mut crate::output::Buffer, tag: u32) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(if (i32.ne (local.get $tag) "));
    attempt!(out.i32(tag));
    attempt!(out.append(b") (then (return (i32.const 0))))\n"));
    live(out)
}

fn child(out: &mut crate::output::Buffer, id: u32, edge: &[u8]) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call $t"));
    attempt!(out.number(u64::from(id)));
    attempt!(out.append(b" (call $kind (call $"));
    attempt!(out.append(edge));
    attempt!(out.append(b" (local.get $h))) (call $boxed_value (call $"));
    attempt!(out.append(edge));
    out.append(b" (local.get $h))))")
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; scalar emits into the bounded growable output buffer through non-const append and numeric formatting APIs."
)]
fn scalar(out: &mut crate::output::Buffer, tag: u32) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(if (i32.ne (local.get $tag) "));
    attempt!(out.i32(tag));
    attempt!(out.append(b") (then (return (i32.const 0))))\n"));
    match tag {
        2 => attempt!(out.append(b"(i64.le_u (local.get $value) (i64.const 1))\n")),
        3 => attempt!(out.append(b"(i64.eqz (local.get $value))\n")),
        _ => attempt!(out.append(b"(i32.const 1)\n")),
    }

    Ok(())
}
