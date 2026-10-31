use super::*;
pub(super) fn emit_boss_view(
    p: &mut Assembler,
    layout: &CartridgeLayout,
    table_y: u32,
) -> Result<()> {
    use crate::sfc_battle_objects::SENTINELS;
    let address = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let info = layout.section("battle_boss_info")?.bytes.clone();
    let height = u16::from_le_bytes([info[0], info[1]]);
    let (slot, frame) = (info[2], u16::from_le_bytes([info[3], info[4]]));
    let enemies = info[6..6 + usize::from(info[5])].to_vec();
    let (rows, offsets, lines) = (
        address("battle_boss_view")?,
        address("battle_boss_rows")?,
        address("battle_boss_lines")?,
    );
    let views = crate::sfc_b1::VIEW_JOB;
    let long_x = |p: &mut Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let pool = |p: &mut Assembler, m, field: u16| {
        emit(p, m, A::AbsoluteLongX, O::Long(0x7e0000 + u32::from(field)));
    };
    let ind =
        |p: &mut Assembler, m, dp: u8| emit(p, m, A::DirectPageIndirectLongIndexedY, O::Byte(dp));
    p.label("battle_boss_check")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    if enemies.is_empty() {
        implied(p, M::Rts);
        return emit_overlay(p, &[], 0, rows, offsets, lines);
    }
    abs(p, M::Lda, crate::sfc_encounter::ENEMY);
    for &e in &enemies {
        byte(p, M::Cmp, e);
        branch(p, M::Beq, "bbc_enemy");
    }
    implied(p, M::Rts);
    p.label("bbc_enemy");
    m16(p);
    word(p, M::Lda, NO_BOSS);
    abs(p, M::Sta, CELL);
    word(p, M::Lda, SENTINELS);
    abs(p, M::Sta, ROW);
    p.label("bbc_group");
    abs(p, M::Lda, ROW);
    implied(p, M::Clc);
    word(p, M::Adc, 16);
    abs(p, M::Sta, COLUMN);
    abs(p, M::Ldx, ROW);
    pool(p, M::Lda, 8);
    p.label("bbc_object");
    implied(p, M::Tax);
    abs(p, M::Cmp, COLUMN);
    branch(p, M::Beq, "bbc_group_done");
    m8(p);
    pool(p, M::Lda, 3);
    byte(p, M::And, 0x60);
    branch(p, M::Beq, "bbc_next");
    pool(p, M::Lda, 0x2a);
    byte(p, M::Cmp, slot);
    branch(p, M::Bne, "bbc_next");
    m16(p);
    pool(p, M::Lda, 0x28);
    word(p, M::Cmp, frame);
    branch(p, M::Bne, "bbc_next16");
    pool(p, M::Lda, 0x22);
    implied(p, M::Clc);
    word(p, M::Adc, (-COORD_FIRST) as u16);
    word(p, M::Cmp, COORDS as u16);
    branch(p, M::Bcs, "bbc_next16");
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    long_x(p, M::Lda, table_y);
    implied(p, M::Sec);
    word(p, M::Sbc, LEFT[3] as u16);
    word(p, M::And, !(BOSS_STEP - 1));
    abs(p, M::Sta, CELL);
    branch(p, M::Bra, "bbc_found");
    p.label("bbc_next");
    m16(p);
    p.label("bbc_next16");
    pool(p, M::Lda, 8);
    branch(p, M::Bra, "bbc_object");
    p.label("bbc_group_done");
    abs(p, M::Lda, ROW);
    implied(p, M::Clc);
    word(p, M::Adc, 32);
    abs(p, M::Sta, ROW);
    word(p, M::Cmp, SENTINELS + 5 * 32);
    branch(p, M::Bne, "bbc_group");
    p.label("bbc_found");
    abs(p, M::Lda, CELL);
    abs(p, M::Cmp, BOSS_SENT);
    branch(p, M::Bne, "bbc_changed");
    m8(p);
    implied(p, M::Rts);
    p.label("bbc_changed").assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Sta, BOSS_SENT);
    abs(p, M::Sta, BOSS_LINE);
    m8(p);
    byte(p, M::Lda, 1);
    abs(p, M::Sta, BOSS_ON);
    abs(p, M::Lda, BOSS_SENT + 1);
    byte(p, M::Cmp, (NO_BOSS >> 8) as u8);
    branch(p, M::Bne, "bbc_send");
    abs(p, M::Stz, BOSS_ON);
    p.label("bbc_send");
    abs(p, M::Stz, QUEUE + 3);
    abs(p, M::Stz, QUEUE + 4);
    m16(p);
    word(p, M::Lda, views as u16);
    abs(p, M::Sta, 0xe0);
    m8(p);
    byte(p, M::Lda, (views >> 16) as u8);
    abs(p, M::Sta, 0xe2);
    m16(p);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    ind(p, M::Lda, 0xe0);
    abs(p, M::Sta, QUEUE);
    implied(p, M::Tax);
    emit(p, M::Ldy, A::Immediate, O::Word(2));
    ind(p, M::Lda, 0xe0);
    m8(p);
    abs(p, M::Sta, QUEUE + 2);
    abs(p, M::Sta, 0xe2);
    abs(p, M::Stx, 0xe0);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("bbc_count");
    ind(p, M::Lda, 0xe0);
    byte(p, M::Cmp, crate::sfc_b1_view::VIEW_KIND);
    branch(p, M::Bne, "bbc_counted");
    implied(p, M::Inx);
    implied(p, M::Iny);
    for _ in 0..7 {
        implied(p, M::Iny);
    }
    emit(p, M::Cpx, A::Immediate, O::Word(4));
    branch(p, M::Bne, "bbc_count");
    p.label("bbc_counted");
    byte(p, M::Lda, 0xff);
    abs(p, M::Sta, crate::sfc_b1_view::STAGED + 2);
    abs(p, M::Stx, QUEUE + 3);
    implied(p, M::Rts);
    emit_overlay(p, &enemies, height, rows, offsets, lines)
}
fn emit_overlay(
    p: &mut Assembler,
    enemies: &[u8],
    height: u16,
    rows: u32,
    offsets: u32,
    lines: u32,
) -> Result<()> {
    use crate::sfc_b1_view::STAGING;
    let long_x = |p: &mut Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let ind =
        |p: &mut Assembler, m, dp: u8| emit(p, m, A::DirectPageIndirectLongIndexedY, O::Byte(dp));
    const PAGE: u16 = 0x0d00;
    let dpv = |p: &mut Assembler, m, at: u16| emit(p, m, A::DirectPage, O::Byte((at - PAGE) as u8));
    const ROW_BASE: u16 = OVERLAY_NOT;
    const _: () = assert!(OVERLAY_ROW >> 8 == PAGE >> 8 && OVERLAY_TILES >> 8 == PAGE >> 8);
    let staging = |at: u16| (STAGING & 0xffff) as u16 + at;
    p.label("battle_view_overlay")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    if enemies.is_empty() {
        implied(p, M::Rts);
        return Ok(());
    }
    abs(p, M::Lda, crate::sfc_battle_field::ACTIVE);
    branch(p, M::Beq, "bvo_none");
    abs(p, M::Lda, BOSS_ON);
    branch(p, M::Bne, "bvo_on");
    p.label("bvo_none");
    implied(p, M::Rts);
    p.label("bvo_on");
    m16(p);
    emit(p, M::Ldy, A::Immediate, O::Word(1));
    ind(p, M::Lda, 0xe4);
    implied(p, M::Sec);
    word(p, M::Sbc, 0x20);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    for block in 0..4u16 {
        word(p, M::Cmp, block * 1664);
        branch(p, M::Beq, "bvo_block");
        implied(p, M::Inx);
    }
    m8(p);
    implied(p, M::Rts);
    p.label("bvo_block").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Txa);
    for _ in 0..5 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Sec);
    abs(p, M::Sbc, BOSS_LINE);
    abs(p, M::Sta, OVERLAY_ROW);
    implied(p, M::Phd);
    implied(p, M::Phb);
    emit(p, M::Pea, A::Absolute, O::Word(PAGE));
    implied(p, M::Pld);
    let bank = (STAGING >> 16) as u16;
    emit(p, M::Pea, A::Absolute, O::Word(bank << 8 | bank));
    implied(p, M::Plb);
    implied(p, M::Plb);
    word(p, M::Lda, 0);
    implied(p, M::Pha);
    p.label("bvo_row");
    dpv(p, M::Lda, OVERLAY_ROW);
    branch(p, M::Bpl, "bvo_row_below");
    p.emit_label_ref(M::Jmp, A::Absolute, "bvo_row_next");
    p.label("bvo_row_below");
    word(p, M::Cmp, height);
    branch(p, M::Bcc, "bvo_row_inside");
    p.emit_label_ref(M::Jmp, A::Absolute, "bvo_rows_done");
    p.label("bvo_row_inside");
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    long_x(p, M::Lda, offsets);
    implied(p, M::Tay);
    emit(p, M::Lda, A::StackRelative, O::Byte(1));
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    long_x(p, M::Lda, lines);
    dpv(p, M::Sta, ROW_BASE);
    implied(p, M::Tyx);
    long_x(p, M::Lda, rows);
    branch(p, M::Beq, "bvo_opaque_done");
    dpv(p, M::Sta, OVERLAY_TILES);
    p.label("bvo_opaque");
    long_x(p, M::Lda, rows + 2);
    implied(p, M::Clc);
    dpv(p, M::Adc, ROW_BASE);
    implied(p, M::Tay);
    for k in 0..4u16 {
        long_x(p, M::Lda, rows + 4 + 2 * u32::from(k));
        emit(p, M::Sta, A::AbsoluteY, O::Word(staging(16 * k)));
    }
    implied(p, M::Txa);
    implied(p, M::Clc);
    word(p, M::Adc, 10);
    implied(p, M::Tax);
    dpv(p, M::Dec, OVERLAY_TILES);
    branch(p, M::Bne, "bvo_opaque");
    p.label("bvo_opaque_done");
    implied(p, M::Inx);
    implied(p, M::Inx);
    long_x(p, M::Lda, rows);
    branch(p, M::Bne, "bvo_parts");
    p.emit_label_ref(M::Jmp, A::Absolute, "bvo_row_next");
    p.label("bvo_parts");
    dpv(p, M::Sta, OVERLAY_TILES);
    p.label("bvo_part");
    long_x(p, M::Lda, rows + 2);
    implied(p, M::Clc);
    dpv(p, M::Adc, ROW_BASE);
    implied(p, M::Tay);
    for k in 0..4u16 {
        emit(p, M::Lda, A::AbsoluteY, O::Word(staging(16 * k)));
        long_x(p, M::And, rows + 4);
        long_x(p, M::Ora, rows + 6 + 2 * u32::from(k));
        emit(p, M::Sta, A::AbsoluteY, O::Word(staging(16 * k)));
    }
    implied(p, M::Txa);
    implied(p, M::Clc);
    word(p, M::Adc, 12);
    implied(p, M::Tax);
    dpv(p, M::Dec, OVERLAY_TILES);
    branch(p, M::Bne, "bvo_part");
    p.label("bvo_row_next");
    dpv(p, M::Inc, OVERLAY_ROW);
    implied(p, M::Pla);
    emit(p, M::Inc, A::Accumulator, O::None);
    implied(p, M::Pha);
    word(p, M::Cmp, 32);
    branch(p, M::Beq, "bvo_rows_done");
    p.emit_label_ref(M::Jmp, A::Absolute, "bvo_row");
    p.label("bvo_rows_done");
    implied(p, M::Pla);
    implied(p, M::Plb);
    implied(p, M::Pld);
    m8(p);
    implied(p, M::Rts);
    Ok(())
}
