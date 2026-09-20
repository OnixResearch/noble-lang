//! Iterative decoding of documented types, stacks, and effect sets.

enum Step<'a> {
    Read(&'a crate::value::Json),
    Pair,
    Sum,
    List,
    ProgramOutput {
        payload: &'a crate::value::Json,
        input_len: usize,
    },
    ProgramFinish {
        payload: &'a crate::value::Json,
        input_len: usize,
        output_len: usize,
    },
}

struct Assembly<'a> {
    pending: Vec<Step<'a>>,
    ready: Vec<noble_kernel::types::Ty>,
}

impl<'a> Assembly<'a> {
    /// Reserve only the children and finish steps scheduled by this value.
    fn reserve_steps(&mut self, count: usize) -> Result<(), String> {
        self.pending
            .try_reserve(count)
            .map_err(|problem| format!("cannot reserve type decoding work: {problem}"))
    }

    fn take(&mut self) -> Result<noble_kernel::types::Ty, String> {
        self.ready
            .pop()
            .ok_or_else(|| "type assembly misses a child".to_string())
    }

    fn binary(&mut self, is_sum: bool) -> Result<noble_kernel::types::Ty, String> {
        let right = self.take()?;
        let left = self.take()?;
        if is_sum {
            Ok(noble_kernel::types::Ty::Sum(
                Box::new(left),
                Box::new(right),
            ))
        } else {
            Ok(noble_kernel::types::Ty::Pair(
                Box::new(left),
                Box::new(right),
            ))
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; take_tail transfers an owned Vec through mem::take or allocating Vec::split_off, neither usable here in const evaluation; reassess when those Vec operations become const."
    )]
    fn take_tail(&mut self, start: usize) -> Vec<noble_kernel::types::Ty> {
        if start == 0 {
            std::mem::take(&mut self.ready)
        } else {
            self.ready.split_off(start)
        }
    }

    fn schedule_stack(
        &mut self,
        entries: &'a [crate::value::Json],
        finish: Step<'a>,
    ) -> Result<(), String> {
        let count = entries
            .len()
            .checked_add(1)
            .ok_or_else(|| "program stack length exceeds addressable work".to_string())?;
        self.reserve_steps(count)?;
        self.pending.push(finish);
        self.pending.extend(entries.iter().rev().map(Step::Read));
        Ok(())
    }

    #[expect(
        tigerstyle::ambiguous_params,
        reason = "Owner: noble-maintainers; input_len and output_len intentionally count entries in the same ready-type stack and come from the named ProgramFinish fields; domain wrappers would not distinguish their units."
    )]
    fn finish_program(
        &mut self,
        payload: &crate::value::Json,
        input_len: usize,
        output_len: usize,
    ) -> Result<noble_kernel::types::Ty, String> {
        let effects = effect_set(payload.field("effects")?)?;
        let output_at = self
            .ready
            .len()
            .checked_sub(output_len)
            .ok_or_else(|| "program assembly misses output types".to_string())?;
        let input_at = output_at
            .checked_sub(input_len)
            .ok_or_else(|| "program assembly misses input types".to_string())?;
        let output = self.take_tail(output_at);
        let input = self.take_tail(input_at);
        Ok(noble_kernel::types::Ty::program(input, output, effects))
    }

    fn expand(
        &mut self,
        value: &'a crate::value::Json,
    ) -> Result<Option<noble_kernel::types::Ty>, String> {
        let (key, payload) = single_key(value)?;
        let primitive = match key {
            "unit" => noble_kernel::types::Ty::Unit,
            "bool" => noble_kernel::types::Ty::Bool,
            "i64" => noble_kernel::types::Ty::I64,
            "text" => noble_kernel::types::Ty::Text,
            "syntax" => noble_kernel::types::Ty::Syntax,
            "pair" | "sum" => {
                let [left, right] = payload.as_arr()? else {
                    return Err(format!("`{key}` needs exactly two payloads"));
                };
                let finish = if key == "pair" { Step::Pair } else { Step::Sum };
                self.reserve_steps(3)?;
                self.pending
                    .extend([finish, Step::Read(right), Step::Read(left)]);
                return Ok(None);
            }
            "list" => {
                self.reserve_steps(2)?;
                self.pending.extend([Step::List, Step::Read(payload)]);
                return Ok(None);
            }
            "program" => {
                let input = payload.field("in")?.as_arr()?;
                self.schedule_stack(
                    input,
                    Step::ProgramOutput {
                        payload,
                        input_len: input.len(),
                    },
                )?;
                return Ok(None);
            }
            "resource" => match payload.as_str()? {
                "test.counter" => {
                    noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE)
                }
                other => return Err(format!("unknown resource kind `{other}`")),
            },
            _ => return Err(format!("unknown type key `{key}`")),
        };
        Ok(Some(primitive))
    }
}

pub(super) fn single_key(
    value: &crate::value::Json,
) -> Result<(&str, &crate::value::Json), String> {
    match value {
        crate::value::Json::Obj(entries) => match entries.as_slice() {
            [(key, payload)] => Ok((key, payload)),
            [] | [_, _, ..] => Err("expected a one-field type object".to_string()),
        },
        crate::value::Json::Null
        | crate::value::Json::Bool(_)
        | crate::value::Json::Int(_)
        | crate::value::Json::Float(_)
        | crate::value::Json::Str(_)
        | crate::value::Json::Arr(_) => Err("expected a one-field type object".to_string()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; ty assembles a fallibly decoded type and reports missing/unused children or reservation failures through Result; malformed documentation must not trigger assertions."
)]
pub(super) fn ty(value: &crate::value::Json) -> Result<noble_kernel::types::Ty, String> {
    let mut assembly = Assembly {
        pending: Vec::new(),
        ready: Vec::new(),
    };
    if let Some(primitive) = assembly.expand(value)? {
        return Ok(primitive);
    }
    while let Some(step) = assembly.pending.pop() {
        #[expect(
            tigerstyle::fragile_exhaustive_enum_match,
            reason = "Owner: noble-maintainers; every Step carries a distinct type-assembly action; adding a step must make this dispatcher fail compilation until its semantics are implemented."
        )]
        let completed = match step {
            Step::Read(value) => assembly.expand(value)?,
            Step::Pair => Some(assembly.binary(false)?),
            Step::Sum => Some(assembly.binary(true)?),
            Step::List => Some(noble_kernel::types::Ty::List(Box::new(assembly.take()?))),
            Step::ProgramOutput { payload, input_len } => {
                let output = payload.field("out")?.as_arr()?;
                assembly.schedule_stack(
                    output,
                    Step::ProgramFinish {
                        payload,
                        input_len,
                        output_len: output.len(),
                    },
                )?;
                None
            }
            Step::ProgramFinish {
                payload,
                input_len,
                output_len,
            } => Some(assembly.finish_program(payload, input_len, output_len)?),
        };
        if let Some(completed) = completed {
            if assembly.pending.is_empty() {
                return if assembly.ready.is_empty() {
                    Ok(completed)
                } else {
                    Err("type assembly left unused children".to_string())
                };
            }
            assembly
                .ready
                .try_reserve(1)
                .map_err(|problem| format!("cannot reserve an assembled type: {problem}"))?;
            assembly.ready.push(completed);
        }
    }
    Err("type assembly produced no completed type".to_string())
}

pub(super) fn stack(value: &crate::value::Json) -> Result<Vec<noble_kernel::types::Ty>, String> {
    value.as_arr()?.iter().map(ty).collect()
}

fn effect_id(value: &crate::value::Json) -> Result<noble_kernel::types::EffId, String> {
    match value {
        crate::value::Json::Str(name) => {
            if name == "test.emit" {
                Ok(noble_kernel::contracts::TEST_EMIT)
            } else {
                Err(format!("unknown effect `{name}`; expected `test.emit`"))
            }
        }
        crate::value::Json::Int(id) => u32::try_from(*id)
            .map(noble_kernel::types::EffId)
            .map_err(|problem| format!("effect identifier {id} is out of range: {problem}")),
        crate::value::Json::Null
        | crate::value::Json::Bool(_)
        | crate::value::Json::Float(_)
        | crate::value::Json::Arr(_)
        | crate::value::Json::Obj(_) => {
            Err("effects are `test.emit` or small integers".to_string())
        }
    }
}

pub(super) fn effect_set(
    value: &crate::value::Json,
) -> Result<noble_kernel::types::EffSet, String> {
    let ids: Vec<noble_kernel::types::EffId> = value
        .as_arr()?
        .iter()
        .map(effect_id)
        .collect::<Result<_, _>>()?;
    Ok(noble_kernel::types::EffSet::from_ids(&ids))
}
