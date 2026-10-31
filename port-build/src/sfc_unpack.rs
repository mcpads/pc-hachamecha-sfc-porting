use crate::Result;
use std::collections::HashMap;
const LITERALS: usize = 0x80;
const LEAST: usize = 3;
const LONGEST: usize = 0x7f + LEAST;
const WINDOW: usize = 0xffff;
const CHAIN: usize = 48;
const ITEM: usize = 0x8000;
pub fn tokens(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut heads: HashMap<[u8; 3], Vec<usize>> = HashMap::new();
    let mut literal: Vec<u8> = Vec::new();
    let longest = |heads: &HashMap<[u8; 3], Vec<usize>>, at: usize| -> (usize, usize) {
        let Some(key) = data.get(at..at + 3) else {
            return (0, 0);
        };
        let mut best = (0, 0);
        for &from in heads
            .get(&[key[0], key[1], key[2]])
            .into_iter()
            .flatten()
            .rev()
            .take(CHAIN)
        {
            if at - from > WINDOW {
                break;
            }
            let run = (0..LONGEST.min(data.len() - at))
                .take_while(|&i| data[from + i] == data[at + i])
                .count();
            if run > best.0 {
                best = (run, at - from);
            }
        }
        best
    };
    let note = |heads: &mut HashMap<[u8; 3], Vec<usize>>, at: usize| {
        if let Some(key) = data.get(at..at + 3) {
            heads.entry([key[0], key[1], key[2]]).or_default().push(at);
        }
    };
    let flush = |out: &mut Vec<Vec<u8>>, literal: &mut Vec<u8>| {
        for run in literal.chunks(LITERALS) {
            let mut token = vec![(run.len() - 1) as u8];
            token.extend(run);
            out.push(token);
        }
        literal.clear();
    };
    let mut at = 0;
    while at < data.len() {
        let (run, distance) = longest(&heads, at);
        let later = if run >= LEAST && at + 1 < data.len() {
            note(&mut heads, at);
            let next = longest(&heads, at + 1).0;
            heads
                .get_mut(&[data[at], data[at + 1], data[at + 2]])
                .map(Vec::pop);
            next
        } else {
            0
        };
        if run >= LEAST && later <= run {
            flush(&mut out, &mut literal);
            let mut token = vec![0x80 | (run - LEAST) as u8];
            token.extend((distance as u16).to_le_bytes());
            out.push(token);
            for i in at..at + run {
                note(&mut heads, i);
            }
            at += run;
        } else {
            literal.push(data[at]);
            note(&mut heads, at);
            at += 1;
        }
    }
    flush(&mut out, &mut literal);
    out
}
pub fn unpack(stream: &[u8], size: usize) -> Result<Vec<u8>> {
    let mut out: Vec<u8> = Vec::with_capacity(size);
    let mut at = 0;
    while out.len() < size {
        let token = *stream.get(at).ok_or("Packed stream ends early")?;
        at += 1;
        if token < 0x80 {
            let n = usize::from(token) + 1;
            out.extend(
                stream
                    .get(at..at + n)
                    .ok_or("Packed literal past the end")?,
            );
            at += n;
        } else {
            let n = usize::from(token & 0x7f) + LEAST;
            let distance = usize::from(u16::from_le_bytes([
                *stream.get(at).ok_or("Packed copy past the end")?,
                *stream.get(at + 1).ok_or("Packed copy past the end")?,
            ]));
            at += 2;
            let from = out
                .len()
                .checked_sub(distance)
                .filter(|_| distance > 0)
                .ok_or("Packed copy before the output")?;
            for i in 0..n {
                out.push(out[from + i]);
            }
        }
    }
    if out.len() != size || at != stream.len() {
        return Err("Packed stream does not end with its output".into());
    }
    Ok(out)
}
pub fn items(data: &[u8]) -> Result<Vec<Vec<u8>>> {
    let mut items: Vec<Vec<u8>> = vec![Vec::new()];
    for token in tokens(data) {
        if items.last().unwrap().len() + token.len() > ITEM {
            items.push(Vec::new());
        }
        items.last_mut().unwrap().extend(token);
    }
    if unpack(&items.concat(), data.len())? != data {
        return Err("Packed stream does not rebuild its data".into());
    }
    Ok(items)
}
pub const PAGE: u16 = 0x0b80;
const SOURCE: u8 = 0;
const OUT: u8 = 3;
const LEFT: u8 = 5;
const INDEX: u8 = 7;
const MADE: u8 = 9;
const TAKEN: u8 = 12;
const COUNT: u8 = 15;
const DISTANCE: u8 = 17;
pub const ACTIVE: u8 = 19;
const SCRATCH: u8 = 21;
const TABLE: u8 = 25;
const LIMIT: u8 = 28;
pub const STREAM_ENTRY: usize = 5;
pub const STREAMS_SECTION: &str = "unpack_streams";
pub const TITLE_STREAM: u16 = STREAM_ENTRY as u16;
const ROOM: u16 = (0x10000 - 0x82 - 0x2000) as u16;
const LAST_LINE: u16 = 200;
pub fn emit(p: &mut w65c816::Assembler, streams: u32) {
    use crate::sfc::{emit, implied};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let dp = |p: &mut w65c816::Assembler, m, at: u8| emit(p, m, A::DirectPage, O::Byte(at));
    let long = |p: &mut w65c816::Assembler, m, at: u32| emit(p, m, A::AbsoluteLong, O::Long(at));
    let page = |at: u8| u32::from(PAGE) + u32::from(at);
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let imm = |p: &mut w65c816::Assembler, m, v: u16| emit(p, m, A::Immediate, O::Word(v));
    let branch = |p: &mut w65c816::Assembler, m, to: &str| {
        p.emit_label_ref(m, A::Relative8, to);
    };
    let jump = |p: &mut w65c816::Assembler, to: &str| {
        p.emit_label_ref(M::Brl, A::Relative16, to);
    };
    let load_item = |p: &mut w65c816::Assembler| {
        dp(p, M::Ldy, INDEX);
        emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(TABLE));
        dp(p, M::Sta, SOURCE);
        implied(p, M::Iny);
        implied(p, M::Iny);
        m8(p);
        emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(TABLE));
        dp(p, M::Sta, SOURCE + 2);
        m16(p);
        implied(p, M::Iny);
        emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(TABLE));
        dp(p, M::Sta, LEFT);
    };
    p.label("unpack_reset").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phd);
    m16(p);
    imm(p, M::Lda, PAGE);
    implied(p, M::Tcd);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(streams));
    dp(p, M::Sta, TABLE);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(streams + 3));
    dp(p, M::Sta, LIMIT);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(streams + 2));
    dp(p, M::Sta, TABLE + 2);
    m16(p);
    for at in [OUT, INDEX, MADE, MADE + 1, TAKEN, TAKEN + 1] {
        emit(p, M::Stz, A::DirectPage, O::Byte(at));
    }
    load_item(p);
    m8(p);
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    dp(p, M::Sta, ACTIVE);
    implied(p, M::Pld);
    implied(p, M::Rtl);
    p.label("unpack_ready").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Ldy, A::Immediate, O::Word(12));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe0));
    long(p, M::Sta, page(SCRATCH));
    emit(p, M::Ldy, A::Immediate, O::Word(14));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe0));
    imm(p, M::And, 0xff);
    long(p, M::Sta, page(SCRATCH + 2));
    long(p, M::Lda, page(MADE));
    implied(p, M::Sec);
    long(p, M::Sbc, page(SCRATCH));
    long(p, M::Lda, page(MADE + 2));
    imm(p, M::And, 0xff);
    long(p, M::Sbc, page(SCRATCH + 2));
    branch(p, M::Bcc, "unpack_not_ready");
    long(p, M::Lda, page(SCRATCH));
    long(p, M::Sta, page(TAKEN));
    m8(p);
    long(p, M::Lda, page(SCRATCH + 2));
    long(p, M::Sta, page(TAKEN + 2));
    implied(p, M::Sec);
    implied(p, M::Rtl);
    p.label("unpack_not_ready")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rtl);
    p.label("unpack_step").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phd);
    implied(p, M::Phb);
    m16(p);
    imm(p, M::Lda, PAGE);
    implied(p, M::Tcd);
    m8(p);
    dp(p, M::Lda, ACTIVE);
    branch(p, M::Beq, "unpack_quit");
    long(p, M::Lda, 0x22a);
    branch(p, M::Beq, "unpack_go");
    p.label("unpack_quit").assume_mode(CpuMode::NATIVE_M8_X16);
    jump(p, "unpack_exit_m8");
    p.label("unpack_go").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    p.label("unpack_token").assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Lda, LEFT);
    branch(p, M::Bne, "unpack_have");
    dp(p, M::Lda, INDEX);
    implied(p, M::Clc);
    imm(p, M::Adc, 5);
    dp(p, M::Sta, INDEX);
    dp(p, M::Cmp, LIMIT);
    branch(p, M::Bcc, "unpack_next_item");
    m8(p);
    emit(p, M::Stz, A::DirectPage, O::Byte(ACTIVE));
    jump(p, "unpack_exit_m8");
    p.label("unpack_next_item")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    load_item(p);
    p.label("unpack_have").assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    long(p, M::Lda, 0x213f);
    long(p, M::Lda, 0x2137);
    long(p, M::Lda, 0x213d);
    implied(p, M::Xba);
    long(p, M::Lda, 0x213d);
    emit(p, M::And, A::Immediate, O::Byte(1));
    implied(p, M::Xba);
    m16(p);
    imm(p, M::Cmp, LAST_LINE);
    branch(p, M::Bcc, "unpack_line_ok");
    imm(p, M::Cmp, 225);
    branch(p, M::Bcs, "unpack_line_ok");
    jump(p, "unpack_exit");
    p.label("unpack_line_ok")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Lda, MADE);
    implied(p, M::Sec);
    dp(p, M::Sbc, TAKEN);
    dp(p, M::Sta, SCRATCH);
    dp(p, M::Lda, MADE + 2);
    dp(p, M::Sbc, TAKEN + 2);
    imm(p, M::And, 0xff);
    branch(p, M::Beq, "unpack_room_high");
    jump(p, "unpack_exit");
    p.label("unpack_room_high")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Lda, SCRATCH);
    imm(p, M::Cmp, ROOM);
    branch(p, M::Bcc, "unpack_room");
    jump(p, "unpack_exit");
    p.label("unpack_room").assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Ldx, OUT);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(SOURCE));
    imm(p, M::And, 0xff);
    imm(p, M::Cmp, 0x80);
    branch(p, M::Bcs, "unpack_copy");
    emit(p, M::Inc, A::Accumulator, O::None);
    dp(p, M::Sta, COUNT);
    implied(p, M::Pha);
    implied(p, M::Iny);
    m8(p);
    p.label("unpack_literal")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(
        p,
        M::Lda,
        A::DirectPageIndirectLongIndexedY,
        O::Byte(SOURCE),
    );
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7f0000));
    implied(p, M::Inx);
    implied(p, M::Iny);
    emit(p, M::Dec, A::DirectPage, O::Byte(COUNT));
    branch(p, M::Bne, "unpack_literal");
    m16(p);
    implied(p, M::Pla);
    branch(p, M::Bra, "unpack_advance");
    p.label("unpack_copy").assume_mode(CpuMode::NATIVE_M16_X16);
    imm(p, M::And, 0x7f);
    implied(p, M::Clc);
    imm(p, M::Adc, LEAST as u16);
    implied(p, M::Pha);
    emit(p, M::Dec, A::Accumulator, O::None);
    dp(p, M::Sta, COUNT);
    emit(p, M::Ldy, A::Immediate, O::Word(1));
    emit(
        p,
        M::Lda,
        A::DirectPageIndirectLongIndexedY,
        O::Byte(SOURCE),
    );
    dp(p, M::Sta, DISTANCE);
    implied(p, M::Txy);
    implied(p, M::Txa);
    implied(p, M::Sec);
    dp(p, M::Sbc, DISTANCE);
    implied(p, M::Tax);
    dp(p, M::Lda, COUNT);
    emit(
        p,
        M::Mvn,
        A::BlockMove,
        O::BlockMove {
            destination_bank: 0x7f,
            source_bank: 0x7f,
        },
    );
    implied(p, M::Tyx);
    emit(p, M::Ldy, A::Immediate, O::Word(3));
    implied(p, M::Pla);
    p.label("unpack_advance")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Stx, OUT);
    implied(p, M::Clc);
    dp(p, M::Adc, MADE);
    dp(p, M::Sta, MADE);
    m8(p);
    dp(p, M::Lda, MADE + 2);
    emit(p, M::Adc, A::Immediate, O::Byte(0));
    dp(p, M::Sta, MADE + 2);
    m16(p);
    dp(p, M::Sty, SCRATCH);
    implied(p, M::Tya);
    implied(p, M::Clc);
    dp(p, M::Adc, SOURCE);
    dp(p, M::Sta, SOURCE);
    dp(p, M::Lda, LEFT);
    implied(p, M::Sec);
    dp(p, M::Sbc, SCRATCH);
    dp(p, M::Sta, LEFT);
    jump(p, "unpack_token");
    p.label("unpack_exit").assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    p.label("unpack_exit_m8")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Plb);
    implied(p, M::Pld);
    implied(p, M::Rtl);
}
