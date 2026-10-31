use super::*;
const CODE: &str = "guardian_code";
const OFFSET: usize = 0x800;
const CAPACITY: usize = 0x160;
const _: () = assert!(OFFSET + CAPACITY <= crate::sfc_battle_objects::bound_child_copy::OFFSET);
pub(super) fn entry(layout: &CartridgeLayout) -> Result<u32> {
    layout.cpu_address(layout.section(CODE)?.offset + OFFSET)
}
pub fn place_code(sections: &mut [Section], layout: &CartridgeLayout) -> Result<()> {
    let address = entry(layout)?;
    let high_bits = layout.cpu_address(layout.section("battle_display_high_bits")?.offset)?;
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    let dp = |p: &mut Assembler, m, at: u16| {
        emit(p, m, A::DirectPage, O::Byte((at - OBJECT_PAGE) as u8))
    };
    let long = |p: &mut Assembler, m, at| emit(p, m, A::AbsoluteLong, O::Long(at));
    let long_x = |p: &mut Assembler, m, at| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let indirect = |p: &mut Assembler, m| {
        emit(
            p,
            m,
            A::DirectPageIndirectLongIndexedY,
            O::Byte((OBJECT_FRAME_POINTER - OBJECT_PAGE) as u8),
        )
    };
    let jump = |p: &mut Assembler, to: &str| {
        p.emit_label_ref(M::Jmp, A::Absolute, to);
    };
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    word(p, M::Ldy, 0);
    indirect(p, M::Lda);
    byte(p, M::Cmp, 1);
    branch(p, M::Bne, "fallback");
    implied(p, M::Iny);
    indirect(p, M::Lda);
    byte(p, M::And, 0x7f);
    byte(p, M::Cmp, 1);
    branch(p, M::Bne, "fallback");
    dp(p, M::Lda, RESOURCE);
    emit(p, M::Inc, A::Accumulator, O::None);
    dp(p, M::Sta, DATA);
    dp(p, M::Eor, SHEET_FRAME);
    m16(p);
    word(p, M::And, 255);
    implied(p, M::Tax);
    long_x(p, M::Lda, HINTS);
    word(p, M::And, 255);
    word(p, M::Cmp, CACHE_ENTRIES);
    branch(p, M::Bcs, "fallback16");
    implied(p, M::Tax);
    m8(p);
    long_x(p, M::Lda, cache(0));
    dp(p, M::Cmp, DATA);
    branch(p, M::Bne, "fallback");
    long_x(p, M::Lda, cache(1));
    dp(p, M::Cmp, SHEET_FRAME);
    branch(p, M::Bne, "fallback");
    long_x(p, M::Lda, cache(2));
    byte(p, M::Cmp, 1);
    branch(p, M::Beq, "hit");
    p.label("fallback").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Clc);
    implied(p, M::Rtl);
    p.label("fallback16").assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rtl);
    p.label("hit").assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Lda, 0);
    long(p, M::Sta, KNOWN);
    byte(p, M::Lda, 1);
    long_x(p, M::Sta, cache(5));
    long_x(p, M::Lda, cache(3));
    for _ in 0..5 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    dp(p, M::Sta, SLOT_FOUND);
    long_x(p, M::Lda, cache(4));
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    dp(p, M::Adc, SLOT_FOUND);
    dp(p, M::Sta, SLOT_FOUND);
    indirect(p, M::Lda);
    byte(p, M::And, 0x80);
    for _ in 0..6 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    abs(p, M::Sta, CHUNK_ATTRIBUTE);
    dp(p, M::Lda, ATTRIBUTE + 1);
    dp(p, M::Sta, SLOT_FOUND + 1);
    m16(p);
    for (offset, origin, at) in [(8, ORIGIN_X, AT_X), (9, ORIGIN_Y, AT_Y)] {
        word(p, M::Ldy, offset);
        indirect(p, M::Lda);
        word(p, M::And, 255);
        implied(p, M::Clc);
        dp(p, M::Adc, origin);
        implied(p, M::Clc);
        word(p, M::Adc, 128);
        dp(p, M::Sta, at);
    }
    dp(p, M::Lda, ARLE_SEEN);
    branch(p, M::Beq, "priority_ready");
    dp(p, M::Lda, SLOT_FOUND);
    word(p, M::And, 0xcfff);
    word(p, M::Ora, 0x1000);
    dp(p, M::Sta, SLOT_FOUND);
    p.label("priority_ready");
    m8(p);
    dp(p, M::Lda, SLOT_FOUND + 1);
    implied(p, M::Clc);
    abs(p, M::Adc, CHUNK_ATTRIBUTE);
    dp(p, M::Sta, SLOT_FOUND + 1);
    m16(p);
    dp(p, M::Lda, OBJECT);
    implied(p, M::Sec);
    word(p, M::Sbc, crate::sfc_battle_objects::POOL);
    for _ in 0..3 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    dp(p, M::Lda, SHEET_FRAME);
    long_x(p, M::Sta, LAST_DRAWN);
    dp(p, M::Lda, RESOURCE);
    emit(p, M::Inc, A::Accumulator, O::None);
    long_x(p, M::Sta, LAST_DRAWN + 2);
    dp(p, M::Lda, ENTRY);
    word(p, M::Cmp, 512);
    branch(p, M::Bcs, "clipped");
    dp(p, M::Lda, AT_X);
    dp(p, M::Cmp, PANEL_LEFT);
    branch(p, M::Bcc, "clipped");
    branch(p, M::Beq, "clipped");
    dp(p, M::Cmp, PANEL_RIGHT);
    branch(p, M::Bcs, "clipped");
    dp(p, M::Lda, AT_Y);
    word(p, M::Cmp, (SPRITE_TOP - 16 + 257) as u16);
    branch(p, M::Bcc, "clipped");
    word(p, M::Cmp, (SPRITE_BOTTOM + 256) as u16);
    branch(p, M::Bcc, "visible");
    p.label("clipped");
    jump(p, "done");
    p.label("visible");
    dp(p, M::Ldx, ENTRY);
    m8(p);
    dp(p, M::Lda, AT_X);
    emit(
        p,
        M::Sta,
        A::AbsoluteX,
        O::Word(crate::sfc_b1_menu::OAM_BUFFER),
    );
    dp(p, M::Lda, AT_Y);
    emit(
        p,
        M::Sta,
        A::AbsoluteX,
        O::Word(crate::sfc_b1_menu::OAM_BUFFER + 1),
    );
    m16(p);
    dp(p, M::Lda, SLOT_FOUND);
    emit(
        p,
        M::Sta,
        A::AbsoluteX,
        O::Word(crate::sfc_b1_menu::OAM_BUFFER + 2),
    );
    dp(p, M::Lda, AT_X);
    word(p, M::Cmp, 256);
    branch(p, M::Bcs, "high_ready");
    dp(p, M::Lda, ENTRY);
    word(p, M::And, 12);
    emit(p, M::Lsr, A::Accumulator, O::None);
    implied(p, M::Tax);
    m8(p);
    long_x(p, M::Lda, high_bits);
    dp(p, M::Sta, CELL);
    m16(p);
    dp(p, M::Lda, ENTRY);
    for _ in 0..4 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    m8(p);
    emit(
        p,
        M::Lda,
        A::AbsoluteX,
        O::Word(crate::sfc_b1_menu::OAM_BUFFER + 512),
    );
    dp(p, M::Ora, CELL);
    emit(
        p,
        M::Sta,
        A::AbsoluteX,
        O::Word(crate::sfc_b1_menu::OAM_BUFFER + 512),
    );
    m16(p);
    p.label("high_ready").assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Lda, ENTRY);
    implied(p, M::Clc);
    word(p, M::Adc, 4);
    dp(p, M::Sta, ENTRY);
    p.label("done");
    m8(p);
    implied(p, M::Sec);
    implied(p, M::Rtl);
    let code = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((address >> 16) as u8, address as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if code.bytes().len() > CAPACITY {
        return Err("Single-block helper exceeds capacity".into());
    }
    let section = sections
        .iter_mut()
        .find(|s| s.name == CODE)
        .ok_or("Missing single-block helper")?;
    let reserved = section
        .bytes
        .get_mut(OFFSET..OFFSET + CAPACITY)
        .ok_or("Missing single-block helper reserve")?;
    if reserved.iter().any(|&byte| byte != 0) {
        return Err("Single-block helper overlaps another guardian-tail owner".into());
    }
    reserved[..code.bytes().len()].copy_from_slice(code.bytes());
    Ok(())
}
