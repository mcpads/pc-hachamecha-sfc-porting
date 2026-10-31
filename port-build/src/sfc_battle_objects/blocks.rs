use super::*;
const SPANS: [(u32, u32); 11] = [
    (0x4952dc, 0x4952f8),
    (0x495310, 0x49531c),
    (0x495324, 0x495330),
    (0x495344, 0x495354),
    (0x4bcb68, 0x4bcb78),
    (0x4bd398, 0x4bd3a0),
    (0x4a81ac, 0x4a81b4),
    (0x4a81b8, 0x4a81c0),
    (0x4953c8, 0x495420),
    (0x495428, 0x495458),
    (0x4bd0e4, 0x4bd114),
];
const FAR_SPAN: usize = SPANS.len() - 1;
const OPCODES: [u8; SPANS.len()] = [
    0xf8, 0xf9, 0xfa, 0xfb, 0xfd, 0xff, 0xa3, 0xa4, 0xa9, 0xab, 0x94,
];
pub(super) fn report() -> serde_json::Value {
    SPANS
        .iter()
        .enumerate()
        .map(|(i, &(start, end))| {
            let mut entry = serde_json::json!(
                { "start" : start, "end" : end, "opcode" : OPCODES[i] }
            );
            if i == FAR_SPAN {
                entry["placement"] = "far_code".into();
            }
            if end == radial_branch::PC {
                entry["successor_entry"] = radial_branch::ENTRY.into();
            }
            entry
        })
        .collect::<Vec<_>>()
        .into()
}
#[derive(Debug)]
struct Arithmetic {
    width: u16,
    operation: u8,
    selector: u8,
    to: u16,
    from: u16,
    immediate: u32,
    length: u16,
}
impl Arithmetic {
    fn may_read_pc(&self) -> bool {
        self.selector == 0xb0
            || (self.selector == 0xa0 && self.from < 0x44 && self.from + self.width > 0x40)
    }
}
fn decode(bytes: &[u8], tag: u8) -> Result<Arithmetic> {
    let &(op, selector, operation, _) = VARIANTS
        .iter()
        .find(|v| v.3 == tag && matches!(v.0, 0x11 | 0x12))
        .ok_or("Compiled body block contains unsupported arithmetic")?;
    let operation = operation.ok_or("Compiled body block comparison")?;
    if !matches!(selector, 0x80 | 0xa0 | 0xb0) || operation > 3 {
        return Err("Compiled body block operand form".into());
    }
    let width = if op == 0x12 { 4 } else { 2 };
    let length = if selector == 0x80 { 8 } else { 4 };
    if bytes.len() < length as usize {
        return Err("Truncated compiled body block command".into());
    }
    let (to, from) = (u16::from(bytes[2]), u16::from(bytes[3]));
    if to < 0x1c
        || to + width > OBJECT_BYTES
        || (to < 0x58 && to + width > 0x40)
        || (selector == 0xa0 && from + width > OBJECT_BYTES)
    {
        return Err("Compiled body block field outside admitted data".into());
    }
    let immediate = if selector == 0x80 {
        u32::from_le_bytes(bytes[4..8].try_into().unwrap())
    } else {
        0
    };
    if operation == 3 && (width != 4 || selector != 0x80 || immediate != 65536) {
        return Err("Compiled body multiply is not own fixed-point scaling".into());
    }
    Ok(Arithmetic {
        width,
        operation,
        selector,
        to,
        from,
        immediate,
        length,
    })
}
#[derive(Debug)]
enum Command {
    Arithmetic(Arithmetic),
    Random(u16),
}
impl Command {
    fn length(&self) -> u16 {
        match self {
            Self::Arithmetic(op) => op.length,
            Self::Random(_) => 4,
        }
    }
    fn may_read_pc(&self) -> bool {
        match self {
            Self::Arithmetic(op) => op.may_read_pc(),
            Self::Random(_) => false,
        }
    }
}
fn parse(bytes: &[u8], first_tag: u8) -> Result<Vec<Command>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let tag = if at == 0 { first_tag } else { bytes[at] };
        let op = if tag == 0x2b {
            let data = bytes
                .get(at..at + 4)
                .ok_or("Truncated compiled RNG command")?;
            Command::Random(u16::from_le_bytes([data[2], data[3]]))
        } else {
            Command::Arithmetic(decode(&bytes[at..], tag)?)
        };
        at += usize::from(op.length());
        out.push(op);
    }
    if out.is_empty() {
        return Err("Compiled body block needs a command".into());
    }
    Ok(out)
}
pub(super) fn patch(image: &mut [u8], commands: &[u32]) -> Result<()> {
    let commands: BTreeSet<_> = commands.iter().copied().collect();
    if commands.iter().any(|&pc| {
        pc.checked_sub(SOURCE_BASE)
            .and_then(|at| image.get(at as usize))
            .is_some_and(|tag| OPCODES.contains(tag))
    }) {
        return Err("Compiled body opcode already used by a source command".into());
    }
    for (i, &(start, end)) in SPANS.iter().enumerate() {
        let bytes = image
            .get_mut((start - SOURCE_BASE) as usize..(end - SOURCE_BASE) as usize)
            .ok_or("Compiled body block outside image")?;
        let ops = parse(bytes, bytes[0])?;
        let mut pc = start;
        for op in ops {
            if !commands.contains(&pc) {
                return Err("Compiled body block is not a command boundary".into());
            }
            pc += u32::from(op.length());
        }
        if !commands.contains(&end) || start >> 15 != end >> 15 {
            return Err("Compiled body block end crosses a PC sign/bank boundary".into());
        }
        bytes[1] = bytes[0];
        bytes[0] = OPCODES[i];
    }
    Ok(())
}
pub(super) fn emit_blocks(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    for (i, &(_, end)) in SPANS.iter().enumerate() {
        let (ops, successor) = placed_block(layout, i)?;
        p.label(format!("bvm_op_{:02x}", OPCODES[i]))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        dp(p, M::Ldx, OBJ);
        if i == FAR_SPAN {
            emit(
                p,
                M::Jsl,
                A::AbsoluteLong,
                O::Long(crate::sfc_far_code::arithmetic_block_entry(layout)?),
            );
            p.assume_mode(CpuMode::NATIVE_M16_X16);
            emit_dispatch(p, end, successor);
            continue;
        }
        let mut pending = 0;
        for (j, command) in ops.iter().enumerate() {
            if pending != 0 && command.may_read_pc() {
                advance(p, pending);
                pending = 0;
            }
            match command {
                Command::Random(divisor) => emit_random(p, *divisor),
                Command::Arithmetic(op) if op.operation == 3 => emit_fixed_scale(p, op.to),
                Command::Arithmetic(op) => {
                    emit_arithmetic(
                        p,
                        op,
                        &format!("bvm_block_{i}_{j}_read"),
                        j + 1 == ops.len(),
                    );
                }
            }
            pending += command.length();
            if j + 1 == ops.len() {
                if !matches!(command, Command::Arithmetic(op) if op.operation != 3) {
                    m16(p);
                }
                emit_dispatch(p, end, successor);
            }
        }
    }
    Ok(())
}
fn placed_block(layout: &CartridgeLayout, i: usize) -> Result<(Vec<Command>, u8)> {
    let (start, end) = SPANS[i];
    let file = IMAGE_OFFSET + (start - SOURCE_BASE) as usize;
    let mut bytes = Vec::new();
    let mut part = 0;
    while let Ok(section) = layout.section(&format!("battle_scripts_{part}")) {
        for at in file..=file + (end - start) as usize {
            if at >= section.offset && at < section.offset + section.bytes.len() {
                bytes.push(section.bytes[at - section.offset]);
            }
        }
        part += 1;
    }
    if bytes.len() != (end - start) as usize + 1 || bytes[0] != OPCODES[i] {
        return Err("Compiled body block missing from kept script sections".into());
    }
    let successor = bytes.pop().unwrap();
    let ops = parse(&bytes, bytes[1])?;
    Ok((ops, successor))
}
fn emit_dispatch(p: &mut Assembler, end: u32, successor: u8) {
    imm16(p, M::Lda, end as u16);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    implied(p, M::Clc);
    implied(p, M::Clv);
    imm16(p, M::Lda, u16::from(successor) * 2);
    implied(p, M::Tax);
    m8(p);
    if end == radial_branch::PC {
        jmp(p, radial_branch::ENTRY);
    } else {
        jmp(p, &format!("bvm_op_{successor:02x}"));
    }
}
fn emit_arithmetic(p: &mut Assembler, op: &Arithmetic, read: &str, last: bool) {
    if op.selector == 0xb0 {
        imm8(p, M::Lda, op.from as u8);
        dp(p, M::Sta, OFF);
        imm8(p, M::Lda, 0xc0);
        jsr(p, "bvm_at");
        m16(p);
        dp(p, M::Stz, TMP);
        dp(p, M::Stz, TMP + 2);
        m8(p);
        dp(p, M::Lda, VALID);
        br(p, M::Beq, read);
        m16(p);
        emit(p, M::Lda, A::DirectPageIndirectIndexedY, O::Byte(PTR));
        dp(p, M::Sta, TMP);
        if op.width == 4 {
            implied(p, M::Iny);
            implied(p, M::Iny);
            emit(p, M::Lda, A::DirectPageIndirectIndexedY, O::Byte(PTR));
            dp(p, M::Sta, TMP + 2);
        }
        m8(p);
        p.label(read);
        m16(p);
    } else {
        m16(p);
        for word in 0..op.width / 2 {
            if op.selector == 0x80 {
                imm16(p, M::Lda, (op.immediate >> (word * 16)) as u16);
            } else {
                ax(p, M::Lda, op.from + word * 2);
            }
            dp(p, M::Sta, TMP + word as u8 * 2);
        }
    }
    for word in 0..op.width / 2 {
        let to = op.to + word * 2;
        if op.operation == 0 {
            dp(p, M::Lda, TMP + word as u8 * 2);
        } else {
            ax(p, M::Lda, to);
            if word == 0 {
                implied(p, if op.operation == 1 { M::Clc } else { M::Sec });
            }
            dp(
                p,
                if op.operation == 1 { M::Adc } else { M::Sbc },
                TMP + word as u8 * 2,
            );
        }
        ax(p, M::Sta, to);
    }
    imm16(p, M::Ldy, op.to + op.width - 2);
    if !last {
        m8(p);
    }
}
pub(super) fn emit_far(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    let (ops, _) = placed_block(layout, FAR_SPAN)?;
    require_far_arithmetic(&ops)?;
    p.label("bvm_arithmetic_block_far")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    for (j, command) in ops.iter().enumerate() {
        let Command::Arithmetic(op) = command else {
            unreachable!()
        };
        emit_arithmetic(
            p,
            op,
            &format!("bvm_block_{FAR_SPAN}_{j}_read"),
            j + 1 == ops.len(),
        );
    }
    implied(p, M::Rtl);
    Ok(())
}
fn require_far_arithmetic(ops: &[Command]) -> Result<()> {
    if !ops.iter().all(|command| {
        matches!(
            command, Command::Arithmetic(op) if op.selector == 0x80 && op.operation
            <= 2
        )
    }) {
        return Err("Far arithmetic block requires literal own-field operations".into());
    }
    Ok(())
}
fn emit_random(p: &mut Assembler, divisor: u16) {
    m16(p);
    imm16(p, M::Ldy, 2);
    imm16(p, M::Lda, divisor);
    abs(p, M::Sta, crate::sfc_battle::DIVISOR);
    m8(p);
    implied(p, M::Phb);
    implied(p, M::Phd);
    m16(p);
    imm16(p, M::Lda, 0);
    implied(p, M::Tcd);
    m8(p);
    implied(p, M::Pha);
    implied(p, M::Plb);
    jsr(p, "battle_rng");
    implied(p, M::Pld);
    implied(p, M::Plb);
    dp(p, M::Ldx, OBJ);
    m16(p);
    abs(p, M::Lda, crate::sfc_battle_act::RANDOM);
    ax(p, M::Sta, 0x58);
    ax(p, M::Stz, 0x5a);
    m8(p);
}
fn emit_fixed_scale(p: &mut Assembler, to: u16) {
    m16(p);
    dp(p, M::Lda, OBJ);
    dp(p, M::Sta, PTR);
    for word in 0..2u8 {
        ax(p, M::Lda, to + u16::from(word) * 2);
        dp(p, M::Sta, TMP + word * 2);
        dp(p, M::Sta, LHS + word * 2);
    }
    dp(p, M::Stz, SRC);
    imm16(p, M::Lda, 1);
    dp(p, M::Sta, SRC + 2);
    dp(p, M::Stz, RES);
    dp(p, M::Lda, LHS);
    dp(p, M::Sta, RES + 2);
    ax(p, M::Stz, to);
    ax(p, M::Sta, to + 2);
    for word in 0..4u8 {
        dp(p, M::Stz, WORK + word * 2);
    }
    implied(p, M::Clv);
    implied(p, M::Sec);
    imm16(p, M::Ldy, to + 2);
    m8(p);
    imm8(p, M::Lda, 4);
    dp(p, M::Sta, WIDTH);
    dp(p, M::Sta, RW);
    imm8(p, M::Lda, 0x83);
    dp(p, M::Sta, MODE);
    imm8(p, M::Lda, 1);
    dp(p, M::Sta, VALID);
    imm8(p, M::Lda, to as u8);
    dp(p, M::Sta, OFF);
}
