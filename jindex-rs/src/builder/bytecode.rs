use anyhow::{anyhow, bail, ensure, Context};

pub(super) fn visit_constant_pool_operands(
    code: &[u8],
    mut visitor: impl FnMut(u16) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut offset = 0;
    while offset < code.len() {
        let opcode = code[offset];
        let instruction_length = match opcode {
            0x12 => {
                visitor(u16::from(read_u8(code, offset + 1)?))?;
                2
            }
            0x13 | 0x14 | 0xb2..=0xb8 | 0xbb | 0xbd | 0xc0 | 0xc1 => {
                visitor(read_u16(code, offset + 1)?)?;
                3
            }
            0xb9 | 0xba => {
                visitor(read_u16(code, offset + 1)?)?;
                5
            }
            0xc5 => {
                visitor(read_u16(code, offset + 1)?)?;
                4
            }
            0x10 | 0x15..=0x19 | 0x36..=0x3a | 0xa9 | 0xbc => 2,
            0x11 | 0x84 | 0x99..=0xa8 | 0xc6 | 0xc7 => 3,
            0xc8 | 0xc9 => 5,
            0xaa => table_switch_length(code, offset)?,
            0xab => lookup_switch_length(code, offset)?,
            0xc4 => wide_length(code, offset)?,
            0x00..=0x0f
            | 0x1a..=0x35
            | 0x3b..=0x83
            | 0x85..=0x98
            | 0xac..=0xb1
            | 0xbe..=0xbf
            | 0xc2..=0xc3 => 1,
            _ => bail!("Unsupported bytecode opcode 0x{opcode:02x} at offset {offset}"),
        };
        offset = offset
            .checked_add(instruction_length)
            .ok_or_else(|| anyhow!("Bytecode offset overflow"))?;
        ensure!(
            offset <= code.len(),
            "Truncated bytecode instruction 0x{opcode:02x} at offset {}",
            offset - instruction_length
        );
    }
    Ok(())
}

fn table_switch_length(code: &[u8], offset: usize) -> anyhow::Result<usize> {
    let operands = aligned_switch_operands(offset)?;
    let low = read_i32(code, offset + operands + 4)?;
    let high = read_i32(code, offset + operands + 8)?;
    ensure!(
        high >= low,
        "tableswitch high value is lower than low value"
    );
    let entry_count = u32::try_from(i64::from(high) - i64::from(low) + 1)? as usize;
    operands
        .checked_add(12)
        .and_then(|length| {
            entry_count
                .checked_mul(4)
                .and_then(|entries| length.checked_add(entries))
        })
        .ok_or_else(|| anyhow!("tableswitch size overflow"))
        .and_then(|length| checked_instruction_length(code, offset, length, "tableswitch"))
}

fn lookup_switch_length(code: &[u8], offset: usize) -> anyhow::Result<usize> {
    let operands = aligned_switch_operands(offset)?;
    let pair_count = read_i32(code, offset + operands + 4)?;
    ensure!(pair_count >= 0, "lookupswitch has a negative pair count");
    operands
        .checked_add(8)
        .and_then(|length| {
            (pair_count as usize)
                .checked_mul(8)
                .and_then(|pairs| length.checked_add(pairs))
        })
        .ok_or_else(|| anyhow!("lookupswitch size overflow"))
        .and_then(|length| checked_instruction_length(code, offset, length, "lookupswitch"))
}

fn aligned_switch_operands(offset: usize) -> anyhow::Result<usize> {
    let after_opcode = offset
        .checked_add(1)
        .ok_or_else(|| anyhow!("Bytecode offset overflow"))?;
    let padding = (4 - (after_opcode & 3)) & 3;
    Ok(1 + padding)
}

fn wide_length(code: &[u8], offset: usize) -> anyhow::Result<usize> {
    let modified_opcode = read_u8(code, offset + 1)?;
    match modified_opcode {
        0x15..=0x19 | 0x36..=0x3a | 0xa9 => Ok(4),
        0x84 => Ok(6),
        _ => bail!("Invalid wide opcode 0x{modified_opcode:02x} at offset {offset}"),
    }
}

fn checked_instruction_length(
    code: &[u8],
    offset: usize,
    length: usize,
    name: &str,
) -> anyhow::Result<usize> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| anyhow!("{name} offset overflow"))?;
    ensure!(end <= code.len(), "Truncated {name} at offset {offset}");
    Ok(length)
}

fn read_u8(code: &[u8], offset: usize) -> anyhow::Result<u8> {
    code.get(offset)
        .copied()
        .with_context(|| format!("Truncated bytecode operand at offset {offset}"))
}

fn read_u16(code: &[u8], offset: usize) -> anyhow::Result<u16> {
    Ok(u16::from_be_bytes([
        read_u8(code, offset)?,
        read_u8(code, offset + 1)?,
    ]))
}

fn read_i32(code: &[u8], offset: usize) -> anyhow::Result<i32> {
    Ok(i32::from_be_bytes([
        read_u8(code, offset)?,
        read_u8(code, offset + 1)?,
        read_u8(code, offset + 2)?,
        read_u8(code, offset + 3)?,
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visits_narrow_and_wide_constant_pool_operands() {
        let code = [
            0x12, 0x07, // ldc #7
            0xb6, 0x01, 0x02, // invokevirtual #258
            0xba, 0x03, 0x04, 0x00, 0x00, // invokedynamic #772
            0xc5, 0x05, 0x06, 0x02, // multianewarray #1286, 2
            0xb1, // return
        ];
        let mut indices = Vec::new();
        visit_constant_pool_operands(&code, |index| {
            indices.push(index);
            Ok(())
        })
        .unwrap();
        assert_eq!(indices, [7, 258, 772, 1286]);
    }

    #[test]
    fn handles_switch_alignment_and_wide_instructions() {
        let code = [
            0x03, // iconst_0
            0xaa, 0x00, 0x00, // tableswitch padding
            0x00, 0x00, 0x00, 0x00, // default
            0x00, 0x00, 0x00, 0x01, // low
            0x00, 0x00, 0x00, 0x02, // high
            0x00, 0x00, 0x00, 0x00, // case 1
            0x00, 0x00, 0x00, 0x00, // case 2
            0xc4, 0x84, 0x00, 0x01, 0x00, 0x02, // wide iinc
            0xb1,
        ];
        assert!(visit_constant_pool_operands(&code, |_| Ok(())).is_ok());
    }

    #[test]
    fn rejects_truncated_and_invalid_instructions() {
        assert!(visit_constant_pool_operands(&[0xb6, 0x00], |_| Ok(())).is_err());
        assert!(visit_constant_pool_operands(&[0xc4, 0xb1], |_| Ok(())).is_err());
        assert!(visit_constant_pool_operands(&[0xca], |_| Ok(())).is_err());
    }
}
