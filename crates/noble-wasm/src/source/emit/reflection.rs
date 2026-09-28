//! Declared-only reflection extensions over the byte-identical historical WAT.

const ORIGINAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/reflection.wat"
));

// All offsets are computed at compile time; missing historical markers
// produce a diagnostic rather than silently losing declared identity.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; checked_sub and checked_add bound the first-match scan, and missing or invalid offsets return None rather than assertion panics; reassess when the heuristic understands fallible validators."
)]
const fn locate(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let last = match haystack.len().checked_sub(needle.len()) {
        Some(last) => last,
        None => return None,
    };
    let mut at = 0usize;
    let mut is_searching = true;
    let mut found = None;
    while is_searching {
        // `at <= len - needle.len()` bounds every compared byte.
        let mut matched = 0;
        let mut is_comparing = true;
        while is_comparing && matched < needle.len() {
            match at.checked_add(matched) {
                Some(index) => {
                    if haystack[index] == needle[matched] {
                        matched += 1;
                    } else {
                        is_comparing = false;
                    }
                }
                None => is_comparing = false,
            }
        }
        if is_comparing && matched == needle.len() {
            found = Some(at);
            is_searching = false;
        } else if at == last {
            is_searching = false;
        } else {
            at += 1;
        }
    }
    found
}

const SUM_START: Option<usize> = locate(
    ORIGINAL.as_bytes(),
    b" (if (i32.or (i32.eq (local.get $tag) (i32.const 12))",
);
const SUM_END: Option<usize> = locate(
    ORIGINAL.as_bytes(),
    b" (if (i32.eq (local.get $tag) (i32.const 14))",
);
const WITNESS_START: Option<usize> = locate(
    ORIGINAL.as_bytes(),
    b" (if (i32.and (global.get $source_reflection)\n",
);
const WITNESS_END: Option<usize> = locate(ORIGINAL.as_bytes(), b"(func $reflect_internal");

const NOMINAL_SUM: &str = r#" (if (i32.or (i32.eq (local.get $tag) (i32.const 12)) (i32.eq (local.get $tag) (i32.const 13))) (then
  (if (i32.eqz (call $w (local.get $value)))
   (then (call $observe_atom (i32.sub (local.get $tag) (i32.const 1)) (i64.const 0)))
   (else
    (if (i32.or (i32.gt_u (call $w (local.get $value)) (i32.const 2))
         (i32.and (i32.eq (call $w (local.get $value)) (i32.const 1))
                  (i32.ne (local.get $tag) (i32.const 12))))
     (then (call $fail (i32.const 4)) (return)))
    (call $observe_atom
     (if (result i32) (i32.eq (call $w (local.get $value)) (i32.const 1))
      (then (i32.const 36)) (else (i32.add (local.get $tag) (i32.const 25))))
     (i64.or (i64.extend_i32_u (call $x (local.get $value)))
             (i64.shl (i64.extend_i32_u (call $y (local.get $value))) (i64.const 32))))
    (call $observe_atom (i32.const 35) (i64.extend_i32_u (call $z (local.get $value))))))
  (call $walk_push (i32.sub (i32.const 0) (call $a (local.get $value)))) (return)))
"#;

const DECLARED_WITNESS: &str = r#" (if (i32.and (global.get $source_reflection)
       (i32.or (i32.eq (local.get $tag) (i32.const 2))
        (i32.or (i32.eq (local.get $tag) (i32.const 15))
         (i32.and (i32.ge_u (local.get $tag) (i32.const 26))
                  (i32.le_u (local.get $tag) (i32.const 31)))))) (then
  (call $observe_atom (i32.const 20) (i64.extend_i32_u (call $y (local.get $recipe))))
  (call $observe_atom (i32.const 21) (i64.extend_i32_u (call $z (local.get $recipe))))
  (call $observe_atom (i32.const 22) (i64.extend_i32_u (call $w (local.get $recipe))))
  (if (i32.and (i32.ge_u (local.get $tag) (i32.const 26))
               (i32.le_u (local.get $tag) (i32.const 30)))
   (then (call $observe_atom (i32.const 35) (i64.extend_i32_u (call $n (local.get $recipe))))))))
)
"#;

struct Sections {
    sum_start: usize,
    sum_end: usize,
    witness_start: usize,
    witness_end: usize,
}

impl Sections {
    const fn checked() -> Result<Self, crate::Diagnostic> {
        let sum_start = match SUM_START {
            Some(at) => at,
            None => return Err(crate::Diagnostic::Defective),
        };
        let sum_end = match SUM_END {
            Some(at) => at,
            None => return Err(crate::Diagnostic::Defective),
        };
        let witness_start = match WITNESS_START {
            Some(at) => at,
            None => return Err(crate::Diagnostic::Defective),
        };
        let witness_end = match WITNESS_END {
            Some(at) => at,
            None => return Err(crate::Diagnostic::Defective),
        };
        if sum_start >= sum_end || sum_end >= witness_start || witness_start >= witness_end {
            return Err(crate::Diagnostic::Defective);
        }
        Ok(Self {
            sum_start,
            sum_end,
            witness_start,
            witness_end,
        })
    }

    fn emit(self, out: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
        attempt!(out.append(&ORIGINAL.as_bytes()[..self.sum_start]));
        attempt!(out.append(NOMINAL_SUM.as_bytes()));
        attempt!(out.append(&ORIGINAL.as_bytes()[self.sum_end..self.witness_start]));
        attempt!(out.append(DECLARED_WITNESS.as_bytes()));
        attempt!(out.append(&ORIGINAL.as_bytes()[self.witness_end..]));
        out.append(b"\n")
    }
}

pub(super) fn write(
    out: &mut crate::output::Buffer,
    declared_modules: bool,
) -> Result<(), crate::Diagnostic> {
    if !declared_modules {
        attempt!(out.append(ORIGINAL.as_bytes()));
        return out.append(b"\n");
    }
    let sections = attempt!(Sections::checked());
    sections.emit(out)
}
