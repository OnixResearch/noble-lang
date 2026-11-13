use alloc::vec::Vec;

use super::super::types::{Instruction, Refusal};

pub(super) const MAX_BYTES: usize = 9;

pub(crate) fn next_instruction(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Instruction, Refusal> {
    let opcode = take(bytes, cursor, 1)?[0];
    Ok(match opcode {
        0 => Instruction::Input(read_u32(bytes, cursor)?),
        1 => {
            let raw: [u8; 8] = take(bytes, cursor, 8)?
                .try_into()
                .map_err(|_| Refusal::Malformed)?;
            Instruction::Constant(u64::from_le_bytes(raw))
        }
        2 => Instruction::Add(take(bytes, cursor, 1)?[0], take(bytes, cursor, 1)?[0]),
        3 => Instruction::Sub(take(bytes, cursor, 1)?[0], take(bytes, cursor, 1)?[0]),
        4 => Instruction::Mul(take(bytes, cursor, 1)?[0], take(bytes, cursor, 1)?[0]),
        5 => Instruction::Eq(take(bytes, cursor, 1)?[0], take(bytes, cursor, 1)?[0]),
        6 => Instruction::Save(take(bytes, cursor, 1)?[0], take(bytes, cursor, 1)?[0]),
        7 => Instruction::Load(take(bytes, cursor, 1)?[0]),
        8 => Instruction::Output(take(bytes, cursor, 1)?[0]),
        _ => return Err(Refusal::Malformed),
    })
}

pub(super) fn count_instructions(bytes: &[u8]) -> Result<usize, Refusal> {
    let mut cursor = 0usize;
    let mut count = 0usize;
    while cursor < bytes.len() {
        let _ = next_instruction(bytes, &mut cursor)?;
        count = count.checked_add(1).ok_or(Refusal::InstructionBound)?;
        if count > super::super::MAX_INSTRUCTIONS {
            return Err(Refusal::InstructionBound);
        }
    }
    Ok(count)
}

pub(crate) fn push_instruction(bytes: &mut Vec<u8>, instruction: Instruction) {
    match instruction {
        Instruction::Input(slot) => {
            bytes.push(0);
            bytes.extend_from_slice(&slot.to_le_bytes());
        }
        Instruction::Constant(bits) => {
            bytes.push(1);
            bytes.extend_from_slice(&bits.to_le_bytes());
        }
        Instruction::Add(left, right) => bytes.extend_from_slice(&[2, left, right]),
        Instruction::Sub(left, right) => bytes.extend_from_slice(&[3, left, right]),
        Instruction::Mul(left, right) => bytes.extend_from_slice(&[4, left, right]),
        Instruction::Eq(left, right) => bytes.extend_from_slice(&[5, left, right]),
        Instruction::Save(age, slot) => bytes.extend_from_slice(&[6, age, slot]),
        Instruction::Load(slot) => bytes.extend_from_slice(&[7, slot]),
        Instruction::Output(age) => bytes.extend_from_slice(&[8, age]),
    }
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, Refusal> {
    let raw: [u8; 4] = take(bytes, cursor, 4)?
        .try_into()
        .map_err(|_| Refusal::Malformed)?;
    Ok(u32::from_le_bytes(raw))
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, len: usize) -> Result<&'a [u8], Refusal> {
    let end = cursor.checked_add(len).ok_or(Refusal::Malformed)?;
    let field = bytes.get(*cursor..end).ok_or(Refusal::Malformed)?;
    *cursor = end;
    Ok(field)
}
